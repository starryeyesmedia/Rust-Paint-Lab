//! Windows-only capture-loss observer. Winit continues to own capture and dispatch.
//! Native pen history and capture notifications share the same event loop. State is looked up by HWND;
//! no Rust allocation address or borrowed reference is stored in native userdata.
use crate::AppEvent;
use paint_input::{
    Buttons, Phase, PointerSample, Tool, lifecycle::MillisecondClock32, normalize_pressure,
    normalize_tilt,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    io,
    rc::Rc,
    sync::{
        Arc, OnceLock,
        atomic::{AtomicU64, Ordering},
        mpsc::{SyncSender, sync_channel},
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::ScreenToClient,
    System::{Performance::QueryPerformanceFrequency, SystemInformation::GetTickCount64},
    UI::{
        Input::Pointer::*,
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::*,
    },
};
use winit::{
    event_loop::EventLoopProxy,
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};
pub struct PenEvent {
    pub sample: PointerSample,
    pub performance_count: u64,
}
use canvas_demo::native_history::{
    HistoryDeduplicator, NativeHistoryFailure, NativeHistoryFailures, NativePenQueue, NativeStamp,
    validate_coordinate_conversion, validate_history_count_query, validate_history_read,
};
const SUBCLASS_ID: usize = 0x52504c35;
const DIAGNOSTIC_COALESCE: Duration = Duration::from_millis(250);
static NATIVE_FAILURE_COUNTS: [AtomicU64; NativeHistoryFailure::ALL.len()] =
    [const { AtomicU64::new(0) }; NativeHistoryFailure::ALL.len()];
static NATIVE_FAILURE_REPORTER: OnceLock<Option<SyncSender<()>>> = OnceLock::new();

/// Native callback failures update fixed counters and enqueue only a nonblocking
/// wake. Formatting, stderr, and the bounded log-file write happen off-thread.
fn ensure_native_failure_reporter() {
    let _ = NATIVE_FAILURE_REPORTER.get_or_init(|| {
        let (sender, receiver) = sync_channel(1);
        let _worker = thread::Builder::new()
            .name("native-pen-diagnostics".into())
            .spawn(move || {
                while receiver.recv().is_ok() {
                    let report_deadline = Instant::now() + DIAGNOSTIC_COALESCE;
                    loop {
                        let remaining =
                            report_deadline.saturating_duration_since(Instant::now());
                        if remaining.is_zero() || receiver.recv_timeout(remaining).is_err() {
                            break;
                        }
                    }

                    let counts = NativeHistoryFailures::from_counts(std::array::from_fn(|index| {
                        NATIVE_FAILURE_COUNTS[index].swap(0, Ordering::Relaxed)
                    }));
                    if counts.is_empty() {
                        continue;
                    }
                    canvas_demo::studio_log!(
                        "NATIVE_PEN_CAPTURE_FAILURE_COUNTS: history_count_query={} invalid_history_count={} history_read={} coordinate_conversion={} pen_event_queue_overflow={}",
                        counts.count(NativeHistoryFailure::HistoryCountQuery),
                        counts.count(NativeHistoryFailure::InvalidHistoryCount),
                        counts.count(NativeHistoryFailure::HistoryRead),
                        counts.count(NativeHistoryFailure::CoordinateConversion),
                        counts.count(NativeHistoryFailure::PenEventQueueOverflow),
                    );
                }
            })
            .ok()?;
        Some(sender)
    });
}

fn record_native_failure(failure: NativeHistoryFailure) {
    let counter = &NATIVE_FAILURE_COUNTS[failure as usize];
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
        Some(count.saturating_add(1))
    });
    if let Some(Some(sender)) = NATIVE_FAILURE_REPORTER.get() {
        let _ = sender.try_send(());
    }
}

struct State {
    releasing: Cell<bool>,
    installed: Cell<bool>,
    proxy: EventLoopProxy<AppEvent>,
    pens: RefCell<NativePenQueue<PenEvent>>,
    clock: RefCell<MillisecondClock32>,
    base_tick: u64,
    base_ms: u64,
    frequency: u64,
    last: RefCell<HistoryDeduplicator>,
}
thread_local! { static STATES: RefCell<BTreeMap<usize, Rc<State>>> = const { RefCell::new(BTreeMap::new()) }; }

pub struct Observer {
    _window: Arc<Window>,
    hwnd: HWND,
    state: Rc<State>,
}
impl Observer {
    pub fn install(
        window: Arc<Window>,
        proxy: EventLoopProxy<AppEvent>,
        epoch: Instant,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let RawWindowHandle::Win32(handle) = window.window_handle()?.as_raw() else {
            return Err("Expected Win32 window".into());
        };
        ensure_native_failure_reporter();
        let hwnd = handle.hwnd.get() as HWND;
        // SAFETY: GetTickCount64 has no pointer or thread-affinity preconditions.
        let boot = unsafe { GetTickCount64() };
        let mut frequency = 0_i64;
        // SAFETY: a valid mutable LARGE_INTEGER output, no native ownership transfer.
        if unsafe { QueryPerformanceFrequency(&mut frequency) } == 0 {
            frequency = 0;
        }
        let mut clock = MillisecondClock32::default();
        let base_tick = clock.extend(boot as u32);
        let state = Rc::new(State {
            releasing: Cell::new(false),
            installed: Cell::new(true),
            proxy,
            pens: RefCell::new(NativePenQueue::default()),
            clock: RefCell::new(clock),
            base_tick,
            base_ms: epoch.elapsed().as_millis() as u64,
            frequency: frequency.max(0) as u64,
            last: RefCell::new(HistoryDeduplicator::default()),
        });
        STATES.with_borrow_mut(|states| {
            states.insert(hwnd as usize, state.clone());
        });
        // SAFETY: Called on the HWND's owning UI thread with a live Arc-owned
        // window. Callback has static code lifetime, no native userdata pointers.
        if unsafe { SetWindowSubclass(hwnd, Some(callback), SUBCLASS_ID, 0) } == 0 {
            STATES.with_borrow_mut(|states| {
                states.remove(&(hwnd as usize));
            });
            return Err(io::Error::other("Cannot observe Windows mouse capture loss").into());
        }
        Ok(Self {
            _window: window,
            hwnd,
            state,
        })
    }
    pub fn drain_pen(&self) -> Vec<PenEvent> {
        self.state.pens.borrow_mut().drain()
    }
    pub fn take_lost(&self) -> bool {
        self.state.pens.borrow_mut().take_lost()
    }
}
impl Drop for Observer {
    fn drop(&mut self) {
        if self.state.installed.replace(false) {
            // SAFETY: Same owning UI thread; Arc keeps HWND alive unless native
            // destruction already removed the callback and cleared installed.
            unsafe {
                RemoveWindowSubclass(self.hwnd, Some(callback), SUBCLASS_ID);
            }
        }
        // Even if removal fails, future callbacks find no state and only forward;
        // no dangling userdata is possible. There is no map borrow across dispatch.
        STATES.with_borrow_mut(|states| {
            states.remove(&(self.hwnd as usize));
        });
    }
}
unsafe extern "system" fn callback(
    hwnd: HWND,
    message: u32,
    w: WPARAM,
    l: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    let state = STATES.with_borrow(|states| states.get(&(hwnd as usize)).cloned());
    // SAFETY: Windows invokes us on the owning thread for a live HWND. All
    // unhandled/native messages are forwarded exactly once. Rc<Cell> state is
    // reentrancy-safe and no RefCell borrow is held during native calls.
    unsafe {
        if let Some(state) = state {
            if matches!(
                message,
                WM_MOUSEMOVE | WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP
            ) && (GetMessageExtraInfo() as usize & 0xffff_ff00) == 0xff51_5700
            {
                return 0; // Pen/touch-promoted mouse; native stream is authoritative.
            }
            if matches!(
                message,
                WM_POINTERDOWN
                    | WM_POINTERUPDATE
                    | WM_POINTERUP
                    | WM_POINTERENTER
                    | WM_POINTERLEAVE
            ) {
                let pointer = (w & 0xffff) as u32;
                let mut kind = 0;
                if GetPointerType(pointer, &mut kind) != 0 && kind == PT_PEN {
                    match pen_history(hwnd, pointer, message, &state) {
                        Ok(events) => {
                            if let Err(failure) = state.pens.borrow_mut().push_batch(events) {
                                record_native_failure(failure);
                            }
                        }
                        Err(failure) => {
                            let failure = state.pens.borrow_mut().fail_closed(failure);
                            record_native_failure(failure);
                        }
                    }
                    let _ = state.proxy.send_event(AppEvent::Wake);
                    return 0;
                }
            }
            if message == WM_POINTERCAPTURECHANGED {
                let result = state.pens.borrow_mut().push(PenEvent {
                    performance_count: 0,
                    sample: PointerSample {
                        source_counter: None,
                        id: (w & 0xffff) as u64,
                        timestamp_ms: state.base_ms,
                        tool: Tool::Pen,
                        phase: Phase::Cancel,
                        position_px: [0.0; 2],
                        pressure: None,
                        tilt_degrees: [None; 2],
                        buttons: Buttons::default(),
                        in_contact: false,
                        in_range: false,
                    },
                });
                if let Err(failure) = result {
                    record_native_failure(failure);
                }
                let _ = state.proxy.send_event(AppEvent::Wake);
            }
            if message == WM_LBUTTONUP {
                // Winit releases implicit capture synchronously while processing
                // mouse-up. That expected notification must not abort a commit.
                let previous = state.releasing.replace(true);
                let result = DefSubclassProc(hwnd, message, w, l);
                state.releasing.set(previous);
                return result;
            }
            if message == WM_CANCELMODE
                || (message == WM_CAPTURECHANGED && l as HWND != hwnd && !state.releasing.get())
            {
                state.pens.borrow_mut().mark_lost();
                let _ = state.proxy.send_event(AppEvent::Wake); // Wake even when live mode is idle.
            }
            if message == WM_NCDESTROY {
                state.installed.set(false);
                RemoveWindowSubclass(hwnd, Some(callback), SUBCLASS_ID);
                STATES.with_borrow_mut(|states| {
                    states.remove(&(hwnd as usize));
                });
            }
        }
        DefSubclassProc(hwnd, message, w, l)
    }
}

// SAFETY: Only called synchronously from the owning window's message callback.
unsafe fn pen_history(
    hwnd: HWND,
    id: u32,
    message: u32,
    state: &State,
) -> Result<Vec<PenEvent>, NativeHistoryFailure> {
    // SAFETY: Typed initialized buffers, checked lengths/results, and no pointers
    // escape the API calls. History is read before advancing native dispatch.
    unsafe {
        let mut count = 0;
        let query_succeeded = GetPointerPenInfoHistory(id, &mut count, std::ptr::null_mut()) != 0;
        let capacity = validate_history_count_query(query_succeeded, count)?;
        let mut history = vec![POINTER_PEN_INFO::default(); capacity];
        let read_succeeded = GetPointerPenInfoHistory(id, &mut count, history.as_mut_ptr()) != 0;
        let valid_count = validate_history_read(read_succeeded, history.len(), count)?;
        history.truncate(valid_count);
        let mut events = Vec::new();
        for pen in history.into_iter().rev() {
            let info = pen.pointerInfo;
            let stamp = NativeStamp {
                counter: info.PerformanceCount,
                timestamp_ms: info.dwTime,
                position_himetric: [info.ptHimetricLocation.x, info.ptHimetricLocation.y],
                pointer_flags: info.pointerFlags,
                pen_flags: pen.penFlags,
                pressure: pen.pressure,
                tilt: [pen.tiltX, pen.tiltY],
                mask: pen.penMask,
            };
            if !state.last.borrow_mut().accept(id, stamp) {
                continue;
            }
            let mut point = info.ptPixelLocation;
            validate_coordinate_conversion(ScreenToClient(hwnd, &mut point) != 0)?;
            let mut position = [point.x as f64, point.y as f64];
            let mut device = RECT::default();
            let mut display = RECT::default();
            if GetPointerDeviceRects(info.sourceDevice, &mut device, &mut display) != 0
                && device.right > device.left
                && device.bottom > device.top
            {
                let mut origin = POINT {
                    x: display.left,
                    y: display.top,
                };
                validate_coordinate_conversion(ScreenToClient(hwnd, &mut origin) != 0)?;
                position = [
                    origin.x as f64
                        + (info.ptHimetricLocation.x - device.left) as f64
                            * (display.right - display.left) as f64
                            / (device.right - device.left) as f64,
                    origin.y as f64
                        + (info.ptHimetricLocation.y - device.top) as f64
                            * (display.bottom - display.top) as f64
                            / (device.bottom - device.top) as f64,
                ];
            }
            let flags = info.pointerFlags;
            let phase = if flags & POINTER_FLAG_CANCELED != 0 || message == WM_POINTERLEAVE {
                Phase::Cancel
            } else if flags & POINTER_FLAG_DOWN != 0 {
                Phase::Down
            } else if flags & POINTER_FLAG_UP != 0 {
                Phase::Up
            } else if flags & POINTER_FLAG_INCONTACT != 0 {
                Phase::Move
            } else {
                Phase::Hover
            };
            let time = state
                .clock
                .borrow_mut()
                .extend(info.dwTime)
                .saturating_sub(state.base_tick)
                + state.base_ms;
            events.push(PenEvent {
                performance_count: info.PerformanceCount,
                sample: PointerSample {
                    source_counter: if info.PerformanceCount == 0 {
                        None
                    } else {
                        paint_input::SourceCounter::new(info.PerformanceCount, state.frequency)
                    },
                    id: id.into(),
                    timestamp_ms: time,
                    tool: Tool::Pen,
                    phase,
                    position_px: position,
                    pressure: (pen.penMask & PEN_MASK_PRESSURE != 0)
                        .then(|| normalize_pressure(pen.pressure as f32, 1024.0))
                        .flatten(),
                    tilt_degrees: [
                        (pen.penMask & PEN_MASK_TILT_X != 0)
                            .then(|| normalize_tilt(pen.tiltX as f32))
                            .flatten(),
                        (pen.penMask & PEN_MASK_TILT_Y != 0)
                            .then(|| normalize_tilt(pen.tiltY as f32))
                            .flatten(),
                    ],
                    buttons: Buttons {
                        primary: flags & POINTER_FLAG_FIRSTBUTTON != 0,
                        secondary: flags & POINTER_FLAG_SECONDBUTTON != 0,
                        barrel: pen.penFlags & PEN_FLAG_BARREL != 0,
                        eraser: pen.penFlags & PEN_FLAG_ERASER != 0,
                        inverted: pen.penFlags & PEN_FLAG_INVERTED != 0,
                    },
                    in_contact: flags & POINTER_FLAG_INCONTACT != 0,
                    in_range: flags & POINTER_FLAG_INRANGE != 0,
                },
            });
        }
        Ok(events)
    }
}
