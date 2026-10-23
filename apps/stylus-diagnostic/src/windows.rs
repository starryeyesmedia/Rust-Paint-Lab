//! All Win32 interaction lives here. This diagnostic deliberately has no canvas.
use std::{cell::RefCell, collections::BTreeMap, collections::VecDeque, io, ptr};

use paint_input::{Buttons, Phase, PointerSample, Tool, normalize_pressure, normalize_tilt};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::LibraryLoader::GetModuleHandleW,
    UI::{HiDpi::*, Input::Pointer::*, WindowsAndMessaging::*},
};

const INTRO: &str = "Rust Paint Lab - Windows pointer diagnostic\r\n\
    Move the pen over this client area; vary pressure, tilt, and barrel button.\r\n\
    None = unavailable, not zero. Touch IDs are independent; gestures are not implemented.\r\n\
    Live data only; no files, network, or telemetry. Close the window to exit.\r\n";

#[derive(Default)]
struct State {
    total: u64,
    active: BTreeMap<u64, Tool>,
    latest: String,
    recent: VecDeque<String>,
}

impl State {
    fn record(&mut self, text: String) {
        self.total += 1;
        self.recent.push_front(text);
        self.recent.truncate(10);
    }

    fn display(&self) -> String {
        format!(
            "{INTRO}\r\nEvents received: {}  Active contacts: {:?}\r\n\r\n{}\r\n\r\nRecent events (newest first):\r\n{}",
            self.total,
            self.active,
            self.latest,
            self.recent.iter().cloned().collect::<Vec<_>>().join("\r\n")
        )
    }
}

thread_local! {
    static STATE: RefCell<State> = RefCell::new(State::default());
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}

/// Read the current event while processing its message; the OS invalidates its
/// event-specific pointer data after dispatch advances to the next message.
fn read_sample(hwnd: HWND, id: u32, message: u32) -> io::Result<(PointerSample, String)> {
    // SAFETY: hwnd comes from our WndProc. All output buffers are initialized,
    // correctly typed, and live for these synchronous Win32 calls. Results are
    // only used after successful return. No borrowed pointers escape this scope.
    unsafe {
        let mut info = POINTER_INFO::default();
        if GetPointerInfo(id, &mut info) == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut point = info.ptPixelLocation;
        if ScreenToClient(hwnd, &mut point) == 0 {
            return Err(io::Error::last_os_error());
        }
        let flags = info.pointerFlags;
        let contact = flags & POINTER_FLAG_INCONTACT != 0;
        let phase = if flags & POINTER_FLAG_CANCELED != 0 || message == WM_POINTERLEAVE {
            Phase::Cancel
        } else if message == WM_POINTERUP {
            Phase::Up
        } else if message == WM_POINTERDOWN {
            Phase::Down
        } else if contact {
            Phase::Move
        } else {
            Phase::Hover
        };
        let mut sample = PointerSample {
            id: id.into(),
            timestamp_ms: info.dwTime.into(),
            tool: match info.pointerType {
                PT_PEN => Tool::Pen,
                PT_TOUCH => Tool::Touch,
                PT_MOUSE => Tool::Mouse,
                _ => Tool::Unknown,
            },
            phase,
            position_px: [point.x.into(), point.y.into()],
            pressure: None,
            tilt_degrees: [None, None],
            buttons: Buttons {
                primary: flags & POINTER_FLAG_FIRSTBUTTON != 0,
                secondary: flags & POINTER_FLAG_SECONDBUTTON != 0,
                ..Buttons::default()
            },
            in_contact: contact,
            in_range: flags & POINTER_FLAG_INRANGE != 0,
        };
        let mut detail = String::new();
        if sample.tool == Tool::Pen {
            let mut pen = POINTER_PEN_INFO::default();
            if GetPointerPenInfo(id, &mut pen) == 0 {
                detail = format!("GetPointerPenInfo failed: {}", io::Error::last_os_error());
            } else {
                if pen.penMask & PEN_MASK_PRESSURE != 0 {
                    sample.pressure = normalize_pressure(pen.pressure as f32, 1024.0);
                }
                if pen.penMask & PEN_MASK_TILT_X != 0 {
                    sample.tilt_degrees[0] = normalize_tilt(pen.tiltX as f32);
                }
                if pen.penMask & PEN_MASK_TILT_Y != 0 {
                    sample.tilt_degrees[1] = normalize_tilt(pen.tiltY as f32);
                }
                sample.buttons.barrel = pen.penFlags & PEN_FLAG_BARREL != 0;
                sample.buttons.eraser = pen.penFlags & PEN_FLAG_ERASER != 0;
                sample.buttons.inverted = pen.penFlags & PEN_FLAG_INVERTED != 0;
                detail = format!(
                    "pen mask=0x{:X} flags=0x{:X} raw pressure={} tilt=({}, {}) rotation={}",
                    pen.penMask, pen.penFlags, pen.pressure, pen.tiltX, pen.tiltY, pen.rotation
                );
            }
        }
        let raw = format!(
            "msg=0x{message:04X} frame={} time={}ms flags=0x{flags:X} button-change={} history={}\r\n{detail}",
            info.frameId, info.dwTime, info.ButtonChangeType, info.historyCount
        );
        Ok((sample, raw))
    }
}

unsafe extern "system" fn wndproc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: Windows invokes this callback with a live window and message-valid
    // parameters. Message-specific pointers are used only in the matching arm.
    // Win32 buffers remain alive for each call; no state borrow spans a Win32
    // call that could reenter this procedure. All other messages go to DefWindowProcW.
    unsafe {
        match message {
            WM_POINTERDOWN | WM_POINTERUPDATE | WM_POINTERUP | WM_POINTERENTER
            | WM_POINTERLEAVE => {
                let id = (w & 0xffff) as u32;
                let result = read_sample(hwnd, id, message);
                STATE.with_borrow_mut(|state| match result {
                    Ok((sample, raw)) => {
                        if matches!(sample.phase, Phase::Up | Phase::Cancel) || !sample.in_contact {
                            state.active.remove(&sample.id);
                        } else if state.active.len() < 32 || state.active.contains_key(&sample.id) {
                            state.active.insert(sample.id, sample.tool);
                        }
                        state.latest = format!(
                            "ID={} {:?} {:?} position=({:.0}, {:.0}) client physical px\r\npressure={:?} tilt degrees={:?}\r\nbuttons={:?}\r\ncontact={} range={}\r\n{}",
                            sample.id, sample.tool, sample.phase,
                            sample.position_px[0], sample.position_px[1], sample.pressure,
                            sample.tilt_degrees, sample.buttons, sample.in_contact, sample.in_range, raw
                        );
                        state.record(format!("id={} {:?} {:?} t={} pressure={:?}", sample.id, sample.tool, sample.phase, sample.timestamp_ms, sample.pressure));
                    }
                    Err(error) => {
                        if matches!(message, WM_POINTERUP | WM_POINTERLEAVE) {
                            state.active.remove(&u64::from(id));
                        }
                        state.latest = format!("GetPointerInfo/ScreenToClient: {error}");
                        state.record(format!("id={id} msg=0x{message:04X}: {error}"));
                    }
                });
                InvalidateRect(hwnd, ptr::null(), 1);
                0
            }
            WM_POINTERCAPTURECHANGED => {
                let id = (w & 0xffff) as u64;
                STATE.with_borrow_mut(|state| {
                    state.active.remove(&id);
                    state.latest = format!("id={id}: capture changed; contact cancelled locally");
                    state.record(format!("id={id} Cancel (capture changed)"));
                });
                InvalidateRect(hwnd, ptr::null(), 1);
                0
            }
            WM_PAINT => {
                let text = STATE.with_borrow(State::display);
                let mut paint = PAINTSTRUCT::default();
                let dc = BeginPaint(hwnd, &mut paint);
                if !dc.is_null() {
                    let mut rect = RECT::default();
                    GetClientRect(hwnd, &mut rect);
                    rect.left += 12;
                    rect.top += 12;
                    let mut text = wide(&text);
                    DrawTextW(
                        dc,
                        text.as_mut_ptr(),
                        -1,
                        &mut rect,
                        DT_LEFT | DT_TOP | DT_NOPREFIX | DT_WORDBREAK,
                    );
                }
                EndPaint(hwnd, &paint);
                0
            }
            WM_DPICHANGED => {
                let suggested = &*(l as *const RECT);
                SetWindowPos(
                    hwnd,
                    ptr::null_mut(),
                    suggested.left,
                    suggested.top,
                    suggested.right - suggested.left,
                    suggested.bottom - suggested.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
                0
            }
            WM_DESTROY => {
                PostQuitMessage(0);
                0
            }
            _ => DefWindowProcW(hwnd, message, w, l),
        }
    }
}

pub fn run() -> io::Result<()> {
    // SAFETY: This thread owns the window and pumps messages until WM_QUIT.
    // UTF-16 strings outlive registration/creation calls. Registered callback
    // has static code lifetime; system-owned cursor/brush are not freed by us.
    unsafe {
        if SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) == 0 {
            return Err(io::Error::last_os_error());
        }
        let instance = GetModuleHandleW(ptr::null());
        if instance.is_null() {
            return Err(io::Error::last_os_error());
        }
        let class = wide("RustPaintLabStylusDiagnostic");
        let title = wide("Rust Paint Lab - Stylus Diagnostic");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wndproc),
            hInstance: instance,
            hCursor: LoadCursorW(ptr::null_mut(), IDC_CROSS),
            hbrBackground: (COLOR_WINDOW + 1) as HBRUSH,
            lpszClassName: class.as_ptr(),
            ..WNDCLASSW::default()
        };
        if RegisterClassW(&wc) == 0 {
            return Err(io::Error::last_os_error());
        }
        let window = CreateWindowExW(
            0,
            class.as_ptr(),
            title.as_ptr(),
            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            1100,
            800,
            ptr::null_mut(),
            ptr::null_mut(),
            instance,
            ptr::null(),
        );
        if window.is_null() {
            return Err(io::Error::last_os_error());
        }
        let mut message = MSG::default();
        loop {
            match GetMessageW(&mut message, ptr::null_mut(), 0, 0) {
                -1 => return Err(io::Error::last_os_error()),
                0 => return Ok(()),
                _ => {
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }
    }
}
