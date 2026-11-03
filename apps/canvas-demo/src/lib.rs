//! Replay canvas demo. Shared input/routing crates remain graphics-independent.
#![deny(unsafe_code)]
extern crate self as canvas_demo;
#[cfg(target_os = "android")]
extern crate self as rfd;

// The Android NativeActivity entry point is the only exception to this crate's
// safe-Rust boundary. It exports the symbol Android requires and delegates to
// the existing winit application loop; no unsafe operations are performed.
#[cfg(target_os = "android")]
#[allow(unsafe_code)]
mod android_entry;
#[cfg(target_os = "android")]
mod android_file_dialog;

pub mod android_input;
pub mod appearance;
pub mod autosave;
pub mod barrel;
pub mod brush_preview;
pub mod color_controls;
pub mod diagnostics;
pub mod platform_paths;
#[cfg(target_os = "android")]
pub use android_file_dialog::FileDialog;
pub mod document_setup;
pub mod drafts;
pub mod gpu;
pub mod home_thumbnails;
pub mod image_placement;
pub mod interaction;
pub mod layer_thumbnails;
pub mod native_history;
pub mod navigation;
pub mod playback;
pub mod raster_gpu;
pub mod recovery_previews;
pub mod scene;
pub mod studio;
pub mod studio_accessibility;
pub mod studio_icons;
pub mod studio_layout;
pub mod studio_toolbar;
pub mod ui_tokens;
pub mod workspace;

/// Emit a bounded application diagnostic without assuming that the GUI process
/// has a console attached (Windows GUI-subsystem executables do not).
#[macro_export]
macro_rules! studio_log {
    ($($argument:tt)*) => {{
        $crate::diagnostics::write(format_args!($($argument)*));
    }};
}
