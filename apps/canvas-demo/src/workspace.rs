//! Persisted, artwork-independent workspace preferences.
use serde::{Deserialize, Serialize};
use std::{fs::File, io::Read, path::PathBuf};

pub const MAX_REFERENCES: usize = 6;
pub const MAX_REFERENCE_PATH: usize = 4096;
pub const MAX_PREFERENCES_BYTES: usize = 64 * 1024;
pub const MAX_RECENT_DOCUMENT_SIZES: usize = 8;
pub const MAX_RECENT_BRUSH_COLORS: usize = 12;
pub const MIN_REFERENCE_ZOOM: f32 = 0.25;
pub const MAX_REFERENCE_ZOOM: f32 = 8.0;

fn default_reference_zoom() -> f32 {
    1.0
}

fn default_reference_pan() -> [f32; 2] {
    [0.0, 0.0]
}

fn default_canvas_zoom() -> f64 {
    1.0
}

fn default_canvas_pan() -> [f64; 2] {
    [0.0, 0.0]
}

fn default_ui_scale() -> f32 {
    1.0
}

fn default_palette_position() -> [f32; 2] {
    [72.0, 54.0]
}

fn default_layers_flyout_position() -> [f32; 2] {
    [760.0, 96.0]
}

fn default_layers_flyout_size() -> [f32; 2] {
    [300.0, 260.0]
}

fn default_brush_flyout_position() -> [f32; 2] {
    [104.0, 72.0]
}

fn default_color_flyout_position() -> [f32; 2] {
    [104.0, 300.0]
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum InspectorTab {
    #[default]
    Layers,
    Brush,
    Edit,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaletteDock {
    #[default]
    Left,
    Right,
    Floating,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Handedness {
    #[default]
    Right,
    Left,
}

pub fn remember_brush_color(colors: &mut Vec<[u8; 3]>, color: [u8; 3]) {
    colors.retain(|existing| *existing != color);
    colors.insert(0, color);
    colors.truncate(MAX_RECENT_BRUSH_COLORS);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppearanceMode {
    Dark,
    Light,
    FollowSystem,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AccentScheme {
    #[default]
    Violet,
    Blue,
    Teal,
    Coral,
    Green,
    Neutral,
    Custom,
}

impl AppearanceMode {
    pub fn is_dark(self, system_dark: bool) -> bool {
        match self {
            Self::Dark => true,
            Self::Light => false,
            Self::FollowSystem => system_dark,
        }
    }
}

impl Default for AppearanceMode {
    fn default() -> Self {
        Self::Dark
    }
}

fn default_show_controls() -> bool {
    false
}

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
    #[serde(default = "default_reference_zoom")]
    pub view_zoom: f32,
    #[serde(default = "default_reference_pan")]
    pub view_pan: [f32; 2],
    #[serde(default)]
    pub position_locked: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkspacePreferences {
    pub version: u32,
    pub dark_theme: bool,
    /// Missing on legacy version-1 settings; derive the mode from `dark_theme`.
    #[serde(default)]
    pub appearance_mode: Option<AppearanceMode>,
    #[serde(default)]
    pub accent_scheme: AccentScheme,
    #[serde(default)]
    pub custom_accent_rgb: Option<[u8; 3]>,
    pub brush_panel_width: f32,
    #[serde(default = "default_show_controls")]
    pub show_controls: bool,
    pub references: Vec<ReferenceLayout>,
    #[serde(default)]
    pub new_document: NewDocumentPreferences,
    #[serde(default)]
    pub recent_document_sizes: Vec<RecentDocumentSize>,
    #[serde(default)]
    pub recent_brush_colors: Vec<[u8; 3]>,
    #[serde(default = "default_canvas_zoom")]
    pub canvas_zoom: f64,
    #[serde(default = "default_canvas_pan")]
    pub canvas_pan: [f64; 2],
    #[serde(default)]
    pub inspector_tab: InspectorTab,
    #[serde(default)]
    pub high_contrast: bool,
    #[serde(default)]
    pub reduced_motion: bool,
    #[serde(default = "default_ui_scale")]
    pub ui_scale: f32,
    #[serde(default)]
    pub palette_dock: PaletteDock,
    #[serde(default)]
    pub palette_collapsed: bool,
    #[serde(default)]
    pub handedness: Handedness,
    #[serde(default = "default_palette_position")]
    pub palette_position: [f32; 2],
    #[serde(default = "default_layers_flyout_position")]
    pub layers_flyout_position: [f32; 2],
    #[serde(default = "default_layers_flyout_size")]
    pub layers_flyout_size: [f32; 2],
    #[serde(default = "default_brush_flyout_position")]
    pub brush_flyout_position: [f32; 2],
    #[serde(default = "default_color_flyout_position")]
    pub color_flyout_position: [f32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecentDocumentSize {
    pub width_px: u32,
    pub height_px: u32,
    pub dpi: u32,
}

impl RecentDocumentSize {
    pub fn valid(self) -> bool {
        (1..=4096).contains(&self.width_px)
            && (1..=4096).contains(&self.height_px)
            && (1..=9600).contains(&self.dpi)
    }
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
            appearance_mode: Some(AppearanceMode::Dark),
            accent_scheme: AccentScheme::Violet,
            custom_accent_rgb: None,
            brush_panel_width: 260.0,
            show_controls: false,
            references: Vec::new(),
            new_document: NewDocumentPreferences::default(),
            recent_document_sizes: Vec::new(),
            recent_brush_colors: Vec::new(),
            canvas_zoom: 1.0,
            canvas_pan: [0.0, 0.0],
            inspector_tab: InspectorTab::Layers,
            high_contrast: false,
            reduced_motion: false,
            ui_scale: 1.0,
            palette_dock: PaletteDock::Left,
            palette_collapsed: false,
            handedness: Handedness::Right,
            palette_position: [72.0, 54.0],
            layers_flyout_position: default_layers_flyout_position(),
            layers_flyout_size: default_layers_flyout_size(),
            brush_flyout_position: default_brush_flyout_position(),
            color_flyout_position: default_color_flyout_position(),
        }
    }
}

impl WorkspacePreferences {
    pub fn effective_appearance_mode(&self) -> AppearanceMode {
        self.appearance_mode.unwrap_or(if self.dark_theme {
            AppearanceMode::Dark
        } else {
            AppearanceMode::Light
        })
    }

    /// Reject unknown schemas and bound untrusted disk data before creating UI.
    pub fn validate(&mut self) -> bool {
        if self.version != 1
            || !self.brush_panel_width.is_finite()
            || !self.canvas_zoom.is_finite()
            || !self.ui_scale.is_finite()
            || !self.palette_position.iter().all(|v| v.is_finite())
            || !self.layers_flyout_position.iter().all(|v| v.is_finite())
            || !self.layers_flyout_size.iter().all(|v| v.is_finite())
            || !self.brush_flyout_position.iter().all(|v| v.is_finite())
            || !self.color_flyout_position.iter().all(|v| v.is_finite())
            || !self.canvas_pan.iter().all(|v| v.is_finite())
            || !self.new_document.validate()
        {
            return false;
        }
        self.brush_panel_width = self.brush_panel_width.clamp(180.0, 640.0);
        self.canvas_zoom = self.canvas_zoom.clamp(0.25, 8.0);
        self.ui_scale = self.ui_scale.clamp(0.8, 1.6);
        self.palette_position = self.palette_position.map(|v| v.clamp(0.0, 32768.0));
        self.layers_flyout_position = self.layers_flyout_position.map(|v| v.clamp(0.0, 32768.0));
        self.layers_flyout_size = [
            self.layers_flyout_size[0].clamp(220.0, 800.0),
            self.layers_flyout_size[1].clamp(160.0, 1200.0),
        ];
        self.brush_flyout_position = self.brush_flyout_position.map(|v| v.clamp(0.0, 32768.0));
        self.color_flyout_position = self.color_flyout_position.map(|v| v.clamp(0.0, 32768.0));
        self.canvas_pan = self.canvas_pan.map(|v| v.clamp(-32768.0, 32768.0));
        if self.accent_scheme == AccentScheme::Custom && self.custom_accent_rgb.is_none() {
            self.accent_scheme = AccentScheme::Violet;
        }
        let mut seen = Vec::new();
        self.recent_document_sizes.retain(|size| {
            if !size.valid() || seen.contains(size) {
                false
            } else {
                seen.push(*size);
                true
            }
        });
        self.recent_document_sizes
            .truncate(MAX_RECENT_DOCUMENT_SIZES);
        let mut seen_colors = Vec::new();
        self.recent_brush_colors.retain(|color| {
            if seen_colors.contains(color) {
                false
            } else {
                seen_colors.push(*color);
                true
            }
        });
        self.recent_brush_colors.truncate(MAX_RECENT_BRUSH_COLORS);
        self.references.truncate(MAX_REFERENCES);
        self.references.retain_mut(|reference| {
            let path_len = reference.path.as_os_str().len();
            if path_len == 0
                || path_len > MAX_REFERENCE_PATH
                || !reference.position.iter().all(|v| v.is_finite())
                || !reference.size.iter().all(|v| v.is_finite())
                || !reference.view_pan.iter().all(|v| v.is_finite())
                || !reference.opacity.is_finite()
                || !reference.view_zoom.is_finite()
            {
                return false;
            }
            reference.position = reference.position.map(|v| v.clamp(-10000.0, 10000.0));
            reference.size = reference.size.map(|v| v.clamp(180.0, 1600.0));
            reference.opacity = reference.opacity.clamp(0.05, 1.0);
            reference.view_zoom = reference
                .view_zoom
                .clamp(MIN_REFERENCE_ZOOM, MAX_REFERENCE_ZOOM);
            reference.view_pan = reference.view_pan.map(|v| v.clamp(-10000.0, 10000.0));
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
        let mut preferences = WorkspacePreferences {
            show_controls: false,
            palette_dock: PaletteDock::Floating,
            palette_collapsed: true,
            handedness: Handedness::Left,
            palette_position: [400.0, 250.0],
            ..Default::default()
        };
        preferences.references.push(ReferenceLayout {
            path: "reference.png".into(),
            position: [f32::NAN, 12.0],
            size: [100.0, 5000.0],
            opacity: 2.0,
            view_zoom: 1.0,
            view_pan: [0.0, 0.0],
            position_locked: false,
        });
        assert!(preferences.validate());
        // Non-finite persisted geometry is discarded rather than reaching egui.
        assert!(preferences.references.is_empty());
        preferences.references.push(ReferenceLayout {
            path: "reference.png".into(),
            position: [12.0, -10.0],
            size: [100.0, 5000.0],
            opacity: 2.0,
            view_zoom: 1.0,
            view_pan: [0.0, 0.0],
            position_locked: false,
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
    fn floating_palette_rejects_nonfinite_position_and_bounds_extreme_values() {
        let mut preferences = WorkspacePreferences {
            palette_dock: PaletteDock::Floating,
            palette_position: [f32::INFINITY, 0.0],
            ..Default::default()
        };
        assert!(!preferences.validate());
        preferences.palette_position = [-100.0, 100_000.0];
        assert!(preferences.validate());
        assert_eq!(preferences.palette_position, [0.0, 32768.0]);
    }

    #[test]
    fn floating_layer_panel_geometry_is_persisted_and_bounded() {
        let mut preferences = WorkspacePreferences {
            layers_flyout_position: [120.0, -40.0],
            layers_flyout_size: [900.0, 90.0],
            brush_flyout_position: [-200.0, 40_000.0],
            color_flyout_position: [300.0, 400.0],
            ..Default::default()
        };
        assert!(preferences.validate());
        assert_eq!(preferences.layers_flyout_position, [120.0, 0.0]);
        assert_eq!(preferences.layers_flyout_size, [800.0, 160.0]);
        assert_eq!(preferences.brush_flyout_position, [0.0, 32768.0]);
        assert_eq!(preferences.color_flyout_position, [300.0, 400.0]);

        let json = serde_json::to_vec(&preferences).unwrap();
        let mut restored: WorkspacePreferences = serde_json::from_slice(&json).unwrap();
        assert!(restored.validate());
        assert_eq!(restored.layers_flyout_position, [120.0, 0.0]);
        assert_eq!(restored.layers_flyout_size, [800.0, 160.0]);
        assert_eq!(restored.brush_flyout_position, [0.0, 32768.0]);
        assert_eq!(restored.color_flyout_position, [300.0, 400.0]);

        restored.layers_flyout_position[0] = f32::INFINITY;
        assert!(!restored.validate());
    }

    #[test]
    fn legacy_reference_layout_defaults_navigation_and_validation_bounds_it() {
        let mut preferences: WorkspacePreferences = serde_json::from_str(
            r#"{"version":1,"dark_theme":true,"brush_panel_width":260,"show_controls":true,"references":[{"path":"reference.png","position":[10,20],"size":[360,300],"opacity":0.7}]}"#,
        )
        .unwrap();
        assert_eq!(preferences.references[0].view_zoom, 1.0);
        assert_eq!(preferences.references[0].view_pan, [0.0, 0.0]);
        assert!(!preferences.references[0].position_locked);
        preferences.references[0].view_zoom = 100.0;
        preferences.references[0].view_pan = [20_000.0, -20_000.0];
        assert!(preferences.validate());
        assert_eq!(preferences.references[0].view_zoom, MAX_REFERENCE_ZOOM);
        assert_eq!(preferences.references[0].view_pan, [10_000.0, -10_000.0]);
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
                view_zoom: 1.0,
                view_pan: [0.0, 0.0],
                position_locked: false,
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
        assert_eq!(preferences.layers_flyout_position, [760.0, 96.0]);
        assert_eq!(preferences.layers_flyout_size, [300.0, 260.0]);
        assert_eq!(preferences.brush_flyout_position, [104.0, 72.0]);
        assert_eq!(preferences.color_flyout_position, [104.0, 300.0]);
        assert!(!preferences.new_document.transparent);
        assert!(!preferences.show_controls);
        assert_eq!(preferences.appearance_mode, None);
        assert_eq!(preferences.canvas_zoom, 1.0);
        assert_eq!(preferences.canvas_pan, [0.0, 0.0]);
        assert_eq!(preferences.inspector_tab, InspectorTab::Layers);
        assert!(!preferences.high_contrast);
        assert!(!preferences.reduced_motion);
        assert_eq!(preferences.ui_scale, 1.0);
        assert_eq!(preferences.palette_dock, PaletteDock::Left);
        assert!(!preferences.palette_collapsed);
        assert_eq!(preferences.handedness, Handedness::Right);
        assert_eq!(preferences.palette_position, [72.0, 54.0]);
        assert_eq!(preferences.accent_scheme, AccentScheme::Violet);
        assert_eq!(
            preferences.effective_appearance_mode(),
            AppearanceMode::Light
        );
    }

    #[test]
    fn canvas_navigation_preferences_roundtrip_and_are_bounded() {
        let mut preferences = WorkspacePreferences {
            canvas_zoom: 3.25,
            canvas_pan: [-1200.0, 740.5],
            ..Default::default()
        };
        assert!(preferences.validate());
        let bytes = serde_json::to_vec(&preferences).unwrap();
        let mut restored: WorkspacePreferences = serde_json::from_slice(&bytes).unwrap();
        assert!(restored.validate());
        assert_eq!(restored.canvas_zoom, 3.25);
        assert_eq!(restored.canvas_pan, [-1200.0, 740.5]);

        restored.canvas_zoom = 99.0;
        restored.canvas_pan = [-1_000_000.0, 1_000_000.0];
        assert!(restored.validate());
        assert_eq!(restored.canvas_zoom, 8.0);
        assert_eq!(restored.canvas_pan, [-32768.0, 32768.0]);
        restored.canvas_zoom = f64::NAN;
        assert!(!restored.validate());
    }

    #[test]
    fn accessibility_preferences_persist_and_clamp_interface_scale() {
        let mut preferences = WorkspacePreferences {
            high_contrast: true,
            reduced_motion: true,
            ui_scale: 1.35,
            ..Default::default()
        };
        assert!(preferences.validate());
        let encoded = serde_json::to_vec(&preferences).unwrap();
        let mut restored: WorkspacePreferences = serde_json::from_slice(&encoded).unwrap();
        assert!(restored.validate());
        assert!(restored.high_contrast);
        assert!(restored.reduced_motion);
        assert_eq!(restored.ui_scale, 1.35);
        restored.ui_scale = 9.0;
        assert!(restored.validate());
        assert_eq!(restored.ui_scale, 1.6);
        restored.ui_scale = f32::NAN;
        assert!(!restored.validate());
    }

    #[test]
    fn appearance_settings_roundtrip_and_invalid_custom_scheme_falls_back_safely() {
        let mut preferences = WorkspacePreferences {
            appearance_mode: Some(AppearanceMode::FollowSystem),
            accent_scheme: AccentScheme::Custom,
            custom_accent_rgb: Some([144, 104, 220]),
            ..Default::default()
        };
        assert!(preferences.validate());
        let bytes = serde_json::to_vec(&preferences).unwrap();
        let mut restored: WorkspacePreferences = serde_json::from_slice(&bytes).unwrap();
        assert!(restored.validate());
        assert_eq!(restored, preferences);
        restored.custom_accent_rgb = None;
        assert!(restored.validate());
        assert_eq!(restored.accent_scheme, AccentScheme::Violet);
    }

    #[test]
    fn recent_brush_colors_are_deduplicated_newest_first_and_bounded() {
        let mut colors = Vec::new();
        for value in 0..MAX_RECENT_BRUSH_COLORS as u8 + 2 {
            remember_brush_color(&mut colors, [value, 20, 30]);
        }
        remember_brush_color(&mut colors, [2, 20, 30]);
        assert_eq!(colors.len(), MAX_RECENT_BRUSH_COLORS);
        assert_eq!(colors[0], [2, 20, 30]);
        assert_eq!(colors.iter().filter(|&&c| c == [2, 20, 30]).count(), 1);

        let mut preferences = WorkspacePreferences {
            recent_brush_colors: colors.clone(),
            ..Default::default()
        };
        assert!(preferences.validate());
        let bytes = serde_json::to_vec(&preferences).unwrap();
        let mut restored: WorkspacePreferences = serde_json::from_slice(&bytes).unwrap();
        assert!(restored.validate());
        assert_eq!(restored.recent_brush_colors, colors);
        restored.recent_brush_colors = vec![[1, 2, 3], [1, 2, 3]];
        assert!(restored.validate());
        assert_eq!(restored.recent_brush_colors, vec![[1, 2, 3]]);
    }

    #[test]
    fn invalid_saved_document_setup_is_rejected() {
        let mut preferences = WorkspacePreferences::default();
        preferences.new_document.width = f64::NAN;
        assert!(!preferences.validate());
    }

    #[test]
    fn recent_dimensions_are_valid_deduplicated_and_bounded() {
        let mut preferences = WorkspacePreferences::default();
        let size = RecentDocumentSize {
            width_px: 1200,
            height_px: 1800,
            dpi: 300,
        };
        preferences.recent_document_sizes = vec![
            size,
            size,
            RecentDocumentSize {
                width_px: 0,
                height_px: 64,
                dpi: 96,
            },
        ];
        preferences
            .recent_document_sizes
            .extend(
                (0..MAX_RECENT_DOCUMENT_SIZES + 2).map(|i| RecentDocumentSize {
                    width_px: 400 + i as u32,
                    height_px: 300,
                    dpi: 96,
                }),
            );
        assert!(preferences.validate());
        assert_eq!(
            preferences.recent_document_sizes.len(),
            MAX_RECENT_DOCUMENT_SIZES
        );
        assert_eq!(preferences.recent_document_sizes[0], size);
        assert_eq!(preferences.recent_document_sizes[1].width_px, 400);
    }
}
