//! App-private state paths for platform hosts that cannot provide HOME/XDG.
use paint_document::PaintError;
use std::path::PathBuf;

/// Install the Android NativeActivity data directory once before Studio starts.
/// Activity recreation may install the same path again in the same process.
#[cfg(target_os = "android")]
pub fn set_android_data_directory(path: PathBuf) -> Result<(), PaintError> {
    paint_document::recovery::set_directory(path.join("bitchpixel").join("recovery"))
}

/// The host-owned private state root. Android data remains inside the app's
/// sandbox; other platforms retain their existing recovery-directory layout.
pub fn state_directory() -> Result<PathBuf, PaintError> {
    paint_document::recovery::directory()?
        .parent()
        .map(PathBuf::from)
        .ok_or(PaintError::Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_paths_remain_under_the_document_recovery_parent() {
        let recovery = paint_document::recovery::directory().expect("desktop recovery directory");
        let state = state_directory().expect("desktop state directory");
        assert_eq!(recovery.parent(), Some(state.as_path()));
    }
}
