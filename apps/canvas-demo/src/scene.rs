//! Bounded command consumer and CPU geometry. No window or GPU calls.
use paint_stroke::{StrokeCommand, StrokeId, StrokeSample};
use std::{collections::BTreeMap, error::Error, fmt};

pub const DOCUMENT_SIZE: [f64; 2] = [800.0, 500.0];
pub const WIDTH: f64 = 8.0;
pub const MAX_STROKES: usize = 64;
pub const MAX_SAMPLES: usize = 8192;
pub const MAX_POINTS_PER_STROKE: usize = 1024;
pub const MAX_VERTICES: usize = MAX_SAMPLES * 12 + 6;
pub const PREVIEW: [f32; 4] = [0.95, 0.3, 0.03, 1.0];
pub const COMMITTED: [f32; 4] = [0.04, 0.2, 0.65, 1.0];
pub const PAPER: [f32; 4] = [0.92, 0.9, 0.84, 1.0];
pub const BACKGROUND: [f64; 4] = [0.035, 0.05, 0.08, 1.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SceneError {
    InvalidCoordinates,
    Capacity,
    InvalidViewport,
}
impl fmt::Display for SceneError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canvas scene: {self:?}")
    }
}
impl Error for SceneError {}

/// All samples, including pressure and final Commit sample, remain available.
#[derive(Debug, Default, PartialEq)]
pub struct Scene {
    provisional: BTreeMap<StrokeId, Vec<StrokeSample>>,
    committed: BTreeMap<StrokeId, Vec<StrokeSample>>,
}
impl Scene {
    pub fn provisional(&self) -> &BTreeMap<StrokeId, Vec<StrokeSample>> {
        &self.provisional
    }
    pub fn committed(&self) -> &BTreeMap<StrokeId, Vec<StrokeSample>> {
        &self.committed
    }
    pub fn sample_count(&self) -> usize {
        self.provisional
            .values()
            .chain(self.committed.values())
            .map(Vec::len)
            .sum()
    }
    pub fn stroke_count(&self) -> usize {
        self.provisional.len() + self.committed.len()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn apply(&mut self, command: &StrokeCommand) -> Result<(), SceneError> {
        let (id, sample, begin, commit) = match *command {
            StrokeCommand::CancelStroke { stroke_id, .. } => {
                self.provisional.remove(&stroke_id);
                return Ok(());
            }
            StrokeCommand::BeginStroke { stroke_id, sample } => (stroke_id, sample, true, false),
            StrokeCommand::UpdateStroke { stroke_id, sample } => (stroke_id, sample, false, false),
            StrokeCommand::CommitStroke { stroke_id, sample } => (stroke_id, sample, false, true),
        };
        if self.committed.contains_key(&id)
            || (begin && self.provisional.contains_key(&id))
            || (!begin && !self.provisional.contains_key(&id))
        {
            return Ok(());
        }
        if !valid_position(sample.position_document.0) {
            self.provisional.remove(&id);
            return Err(SceneError::InvalidCoordinates);
        }
        if self.sample_count() == MAX_SAMPLES
            || (begin && self.stroke_count() == MAX_STROKES)
            || self
                .provisional
                .get(&id)
                .is_some_and(|p| p.len() == MAX_POINTS_PER_STROKE)
        {
            self.provisional.remove(&id);
            return Err(SceneError::Capacity);
        }
        if begin {
            self.provisional.insert(id, vec![sample]);
        } else {
            let points = self
                .provisional
                .get_mut(&id)
                .expect("checked active transaction");
            // Exact retransmissions add no duplicate sample; a distinct final
            // sample (even at the same position) is retained with its attributes.
            if points.last() != Some(&sample) {
                points.push(sample);
            }
            if commit {
                self.committed.insert(
                    id,
                    self.provisional.remove(&id).expect("active transaction"),
                );
            }
        }
        Ok(())
    }
    pub fn mesh(&self, size: [u32; 2]) -> Result<Vec<Vertex>, SceneError> {
        let mapping = CanvasMapping::new(size)?;
        let mut vertices = Vec::with_capacity((self.sample_count() * 12 + 6).min(MAX_VERTICES));
        quad(
            &mut vertices,
            &mapping,
            [[0.0, 0.0], [800.0, 0.0], [800.0, 500.0], [0.0, 500.0]],
            PAPER,
        )?;
        for (strokes, color) in [(&self.committed, COMMITTED), (&self.provisional, PREVIEW)] {
            for points in strokes.values() {
                // Square caps/joins, fixed 8 document units; no pressure width.
                for p in points {
                    let [x, y] = p.position_document.0;
                    let h = WIDTH / 2.0;
                    quad(
                        &mut vertices,
                        &mapping,
                        [
                            [x - h, y - h],
                            [x + h, y - h],
                            [x + h, y + h],
                            [x - h, y + h],
                        ],
                        color,
                    )?;
                }
                for pair in points.windows(2) {
                    let [x, y] = pair[0].position_document.0;
                    let [xx, yy] = pair[1].position_document.0;
                    let dx = xx - x;
                    let dy = yy - y;
                    let length = dx.hypot(dy);
                    if length == 0.0 {
                        continue;
                    }
                    let nx = -dy / length * WIDTH / 2.0;
                    let ny = dx / length * WIDTH / 2.0;
                    quad(
                        &mut vertices,
                        &mapping,
                        [
                            [x + nx, y + ny],
                            [xx + nx, yy + ny],
                            [xx - nx, yy - ny],
                            [x - nx, y - ny],
                        ],
                        color,
                    )?;
                }
            }
        }
        Ok(vertices)
    }
}

fn valid_position(p: [f64; 2]) -> bool {
    p.iter().all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
}

/// Fit the 800x500 document inside physical viewport pixels with a 16px margin.
/// Document +y down maps to clip-space +y up. Aspect ratio is preserved.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CanvasMapping {
    size: [f64; 2],
    scale: f64,
    origin: [f64; 2],
    document: [f64; 2],
    viewport: [f64; 4],
}
impl CanvasMapping {
    pub fn new(size: [u32; 2]) -> Result<Self, SceneError> {
        if size.iter().any(|v| *v == 0 || *v > 8192) {
            return Err(SceneError::InvalidViewport);
        }
        Self::for_document(
            size,
            DOCUMENT_SIZE,
            [0.0, 0.0, size[0] as f64, size[1] as f64],
        )
    }
    pub fn for_document(
        size: [u32; 2],
        document: [f64; 2],
        rect: [f64; 4],
    ) -> Result<Self, SceneError> {
        if size.contains(&0)
            || size.iter().any(|n| *n > 8192)
            || !document
                .iter()
                .all(|n| n.is_finite() && *n > 0.0 && *n <= 4096.0)
            || !rect.iter().all(|n| n.is_finite())
            || rect[0] < 0.0
            || rect[1] < 0.0
            || rect[2] > size[0] as f64
            || rect[3] > size[1] as f64
            || rect[2] <= rect[0]
            || rect[3] <= rect[1]
        {
            return Err(SceneError::InvalidViewport);
        }
        let scale = ((rect[2] - rect[0] - 32.0).max(1.0) / document[0])
            .min((rect[3] - rect[1] - 32.0).max(1.0) / document[1]);
        Ok(Self {
            size: size.map(f64::from),
            scale,
            origin: [
                rect[0] + (rect[2] - rect[0] - document[0] * scale) / 2.0,
                rect[1] + (rect[3] - rect[1] - document[1] * scale) / 2.0,
            ],
            document,
            viewport: rect,
        })
    }
    pub fn navigated(mut self, zoom: f64, pan: [f64; 2]) -> Result<Self, SceneError> {
        if !zoom.is_finite()
            || !(0.25..=8.0).contains(&zoom)
            || !pan.iter().all(|n| n.is_finite() && n.abs() <= 32768.0)
        {
            return Err(SceneError::InvalidViewport);
        }
        let center = [
            (self.viewport[0] + self.viewport[2]) / 2.0,
            (self.viewport[1] + self.viewport[3]) / 2.0,
        ];
        self.scale *= zoom;
        self.origin =
            std::array::from_fn(|i| center[i] - self.document[i] * self.scale / 2.0 + pan[i]);
        Ok(self)
    }
    pub fn document_size(self) -> [f64; 2] {
        self.document
    }
    /// Inverse of the displayed document mapping for the shared stroke router.
    pub fn routing_viewport(self) -> paint_stroke::coordinates::Viewport {
        paint_stroke::coordinates::Viewport::new(self.origin, self.scale)
            .expect("validated mapping")
    }
    pub fn contains_client(self, client: [f64; 2]) -> bool {
        if client[0] < self.viewport[0]
            || client[1] < self.viewport[1]
            || client[0] > self.viewport[2]
            || client[1] > self.viewport[3]
        {
            return false;
        }
        self.routing_viewport()
            .to_document(client)
            .is_ok_and(|point| {
                point
                    .0
                    .iter()
                    .zip(self.document)
                    .all(|(value, limit)| (0.0..=limit).contains(value))
            })
    }
    /// Integer interior paper bounds for clipping stroke caps at canvas edges.
    pub fn scissor(self) -> [u32; 4] {
        let start: [u32; 2] = std::array::from_fn(|i| {
            self.origin[i]
                .max(self.viewport[i])
                .ceil()
                .clamp(0.0, self.size[i]) as u32
        });
        let end: [u32; 2] = std::array::from_fn(|i| {
            (self.origin[i] + self.document[i] * self.scale)
                .floor()
                .min(self.viewport[i + 2])
                .clamp(0.0, self.size[i]) as u32
        });
        [
            start[0],
            start[1],
            end[0].saturating_sub(start[0]),
            end[1].saturating_sub(start[1]),
        ]
    }
    pub fn to_clip(self, document: [f64; 2]) -> Result<[f32; 2], SceneError> {
        if !document
            .iter()
            .all(|v| v.is_finite() && v.abs() <= 2_000_000.0)
        {
            return Err(SceneError::InvalidCoordinates);
        }
        let p = [
            (document[0] * self.scale + self.origin[0]) / self.size[0] * 2.0 - 1.0,
            1.0 - (document[1] * self.scale + self.origin[1]) / self.size[1] * 2.0,
        ];
        let out = p.map(|v| v as f32);
        if !out.iter().all(|v| v.is_finite()) {
            return Err(SceneError::InvalidCoordinates);
        }
        Ok(out)
    }
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}
fn quad(
    out: &mut Vec<Vertex>,
    mapping: &CanvasMapping,
    points: [[f64; 2]; 4],
    color: [f32; 4],
) -> Result<(), SceneError> {
    for i in [0, 1, 2, 0, 2, 3] {
        out.push(Vertex {
            position: mapping.to_clip(points[i])?,
            color,
        });
    }
    Ok(())
}
