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
        config.parse("usb_enabled = true\nusb_device = 0b05:1234\nusb_disconnect_input = 7\nusb_connect_input = 0x08\n")
        XCTAssertTrue(config.usb.active)
        XCTAssertEqual(config.usb.input(connected: false), 7)
        XCTAssertEqual(config.usb.input(connected: true), 8)
        var parsed = Config()
        parsed.parse(config.serialized())
        XCTAssertEqual(parsed, config)
        for invalid in ["", "0", "65536", "invalid"] {
            parsed.parse("usb_connect_input = \(invalid)\n")
            XCTAssertNil(parsed.usb.connectInput)
            XCTAssertEqual(parsed.usb.disconnectInput, 7)
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
}
