//! Replay canvas demo. Shared input/routing crates remain graphics-independent.
#![forbid(unsafe_code)]
pub mod appearance;
pub mod autosave;
pub mod barrel;
pub mod brush_preview;
pub mod color_controls;
pub mod diagnostics;
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
