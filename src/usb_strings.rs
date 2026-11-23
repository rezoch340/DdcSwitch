pub struct UsbStrings {
    pub title: &'static str,
    pub enabled: &'static str,
    pub device: &'static str,
    pub refresh: &'static str,
    pub connect: &'static str,
    pub hint: &'static str,
    pub footer: &'static str,
    pub invalid: &'static str,
    pub choose: &'static str,
}

pub fn text() -> &'static UsbStrings {
    match crate::i18n::t().lang_name {
        "简体中文" => &CHINESE,
        "繁體中文" => &TRADITIONAL,
        _ => &ENGLISH,
    }
}

const ENGLISH: UsbStrings = UsbStrings {
    title: "USB switching", enabled: "Switch input when USB connects", device: "Watched USB device", refresh: "Refresh devices",
    connect: "Input for this computer",
    hint: "Select this computer's monitor input. Custom numeric input codes are supported.",
    footer: "Choose the keyboard or receiver moved by your USB switch. Only this device's arrival switches to this computer. Removal, other devices, startup and wake do not switch. Identical VID:PID devices are grouped. Confirm here, then save with OK in the main settings window.",
    invalid: "Enter a valid input code (1–65535) for this computer.",
    choose: "Choose the watched device (VID:PID) and this computer's input.",
};
const CHINESE: UsbStrings = UsbStrings {
    title: "USB 联动", enabled: "USB 接入时切换输入源", device: "监听的 USB 设备", refresh: "刷新设备",
    connect: "本机输入源",
    hint: "选择这台电脑连接的显示器输入源，支持自定义数字代码。",
    footer: "选择随切换器移动的键盘或接收器。仅所选设备接入时切到本机；拔出、其他设备插拔、启动和唤醒均不切屏。相同 VID:PID 的同型号设备按一组识别。此处确定后，请在主设置窗口确定保存。",
    invalid: "请输入本机的有效输入源代码（1–65535）。",
    choose: "请选择监听设备（VID:PID）及本机输入源。",
};
const TRADITIONAL: UsbStrings = UsbStrings {
    title: "USB 連動", enabled: "USB 接入時切換輸入來源", device: "監聽的 USB 裝置", refresh: "重新整理裝置",
    connect: "本機輸入來源",
    hint: "選擇這台電腦連接的螢幕輸入來源，支援自訂數字代碼。",
    footer: "選擇隨切換器移動的鍵盤或接收器。僅所選裝置接入時切至本機；拔出、其他裝置插拔、啟動及喚醒均不切換。相同 VID:PID 的同型號裝置視為一組。此處確定後，請於主設定視窗確定儲存。",
    invalid: "請輸入本機的有效輸入來源代碼（1–65535）。",
    choose: "請選擇監聽裝置（VID:PID）及本機輸入來源。",
};
