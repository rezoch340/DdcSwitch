// USB settings currently have English and Chinese copy; other languages fall back to English.
struct USBStrings {
    let title: String
    let enabled: String
    let device: String
    let refresh: String
    let connect: String
    let disconnect: String
    let hint: String
    let footer: String
    let invalid: String
    let choose: String

    static var current: USBStrings {
        switch I18n.currentCode {
        case "zh-Hans": return chinese
        case "zh-Hant": return traditional
        default: return english
        }
    }

    static let english = USBStrings(
        title: "USB switching", enabled: "Switch input on USB changes", device: "USB device", refresh: "Refresh devices",
        connect: "When connected", disconnect: "When disconnected",
        hint: "Leave an action blank to do nothing. Enter a numeric input code if your monitor uses custom values.",
        footer: "Choose a keyboard, receiver or hub that moves with your USB switch. Use “When disconnected” to let the current computer hand the display to the other one. Devices with the same VID:PID are treated as one group; only the first connection and last disconnection trigger an action. Startup and wake do not switch inputs.",
        invalid: "Enter a valid input code (1–65535), or leave the action blank.",
        choose: "Choose a device (VID:PID) and set at least one action.")
    static let chinese = USBStrings(
        title: "USB 联动", enabled: "USB 插拔时切换输入源", device: "USB 设备", refresh: "刷新设备",
        connect: "接入时切换到", disconnect: "拔出时切换到",
        hint: "动作留空表示不切换。显示器使用自定义值时，可直接输入数字代码。",
        footer: "选择随 USB 切换器移动的键盘、接收器或集线器。建议设置“拔出时切换到”，让当前电脑把画面交给另一台。相同 VID:PID 的设备按一组识别，仅首次接入和最后拔出触发。启动和唤醒不会切换输入源。",
        invalid: "请输入有效输入源代码（1–65535），或留空禁用该动作。",
        choose: "请选择设备（VID:PID），并至少设置一个切换动作。")
    static let traditional = USBStrings(
        title: "USB 連動", enabled: "USB 插拔時切換輸入來源", device: "USB 裝置", refresh: "重新整理裝置",
        connect: "接入時切換至", disconnect: "拔出時切換至",
        hint: "動作留空表示不切換。螢幕使用自訂值時，可直接輸入數字代碼。",
        footer: "選擇隨 USB 切換器移動的鍵盤、接收器或集線器。建議設定「拔出時切換至」，讓目前電腦把畫面交給另一台。相同 VID:PID 的裝置視為一組，僅首次接入和最後拔出觸發。啟動及喚醒不會切換輸入來源。",
        invalid: "請輸入有效的輸入來源代碼（1–65535），或留空停用此動作。",
        choose: "請選擇裝置（VID:PID），並至少設定一個切換動作。")
}
