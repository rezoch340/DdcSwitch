pub struct UsbStrings {
    pub title: &'static str,
    pub enabled: &'static str,
    pub device: &'static str,
    pub refresh: &'static str,
    pub connect: &'static str,
    pub disconnect: &'static str,
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
    title: "USB switching", enabled: "Switch input on USB changes", device: "USB device", refresh: "Refresh devices",
    connect: "When connected", disconnect: "When disconnected",
    hint: "Leave an action blank to do nothing. Custom numeric input codes are supported.",
    footer: "Choose a keyboard, receiver or hub that moves with your USB switch. On disconnection, hand the display to the other computer. Matching VID:PID devices form one group: first arrival / last removal only. Startup and wake do not switch. Confirm here, then save with OK in the main settings window.",
    invalid: "Enter a valid input code (1–65535), or leave the action blank.",
    choose: "Choose a device (VID:PID) and set at least one action.",
};
const CHINESE: UsbStrings = UsbStrings {
    title: "USB 联动", enabled: "USB 插拔时切换输入源", device: "USB 设备", refresh: "刷新设备",
    connect: "接入时切换到", disconnect: "拔出时切换到",
    hint: "动作留空表示不切换。支持直接输入自定义数字代码。",
    footer: "选择随 USB 切换器移动的键盘、接收器或集线器。建议设置拔出动作，让当前电脑把画面交给另一台。相同 VID:PID 按一组识别，仅首次接入和最后拔出触发。启动和唤醒不会切屏。此处确定后，请在主设置窗口确定保存。",
    invalid: "请输入有效输入源代码（1–65535），或留空禁用该动作。",
    choose: "请选择设备（VID:PID），并至少设置一个切换动作。",
};
const TRADITIONAL: UsbStrings = UsbStrings {
    title: "USB 連動", enabled: "USB 插拔時切換輸入來源", device: "USB 裝置", refresh: "重新整理裝置",
    connect: "接入時切換至", disconnect: "拔出時切換至",
    hint: "動作留空表示不切換。支援直接輸入自訂數字代碼。",
    footer: "選擇隨 USB 切換器移動的鍵盤、接收器或集線器。建議設定拔出動作，讓目前電腦把畫面交給另一台。相同 VID:PID 視為一組，僅首次接入和最後拔出觸發。啟動及喚醒不會切換。此處確定後，請於主設定視窗確定儲存。",
    invalid: "請輸入有效的輸入來源代碼（1–65535），或留空停用此動作。",
    choose: "請選擇裝置（VID:PID），並至少設定一個切換動作。",
};
