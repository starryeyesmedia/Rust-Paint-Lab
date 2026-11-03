//! Android's Storage Access Framework picker is a separate platform milestone.
//! Keep shared Studio call sites buildable without linking rfd's desktop-only
//! native backends. Private drafts and recovery remain available independently.
use std::path::{Path, PathBuf};

#[derive(Default)]
pub struct FileDialog;

impl FileDialog {
    pub fn new() -> Self {
        Self
    }

    pub fn set_title(self, _title: impl Into<String>) -> Self {
        self
    }

    pub fn add_filter(self, _name: impl Into<String>, _extensions: &[impl ToString]) -> Self {
        self
    }

    pub fn set_directory(self, _path: impl AsRef<Path>) -> Self {
        self
    }

    pub fn set_file_name(self, _name: impl Into<String>) -> Self {
        self
    }

    /// No Android document picker is wired in this milestone.
    pub fn pick_file(self) -> Option<PathBuf> {
        None
    }

    /// No Android document picker is wired in this milestone.
    pub fn pick_files(self) -> Option<Vec<PathBuf>> {
        None
    }

    /// No Android document picker is wired in this milestone.
    pub fn save_file(self) -> Option<PathBuf> {
        None
    }
}
