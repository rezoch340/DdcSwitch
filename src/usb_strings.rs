pub struct UsbStrings {
    pub title: &'static str,
    pub enabled: &'static str,
    pub device: &'static str,
    pub refresh: &'static str,
    pub monitor: &'static str,
    pub offline: &'static str,
    pub disconnect: &'static str,
    pub hint: &'static str,
    pub protection: &'static str,
    pub footer: &'static str,
    pub invalid: &'static str,
    pub choose: &'static str,
    pub duplicate: &'static str,
}

pub fn text() -> &'static UsbStrings {
    match crate::i18n::t().lang_name {
        "简体中文" => &CHINESE,
        "繁體中文" => &TRADITIONAL,
        _ => &ENGLISH,
    }
}

const ENGLISH: UsbStrings = UsbStrings {
    title: "USB switching", enabled: "Switch inputs when the selected USB device disconnects", device: "Watched USB device", refresh: "Refresh devices and displays",
    monitor: "Display", offline: "(offline)", disconnect: "On disconnect, switch to",
    hint: "Choose the other computer's input for each display. Custom numeric codes are supported.",
    protection: "USB mode blocks switching at shutdown, restart, logout and sleep, and cancels queued commands.",
    footer: "Arrival, startup and wake do not switch. Identical VID:PID devices are grouped. Confirm here, then save with OK in the main settings window.",
    invalid: "Enter a valid input code (1–65535) for each enabled display.",
    choose: "Choose a USB device (VID:PID) and enable at least one display with a destination input.",
    duplicate: "Choose a different monitor for each display row.",
};
const CHINESE: UsbStrings = UsbStrings {
    title: "USB 联动",
    enabled: "所选 USB 设备断开时切换输入源",
    device: "监听的 USB 设备",
    refresh: "刷新设备和显示器",
    monitor: "显示器",
    offline: "（未连接）",
    disconnect: "断开时切换到",
    hint: "每块屏幕选择另一台电脑连接的输入源，支持自定义数字代码。",
    protection: "USB 联动开启时，关机、重启、注销和睡眠不切屏，并取消排队中的命令。",
    footer: "接入、启动及唤醒不切屏。同 VID:PID 设备按一组识别。此处确定后，请在主设置窗口确定保存。",
    invalid: "请为每块启用的显示器输入有效代码（1–65535）。",
    choose: "请选择 USB 设备（VID:PID），并为至少一块显示器设置目标输入源。",
    duplicate: "两行不能选择同一块显示器。",
};
const TRADITIONAL: UsbStrings = UsbStrings {
    title: "USB 連動",
    enabled: "所選 USB 裝置斷開時切換輸入來源",
    device: "監聽的 USB 裝置",
    refresh: "重新整理裝置與螢幕",
    monitor: "螢幕",
    offline: "（未連接）",
    disconnect: "斷開時切換至",
    hint: "每個螢幕選擇另一台電腦連接的輸入來源，支援自訂數字代碼。",
    protection: "USB 連動開啟時，關機、重新啟動、登出及睡眠不切換，並取消排隊中的命令。",
    footer: "接入、啟動及喚醒不切換。同 VID:PID 裝置視為一組。此處確定後，請於主設定視窗確定儲存。",
    invalid: "請為每個啟用的螢幕輸入有效代碼（1–65535）。",
    choose: "請選擇 USB 裝置（VID:PID），並為至少一個螢幕設定目標輸入來源。",
    duplicate: "兩列不能選擇同一個螢幕。",
};
