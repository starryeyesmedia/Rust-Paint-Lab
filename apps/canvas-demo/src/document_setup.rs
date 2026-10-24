//! Validated, platform-independent settings for the New Document workflow.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DocumentUnit {
    Pixels,
    Inches,
    Millimeters,
    Centimeters,
}

impl DocumentUnit {
    pub const ALL: [Self; 4] = [
        Self::Pixels,
        Self::Inches,
        Self::Millimeters,
        Self::Centimeters,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Pixels => "Pixels",
            Self::Inches => "Inches",
            Self::Millimeters => "Millimeters",
            Self::Centimeters => "Centimeters",
        }
    }

    /// Convert the requested physical extent to pixels at the selected DPI.
    pub fn to_pixels(self, value: f64, dpi: u32) -> Option<u32> {
        if !value.is_finite() || value <= 0.0 || !(1..=9600).contains(&dpi) {
            return None;
        }
        let px = value * f64::from(dpi)
            / match self {
                Self::Pixels => f64::from(dpi),
                Self::Inches => 1.0,
                Self::Millimeters => 25.4,
                Self::Centimeters => 2.54,
            };
        if !px.is_finite() || px > 4096.0 {
            return None;
        }
        let rounded = px.round();
        (1.0..=4096.0).contains(&rounded).then_some(rounded as u32)
    }

    pub fn from_pixels(self, pixels: u32, dpi: u32) -> Option<f64> {
        if pixels == 0 || !(1..=9600).contains(&dpi) {
            return None;
        }
        let pixels = f64::from(pixels);
        let dpi = f64::from(dpi);
        Some(match self {
            Self::Pixels => pixels,
            Self::Inches => pixels / dpi,
            Self::Millimeters => pixels * 25.4 / dpi,
            Self::Centimeters => pixels * 2.54 / dpi,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DocumentPreset {
    pub name: &'static str,
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
}

pub const PRESETS: &[DocumentPreset] = &[
    DocumentPreset {
        name: "Square Illustration",
        width: 2048,
        height: 2048,
        dpi: 300,
    },
    DocumentPreset {
        name: "HD Screen",
        width: 1280,
        height: 720,
        dpi: 96,
    },
    DocumentPreset {
        name: "Full HD Screen",
        width: 1920,
        height: 1080,
        dpi: 96,
    },
    DocumentPreset {
        name: "4K Screen",
        width: 3840,
        height: 2160,
        dpi: 96,
    },
    DocumentPreset {
        name: "Portrait Illustration",
        width: 2048,
        height: 3072,
        dpi: 300,
    },
    DocumentPreset {
        name: "Landscape Illustration",
        width: 3072,
        height: 2048,
        dpi: 300,
    },
    DocumentPreset {
        name: "Comic Page",
        width: 2550,
        height: 3300,
        dpi: 300,
    },
    DocumentPreset {
        name: "Storyboard Frame",
        width: 1920,
        height: 1080,
        dpi: 96,
    },
    DocumentPreset {
        name: "Social Square",
        width: 1080,
        height: 1080,
        dpi: 96,
    },
    DocumentPreset {
        name: "Vertical Video",
        width: 1080,
        height: 1920,
        dpi: 96,
    },
    DocumentPreset {
        name: "US Letter",
        width: 2550,
        height: 3300,
        dpi: 300,
    },
];

pub fn build_state(
    width: f64,
    height: f64,
    unit: DocumentUnit,
    dpi: u32,
    background: [u8; 3],
    transparent: bool,
) -> Result<paint_document::State, paint_document::PaintError> {
    let width = unit
        .to_pixels(width, dpi)
        .ok_or(paint_document::PaintError::Invalid)?;
    let height = unit
        .to_pixels(height, dpi)
        .ok_or(paint_document::PaintError::Invalid)?;
    let mut state = paint_document::State::new(width, height, dpi)?;
    state.background = (!transparent).then_some(background);
    Ok(state)
}

pub fn sanitized_document_stem(name: &str) -> String {
    let stem: String = name
        .trim()
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '-'
            } else {
                c
            }
        })
        .collect();
    let stem = stem.trim_matches([' ', '.', '-']);
    if stem.is_empty() {
        "untitled".into()
    } else {
        stem.chars().take(80).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unit_conversion_uses_requested_dpi_and_rounds_to_pixels() {
        assert_eq!(DocumentUnit::Pixels.to_pixels(101.4, 300), Some(101));
        assert_eq!(DocumentUnit::Inches.to_pixels(8.5, 300), Some(2550));
        assert_eq!(DocumentUnit::Millimeters.to_pixels(25.4, 300), Some(300));
        assert_eq!(DocumentUnit::Centimeters.to_pixels(2.54, 300), Some(300));
        assert!((DocumentUnit::Inches.from_pixels(2550, 300).unwrap() - 8.5).abs() < 1e-9);
        assert!((DocumentUnit::Millimeters.from_pixels(300, 300).unwrap() - 25.4).abs() < 1e-9);
    }

    #[test]
    fn unit_conversion_rejects_nonfinite_oversize_and_invalid_resolution() {
        assert_eq!(DocumentUnit::Pixels.to_pixels(f64::NAN, 300), None);
        assert_eq!(DocumentUnit::Inches.to_pixels(1.0, 0), None);
        assert_eq!(DocumentUnit::Pixels.to_pixels(4097.0, 96), None);
        assert_eq!(DocumentUnit::Inches.to_pixels(f64::INFINITY, 96), None);
    }

    #[test]
    fn presets_fit_current_native_document_limits() {
        assert!(PRESETS.iter().all(|p| p.width > 0
            && p.height > 0
            && p.width <= 4096
            && p.height <= 4096
            && (1..=9600).contains(&p.dpi)));
    }

    #[test]
    fn document_name_is_safe_and_never_empty() {
        assert_eq!(
            sanitized_document_stem("  sketch: / one. "),
            "sketch- - one"
        );
        assert_eq!(sanitized_document_stem("..."), "untitled");
        assert_eq!(sanitized_document_stem("x".repeat(100).as_str()).len(), 80);
    }

    #[test]
    fn setup_builds_requested_document_state_without_allocating_artwork_tiles() {
        let state = build_state(8.5, 11.0, DocumentUnit::Inches, 300, [12, 34, 56], false).unwrap();
        assert_eq!([state.width, state.height, state.dpi], [2550, 3300, 300]);
        assert_eq!(state.background, Some([12, 34, 56]));
        assert_eq!(state.tile_count(), 0);
        let transparent =
            build_state(512.0, 512.0, DocumentUnit::Pixels, 96, [1, 2, 3], true).unwrap();
        assert_eq!(transparent.background, None);
        assert!(build_state(4097.0, 512.0, DocumentUnit::Pixels, 96, [0; 3], false).is_err());
    }
}
