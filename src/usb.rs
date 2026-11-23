//! USB identity and edge detection, independent of the platform notification API.

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsbConfig {
    pub enabled: bool,
    pub device: String,
    pub connect_input: Option<u32>,
}

impl UsbConfig {
    pub fn active(&self) -> bool {
        self.enabled && device_id(&self.device).is_some() && self.connect_input.is_some()
    }

    pub fn input(&self, connected: bool) -> Option<u32> {
        if connected {
            self.connect_input
        } else {
            None
        }
    }
}

/// VID:PID, in hexadecimal. A description may follow it in an editable picker.
pub fn device_id(text: &str) -> Option<String> {
    let token = text.split_whitespace().next()?;
    let (vendor, product) = token.split_once(':')?;
    if vendor.len() != 4
        || product.len() != 4
        || !vendor.bytes().all(|byte| byte.is_ascii_hexdigit())
        || !product.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return None;
    }
    Some(format!("{}:{}", vendor.to_ascii_uppercase(), product.to_ascii_uppercase()))
}

/// SetupAPI includes composite interface children; watch their physical USB parent only.
pub fn windows_instance_identity(instance: &str) -> Option<String> {
    let upper = instance.to_ascii_uppercase();
    let hardware = upper.strip_prefix("USB\\")?.split('\\').next()?;
    if hardware.contains("&MI_") {
        return None;
    }
    let vendor = hardware.strip_prefix("VID_")?.get(..4)?;
    let product = hardware.split("&PID_").nth(1)?.get(..4)?;
    device_id(&format!("{vendor}:{product}"))
}

/// Presence is aggregated by VID:PID: only the first arrival / last removal is an edge.
/// No edge on startup, configuration changes or the first sample after sleep.
#[derive(Default)]
pub struct Presence {
    previous: Option<bool>,
}

impl Presence {
    pub fn reset(&mut self) {
        self.previous = None;
    }

    pub fn observe(&mut self, present: bool) -> Option<bool> {
        let previous = self.previous.replace(present);
        previous.filter(|before| *before != present).map(|_| present)
    }
}

#[cfg(windows)]
#[path = "usb_windows.rs"]
mod native;
#[cfg(windows)]
pub use native::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_physical_windows_usb_nodes_are_matched() {
        assert_eq!(windows_instance_identity("USB\\VID_0b05&PID_1234\\serial"), Some("0B05:1234".into()));
        assert_eq!(windows_instance_identity("USB\\VID_0B05&PID_1234&MI_00\\serial"), None);
        assert_eq!(windows_instance_identity("HID\\VID_0B05&PID_1234\\serial"), None);
        assert_eq!(windows_instance_identity("USB\\ROOT_HUB30\\serial"), None);
    }

    #[test]
    fn identity_is_strict_and_accepts_picker_labels() {
        assert_eq!(device_id("0b05:1abc — Keyboard"), Some("0B05:1ABC".into()));
        for invalid in ["", "123:4567", "12345:6789", "1234:zzzz", "1234:5678:9999"] {
            assert_eq!(device_id(invalid), None);
        }
    }

    #[test]
    fn baseline_duplicates_and_rebaseline_do_not_trigger() {
        let mut presence = Presence::default();
        assert_eq!(presence.observe(true), None);
        assert_eq!(presence.observe(true), None);
        assert_eq!(presence.observe(false), Some(false));
        assert_eq!(presence.observe(false), None);
        assert_eq!(presence.observe(true), Some(true));
        presence.reset();
        assert_eq!(presence.observe(false), None);
        assert_eq!(presence.observe(true), Some(true));
    }

    #[test]
    fn only_connections_switch_and_default_is_off() {
        let mut config = UsbConfig::default();
        assert!(!config.active());
        config.enabled = true;
        config.device = "1234:5678".into();
        assert!(!config.active());
        assert_eq!(config.input(true), None);
        config.connect_input = Some(8);
        assert!(config.active());
        assert_eq!(config.input(true), Some(8));
        assert_eq!(config.input(false), None);
        config.enabled = false;
        assert!(!config.active());
    }

    #[test]
    fn identical_devices_and_unrelated_changes_do_not_repeat_edges() {
        let mut presence = Presence::default();
        let identity = "1234:5678";
        let snapshots = [
            vec![identity],
            vec![identity, identity],
            vec![identity, "ABCD:EF01"],
            vec!["ABCD:EF01"],
            vec![],
            vec![identity],
        ];
        let expected = [None, None, None, Some(false), None, Some(true)];
        for (devices, result) in snapshots.iter().zip(expected) {
            assert_eq!(presence.observe(devices.contains(&identity)), result);
        }
    }

    #[test]
    fn only_selected_keyboard_arrivals_send_commands() {
        let config = UsbConfig { enabled: true, device: "1234:5678".into(), connect_input: Some(8) };
        let mut presence = Presence::default();
        let snapshots = [
            vec![],
            vec!["ABCD:EF01"],
            vec!["ABCD:EF01", "1234:5678"],
            vec!["1234:5678"],
            vec![],
            vec!["1234:5678"],
        ];
        let commands: Vec<u32> = snapshots
            .iter()
            .filter_map(|devices| presence.observe(devices.contains(&config.device.as_str())))
            .filter_map(|connected| config.input(connected))
            .collect();
        assert_eq!(commands, vec![8, 8]);
    }
}
