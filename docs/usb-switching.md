# USB-triggered input switching / USB 联动切屏

This fork adds USB triggers to DdcSwitch on Windows and macOS. Video cables stay connected directly to the computers; a USB switch moves the keyboard/receiver/hub. DdcSwitch sends the input command to the monitor selected in its existing settings.

## Setup

1. Run DdcSwitch on both computers while logged in. Open **Settings → USB switching** on Windows (button at the bottom), or the **USB switching** pane on macOS.
2. Enable USB switching. Select a keyboard, receiver or hub that actually disappears from the old computer when you press the USB switch. The switch itself may not enumerate as a device. **Refresh devices** lists connected hardware; you can also enter a hexadecimal `VID:PID` manually for an absent device. `--list-usb` prints the same identities without sending any DDC command.
3. Set **Input for this computer**. Only the selected device's arrival sends this input command. Removal and other devices do not switch the display. Input values use the app's existing parser: decimal, hex, or aliases. Custom numbers are preserved.
4. On Windows, confirm the USB dialog, then save with **OK** in the main settings window. macOS saves automatically. The feature does nothing when the app is not running; enable the existing launch-at-login option if wanted.
5. Test one direction at a time with both computers awake and producing video. Verify the physical picture: successful DDC transport alone does not prove the monitor changed inputs.

Only the computer receiving the selected USB device sends a command. Its monitor connection must accept DDC/CI while another input is displayed. Some monitors reject commands from inactive inputs; verify this on the actual hardware. The app does not fall back to switching on removal.

Example: a G7C II tested with Windows on DP1 and Mac on DP2 uses custom VCP `0x60` values **7** and **8**, respectively. These values are device-specific. Set the Mac's local input to **8** and Windows' local input to **7**. This mapping has been verified, but connection-triggered switching from an inactive input still requires hardware testing. The standard `dp1`/`dp2` aliases mean 15/16 and are wrong for this example. The app may display the standard S-Video names for 7/8 in other screens; the numeric value is what is sent.

```ini
# On the Mac in the example. Replace this example ID with the watched USB device.
usb_enabled = true
usb_device = 1234:5678
usb_connect_input = 8
```

On Windows, use the same device's VID:PID and `usb_connect_input = 7`.

## Behavior and limits

- Default off. The old `usb_disconnect_input` key is ignored and removed on the next save. If upgrading from removal-only settings, choose this computer's input; the old outgoing input must not be copied into it. USB settings do not enable or disable the separate shutdown/sleep actions.
- Native notifications: IOKit matching/termination on macOS; `WM_DEVICECHANGE` plus SetupAPI on Windows. No background polling while idle.
- USB changes settle for 350 ms before sampling. Bursts of composite-device notifications are coalesced. An unplug/replug completed inside that interval may intentionally produce no action.
- Only the chosen VID:PID is watched, not all keyboards or all USB devices. Devices with that same VID:PID are one group. The first arrival triggers; the last removal rearms detection without switching. Changing an unrelated device does not trigger. Serial-number and per-port selection are not implemented. Pick a device model with no permanently connected duplicate.
- Startup, changed configuration and resume establish a baseline without switching. USB-triggered actions are suppressed during system sleep; after resume they wait at least two seconds and rebaseline. A USB connection that only wakes the receiving computer may therefore need a subsequent disconnect/reconnect to trigger.
- Enumeration failure does not count as removal. A switch failure is logged and uses the existing error notification. No endless retry or automatic input bouncing.
- A monitor must accept DDC/CI on the path used. A USB switch does not add video switching or wake a powered-off computer. Multiple monitors follow the existing single selected-monitor setting.
- USB UI copy is available in English, simplified Chinese and traditional Chinese; other app languages use English for this new section.

## 中文速用

两台电脑各运行 DdcSwitch。Windows 点设置底部的 **USB 联动**，Mac 选择 **USB 联动** 页。启用后选择会随切换器移动的键盘、接收器或 Hub；如果切换器本身不出现在列表里，选择其下游设备。

只设置 **本机输入源**：指定键盘或接收器接入哪台电脑，就由哪台电脑把画面切到自己。拔出及其他设备插拔都不触发。上面的 G7C II 示例中，Mac 填 **8**，Windows 填 **7**，不要照搬标准 `dp1` / `dp2` 别名。Windows 需先确定 USB 对话框，再确定主设置窗口；Mac 自动保存。旧版拔出指令会被忽略，需要重新选择本机输入源。

首次启动、保存设置和系统唤醒不会触发切屏；同型号同 VID:PID 的设备按一组处理。接入端需要能从非当前视频输入发送有效的 DDC 命令，因此先在两端均在线时按 USB 切换器验证画面，再启用原有的开机启动。USB 仅唤醒电脑时可能需要再切出、切入一次。睡眠和关机切换是另外的设置，按需要单独配置。

## Build and validation

Windows: `cargo test`, `cargo build --release` with the MSVC toolchain. macOS: `swift test` from `macos` with full Xcode (XCTest required), then `./build.sh`. The build script accepts `DDCSWITCH_DIST`, `SWIFT_BUILD_PATH` and `SWIFT_CACHE_PATH` for explicit output/cache directories; `TMPDIR` controls temporary icon files. `--config` keeps diagnostic settings and logs together on both platforms.

Actual end-to-end USB-button testing is required for each hardware combination. Unit tests cover configuration, nonstandard input values, startup/rebaseline and duplicate/aggregate presence transitions; they do not establish physical monitor behavior.
