#![allow(unsafe_code)]

use winit::platform::android::activity::AndroidApp;

#[path = "main.rs"]
mod app;

/// NativeActivity calls this Rust ABI entry after the activity is created.
/// AndroidApp owns the platform handles; winit receives a clone for its event
/// loop and continues to own window, surface, and activity lifecycle handling.
#[unsafe(no_mangle)]
pub fn android_main(android_app: AndroidApp) {
    let Some(private_data) = android_app.internal_data_path() else {
        eprintln!("BitchPixel could not locate its Android private data directory");
        return;
    };
    if let Err(error) = crate::platform_paths::set_android_data_directory(private_data) {
        eprintln!("BitchPixel could not initialize private storage: {error}");
        return;
    }

    crate::diagnostics::initialize();
    std::panic::set_hook(Box::new(|panic| {
        crate::diagnostics::write(format_args!("Application panic: {panic}"));
    }));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        app::run_android(android_app)
    }));
    match result {
        Ok(Ok(())) => {}
        Ok(Err(error)) => crate::diagnostics::write(format_args!(
            "BitchPixel could not start or continue: {error}"
        )),
        Err(panic) => crate::diagnostics::write(format_args!(
            "BitchPixel stopped unexpectedly: {panic:?}"
        )),
    }
}
