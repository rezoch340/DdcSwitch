use super::{windows_instance_identity, Presence, RunGate, UsbConfig};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};
use windows::core::{w, PCWSTR};
use windows::Win32::Devices::DeviceAndDriverInstallation::*;
use windows::Win32::Devices::Properties::{
    DEVPKEY_Device_BusReportedDeviceDesc, DEVPKEY_Device_InstanceId, DEVPROPTYPE, DEVPROP_TYPE_STRING,
};
use windows::Win32::Devices::Usb::GUID_DEVINTERFACE_USB_DEVICE;
use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, HANDLE, HWND};
use windows::Win32::UI::WindowsAndMessaging::{
    KillTimer, RegisterDeviceNotificationW, SetTimer, UnregisterDeviceNotification, DBT_DEVTYP_DEVICEINTERFACE,
    DEVICE_NOTIFY_WINDOW_HANDLE, DEV_BROADCAST_DEVICEINTERFACE_W, HDEVNOTIFY,
};

pub const USB_TIMER: usize = 0xDDC1;

/// Device nodes can linger during PnP teardown. Presence follows enabled USB interfaces.
fn active_device_ids() -> Result<Vec<String>, String> {
    unsafe {
        for _attempt in 0..3 {
            let mut length = 0;
            let status = CM_Get_Device_Interface_List_SizeW(
                &mut length,
                &GUID_DEVINTERFACE_USB_DEVICE,
                PCWSTR::null(),
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            );
            if status != CR_SUCCESS {
                return Err(format!("USB interface size query failed: {}", status.0));
            }
            let mut buffer = vec![0u16; length.max(2) as usize];
            let status = CM_Get_Device_Interface_ListW(
                &GUID_DEVINTERFACE_USB_DEVICE,
                PCWSTR::null(),
                &mut buffer,
                CM_GET_DEVICE_INTERFACE_LIST_PRESENT,
            );
            if status == CR_BUFFER_SMALL {
                continue;
            }
            if status != CR_SUCCESS {
                return Err(format!("USB interface enumeration failed: {}", status.0));
            }
            let mut identities = Vec::new();
            for path in buffer.split_inclusive(|unit| *unit == 0).take_while(|path| path.len() > 1) {
                // Interface paths are opaque: ask Windows for the owning device instance ID.
                let mut instance = [0u16; 512];
                let mut size = std::mem::size_of_val(&instance) as u32;
                let mut kind = DEVPROPTYPE::default();
                let status = CM_Get_Device_Interface_PropertyW(
                    PCWSTR(path.as_ptr()),
                    &DEVPKEY_Device_InstanceId,
                    &mut kind,
                    Some(instance.as_mut_ptr().cast()),
                    &mut size,
                    0,
                );
                if status != CR_SUCCESS || kind != DEVPROP_TYPE_STRING {
                    // A raced/partial snapshot must never turn into a false removal.
                    return Err(format!("USB interface identity query failed: {}", status.0));
                }
                let end = instance.iter().position(|unit| *unit == 0).unwrap_or(instance.len());
                if let Some(identity) = windows_instance_identity(&String::from_utf16_lossy(&instance[..end])) {
                    identities.push(identity);
                }
            }
            return Ok(identities);
        }
        Err("USB interfaces kept changing during enumeration".into())
    }
}

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
                let Some(identity) = windows_instance_identity(&instance) else {
                    continue;
                };
                let name = bus_name(devices, &info)
                    .or_else(|| property(devices, &info, SPDRP_FRIENDLYNAME))
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

unsafe fn bus_name(devices: HDEVINFO, info: &SP_DEVINFO_DATA) -> Option<String> {
    let mut buffer = [0u8; 2048];
    let mut kind = DEVPROPTYPE::default();
    SetupDiGetDevicePropertyW(
        devices,
        info,
        &DEVPKEY_Device_BusReportedDeviceDesc,
        &mut kind,
        Some(&mut buffer),
        None,
        0,
    )
    .ok()?;
    if kind != DEVPROP_TYPE_STRING {
        return None;
    }
    let units: Vec<u16> = buffer
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .take_while(|unit| *unit != 0)
        .collect();
    let name = String::from_utf16_lossy(&units);
    (!name.trim().is_empty()).then_some(name)
}

#[derive(Default)]
pub struct Watcher {
    pub gate: RunGate,
    config: UsbConfig,
    presence: Presence,
    suspended: bool,
    resume_until: Option<Instant>,
    pending_since: Option<Instant>,
    notification: Option<HDEVNOTIFY>,
}

impl Watcher {
    pub fn configure(&mut self, hwnd: HWND, config: &UsbConfig) {
        self.gate.pause();
        unsafe {
            let _ = KillTimer(Some(hwnd), USB_TIMER);
        }
        self.pending_since = None;
        self.config = config.clone();
        self.presence.reset();
        if self.config.active() && !self.suspended {
            self.register_notifications(hwnd);
            self.gate.resume();
            if self.resume_until.is_some() {
                // A settings save must not cancel the pending wake rebaseline.
                self.changed(hwnd);
            } else {
                // Establish a baseline, never switch on save/start.
                if let Err(error) = self.sample() {
                    crate::applog::log(error);
                    self.changed(hwnd);
                }
            }
        }
    }

    fn register_notifications(&mut self, hwnd: HWND) {
        if self.notification.is_some() {
            return;
        }
        let filter = DEV_BROADCAST_DEVICEINTERFACE_W {
            dbcc_size: std::mem::size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32,
            dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE.0,
            dbcc_classguid: GUID_DEVINTERFACE_USB_DEVICE,
            ..Default::default()
        };
        match unsafe {
            RegisterDeviceNotificationW(
                HANDLE(hwnd.0),
                (&filter as *const DEV_BROADCAST_DEVICEINTERFACE_W).cast(),
                DEVICE_NOTIFY_WINDOW_HANDLE,
            )
        } {
            Ok(notification) => {
                self.notification = Some(notification);
                crate::applog::log("USB interface notifications registered");
            }
            Err(error) => crate::applog::log(format!("USB notification registration failed: {error}")),
        }
    }

    pub fn device_changed(&mut self, hwnd: HWND, event: usize) {
        if self.config.active() && !self.suspended {
            crate::applog::log(format!("USB device event 0x{event:04X}"));
            self.changed(hwnd);
        }
    }

    pub fn changed(&mut self, hwnd: HWND) {
        // SetTimer resets an existing timer. Coalesce a burst without postponing the first check.
        if self.config.active() && !self.suspended && self.pending_since.is_none() {
            if unsafe { SetTimer(Some(hwnd), USB_TIMER, 350, None) } != 0 {
                self.pending_since = Some(Instant::now());
            } else {
                crate::applog::log("USB verification timer could not be started");
            }
        }
    }

    pub fn stop(&mut self, hwnd: HWND) {
        self.suspend(hwnd);
        if let Some(notification) = self.notification.take() {
            let _ = unsafe { UnregisterDeviceNotification(notification) };
        }
    }

    pub fn suspend(&mut self, hwnd: HWND) {
        self.gate.pause();
        self.suspended = true;
        self.resume_until = None;
        self.pending_since = None;
        self.presence.reset();
        unsafe {
            let _ = KillTimer(Some(hwnd), USB_TIMER);
        }
    }

    pub fn resume(&mut self, hwnd: HWND) {
        self.gate.resume();
        self.suspended = false;
        self.presence.reset();
        self.resume_until = Some(Instant::now() + Duration::from_secs(2));
        self.changed(hwnd);
    }

    pub fn timer(&mut self, hwnd: HWND) -> Option<()> {
        let queued = self.pending_since.take().map(|started| started.elapsed()).unwrap_or_default();
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
        let started = Instant::now();
        let change = match self.sample() {
            Ok(change) => change,
            Err(error) => {
                crate::applog::log(error);
                self.changed(hwnd);
                return None;
            }
        };
        change.and_then(|connected| {
            if connected {
                self.gate.resume();
            } // Returning the device cancels a queued departure.
            crate::applog::log(format!(
                "USB {} {} (queued {} ms, scan {} ms)",
                self.config.device,
                if connected { "connected" } else { "disconnected" },
                queued.as_millis(),
                started.elapsed().as_millis(),
            ));
            self.config.triggered(connected).then_some(())
        })
    }

    fn sample(&mut self) -> Result<Option<bool>, String> {
        active_device_ids().map(|devices| self.presence.observe(devices.contains(&self.config.device)))
    }
}
