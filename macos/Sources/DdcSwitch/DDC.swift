// DDC/CI over I2C. Apple silicon talks to external displays through the DCP's IOAVService
// (a private IOKit API that has been stable since macOS 11). Intel Macs are not supported.
// Monitors are re-enumerated for every operation so display changes are picked up.

import Foundation
import IOKit

@_silgen_name("IOAVServiceCreateWithService")
private func IOAVServiceCreateWithService(_ allocator: CFAllocator?, _ service: io_service_t) -> Unmanaged<CFTypeRef>?

@_silgen_name("IOAVServiceReadI2C")
private func IOAVServiceReadI2C(_ service: CFTypeRef, _ chipAddress: UInt32, _ offset: UInt32,
                                _ buffer: UnsafeMutableRawPointer, _ size: UInt32) -> IOReturn

@_silgen_name("IOAVServiceWriteI2C")
private func IOAVServiceWriteI2C(_ service: CFTypeRef, _ chipAddress: UInt32, _ dataAddress: UInt32,
                                 _ buffer: UnsafeMutableRawPointer, _ size: UInt32) -> IOReturn

struct DDCError: Error, CustomStringConvertible {
    let description: String
}

/// DDC/CI addresses: the display listens at 0x37 (0x6E for writes), the host is 0x51.
private let displayAddress: UInt8 = 0x37
private let hostAddress: UInt8 = 0x51

final class Monitor {
    let name: String
    private let service: CFTypeRef

    init(name: String, service: CFTypeRef) {
        self.name = name
        self.service = service
    }

    /// Sends a DDC/CI message (`payload` starts with the opcode) and, if `replyLength` > 0, reads the answer.
    func transact(_ payload: [UInt8], replyLength: Int, delay: useconds_t = 50_000,
                  permitted: () -> Bool = { true }) -> (IOReturn, [UInt8]) {
        var packet = [0x80 | UInt8(payload.count)] + payload
        packet.append(packet.reduce(0x6E ^ hostAddress, ^))
        // Some displays (including G7C II) acknowledge the first request but only
        // act/reply after a second identical request, as used by m1ddc.
        var status: IOReturn = KERN_SUCCESS
        for _ in 0..<2 {
            usleep(10_000)
            guard permitted() else { return (kIOReturnAborted, []) }
            status = IOAVServiceWriteI2C(service, UInt32(displayAddress), UInt32(hostAddress), &packet, UInt32(packet.count))
            if status != KERN_SUCCESS { return (status, []) }
        }
        guard status == KERN_SUCCESS, replyLength > 0 else { return (status, []) }
        usleep(delay)
        var reply = [UInt8](repeating: 0, count: replyLength)
        status = IOAVServiceReadI2C(service, UInt32(displayAddress), UInt32(hostAddress), &reply, UInt32(replyLength))
        return (status, reply)
    }
}

enum DDC {
    /// Serialises DDC/CI traffic: the monitor's I2C bus doesn't like concurrent requests.
    private static let lock = NSLock()

    private static func locked<T>(_ body: () throws -> T) rethrows -> T {
        lock.lock()
        defer { lock.unlock() }
        return try body()
    }

    /// Walks the IORegistry. Each external display's framebuffer
    /// (IOMobileFramebufferShim / AppleCLCD2, which carries the product name) comes
    /// before the DCPAVServiceProxy that drives its DDC channel.
    static func enumerate() -> [Monitor] {
        var out: [Monitor] = []
        var iterator: io_iterator_t = 0
        let root = IORegistryGetRootEntry(kIOMainPortDefault)
        defer { IOObjectRelease(root) }
        guard IORegistryEntryCreateIterator(root, kIOServicePlane, IOOptionBits(kIORegistryIterateRecursively), &iterator) == KERN_SUCCESS else {
            return out
        }
        defer { IOObjectRelease(iterator) }
        var lastName: String?
        while case let entry = IOIteratorNext(iterator), entry != 0 {
            defer { IOObjectRelease(entry) }
            var nameBuf = [CChar](repeating: 0, count: 128)
            IORegistryEntryGetName(entry, &nameBuf)
            let name = String(cString: nameBuf)
            if name == "IOMobileFramebufferShim" || name == "AppleCLCD2" {
                if let attrs = property(entry, "DisplayAttributes") as? [String: Any],
                   let product = attrs["ProductAttributes"] as? [String: Any] {
                    lastName = product["ProductName"] as? String
                }
            } else if name == "DCPAVServiceProxy" {
                guard property(entry, "Location") as? String == "External",
                      let service = IOAVServiceCreateWithService(kCFAllocatorDefault, entry)?.takeRetainedValue() else { continue }
                let fallback = "Display \(out.count + 1)"
                out.append(Monitor(name: (lastName?.isEmpty == false ? lastName! : fallback), service: service))
                lastName = nil
            }
        }
        return out
    }

    private static func property(_ entry: io_registry_entry_t, _ key: String) -> Any? {
        IORegistryEntryCreateCFProperty(entry, key as CFString, kCFAllocatorDefault, 0)?.takeRetainedValue()
    }

    /// Picks the monitor named `name` if present, otherwise the 1-based `index`.
    private static func resolve(_ monitors: [Monitor], _ index: Int, _ name: String) throws -> Monitor {
        if !name.isEmpty, let m = monitors.first(where: { $0.name == name }) { return m }
        let i = max(index, 1) - 1
        guard i < monitors.count else {
            throw DDCError(description: I18n.fmt(I18n.t.errMonitorNotFound, index, monitors.count))
        }
        return monitors[i]
    }

    private static func hex(_ status: IOReturn) -> String { String(format: "0x%08X", UInt32(bitPattern: status)) }

    /// DDC replies end with a checksum over the bytes before it, seeded with the virtual host address 0x50.
    static func checksumOK(_ reply: [UInt8], length: Int) -> Bool {
        guard length >= 2, reply.count >= length else { return false }
        return reply[..<(length - 1)].reduce(0x50, ^) == reply[length - 1]
    }

    static func parseVCPReply(_ reply: [UInt8], code: UInt8) -> (current: UInt32, maximum: UInt32)? {
        guard reply.count >= 11, reply[0] == 0x6E, reply[1] == 0x88,
              reply[2] == 0x02, reply[3] == 0, reply[4] == code,
              checksumOK(reply, length: 11) else { return nil }
        return ((UInt32(reply[8]) << 8) | UInt32(reply[9]), (UInt32(reply[6]) << 8) | UInt32(reply[7]))
    }

    static func listNames() -> [String] {
        locked { enumerate().map(\.name) }
    }

    /// Sets a VCP feature (retrying a few times, DDC/CI is flaky). Returns the monitor name.
    @discardableResult
    static func setVCP(index: Int, name: String, code: UInt8, value: UInt32) throws -> String {
        try locked {
            let monitor = try resolve(enumerate(), index, name)
            try writeVCP(monitor, code: code, value: value, permitted: { true })
            return monitor.name
        }
    }

    static func switchUSB(_ targets: [USBTarget], permitted: () -> Bool) -> [Result<String, DDCError>] {
        locked {
            guard permitted() else { return [] }
            // Retain every service before the first write changes the display topology.
            let monitors = enumerate()
            return executeUSBTargets(targets, names: monitors.map(\.name), permitted: permitted) { index, input in
                try writeVCP(monitors[index], code: vcpInput, value: input, permitted: permitted)
            }
        }
    }

    private static func writeVCP(_ monitor: Monitor, code: UInt8, value: UInt32, permitted: () -> Bool) throws {
        var last: IOReturn = 0
        for attempt in 0..<4 {
            if attempt > 0 { usleep(80_000) }
            guard permitted() else { throw DDCError(description: "USB switch cancelled") }
            let (status, _) = monitor.transact([0x03, code, UInt8((value >> 8) & 0xFF), UInt8(value & 0xFF)], replyLength: 0, permitted: permitted)
            if status == KERN_SUCCESS { return }
            last = status
        }
        throw DDCError(description: I18n.fmt(I18n.t.errSetFailed, monitor.name, hex(last)))
    }

    /// Reads a VCP feature, returning (current, maximum).
    static func getVCP(index: Int, name: String, code: UInt8) throws -> (current: UInt32, maximum: UInt32) {
        try locked {
            let m = try resolve(enumerate(), index, name)
            var last: IOReturn = kIOReturnError
            // Monitors often reject requests that arrive right after another one; back off.
            for delay: useconds_t in [0, 100_000, 300_000, 600_000] {
                usleep(delay)
                let (status, reply) = m.transact([0x01, code], replyLength: 11)
                guard status == KERN_SUCCESS else { last = status; continue }
                guard let value = parseVCPReply(reply, code: code) else {
                    last = kIOReturnBadArgument
                    continue
                }
                return value
            }
            if last == kIOReturnBadArgument {
                let message: String
                switch I18n.currentCode {
                case "zh-Hans": message = "未收到有效的 DDC/CI 回复"
                case "zh-Hant": message = "未收到有效的 DDC/CI 回覆"
                default: message = "No valid DDC/CI reply received"
                }
                throw DDCError(description: "\(m.name): \(message)")
            }
            throw DDCError(description: I18n.fmt(I18n.t.errReadFailed, m.name, hex(last)))
        }
    }

    /// Current input source. Only the low byte is meaningful: e.g. Dell answers 0x1919 for USB-C (0x19).
    static func getInput(index: Int, name: String) throws -> UInt32 {
        try getVCP(index: index, name: name, code: vcpInput).current & 0xFF
    }

    /// Raw MCCS capabilities string (slow: ~1-3 s on most monitors).
    static func capabilities(index: Int, name: String) throws -> String {
        try locked {
            let m = try resolve(enumerate(), index, name)
            for attempt in 0..<3 {
                if attempt > 0 { usleep(500_000) }
                var data: [UInt8] = []
                var failures = 0
                while data.count < 8192 && failures < 5 {
                    let offset = data.count
                    let (status, r) = m.transact([0xF3, UInt8(offset >> 8), UInt8(offset & 0xFF)], replyLength: 38)
                    // 6E <0x80|len> E3 <offset hi> <offset lo> <data...> <checksum>
                    let length = Int(r.count > 1 ? r[1] & 0x7F : 0)
                    guard status == KERN_SUCCESS, r[2] == 0xE3, length >= 3, length <= 35,
                          Int(r[3]) << 8 | Int(r[4]) == offset, checksumOK(r, length: length + 3) else {
                        failures += 1
                        usleep(50_000)
                        continue
                    }
                    if length == 3 {
                        let text = String(decoding: data.prefix { $0 != 0 }, as: UTF8.self)
                        if !text.isEmpty { return text }
                        break
                    }
                    data += r[5..<(2 + length)]
                    failures = 0
                }
            }
            throw DDCError(description: I18n.fmt(I18n.t.errCapsFailed, m.name))
        }
    }

    /// Values listed for VCP 0x60 in an MCCS capabilities string, e.g. `vcp(... 60( 19 0F 11) ...)`.
    static func parseInputCaps(_ caps: String) -> [UInt32] {
        let lower = caps.lowercased()
        guard let start = lower.range(of: "vcp(") else { return [] }
        let s = Array(lower[start.upperBound...].utf8)
        var i = 0
        func skipWhitespace() { while i < s.count && (s[i] == 0x20 || s[i] == 0x09 || s[i] == 0x0A || s[i] == 0x0D) { i += 1 } }
        func isHex(_ c: UInt8) -> Bool { (0x30...0x39).contains(c) || (0x61...0x66).contains(c) }
        func readHex() -> UInt32? {
            let st = i
            while i < s.count && isHex(s[i]) { i += 1 }
            return st == i ? nil : UInt32(String(decoding: s[st..<i], as: UTF8.self), radix: 16)
        }
        while true {
            skipWhitespace()
            if i >= s.count || s[i] == UInt8(ascii: ")") { return [] }
            guard let code = readHex() else {
                i += 1 // unexpected character, skip it
                continue
            }
            skipWhitespace()
            if i < s.count && s[i] == UInt8(ascii: "(") {
                i += 1
                var values: [UInt32] = []
                while true {
                    skipWhitespace()
                    if i >= s.count || s[i] == UInt8(ascii: ")") {
                        i += 1
                        break
                    }
                    if let v = readHex() { values.append(v) } else { i += 1 }
                }
                if code == 0x60 { return values }
            }
        }
    }
}
