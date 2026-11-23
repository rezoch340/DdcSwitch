# USB-triggered input switching / USB 联动切屏

This fork adds USB removal triggers to DdcSwitch on Windows and macOS. Video cables stay connected directly to the computers; a USB switch moves the keyboard/receiver/hub. The outgoing computer locally sends up to two displays to individually configured inputs. No network relay is used.

## Setup

1. Run DdcSwitch on both computers while logged in. Open **Settings → USB switching** on Windows (button at the bottom), or the **USB switching** pane on macOS.
2. Enable USB switching. Select the named keyboard, receiver or hub that disappears from this computer when you press the USB switch. Windows prefers the USB bus-reported product name over generic driver descriptions. The selected name is saved even while unplugged. **Refresh devices and displays** updates the lists; `--list-usb` prints devices without DDC commands. An absent device can also be entered as hexadecimal `VID:PID`.
3. Enable one or both display rows. For each, choose its monitor and **On disconnect, switch to**: the input connected to the **other** computer. Input values accept decimal, hex or standard aliases. Custom numbers are preserved. Disabled rows are not switched.
4. On Windows, confirm the USB dialog, then save with **OK** in the main settings window. macOS saves automatically. The feature does nothing when the app is not running; enable the existing launch-at-login option if wanted.
5. Test one direction at a time with both computers awake and producing video. Verify the physical picture: successful DDC transport alone does not prove the monitor changed inputs.

Only the computer losing the selected USB device sends a command. The receiving computer does nothing on arrival. This allows the currently displayed host to hand off a monitor that ignores DDC commands from inactive video inputs.

Example: a G7C II tested with Windows on DP1 and Mac on DP2 uses custom VCP `0x60` values **7** and **8**, respectively. Set **Mac removal → 7 (Windows)** and **Windows removal → 8 (Mac)**. Mac arrival detection worked, but a local command from the inactive Mac input did not change the picture. On 2026-11-22, the final native Mac app detected removal of a ROG FALCHION ACE HFX, sent `7` locally and switched the physical picture to Windows, confirmed by the user. The standard `dp1`/`dp2` aliases mean 15/16 and are wrong for this display; the number is what is sent, regardless of standard labels elsewhere in the app.

The same hardware returned a null DDC frame to a single request (`6E 80 BE …`) and a valid current-input reply after two requests (`6E 88 02 00 60 00 00 0E 00 08 D2`). The Mac implementation now repeats requests with a 10 ms interval, matching the independently tested m1ddc behavior. Current input and capabilities were both read successfully after this fix. The monitor advertises standard 15/16 inputs in its capabilities despite using 7/8 in this setup; keep the physically verified numbers. Custom names from the existing shortcut settings are used for the current-input display even if those shortcuts are disabled.

```ini
# On the Mac in the example. Use the USB device you selected.
usb_enabled = true
usb_device = 1234:5678
usb_device_name = My keyboard
usb_target1_enabled = true
usb_target1_monitor = 1
usb_target1_name = G7C II
usb_target1_input = 7
usb_target2_enabled = false
shutdown_switch = false
sleep_switch = false
```

On Windows, select the same device's VID:PID and the Windows-reported monitor name, with `usb_target1_input = 8`. The native Windows app was also built, installed and tested: removing the selected ROG keyboard from Windows switched G7C II back to Mac, confirmed by the user. Both directions are now physically verified on this setup. Each host saves its own monitor names.

The ASUS VG259 was subsequently connected to Mac through a Belkin Thunderbolt dock. After reseating HDMI, Mac detected it at 1080p/144 Hz, read its capabilities, and repeatedly read current input **18 (HDMI2)**. Use the readback rather than an earlier assumption about which physical HDMI socket was connected. The installed two-display configuration is:

| Outgoing host | G7C II destination | VG259 destination |
| --- | --- | --- |
| Mac | Windows DP1, **7** | Windows DP, **15** |
| Windows | Mac DP2, **8** | Mac HDMI2, **18** |

Both rows are enabled. Mac names are `G7C II` and `VG259`; Windows names are `G7C II (DisplayPort)` and `VG259 (DisplayPort)`. Keep both video connections attached. Transport logs confirm both commands are sent; verify both physical pictures after each handoff.

## Behavior and limits

- Default off. The old `usb_connect_input` and `usb_disconnect_input` keys are ignored and removed on the next save. Select the destinations explicitly when upgrading; old local-input settings cannot safely become outgoing destinations.
- Shutdown switching now defaults off. USB mode suppresses the legacy shutdown/sleep actions even with old enabled settings or incomplete USB settings. Windows pauses at `WM_QUERYENDSESSION`, checks `SM_SHUTTINGDOWN` before USB writes, and rebaselines if shutdown is cancelled. Mac pauses at logout/power-off and termination notifications. System sleep and app exit also cancel queued work. Cancellation cannot undo a command already sent to the monitor.
- Native notifications: IOKit matching/termination on macOS; explicitly registered `GUID_DEVINTERFACE_USB_DEVICE` notifications plus generic `WM_DEVICECHANGE` on Windows. Windows samples enabled USB interfaces through Configuration Manager; SetupAPI remains responsible for the named device picker. No background polling while idle.
- USB changes settle for 350 ms before sampling. On Windows, later events do not restart an already pending check, so a composite-device event burst cannot keep postponing it. An unplug/replug completed inside that interval may intentionally produce no action.
- Only the chosen VID:PID is watched, not all keyboards or all USB devices. Identical VID:PID devices are one group: the last removal triggers, and arrival only rearms detection. Unrelated devices do not trigger. Serial-number and per-port selection are not implemented; avoid a permanently connected duplicate.
- Startup, changed configuration and resume establish a baseline without switching. After system sleep, wait at least two seconds and rebaseline. A USB removal during sleep/wake is not replayed. A queued command stays cancelled even after resuming.
- Enumeration failure does not count as removal. Windows retries the presence check after a failed or raced snapshot. DDC writes have bounded retries; a switch failure is logged and uses the existing error notification. There is no automatic input bouncing.
- The two USB display rows are independent of the original hotkey/power monitor setting. Both monitor handles are captured before switching the first screen. A missing or failed display does not prevent trying the other; a missing named monitor never falls back to an unrelated monitor. Duplicate targets are not sent twice. Identically named displays use the saved index to disambiguate; verify the mapping after recabling.
- A monitor must accept DDC/CI on the path used. A USB switch does not add a video cable or wake a powered-off computer. Keep both hosts awake during initial testing. A driver may invalidate display handles after the first switch; real dual-display testing is still required.
- USB UI copy is available in English, simplified Chinese and traditional Chinese; other app languages use English for this new section.

## 中文速用

两台电脑各运行 DdcSwitch。Windows 点设置底部的 **USB 联动**，Mac 选择 **USB 联动** 页。启用后选择会随切换器移动的键盘、接收器或 Hub；如果切换器本身不出现在列表里，选择其下游设备。

每块屏幕分别设置 **断开时切换到**，填写另一台电脑的输入源。指定键盘从哪台电脑断开，就由那台电脑本地发命令交出画面，接入端不发命令。G7C II 示例：**Mac 填 7 → Windows，Windows 填 8 → Mac**，不要照搬标准 `dp1` / `dp2`。Windows 需先确定 USB 对话框，再确定主设置窗口；Mac 自动保存。旧版单输入源配置会被忽略，需要重新选两行中的目标。华硕现已通过 Belkin 雷电坞接入 Mac，DDC 实测当前输入为 **18／HDMI2**；第二行配置为 **Mac 填 15 → Windows DP，Windows 填 18 → Mac HDMI2**。

首次启动、保存设置、设备接入和系统唤醒都不切屏。USB 联动开启时，**关机、重启、注销和睡眠不切屏**；排队中的命令作废，旧配置中的关机／睡眠切屏也会被禁用。Windows 取消关机后重新识别设备状态，不补发断开。突然断电不是可识别的 USB 手动切换事件；已经发出的 DDC 命令无法撤回。

## Build and validation

Windows: `cargo test`, `cargo build --release` with the MSVC toolchain. macOS: `swift test` from `macos` with full Xcode (XCTest required), then `./build.sh`. The build script accepts `DDCSWITCH_DIST`, `SWIFT_BUILD_PATH` and `SWIFT_CACHE_PATH` for explicit output/cache directories; `TMPDIR` controls temporary icon files. `--config` keeps diagnostic settings and logs together on both platforms.

Actual end-to-end USB-button and shutdown testing is required for each hardware combination. Unit tests cover two-display configuration, removal-only events, startup/rebaseline, duplicate devices/targets, missing monitors, failure continuation, cancellation across pause/resume and suppression of legacy power settings. They do not establish physical monitor or operating-system shutdown behavior.

Validation for this branch: Mac debug/release builds, 58 standalone Swift assertions (including native USB listener lifecycle), 13 portable Rust tests, Windows MSVC-target `cargo check --all-targets`, and a Windows x64 release executable cross-compiled with cargo-xwin and run on the Windows desktop. Native USB-triggered G7C II handoff passed in both directions. The Mac settings were inspected in light and dark mode, and the native Windows USB dialog was opened and captured. Targeted simulated `WM_QUERYENDSESSION` followed by `WM_ENDSESSION(FALSE)` was accepted by the running Windows app and logged without a switch. This environment has Command Line Tools without XCTest, so `swift test` itself was not run. Real shutdown and the physical result on both displays still require hardware validation. The cross-built Windows executable has a runtime tray icon but no embedded EXE resource icon because `rc.exe` was unavailable.

Windows latency investigation: two handoffs showed roughly 12 seconds between Mac detecting the keyboard's arrival and Windows confirming its removal, followed by only 109–125 ms for the DDC batch. Host clock comparison differed by less than one second. The old watcher relied on generic PnP events, read device-node descriptions for every sample, and reset its 350 ms timer on every event. The revised watcher registers USB interface notifications, reads active interfaces without fetching friendly names, and retains the first pending timer. Logs now include event receipt, queue/scan duration and DDC batch duration. The revised Windows build passed compilation and the 13 portable tests and registered its notification successfully on the PC; its physical latency improvement must be measured with another USB-button cycle.
