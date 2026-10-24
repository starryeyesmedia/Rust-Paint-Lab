//! Persisted, artwork-independent workspace preferences.
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read, path::PathBuf};

pub const MAX_REFERENCES: usize = 6;
pub const MAX_REFERENCE_PATH: usize = 4096;
pub const MAX_PREFERENCES_BYTES: usize = 64 * 1024;

pub fn load(path: &std::path::Path) -> Option<WorkspacePreferences> {
    let mut bytes = Vec::new();
    File::open(path)
        .ok()?
        .take(MAX_PREFERENCES_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > MAX_PREFERENCES_BYTES {
        return None;
    }
    let mut preferences: WorkspacePreferences = serde_json::from_slice(&bytes).ok()?;
    preferences.validate().then_some(preferences)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReferenceLayout {
    pub path: PathBuf,
    pub position: [f32; 2],
    pub size: [f32; 2],
    pub opacity: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspacePreferences {
    pub version: u32,
    pub dark_theme: bool,
    pub brush_panel_width: f32,
    pub references: Vec<ReferenceLayout>,
    #[serde(default)]
    pub new_document: NewDocumentPreferences,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NewDocumentPreferences {
    pub width: f64,
    pub height: f64,
    pub dpi: u32,
    pub unit: crate::document_setup::DocumentUnit,
    pub background: [u8; 3],
    pub transparent: bool,
}

impl Default for NewDocumentPreferences {
    fn default() -> Self {
        Self {
            width: 1600.0,
            height: 1000.0,
            dpi: 300,
            unit: crate::document_setup::DocumentUnit::Pixels,
            background: [255; 3],
            transparent: false,
        }
    }
}

impl NewDocumentPreferences {
    fn validate(&mut self) -> bool {
        self.unit.to_pixels(self.width, self.dpi).is_some()
            && self.unit.to_pixels(self.height, self.dpi).is_some()
    }
}

impl Default for WorkspacePreferences {
    fn default() -> Self {
        Self {
            version: 1,
            dark_theme: true,
            brush_panel_width: 260.0,
            references: Vec::new(),
            new_document: NewDocumentPreferences::default(),
        }
    }
}

impl WorkspacePreferences {
    /// Reject unknown schemas and bound untrusted disk data before creating UI.
    pub fn validate(&mut self) -> bool {
        if self.version != 1 || !self.brush_panel_width.is_finite() || !self.new_document.validate()
        {
            return false;
        }
        self.brush_panel_width = self.brush_panel_width.clamp(180.0, 640.0);
        self.references.truncate(MAX_REFERENCES);
        self.references.retain_mut(|reference| {
            let path_len = reference.path.as_os_str().len();
            if path_len == 0
                || path_len > MAX_REFERENCE_PATH
                || !reference.position.iter().all(|v| v.is_finite())
                || !reference.size.iter().all(|v| v.is_finite())
                || !reference.opacity.is_finite()
            {
                return false;
            }
            reference.position = reference.position.map(|v| v.clamp(-10000.0, 10000.0));
            reference.size = reference.size.map(|v| v.clamp(180.0, 1600.0));
            reference.opacity = reference.opacity.clamp(0.05, 1.0);
            true
        });
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_round_trip_and_bound_saved_layout() {
        let mut preferences = WorkspacePreferences::default();
        preferences.references.push(ReferenceLayout {
            path: "reference.png".into(),
            position: [f32::NAN, 12.0],
            size: [100.0, 5000.0],
            opacity: 2.0,
        });
        assert!(preferences.validate());
        // Non-finite persisted geometry is discarded rather than reaching egui.
        assert!(preferences.references.is_empty());
        preferences.references.push(ReferenceLayout {
            path: "reference.png".into(),
            position: [12.0, -10.0],
            size: [100.0, 5000.0],
            opacity: 2.0,
        });
        assert!(preferences.validate());
        assert_eq!(preferences.references[0].size, [180.0, 1600.0]);
        assert_eq!(preferences.references[0].opacity, 1.0);
        let json = serde_json::to_vec(&preferences).unwrap();
        let mut restored: WorkspacePreferences = serde_json::from_slice(&json).unwrap();
        assert!(restored.validate());
        assert_eq!(restored, preferences);
    }

    #[test]
    fn preferences_reject_unknown_versions_and_limit_reference_count() {
        let mut preferences = WorkspacePreferences {
            version: 99,
            ..Default::default()
        };
        assert!(!preferences.validate());
        preferences.version = 1;
        for i in 0..MAX_REFERENCES + 2 {
            preferences.references.push(ReferenceLayout {
                path: format!("{i}.png").into(),
                position: [0.0, 0.0],
                size: [300.0, 300.0],
                opacity: 0.5,
            });
        }
        assert!(preferences.validate());
        assert_eq!(preferences.references.len(), MAX_REFERENCES);
    }

    #[test]
    fn oversized_preferences_file_is_ignored_before_json_decode() {
        let path = std::env::temp_dir().join(format!(
            "rpl-workspace-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::write(&path, vec![b' '; MAX_PREFERENCES_BYTES + 1]).unwrap();
        assert!(load(&path).is_none());
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn older_preferences_load_with_safe_document_setup_defaults() {
        let mut preferences: WorkspacePreferences = serde_json::from_str(
            r#"{"version":1,"dark_theme":false,"brush_panel_width":300,"references":[]}"#,
        )
        .unwrap();
        assert!(preferences.validate());
        assert_eq!(preferences.new_document.width, 1600.0);
        assert_eq!(preferences.new_document.height, 1000.0);
        assert!(!preferences.new_document.transparent);
    }

    #[test]
    fn invalid_saved_document_setup_is_rejected() {
        let mut preferences = WorkspacePreferences::default();
        preferences.new_document.width = f64::NAN;
        assert!(!preferences.validate());
    }
}
