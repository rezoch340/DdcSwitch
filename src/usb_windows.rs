use super::{windows_instance_identity, Presence, UsbConfig};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use windows::core::w;
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, HWND};
use windows::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

pub const USB_TIMER: usize = 0xDDC1;

/// Enumerate physical USB nodes, excluding composite interface children (MI_xx).
pub fn list_devices() -> Result<Vec<(String, String)>, String> {
    unsafe {
        let devices = SetupDiGetClassDevsW(None, w!("USB"), None, DIGCF_PRESENT | DIGCF_ALLCLASSES)
            .map_err(|error| error.to_string())?;
        let result = (|| {
            let mut found = BTreeMap::new();
            let mut index = 0;
            loop {
                let mut info =
                    SP_DEVINFO_DATA { cbSize: std::mem::size_of::<SP_DEVINFO_DATA>() as u32, ..Default::default() };
                if let Err(error) = SetupDiEnumDeviceInfo(devices, index, &mut info) {
                    if error.code() == ERROR_NO_MORE_ITEMS.to_hresult() {
                        break;
                    }
                    return Err(error.to_string());
                }
                index += 1;
                let mut instance = [0u16; 512];
                // A partial snapshot must never be interpreted as a disconnection.
                SetupDiGetDeviceInstanceIdW(devices, &info, Some(&mut instance), None)
                    .map_err(|error| error.to_string())?;
                let instance = String::from_utf16_lossy(
                    &instance[..instance.iter().position(|unit| *unit == 0).unwrap_or(instance.len())],
                );
                let Some(identity) = windows_instance_identity(&instance) else { continue };
                let name = property(devices, &info, SPDRP_FRIENDLYNAME)
                    .or_else(|| property(devices, &info, SPDRP_DEVICEDESC))
                    .unwrap_or_else(|| identity.clone());
                found.entry(identity).or_insert(name);
            }
            Ok(found.into_iter().collect())
        })();
        let _ = SetupDiDestroyDeviceInfoList(devices);
        result
    }
}

unsafe fn property(devices: HDEVINFO, info: &SP_DEVINFO_DATA, key: SETUP_DI_REGISTRY_PROPERTY) -> Option<String> {
    let mut buffer = [0u8; 2048];
    SetupDiGetDeviceRegistryPropertyW(devices, info, key, None, Some(&mut buffer), None).ok()?;
    let units: Vec<u16> = buffer
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    Some(String::from_utf16_lossy(&units))
}

#[derive(Default)]
pub struct Watcher {
    config: UsbConfig,
    presence: Presence,
    suspended: bool,
    resume_until: Option<Instant>,
}

impl Watcher {
    pub fn configure(&mut self, hwnd: HWND, config: &UsbConfig) {
        unsafe {
            let _ = KillTimer(Some(hwnd), USB_TIMER);
        }
        self.config = config.clone();
        self.presence.reset();
        if self.config.active() && !self.suspended {
            if self.resume_until.is_some() {
                // A settings save must not cancel the pending wake rebaseline.
                self.changed(hwnd);
            } else {
                self.sample(); // establish a baseline, never switch on save/start
            }
        }
    }

    pub fn changed(&self, hwnd: HWND) {
        if self.config.active() && !self.suspended {
            unsafe {
                SetTimer(Some(hwnd), USB_TIMER, 350, None);
            }
        }
    }

    pub fn suspend(&mut self, hwnd: HWND) {
        self.suspended = true;
        self.resume_until = None;
        self.presence.reset();
        unsafe {
            let _ = KillTimer(Some(hwnd), USB_TIMER);
        }
    }

    pub fn resume(&mut self, hwnd: HWND) {
        self.suspended = false;
        self.presence.reset();
        self.resume_until = Some(Instant::now() + Duration::from_secs(2));
        self.changed(hwnd);
    }

    pub fn timer(&mut self, hwnd: HWND) -> Option<u32> {
        unsafe {
            let _ = KillTimer(Some(hwnd), USB_TIMER);
        }
        if !self.config.active() || self.suspended {
            return None;
        }
        if let Some(until) = self.resume_until {
            if Instant::now() < until {
                self.changed(hwnd);
                return None;
            }
            self.resume_until = None;
            self.presence.reset();
        }
        self.sample().and_then(|connected| {
            crate::applog::log(format!(
                "USB {} {}",
                self.config.device,
                if connected { "connected" } else { "disconnected" }
            ));
            self.config.input(connected)
        })
    }

    fn sample(&mut self) -> Option<bool> {
        match list_devices() {
            Ok(devices) => self.presence.observe(devices.iter().any(|(identity, _)| *identity == self.config.device)),
            Err(error) => {
                crate::applog::log(format!("USB enumeration failed: {error}"));
                None
            }
        }
    }
}
