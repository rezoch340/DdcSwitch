// Owned native dialog; changes are staged in the main settings window until its OK.
use super::*;
use crate::usb::{self, UsbConfig};
use crate::usb_strings::text;

const USB_CLASS: PCWSTR = w!("DdcSwitch.USBSettings");
const IDC_ENABLED: i32 = 2100;
const IDC_DEVICE: i32 = 2101;
const IDC_REFRESH: i32 = 2102;
const IDC_CONNECT: i32 = 2103;
const IDC_DISCONNECT: i32 = 2104;

struct UsbUi {
    hwnd: HWND,
    parent: HWND,
    enabled: HWND,
    device: HWND,
    connect: HWND,
    disconnect: HWND,
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
        let mut bounds = RECT { left: 0, top: 0, right: scale(700), bottom: scale(360) };
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
        let enabled = builder.check(copy.enabled, 22, 16, 650, IDC_ENABLED);
        builder.label(copy.device, 22, 54, 120);
        let device = builder.combo(true, 150, 50, 380, IDC_DEVICE);
        builder.button(copy.refresh, 540, 49, 138, IDC_REFRESH, false);
        builder.label(copy.connect, 22, 94, 180);
        let connect = builder.combo(true, 218, 90, 200, IDC_CONNECT);
        builder.label(copy.disconnect, 22, 134, 180);
        let disconnect = builder.combo(true, 218, 130, 200, IDC_DISCONNECT);
        builder.label(copy.hint, 22, 171, 655);
        builder.add(w!("STATIC"), copy.footer, 0, WINDOW_EX_STYLE(0), 22, 203, 655, 70, 0);
        let status = builder.label("", 22, 280, 655);
        builder.button(t().btn_ok, 466, 320, 100, IDC_OK, true);
        builder.button(t().btn_cancel, 578, 320, 100, IDC_CANCEL, false);
        set_checked(enabled, config.enabled);
        for control in [connect, disconnect] {
            fill_input_combo(control, STANDARD_INPUTS);
            add_item(control, "");
        }
        set_text(device, &config.device);
        set_text(connect, &config.connect_input.map(|input| input.to_string()).unwrap_or_default());
        set_text(disconnect, &config.disconnect_input.map(|input| input.to_string()).unwrap_or_default());
        USB_UI.with(|state| {
            *state.borrow_mut() = Some(UsbUi { hwnd, parent, enabled, device, connect, disconnect, status })
        });
        refresh();
        update_enabled();
        enable(parent, false);
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
}

fn refresh() {
    let devices = usb::list_devices();
    USB_UI.with(|state| {
        let state = state.borrow();
        let Some(dialog) = state.as_ref() else { return };
        match devices {
            Ok(devices) => {
                let previous = get_text(dialog.device);
                send(dialog.device, CB_RESETCONTENT, 0, 0);
                for (identity, name) in devices {
                    add_item(dialog.device, &format!("{identity} — {name}"));
                }
                set_text(dialog.device, &previous);
                set_text(dialog.status, "");
            }
            Err(error) => set_text(dialog.status, &format!("USB: {error}")),
        }
    });
}

fn update_enabled() {
    USB_UI.with(|state| {
        if let Some(dialog) = state.borrow().as_ref() {
            for control in [dialog.device, dialog.connect, dialog.disconnect] {
                enable(control, checked(dialog.enabled));
            }
        }
    });
}

fn accept(hwnd: HWND) {
    let result = USB_UI.with(|state| {
        let state = state.borrow();
        let dialog = state.as_ref()?;
        let enabled = checked(dialog.enabled);
        let connect = get_text(dialog.connect);
        let disconnect = get_text(dialog.disconnect);
        let config = UsbConfig {
            enabled,
            device: usb::device_id(&get_text(dialog.device)).unwrap_or_default(),
            connect_input: config::parse_input(&connect),
            disconnect_input: config::parse_input(&disconnect),
        };
        if enabled {
            if [&connect, &disconnect]
                .iter()
                .any(|value| !value.trim().is_empty() && config::parse_input(value).is_none())
            {
                return Some(Err(text().invalid));
            }
            if !config.active() {
                return Some(Err(text().choose));
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
                IDC_REFRESH => refresh(),
                IDC_ENABLED => update_enabled(),
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
