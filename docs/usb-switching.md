# USB-triggered input switching / USB 联动切屏

This fork adds USB triggers to DdcSwitch on Windows and macOS. Video cables stay connected directly to the computers; a USB switch moves the keyboard/receiver/hub. DdcSwitch sends the input command to the monitor selected in its existing settings.

## Setup

1. Run DdcSwitch on both computers while logged in. Open **Settings → USB switching** on Windows (button at the bottom), or the **USB switching** pane on macOS.
2. Enable USB switching. Select a keyboard, receiver or hub that actually disappears from the old computer when you press the USB switch. The switch itself may not enumerate as a device. **Refresh devices** lists connected hardware; you can also enter a hexadecimal `VID:PID` manually for an absent device. `--list-usb` prints the same identities without sending any DDC command.
3. Set the desired input for **When connected**, **When disconnected**, or both. Leave an action blank to do nothing. Input values use the app's existing parser: decimal, hex, or aliases. Custom numbers are preserved.
4. On Windows, confirm the USB dialog, then save with **OK** in the main settings window. macOS saves automatically. The feature does nothing when the app is not running; enable the existing launch-at-login option if wanted.
5. Test one direction at a time with both computers awake and producing video. Verify the physical picture: successful DDC transport alone does not prove the monitor changed inputs.

For monitors that only accept commands from the currently displayed computer, use **disconnection on the outgoing computer**. Do not rely on the incoming computer being able to send a command over its inactive video connection.

Example: a G7C II tested with Windows on DP1 and Mac on DP2 uses custom VCP `0x60` values **7** and **8**, respectively. These values are device-specific. Use **7** for the Mac's disconnection action and **8** for Windows' disconnection action; leave connection actions empty. The standard `dp1`/`dp2` aliases mean 15/16 and are wrong for that example. The app may display the standard S-Video names for 7/8 in other screens; the numeric value is what is sent.

```ini
# On the Mac in the example. Replace this example ID with the watched USB device.
usb_enabled = true
usb_device = 1234:5678
usb_connect_input =
usb_disconnect_input = 7
```

On Windows, use the same device's VID:PID and `usb_disconnect_input = 8`.

## Behavior and limits

- Default off; existing configuration files keep their behavior. USB settings do not enable or disable the separate shutdown/sleep actions.
- Native notifications: IOKit matching/termination on macOS; `WM_DEVICECHANGE` plus SetupAPI on Windows. No background polling while idle.
- USB changes settle for 350 ms before sampling. Bursts of composite-device notifications are coalesced. An unplug/replug completed inside that interval may intentionally produce no action.
- Devices with the same VID:PID are one group. The first connection and last disconnection trigger; changing an unrelated device does not. Serial-number and per-port selection are not implemented. Pick a device model with no permanently connected duplicate.
- Startup, changed configuration and resume establish a baseline without switching. USB-triggered actions are suppressed during system sleep; after resume they wait at least two seconds and rebaseline. Switching USB while the old host is asleep cannot make that host send DDC.
- Enumeration failure does not count as removal. A switch failure is logged and uses the existing error notification. No endless retry or automatic input bouncing.
- A monitor must accept DDC/CI on the path used. A USB switch does not add video switching or wake a powered-off computer. Multiple monitors follow the existing single selected-monitor setting.
- USB UI copy is available in English, simplified Chinese and traditional Chinese; other app languages use English for this new section.

## 中文速用

两台电脑各运行 DdcSwitch。Windows 点设置底部的 **USB 联动**，Mac 选择 **USB 联动** 页。启用后选择会随切换器移动的键盘、接收器或 Hub；如果切换器本身不出现在列表里，选择其下游设备。

建议只填 **拔出时切换到**：由旧电脑在 USB 离开时把画面交给另一台。上面的 G7C II 示例中，Mac 填 **7**，Windows 填 **8**，接入动作留空。不要照搬标准 `dp1` / `dp2` 别名。Windows 需先确定 USB 对话框，再确定主设置窗口；Mac 自动保存。

首次启动、保存设置和系统唤醒不会触发切屏；同型号同 VID:PID 的设备按一组处理。先在两端均在线时手动按 USB 切换器验证，再启用原有的开机启动。睡眠和关机切换是另外的设置，按需要单独配置。

## Build and validation

Windows: `cargo test`, `cargo build --release` with the MSVC toolchain. macOS: `swift test` from `macos` with full Xcode (XCTest required), then `./build.sh`. The build script accepts `DDCSWITCH_DIST`, `SWIFT_BUILD_PATH` and `SWIFT_CACHE_PATH` for explicit output/cache directories; `TMPDIR` controls temporary icon files. `--config` keeps diagnostic settings and logs together on both platforms.

Actual end-to-end USB-button testing is required for each hardware combination. Unit tests cover configuration, nonstandard input values, startup/rebaseline and duplicate/aggregate presence transitions; they do not establish physical monitor behavior.
