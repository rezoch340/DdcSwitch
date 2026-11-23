// Owned native dialog; changes are staged in the main settings window until its OK.
use super::*;
use crate::usb::{self, UsbConfig};
use crate::usb_strings::text;

const USB_CLASS: PCWSTR = w!("DdcSwitch.USBSettings");
const IDC_ENABLED: i32 = 2100;
const IDC_DEVICE: i32 = 2101;
const IDC_REFRESH: i32 = 2102;
const IDC_TARGET_BASE: i32 = 2120;

struct TargetUi {
    enabled: HWND,
    monitor: HWND,
    input: HWND,
    choices: Vec<(usize, String)>,
}

struct UsbUi {
    hwnd: HWND,
    parent: HWND,
    enabled: HWND,
    device: HWND,
    targets: [TargetUi; usb::TARGET_SLOTS],
    status: HWND,
}

thread_local! { static USB_UI: RefCell<Option<UsbUi>> = const { RefCell::new(None) }; }

pub(super) fn register_class(hinst: HINSTANCE) {
    unsafe {
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(window_proc),
            hInstance: hinst,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: GetSysColorBrush(COLOR_WINDOW),
            lpszClassName: USB_CLASS,
            ..Default::default()
        };
        RegisterClassExW(&class);
    }
}

pub(super) fn is_dialog_message(message: &MSG) -> bool {
    // IsDialogMessage can synchronously close the dialog; do not hold the RefCell borrow.
    let hwnd = USB_UI.with(|state| state.borrow().as_ref().map(|dialog| dialog.hwnd));
    hwnd.is_some_and(|hwnd| unsafe { IsDialogMessageW(hwnd, message).as_bool() })
}

pub(super) fn open() {
    let Some((parent, font, config)) =
        UI.with(|state| state.borrow().as_ref().map(|state| (state.hwnd, state.font, state.base.usb.clone())))
    else {
        return;
    };
    unsafe {
        let copy = text();
        let dpi = GetDpiForSystem();
        let scale = |value: i32| value * dpi as i32 / 96;
        let hinst = HINSTANCE(HINST.with(|state| state.get()) as *mut _);
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU;
        let extended = WS_EX_CONTROLPARENT;
        let mut bounds = RECT { left: 0, top: 0, right: scale(740), bottom: scale(510) };
        let _ = AdjustWindowRectExForDpi(&mut bounds, style, false, extended, dpi);
        let width = bounds.right - bounds.left;
        let height = bounds.bottom - bounds.top;
        let Ok(hwnd) = CreateWindowExW(
            extended,
            USB_CLASS,
            &HSTRING::from(copy.title),
            style,
            (GetSystemMetrics(SM_CXSCREEN) - width) / 2,
            (GetSystemMetrics(SM_CYSCREEN) - height) / 2,
            width,
            height,
            Some(parent),
            None,
            Some(hinst),
            None,
        ) else {
            return;
        };
        let builder = Builder { parent: hwnd, font, dpi: dpi as i32, hinst };
        let enabled = builder.check(copy.enabled, 22, 16, 695, IDC_ENABLED);
        builder.label(copy.device, 22, 54, 160);
        let device = builder.combo(true, 190, 50, 528, IDC_DEVICE);
        builder.button(copy.refresh, 22, 86, 240, IDC_REFRESH, false);
        let targets = std::array::from_fn(|slot| {
            let offset = slot as i32 * 3;
            let top = 134 + slot as i32 * 94;
            let enabled =
                builder.check(&format!("{} {}", copy.monitor, slot + 1), 22, top, 150, IDC_TARGET_BASE + offset);
            let monitor = builder.combo(false, 190, top - 2, 528, IDC_TARGET_BASE + offset + 1);
            builder.label(copy.disconnect, 22, top + 38, 220);
            let input = builder.combo(true, 250, top + 34, 220, IDC_TARGET_BASE + offset + 2);
            fill_input_combo(input, STANDARD_INPUTS);
            set_checked(enabled, config.targets[slot].enabled);
            set_text(input, &config.targets[slot].input.map(|input| input.to_string()).unwrap_or_default());
            TargetUi { enabled, monitor, input, choices: Vec::new() }
        });
        builder.add(w!("STATIC"), copy.hint, 0, WINDOW_EX_STYLE(0), 22, 326, 695, 36, 0);
        builder.add(w!("STATIC"), copy.protection, 0, WINDOW_EX_STYLE(0), 22, 365, 695, 36, 0);
        builder.add(w!("STATIC"), copy.footer, 0, WINDOW_EX_STYLE(0), 22, 405, 695, 36, 0);
        let status = builder.label("", 22, 444, 695);
        builder.button(t().btn_ok, 506, 474, 100, IDC_OK, true);
        builder.button(t().btn_cancel, 618, 474, 100, IDC_CANCEL, false);
        set_checked(enabled, config.enabled);
        set_text(device, &config.device_label());
        USB_UI.with(|state| *state.borrow_mut() = Some(UsbUi { hwnd, parent, enabled, device, targets, status }));
        refresh(Some(&config));
        update_enabled();
        enable(parent, false);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}

fn refresh(initial: Option<&UsbConfig>) {
    let devices = usb::list_devices();
    let names = ddc::list_names();
    USB_UI.with(|state| {
        let mut state = state.borrow_mut();
        let Some(dialog) = state.as_mut() else { return };
        for (slot, target) in dialog.targets.iter_mut().enumerate() {
            let previous = initial
                .map(|config| (config.targets[slot].monitor, config.targets[slot].name.clone()))
                .or_else(|| selected_target(target));
            target.choices = names.iter().enumerate().map(|(index, name)| (index + 1, name.clone())).collect();
            let selected = previous.and_then(|(number, name)| {
                if name.is_empty() {
                    return None;
                }
                if let Some(index) = target.choices.iter().position(|choice| *choice == (number, name.clone())) {
                    return Some(index);
                }
                if let Some(index) = target.choices.iter().position(|choice| choice.1 == name) {
                    return Some(index);
                }
                target.choices.push((number, name));
                Some(target.choices.len() - 1)
            });
            send(target.monitor, CB_RESETCONTENT, 0, 0);
            for (number, name) in &target.choices {
                let suffix = if names.contains(name) { "" } else { text().offline };
                add_item(target.monitor, &format!("{number}. {name} {suffix}"));
            }
            send(target.monitor, CB_SETCURSEL, selected.map(|index| index as usize).unwrap_or(usize::MAX), 0);
        }
        match devices {
            Ok(devices) => {
                let previous = get_text(dialog.device);
                send(dialog.device, CB_RESETCONTENT, 0, 0);
                let identity = usb::device_id(&previous);
                let mut selected_label = previous;
                for (device_id, name) in devices {
                    let label = format!("{device_id} — {name}");
                    if identity.as_ref() == Some(&device_id) {
                        selected_label = label.clone();
                    }
                    add_item(dialog.device, &label);
                }
                set_text(dialog.device, &selected_label);
                set_text(dialog.status, "");
            }
            Err(error) => set_text(dialog.status, &format!("USB: {error}")),
        }
    });
}

fn update_enabled() {
    USB_UI.with(|state| {
        if let Some(dialog) = state.borrow().as_ref() {
            let enabled = checked(dialog.enabled);
            enable(dialog.device, enabled);
            for target in &dialog.targets {
                enable(target.enabled, enabled);
                enable(target.monitor, enabled && checked(target.enabled));
                enable(target.input, enabled && checked(target.enabled));
            }
        }
    });
}

fn selected_target(target: &TargetUi) -> Option<(usize, String)> {
    let selected = send(target.monitor, CB_GETCURSEL, 0, 0);
    usize::try_from(selected).ok().and_then(|index| target.choices.get(index).cloned())
}

fn accept(hwnd: HWND) {
    let result = USB_UI.with(|state| {
        let state = state.borrow();
        let dialog = state.as_ref()?;
        let enabled = checked(dialog.enabled);
        let device = get_text(dialog.device);
        let config = UsbConfig {
            enabled,
            device: usb::device_id(&device).unwrap_or_default(),
            device_name: device.split_once(" — ").map(|(_, name)| name.to_string()).unwrap_or_default(),
            targets: std::array::from_fn(|slot| {
                let target = &dialog.targets[slot];
                let (monitor, name) = selected_target(target).unwrap_or_default();
                usb::UsbTarget {
                    enabled: checked(target.enabled),
                    monitor,
                    name,
                    input: config::parse_input(&get_text(target.input)),
                }
            }),
        };
        if enabled {
            if config.targets.iter().any(|target| target.enabled && target.input.is_none()) {
                return Some(Err(text().invalid));
            }
            if !config.active() || config.targets.iter().any(|target| target.enabled && !target.valid()) {
                return Some(Err(text().choose));
            }
            if config.targets[0].enabled
                && config.targets[1].enabled
                && config.targets[0].monitor == config.targets[1].monitor
                && config.targets[0].name == config.targets[1].name
            {
                return Some(Err(text().duplicate));
            }
        }
        Some(Ok(config))
    });
    match result {
        Some(Ok(config)) => {
            UI.with(|state| {
                if let Some(state) = state.borrow_mut().as_mut() {
                    state.base.usb = config;
                }
            });
            update_enabled_states();
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
        }
        Some(Err(error)) => message_box(hwnd, error, MB_ICONERROR),
        None => {}
    }
}

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match message {
        WM_COMMAND => {
            match (wparam.0 & 0xFFFF) as i32 {
                IDC_OK => accept(hwnd),
                IDC_CANCEL => {
                    let _ = DestroyWindow(hwnd);
                }
                IDC_REFRESH => refresh(None),
                IDC_ENABLED => update_enabled(),
                control if (IDC_TARGET_BASE..IDC_TARGET_BASE + 6).contains(&control) => update_enabled(),
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        WM_DESTROY => {
            if let Some(dialog) = USB_UI.with(|state| state.borrow_mut().take()) {
                enable(dialog.parent, true);
                let _ = SetForegroundWindow(dialog.parent);
            }
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC => {
            SetBkColor(HDC(wparam.0 as *mut _), COLORREF(GetSysColor(COLOR_WINDOW)));
            LRESULT(GetSysColorBrush(COLOR_WINDOW).0 as isize)
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}
