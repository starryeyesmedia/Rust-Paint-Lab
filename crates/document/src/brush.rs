//! Deterministic brush settings. No OS, GUI, GPU, or random process state.
use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BrushKind {
    Round,
    Ink,
    Pencil,
    Marker,
    Airbrush,
    Soft,
    Textured,
    Calligraphy,
    Eraser,
    Wash,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Brush {
    pub kind: BrushKind,
    pub size: f64,
    pub opacity: f64,
    pub flow: f64,
    pub hardness: f64,
    pub spacing: f64,
    pub pressure_size: bool,
    pub pressure_opacity: bool,
    pub tilt: bool,
    pub angle: f64,
    pub smoothing: f64,
    pub color: [u8; 3],
}
impl Default for Brush {
    fn default() -> Self {
        Self::preset(BrushKind::Round)
    }
}
impl Brush {
    pub fn preset(kind: BrushKind) -> Self {
        let mut b = Self {
            kind,
            size: 18.0,
            opacity: 1.0,
            flow: 1.0,
            hardness: 0.85,
            spacing: 0.15,
            pressure_size: true,
            pressure_opacity: false,
            tilt: false,
            angle: 0.0,
            smoothing: 0.0,
            color: [35, 65, 180],
        };
        match kind {
            BrushKind::Ink => {
                b.size = 8.0;
                b.hardness = 1.0;
            }
            BrushKind::Pencil => {
                b.size = 5.0;
                b.flow = 0.4;
                b.pressure_opacity = true;
            }
            BrushKind::Marker => {
                b.size = 30.0;
                b.opacity = 0.35;
                b.hardness = 1.0;
            }
            BrushKind::Airbrush => {
                b.size = 100.0;
                b.flow = 0.08;
                b.hardness = 0.0;
            }
            BrushKind::Soft => {
                b.size = 45.0;
                b.hardness = 0.0;
            }
            BrushKind::Textured => {
                b.size = 35.0;
                b.flow = 0.6;
            }
            BrushKind::Calligraphy => {
                b.size = 30.0;
                b.angle = -0.6;
                b.tilt = true;
            }
            BrushKind::Wash => {
                b.size = 70.0;
                b.opacity = 0.25;
                b.flow = 0.25;
                b.hardness = 0.0;
            }
            _ => {}
        }
        b
    }
    pub fn validate(&self) -> bool {
        self.size.is_finite()
            && (1.0..=256.0).contains(&self.size)
            && self.angle.is_finite()
            && self.angle.abs() <= 100.0
            && [self.opacity, self.flow, self.hardness, self.smoothing]
                .iter()
                .all(|x| x.is_finite() && (0.0..=1.0).contains(x))
            && self.spacing.is_finite()
            && (0.05..=1.0).contains(&self.spacing)
    }
    pub fn encode(&self) -> Result<String, crate::PaintError> {
        if !self.validate() {
            return Err(crate::PaintError::Invalid);
        }
        Ok(serde_json::to_string_pretty(
            &serde_json::json!({"version":1,"brush":self}),
        )?)
    }
    pub fn decode(text: &str) -> Result<Self, crate::PaintError> {
        if text.len() > 16384 {
            return Err(crate::PaintError::Limit);
        }
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Preset {
            version: u32,
            brush: Brush,
        }
        let p: Preset = serde_json::from_str(text)?;
        if p.version != 1 || !p.brush.validate() {
            return Err(crate::PaintError::Invalid);
        }
        Ok(p.brush)
    }
}
