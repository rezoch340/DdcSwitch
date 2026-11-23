//! USB identity and edge detection, independent of the platform notification API.

use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};

pub const TARGET_SLOTS: usize = 2;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsbTarget {
    pub enabled: bool,
    pub monitor: usize,
    pub name: String,
    pub input: Option<u32>,
}

impl UsbTarget {
    pub fn valid(&self) -> bool {
        self.monitor > 0 && !self.name.trim().is_empty() && self.input.is_some_and(|input| (1..=65535).contains(&input))
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsbConfig {
    pub enabled: bool,
    pub device: String,
    pub device_name: String,
    pub targets: [UsbTarget; TARGET_SLOTS],
}

impl UsbConfig {
    pub fn active(&self) -> bool {
        self.enabled
            && device_id(&self.device).is_some()
            && self.targets.iter().any(|target| target.enabled && target.valid())
    }

    pub fn triggered(&self, connected: bool) -> bool {
        self.active() && !connected
    }

    pub fn device_label(&self) -> String {
        if self.device_name.is_empty() {
            self.device.clone()
        } else {
            format!("{} — {}", self.device, self.device_name)
        }
    }
}

/// A new epoch invalidates queued work even if the app resumes before it gets the DDC lock.
#[derive(Clone)]
pub struct RunGate(Arc<AtomicU64>);

impl Default for RunGate {
    fn default() -> Self {
        Self(Arc::new(AtomicU64::new(1)))
    }
}

#[derive(Clone)]
pub struct RunTicket {
    gate: RunGate,
    epoch: u64,
}

impl RunGate {
    pub fn pause(&self) {
        let _ = self.0.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |epoch| Some(epoch.wrapping_add(2) | 1));
    }
    pub fn resume(&self) {
        let _ = self.0.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |epoch| Some(epoch.wrapping_add(2) & !1));
    }
    pub fn ticket(&self) -> RunTicket {
        RunTicket { gate: self.clone(), epoch: self.0.load(Ordering::SeqCst) }
    }
}

impl RunTicket {
    pub fn valid(&self) -> bool {
        self.epoch & 1 == 0 && self.gate.0.load(Ordering::SeqCst) == self.epoch
    }
}

/// Resolve all targets against one snapshot. Never fall back to an unrelated screen.
/// A failed/missing screen does not prevent the next screen from being attempted.
pub fn execute_targets(
    targets: &[UsbTarget],
    names: &[String],
    permitted: impl Fn() -> bool,
    mut write: impl FnMut(usize, u32) -> Result<(), String>,
) -> Vec<Result<String, String>> {
    let mut results = Vec::new();
    let mut used = Vec::new();
    for target in targets.iter().filter(|target| target.enabled) {
        if !permitted() {
            break;
        }
        let matching: Vec<usize> =
            names.iter().enumerate().filter_map(|(index, name)| (*name == target.name).then_some(index)).collect();
        let selected = match matching.as_slice() {
            [index] => Some(*index),
            _ => target.monitor.checked_sub(1).filter(|index| matching.contains(index)),
        };
        if !target.valid() || selected.is_none() {
            results.push(Err(format!("USB target unavailable or invalid: {}", target.name)));
            continue;
        }
        let selected = selected.unwrap();
        if used.contains(&selected) {
            results.push(Err(format!("Duplicate USB target: {}", target.name)));
            continue;
        }
        used.push(selected);
        let result = write(selected, target.input.unwrap());
        if !permitted() {
            break;
        }
        results.push(result.map(|_| format!("{} → {}", target.name, target.input.unwrap())));
    }
    results
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
    fn only_removals_switch_and_default_is_off() {
        let mut config = UsbConfig::default();
        assert!(!config.active());
        config.enabled = true;
        config.device = "1234:5678".into();
        assert!(!config.active());
        assert!(!config.triggered(false));
        config.targets[0] = UsbTarget { enabled: true, monitor: 1, name: "G7C II".into(), input: Some(7) };
        assert!(config.active());
        assert!(!config.triggered(true));
        assert!(config.triggered(false));
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
    fn only_selected_keyboard_removals_send_commands() {
        let config = UsbConfig {
            enabled: true,
            device: "1234:5678".into(),
            targets: [target("First", 1, 7), UsbTarget::default()],
            ..Default::default()
        };
        let mut presence = Presence::default();
        let snapshots =
            [vec![], vec!["ABCD:EF01"], vec!["ABCD:EF01", "1234:5678"], vec!["1234:5678"], vec![], vec!["1234:5678"]];
        let commands: Vec<bool> = snapshots
            .iter()
            .filter_map(|devices| presence.observe(devices.contains(&config.device.as_str())))
            .filter(|connected| config.triggered(*connected))
            .collect();
        assert_eq!(commands, vec![false]);
    }

    fn target(name: &str, monitor: usize, input: u32) -> UsbTarget {
        UsbTarget { enabled: true, monitor, name: name.into(), input: Some(input) }
    }

    #[test]
    fn pause_cancels_queued_work_even_after_resume() {
        let gate = RunGate::default();
        assert!(!gate.ticket().valid());
        gate.resume();
        let queued = gate.ticket();
        assert!(queued.valid());
        gate.pause();
        assert!(!queued.valid());
        gate.resume();
        assert!(!queued.valid());
        assert!(gate.ticket().valid());
        let targets = [target("First", 1, 7), target("Second", 2, 8)];
        let names = ["First".into(), "Second".into()];
        let current = gate.ticket();
        let mut writes = 0;
        execute_targets(
            &targets,
            &names,
            || current.valid(),
            |_, _| {
                writes += 1;
                gate.pause();
                Ok(())
            },
        );
        assert_eq!(writes, 1);
        execute_targets(
            &targets,
            &names,
            || queued.valid(),
            |_, _| {
                writes += 1;
                Ok(())
            },
        );
        assert_eq!(writes, 1);
    }

    #[test]
    fn batch_follows_names_and_continues_after_failure() {
        let targets = [target("First", 1, 7), target("Second", 2, 8)];
        let mut writes = Vec::new();
        let results = execute_targets(
            &targets,
            &["Second".into(), "First".into()],
            || true,
            |index, input| {
                writes.push((index, input));
                if index == 1 {
                    Err("first screen failed".into())
                } else {
                    Ok(())
                }
            },
        );
        assert_eq!(writes, [(1, 7), (0, 8)]);
        assert!(results[0].is_err());
        assert!(results[1].is_ok());
    }

    #[test]
    fn missing_and_duplicate_targets_never_switch_another_screen() {
        let targets = [target("First", 1, 7), target("Second", 2, 8)];
        let mut writes = Vec::new();
        execute_targets(
            &targets,
            &["Second".into()],
            || true,
            |index, input| {
                writes.push((index, input));
                Ok(())
            },
        );
        assert_eq!(writes, [(0, 8)]);
        writes.clear();
        execute_targets(
            &[targets[0].clone(), targets[0].clone()],
            &["First".into()],
            || true,
            |index, input| {
                writes.push((index, input));
                Ok(())
            },
        );
        assert_eq!(writes, [(0, 7)]);
    }
}
