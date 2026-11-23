import XCTest
@testable import DdcSwitch

final class USBTests: XCTestCase {
    func testIdentityParsing() {
        XCTAssertEqual(usbDeviceID("0b05:1abc — Keyboard"), "0B05:1ABC")
        for invalid in ["", "123:4567", "12345:6789", "1234:zzzz", "1234:5678:9999"] {
            XCTAssertNil(usbDeviceID(invalid))
        }
    }

    func testStartupDuplicatesAndResume() {
        var presence = USBPresence()
        XCTAssertNil(presence.observe(true))
        XCTAssertNil(presence.observe(true))
        XCTAssertEqual(presence.observe(false), false)
        XCTAssertNil(presence.observe(false))
        XCTAssertEqual(presence.observe(true), true)
        presence.reset()
        XCTAssertNil(presence.observe(false))
        XCTAssertEqual(presence.observe(true), true)
    }

    func testBackwardCompatibilityAndRoundtrip() {
        var config = Config()
        config.parse("monitor = 2\nshutdown_input = 17\n")
        XCTAssertFalse(config.usb.active)
        config.parse("usb_enabled = true\nusb_device = 0b05:1234\nusb_disconnect_input = 7\n")
        XCTAssertFalse(config.usb.active)
        config.parse("usb_connect_input = 0x08\n")
        XCTAssertFalse(config.usb.active)
        config.parse("usb_device_name = Keyboard\nusb_target1_enabled = true\nusb_target1_monitor = 1\nusb_target1_name = G7C II\nusb_target1_input = 7\nusb_target2_enabled = true\nusb_target2_monitor = 2\nusb_target2_name = ASUS\nusb_target2_input = 17\n")
        XCTAssertTrue(config.usb.active)
        XCTAssertTrue(config.usb.triggered(connected: false))
        XCTAssertFalse(config.usb.triggered(connected: true))
        var parsed = Config()
        parsed.parse(config.serialized())
        XCTAssertEqual(parsed, config)
        XCTAssertFalse(parsed.serialized().contains("usb_connect_input"))
        for invalid in ["", "0", "65536", "invalid"] {
            parsed.parse("usb_target1_input = \(invalid)\nusb_target2_enabled = false\n")
            XCTAssertNil(parsed.usb.targets[0].input)
            XCTAssertFalse(parsed.usb.active)
        }
        parsed.parse("usb_device = invalid\n")
        XCTAssertFalse(parsed.usb.active)
    }

    func testFirstArrivalAndLastRemovalOfIdenticalDevices() {
        var presence = USBPresence()
        let identity = "1234:5678"
        let snapshots = [[identity], [identity, identity], [identity, "ABCD:EF01"], ["ABCD:EF01"], [], [identity]]
        let expected: [Bool?] = [nil, nil, nil, false, nil, true]
        for (devices, result) in zip(snapshots, expected) {
            XCTAssertEqual(presence.observe(devices.contains(identity)), result)
        }
    }

    func testOnlySelectedKeyboardRemovalsSendCommands() {
        let config = USBConfig(enabled: true, device: "1234:5678", targets: [USBTarget(enabled: true, monitor: 1, name: "G7C II", input: 7)])
        var presence = USBPresence()
        let snapshots = [[], ["ABCD:EF01"], ["ABCD:EF01", "1234:5678"], ["1234:5678"], [], ["1234:5678"]]
        let commands = snapshots.compactMap { presence.observe($0.contains(config.device)) }
            .filter { config.triggered(connected: $0) }
        XCTAssertEqual(commands, [false])
    }

    func testPowerProtectionAndCancellation() {
        var config = Config()
        XCTAssertFalse(config.shutdownSwitch)
        config.parse("shutdown_switch = true\nsleep_switch = true\nusb_enabled = true\n")
        XCTAssertFalse(config.shutdownSwitchAllowed)
        XCTAssertFalse(config.sleepSwitchAllowed)
        let gate = USBRunGate()
        XCTAssertFalse(gate.ticket()())
        gate.resume()
        let queued = gate.ticket()
        XCTAssertTrue(queued())
        gate.pause()
        XCTAssertFalse(queued())
        gate.resume()
        XCTAssertFalse(queued())
        XCTAssertTrue(gate.ticket()())
        let targets = [USBTarget(enabled: true, monitor: 1, name: "First", input: 7), USBTarget(enabled: true, monitor: 2, name: "Second", input: 8)]
        let current = gate.ticket()
        var writes = 0
        _ = executeUSBTargets(targets, names: ["First", "Second"], permitted: current) { _, _ in writes += 1; gate.pause() }
        XCTAssertEqual(writes, 1)
        _ = executeUSBTargets(targets, names: ["First", "Second"], permitted: queued) { _, _ in writes += 1 }
        XCTAssertEqual(writes, 1)
    }

    func testBatchFailureMissingReorderAndDuplicates() {
        let targets = [USBTarget(enabled: true, monitor: 1, name: "First", input: 7), USBTarget(enabled: true, monitor: 2, name: "Second", input: 8)]
        var writes: [Int] = []
        let results = executeUSBTargets(targets, names: ["Second", "First"], permitted: { true }) { index, _ in
            writes.append(index)
            if index == 1 { throw DDCError(description: "test failure") }
        }
        XCTAssertEqual(writes, [1, 0])
        XCTAssertEqual(results.count, 2)
        writes = []
        _ = executeUSBTargets(targets, names: ["Second"], permitted: { true }) { index, _ in writes.append(index) }
        XCTAssertEqual(writes, [0])
        writes = []
        _ = executeUSBTargets([targets[0], targets[0]], names: ["First"], permitted: { true }) { index, _ in writes.append(index) }
        XCTAssertEqual(writes, [0])
    }
}
