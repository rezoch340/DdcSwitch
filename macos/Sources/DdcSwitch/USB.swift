import Foundation
import IOKit

let usbTargetSlots = 2

struct USBTarget: Equatable {
    var enabled = false
    var monitor = 0
    var name = ""
    var input: UInt32?

    var valid: Bool { monitor > 0 && !name.trimmingCharacters(in: .whitespaces).isEmpty && input.map { (1...65535).contains($0) } == true }
}

struct USBConfig: Equatable {
    var enabled = false
    var device = ""
    var deviceName = ""
    var targets = [USBTarget](repeating: USBTarget(), count: usbTargetSlots)

    var active: Bool { enabled && usbDeviceID(device) != nil && targets.contains { $0.enabled && $0.valid } }
    var deviceLabel: String { deviceName.isEmpty ? device : "\(device) — \(deviceName)" }
    func triggered(connected: Bool) -> Bool { active && !connected }
}

/// Invalidates queued work across sleep, shutdown and settings changes, even after resuming.
final class USBRunGate {
    private let lock = NSLock()
    private var epoch: UInt64 = 1

    func pause() { lock.lock(); defer { lock.unlock() }; epoch = (epoch &+ 2) | 1 }
    func resume() { lock.lock(); defer { lock.unlock() }; epoch = (epoch &+ 2) & ~1 }
    func ticket() -> () -> Bool {
        lock.lock()
        let version = epoch
        lock.unlock()
        return { [self] in
            lock.lock(); defer { lock.unlock() }
            return version & 1 == 0 && epoch == version
        }
    }
}

/// All targets use one snapshot; a missing screen never falls back to a different monitor.
func executeUSBTargets(_ targets: [USBTarget], names: [String], permitted: () -> Bool,
                       write: (Int, UInt32) throws -> Void) -> [Result<String, DDCError>] {
    var results: [Result<String, DDCError>] = []
    var used: Set<Int> = []
    for target in targets where target.enabled {
        guard permitted() else { break }
        let matches = names.indices.filter { names[$0] == target.name }
        let selected = matches.count == 1 ? matches.first : matches.first { $0 == target.monitor - 1 }
        guard target.valid, let selected, let input = target.input else {
            results.append(.failure(DDCError(description: "USB target unavailable or invalid: \(target.name)")))
            continue
        }
        guard used.insert(selected).inserted else {
            results.append(.failure(DDCError(description: "Duplicate USB target: \(target.name)")))
            continue
        }
        do {
            try write(selected, input)
            guard permitted() else { break }
            results.append(.success("\(target.name) → \(input)"))
        } catch {
            guard permitted() else { break }
            results.append(.failure(DDCError(description: "\(target.name): \(error)")))
        }
    }
    return results
}

/// The picker may append a display name. Persist only the hexadecimal VID:PID.
func usbDeviceID(_ text: String) -> String? {
    guard let token = text.split(whereSeparator: \.isWhitespace).first else { return nil }
    let parts = token.split(separator: ":", omittingEmptySubsequences: false)
    guard parts.count == 2, parts.allSatisfy({ $0.count == 4 && $0.utf8.allSatisfy { byte in
        (48...57).contains(byte) || (65...70).contains(byte) || (97...102).contains(byte)
    } }) else { return nil }
    return token.uppercased()
}

struct USBPresence {
    private var previous: Bool?
    mutating func reset() { previous = nil }
    mutating func observe(_ present: Bool) -> Bool? {
        defer { previous = present }
        guard let previous, previous != present else { return nil }
        return present
    }
}

struct USBDevice: Equatable {
    let identity: String
    let name: String
    var label: String { "\(identity) — \(name)" }

    static func list() throws -> [USBDevice] {
        var iterator: io_iterator_t = 0
        let result = IOServiceGetMatchingServices(kIOMainPortDefault, IOServiceMatching("IOUSBHostDevice"), &iterator)
        guard result == KERN_SUCCESS else { throw USBError(operation: "enumeration", code: result) }
        defer { IOObjectRelease(iterator) }
        var devices: [String: USBDevice] = [:]
        while case let service = IOIteratorNext(iterator), service != 0 {
            defer { IOObjectRelease(service) }
            func property(_ key: String) -> Any? {
                IORegistryEntryCreateCFProperty(service, key as CFString, kCFAllocatorDefault, 0)?.takeRetainedValue()
            }
            guard let vendor = property("idVendor") as? NSNumber,
                  let product = property("idProduct") as? NSNumber else {
                // Do not turn a partial/racing enumeration into a false removal.
                throw USBError(operation: "device identity", code: kIOReturnNotReady)
            }
            let identity = String(format: "%04X:%04X", vendor.uint16Value, product.uint16Value)
            let name = property("USB Product Name") as? String ?? identity
            devices[identity] = USBDevice(identity: identity, name: name)
        }
        return devices.values.sorted { $0.identity < $1.identity }
    }
}

private struct USBError: Error, CustomStringConvertible {
    let operation: String
    let code: kern_return_t
    var description: String { "USB \(operation) failed (\(code))" }
}

/// Main-thread IOKit notifications, with a 350 ms quiet period for composite/hub bursts.
final class USBMonitor {
    let gate = USBRunGate()
    var onRemoval: (([USBTarget], @escaping () -> Bool) -> Void)?
    var onError: ((String) -> Void)?
    private var config = USBConfig()
    private var presence = USBPresence()
    private var port: IONotificationPortRef?
    private var arrivals: io_iterator_t = 0
    private var removals: io_iterator_t = 0
    private var pending: DispatchWorkItem?
    private var suspended = false
    private var resumeAfter: Date?

    func configure(_ config: USBConfig) {
        stop()
        self.config = config
        guard config.active else { return }
        guard let port = IONotificationPortCreate(kIOMainPortDefault) else {
            report("USB notification port could not be created")
            return
        }
        self.port = port
        IONotificationPortSetDispatchQueue(port, DispatchQueue.main)
        let context = Unmanaged.passUnretained(self).toOpaque()
        let callback: IOServiceMatchingCallback = { context, iterator in
            USBMonitor.drain(iterator)
            guard let context else { return }
            Unmanaged<USBMonitor>.fromOpaque(context).takeUnretainedValue().changed()
        }
        let arrivalResult = IOServiceAddMatchingNotification(port, kIOFirstMatchNotification,
            IOServiceMatching("IOUSBHostDevice"), callback, context, &arrivals)
        let removalResult = IOServiceAddMatchingNotification(port, kIOTerminatedNotification,
            IOServiceMatching("IOUSBHostDevice"), callback, context, &removals)
        guard arrivalResult == KERN_SUCCESS, removalResult == KERN_SUCCESS else {
            stop()
            report("USB notifications failed (\(arrivalResult), \(removalResult))")
            return
        }
        // Draining arms the notifications; initial devices are not connection events.
        Self.drain(arrivals)
        Self.drain(removals)
        if !suspended { gate.resume() }
        // Saving while wake is settling must keep the scheduled rebaseline alive.
        if resumeAfter != nil { changed() }
        else if !suspended { _ = sample() }
    }

    func stop() {
        gate.pause()
        pending?.cancel()
        pending = nil
        if let port {
            IONotificationPortSetDispatchQueue(port, nil)
            IONotificationPortDestroy(port)
        }
        port = nil
        if arrivals != 0 { IOObjectRelease(arrivals); arrivals = 0 }
        if removals != 0 { IOObjectRelease(removals); removals = 0 }
        presence.reset()
    }

    func suspend() {
        gate.pause()
        suspended = true
        resumeAfter = nil
        pending?.cancel()
        presence.reset()
    }

    func resume() {
        gate.resume()
        suspended = false
        resumeAfter = Date(timeIntervalSinceNow: 2)
        presence.reset()
        changed()
    }

    private static func drain(_ iterator: io_iterator_t) {
        guard iterator != 0 else { return }
        while case let service = IOIteratorNext(iterator), service != 0 { IOObjectRelease(service) }
    }

    private func changed() {
        guard config.active, port != nil, !suspended else { return }
        pending?.cancel()
        let work = DispatchWorkItem { [weak self] in
            guard let self, !self.suspended else { return }
            if self.resumeAfter != nil {
                self.resumeAfter = nil
                self.presence.reset()
            }
            guard let connected = self.sample() else { return }
            if connected { self.gate.resume() } // Returning the device cancels a queued departure.
            AppLog.log("USB \(self.config.device) \(connected ? "connected" : "disconnected")")
            if self.config.triggered(connected: connected) {
                self.onRemoval?(self.config.targets, self.gate.ticket())
            }
        }
        pending = work
        DispatchQueue.main.asyncAfter(deadline: .now() + max(0.35, resumeAfter?.timeIntervalSinceNow ?? 0), execute: work)
    }

    private func sample() -> Bool? {
        do { return presence.observe(try USBDevice.list().contains { $0.identity == config.device }) }
        catch { AppLog.log("USB enumeration failed: \(error)"); return nil }
    }

    private func report(_ message: String) {
        AppLog.log(message)
        onError?(message)
    }

    deinit { stop() }
}
