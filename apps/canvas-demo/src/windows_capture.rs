//! Windows-only capture-loss observer. Winit continues to own capture and dispatch.
//! No pointer/stylus APIs and no second event loop. State is looked up by HWND;
//! no Rust allocation address or borrowed reference is stored in native userdata.
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    io,
    rc::Rc,
    sync::Arc,
};
use windows_sys::Win32::{
    Foundation::*,
    UI::{
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{WM_CANCELMODE, WM_CAPTURECHANGED, WM_LBUTTONUP, WM_NCDESTROY},
    },
};
use winit::{
    event_loop::EventLoopProxy,
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};
const SUBCLASS_ID: usize = 0x52504c35;
struct State {
    releasing: Cell<bool>,
    pending: Cell<bool>,
    installed: Cell<bool>,
    proxy: EventLoopProxy<()>,
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
        proxy: EventLoopProxy<()>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let RawWindowHandle::Win32(handle) = window.window_handle()?.as_raw() else {
            return Err("Expected Win32 window".into());
        };
        let hwnd = handle.hwnd.get() as HWND;
        let state = Rc::new(State {
            releasing: Cell::new(false),
            pending: Cell::new(false),
            installed: Cell::new(true),
            proxy,
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
    pub fn take_lost(&self) -> bool {
        self.state.pending.replace(false)
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
                state.pending.set(true);
                let _ = state.proxy.send_event(()); // Wake even when live mode is idle.
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
