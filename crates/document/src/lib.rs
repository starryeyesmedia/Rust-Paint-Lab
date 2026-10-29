//! Authoritative sparse raster documents and independent provisional transactions.
//! Pixels are premultiplied encoded-sRGB RGBA8; this is not ICC color management.
#![forbid(unsafe_code)]
pub mod brush;
pub mod editing;
pub mod journal;
pub mod openraster;
pub mod recent;
pub mod stabilization;
pub mod storage;
pub mod vector;
use brush::{Brush, BrushKind};
use paint_stroke::{StrokeCommand, StrokeId, StrokeSample};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};
pub const TILE: u32 = 64;
pub const TILE_BYTES: usize = (TILE * TILE * 4) as usize;
pub const MASK_TILE_BYTES: usize = (TILE * TILE) as usize;
pub const MAX_TILES: usize = 8192; // Shared 128 MiB budget for raster and mask tiles.
pub const HISTORY_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_LAYERS: usize = 32;
pub type TileKey = (u32, u32);
pub type TileMap = BTreeMap<TileKey, Arc<Vec<u8>>>;
pub type MaskTileMap = BTreeMap<TileKey, Arc<Vec<u8>>>;
/// Operation used for the lifetime of a single pointer stroke. This is not
/// persisted in a brush preset or document: only the resulting pixels are.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub enum StrokeMode {
    #[default]
    Paint,
    /// Paint grayscale values into the selected layer's editable alpha mask.
    MaskPaint,
    Smudge,
    /// Blend the active color into raster pixels while depositing new paint.
    /// Strength is transient tool configuration and is intentionally excluded
    /// from the native document and brush-preset schemas.
    ColorMix {
        strength: f64,
    },
}

/// Transient radial/reflection drawing assistance. The normalized center and
/// selected mode belong to the local workspace; symmetry itself is not stored
/// in the native artwork file. Stroke transactions snapshot these settings so
/// changing the panel cannot alter an in-flight stroke.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SymmetryMode {
    #[default]
    Off,
    Vertical,
    Horizontal,
    VerticalAndHorizontal,
    Radial,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Symmetry {
    pub mode: SymmetryMode,
    /// Radial axis count, bounded to keep work per sample predictable.
    pub axes: u8,
    /// Origin relative to the current document bounds, from 0 to 1.
    pub center: [f64; 2],
}

impl Default for Symmetry {
    fn default() -> Self {
        Self {
            mode: SymmetryMode::Off,
            axes: 6,
            center: [0.5, 0.5],
        }
    }
}

impl Symmetry {
    pub fn validate(self) -> bool {
        (2..=16).contains(&self.axes)
            && self
                .center
                .iter()
                .all(|value| value.is_finite() && (0.0..=1.0).contains(value))
    }

    fn sample_count(self) -> usize {
        match self.mode {
            SymmetryMode::Off => 1,
            SymmetryMode::Vertical | SymmetryMode::Horizontal => 2,
            SymmetryMode::VerticalAndHorizontal => 4,
            SymmetryMode::Radial => usize::from(self.axes),
        }
    }

    fn matrix(self, index: usize) -> [f64; 4] {
        const IDENTITY: [f64; 4] = [1.0, 0.0, 0.0, 1.0];
        match self.mode {
            SymmetryMode::Off => IDENTITY,
            SymmetryMode::Vertical => {
                if index == 0 {
                    IDENTITY
                } else {
                    [-1.0, 0.0, 0.0, 1.0]
                }
            }
            SymmetryMode::Horizontal => {
                if index == 0 {
                    IDENTITY
                } else {
                    [1.0, 0.0, 0.0, -1.0]
                }
            }
            SymmetryMode::VerticalAndHorizontal => match index {
                0 => IDENTITY,
                1 => [-1.0, 0.0, 0.0, 1.0],
                2 => [1.0, 0.0, 0.0, -1.0],
                _ => [-1.0, 0.0, 0.0, -1.0],
            },
            SymmetryMode::Radial => {
                let angle = std::f64::consts::TAU * index as f64 / f64::from(self.axes);
                let (sin, cos) = angle.sin_cos();
                [cos, sin, -sin, cos]
            }
        }
    }

    fn transform_sample(
        self,
        mut sample: StrokeSample,
        [a, b, c, d]: [f64; 4],
        width: u32,
        height: u32,
    ) -> StrokeSample {
        let center_x = self.center[0] * f64::from(width);
        let center_y = self.center[1] * f64::from(height);
        let [x, y] = sample.position_document.0;
        sample.position_document.0 = [
            center_x + a * (x - center_x) + c * (y - center_y),
            center_y + b * (x - center_x) + d * (y - center_y),
        ];

        // Rotate a complete tilt vector along with radial copies. For a single
        // available tilt component only axis-aligned reflections are exact;
        // retain the missing-data distinction for rotations instead of
        // fabricating the unsupported component.
        match sample.tilt_degrees {
            [Some(x), Some(y)] => {
                let x = f64::from(x).to_radians().tan();
                let y = f64::from(y).to_radians().tan();
                sample.tilt_degrees = [
                    Some((a * x + c * y).atan().to_degrees() as f32),
                    Some((b * x + d * y).atan().to_degrees() as f32),
                ];
            }
            [Some(x), None] if b == 0.0 && c == 0.0 => {
                sample.tilt_degrees[0] = Some(x * a.signum() as f32);
            }
            [None, Some(y)] if b == 0.0 && c == 0.0 => {
                sample.tilt_degrees[1] = Some(y * d.signum() as f32);
            }
            _ => {}
        }
        sample
    }

    /// Return clipped document-space guide segments for the current mode.
    /// Radial guides use one bounded spoke per rotational copy.
    pub fn guide_lines(self, width: u32, height: u32) -> Vec<[[f64; 2]; 2]> {
        if !self.validate() || self.mode == SymmetryMode::Off || width == 0 || height == 0 {
            return Vec::new();
        }
        let origin = [
            self.center[0] * f64::from(width),
            self.center[1] * f64::from(height),
        ];
        if self.mode == SymmetryMode::Radial {
            return (0..self.axes)
                .filter_map(|index| {
                    let angle = std::f64::consts::TAU * f64::from(index) / f64::from(self.axes);
                    clipped_guide_ray(origin, [angle.cos(), angle.sin()], width, height)
                })
                .collect();
        }
        let directions: Vec<[f64; 2]> = match self.mode {
            SymmetryMode::Off => Vec::new(),
            SymmetryMode::Vertical => vec![[0.0, 1.0]],
            SymmetryMode::Horizontal => vec![[1.0, 0.0]],
            SymmetryMode::VerticalAndHorizontal => vec![[1.0, 0.0], [0.0, 1.0]],
            SymmetryMode::Radial => unreachable!("radial guides return bounded spokes above"),
        };
        directions
            .into_iter()
            .filter_map(|direction| clipped_guide_line(origin, direction, width, height))
            .collect()
    }
}

fn clipped_guide_ray(
    origin: [f64; 2],
    direction: [f64; 2],
    width: u32,
    height: u32,
) -> Option<[[f64; 2]; 2]> {
    let (width, height) = (f64::from(width), f64::from(height));
    let mut distance = f64::INFINITY;
    for (coordinate, delta, limit) in [
        (origin[0], direction[0], width),
        (origin[1], direction[1], height),
    ] {
        if delta > 1.0e-12 {
            distance = distance.min((limit - coordinate) / delta);
        } else if delta < -1.0e-12 {
            distance = distance.min((0.0 - coordinate) / delta);
        }
    }
    if !distance.is_finite() || distance <= 1.0e-9 {
        return None;
    }
    let end = [
        (origin[0] + direction[0] * distance).clamp(0.0, width),
        (origin[1] + direction[1] * distance).clamp(0.0, height),
    ];
    Some([origin, end])
}

fn clipped_guide_line(
    origin: [f64; 2],
    direction: [f64; 2],
    width: u32,
    height: u32,
) -> Option<[[f64; 2]; 2]> {
    let (width, height) = (f64::from(width), f64::from(height));
    let mut intersections = Vec::with_capacity(4);
    for x in [0.0, width] {
        if direction[0].abs() > 1.0e-9 {
            let t = (x - origin[0]) / direction[0];
            let y = origin[1] + t * direction[1];
            if (-1.0e-9..=height + 1.0e-9).contains(&y) {
                intersections.push([x, y.clamp(0.0, height)]);
            }
        }
    }
    for y in [0.0, height] {
        if direction[1].abs() > 1.0e-9 {
            let t = (y - origin[1]) / direction[1];
            let x = origin[0] + t * direction[0];
            if (-1.0e-9..=width + 1.0e-9).contains(&x) {
                let point = [x.clamp(0.0, width), y];
                if !intersections.iter().any(|other: &[f64; 2]| {
                    (other[0] - point[0]).abs() < 1.0e-9 && (other[1] - point[1]).abs() < 1.0e-9
                }) {
                    intersections.push(point);
                }
            }
        }
    }
    if intersections.len() < 2 {
        return None;
    }
    let mut pair = (0, 1);
    let mut longest = 0.0;
    for left in 0..intersections.len() {
        for right in left + 1..intersections.len() {
            let distance = (intersections[left][0] - intersections[right][0])
                .hypot(intersections[left][1] - intersections[right][1]);
            if distance > longest {
                pair = (left, right);
                longest = distance;
            }
        }
    }
    (longest > 1.0e-9).then_some([intersections[pair.0], intersections[pair.1]])
}

#[cfg(test)]
mod symmetry_transform_tests {
    use super::{Symmetry, SymmetryMode};
    use paint_input::{Buttons, SourceCounter, Tool};
    use paint_stroke::{StrokeSample, coordinates::DocumentPoint};

    fn sample(tilt_degrees: [Option<f32>; 2]) -> StrokeSample {
        StrokeSample {
            pointer_id: 91,
            source_counter: Some(SourceCounter {
                ticks: 42,
                ticks_per_second: std::num::NonZeroU64::new(1_000).unwrap(),
            }),
            tool: Tool::Pen,
            position_client_px: [17.0, 23.0],
            position_document: DocumentPoint([32.0, 64.0]),
            pressure: Some(0.37),
            tilt_degrees,
            buttons: Buttons {
                primary: true,
                barrel: true,
                ..Default::default()
            },
            source_time_ms: 123,
            time_ms: 125,
        }
    }

    #[test]
    fn rotated_copies_preserve_input_identity_pressure_and_unsupported_tilt() {
        let symmetry = Symmetry {
            mode: SymmetryMode::Radial,
            axes: 4,
            center: [0.5, 0.5],
        };
        let original = sample([Some(23.0), None]);
        let copy = symmetry.transform_sample(original, symmetry.matrix(1), 128, 128);
        assert_eq!(copy.position_document.0, [64.0, 32.0]);
        assert_eq!(copy.pointer_id, original.pointer_id);
        assert_eq!(copy.source_counter, original.source_counter);
        assert_eq!(copy.pressure, original.pressure);
        assert_eq!(copy.tilt_degrees, original.tilt_degrees);
        assert_eq!(copy.buttons, original.buttons);
        assert_eq!(copy.time_ms, original.time_ms);

        let complete = sample([Some(10.0), Some(0.0)]);
        let rotated = symmetry.transform_sample(complete, symmetry.matrix(1), 128, 128);
        assert!(rotated.tilt_degrees[0].unwrap().abs() < 1.0e-5);
        assert!((rotated.tilt_degrees[1].unwrap() - 10.0).abs() < 1.0e-5);

        let reflection = Symmetry {
            mode: SymmetryMode::Vertical,
            ..symmetry
        };
        let reflected = reflection.transform_sample(original, reflection.matrix(1), 128, 128);
        assert_eq!(reflected.tilt_degrees, [Some(-23.0), None]);
    }
}

#[derive(Debug)]
pub enum PaintError {
    Invalid,
    Format(String),
    Limit,
    Locked,
    Busy,
    Conflict,
    Io(std::io::Error),
    Json(serde_json::Error),
    Png(String),
    Image(String),
}
impl std::fmt::Display for PaintError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Format(message) => {
                write!(f, "Unsupported or invalid interchange file: {message}")
            }
            Self::Invalid => write!(f, "Invalid or unsupported document data or settings"),
            Self::Limit => write!(
                f,
                "Operation exceeds the alpha's document, brush, layer or memory limits"
            ),
            Self::Locked => write!(f, "The selected layer is locked or hidden"),
            Self::Busy => write!(
                f,
                "The project is busy in another operation or application instance"
            ),
            Self::Conflict => write!(
                f,
                "File already exists or changed outside this session. Open it first or choose a new filename"
            ),
            Self::Io(e) => write!(f, "File operation failed: {e}"),
            Self::Json(e) => write!(f, "Invalid project or preset metadata: {e}"),
            Self::Png(e) => write!(f, "PNG operation failed: {e}"),
            Self::Image(e) => write!(f, "Image could not be decoded: {e}"),
        }
    }
}
impl std::error::Error for PaintError {}
impl From<std::io::Error> for PaintError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<serde_json::Error> for PaintError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Blend {
    Normal,
    Multiply,
    Screen,
    Overlay,
}
/// Editable 8-bit per-pixel alpha mask. Missing sparse tiles are fully white.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LayerMask {
    pub enabled: bool,
    pub inverted: bool,
    #[serde(skip)]
    pub tiles: MaskTileMap,
}
impl Default for LayerMask {
    fn default() -> Self {
        Self {
            enabled: true,
            inverted: false,
            tiles: BTreeMap::new(),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: u8,
    pub blend: Blend,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mask: Option<LayerMask>,
    /// Editable vector artwork is composited above this layer's raster tiles.
    /// Empty in v1/v2 projects; nonempty paths select native format v3.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vector_paths: Vec<vector::VectorPath>,
    #[serde(skip)]
    pub tiles: TileMap,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    #[serde(skip)]
    pub selection: Option<editing::Selection>,
    pub width: u32,
    pub height: u32,
    pub dpi: u32,
    pub layers: Vec<Layer>,
    pub selected: u64,
    pub background: Option<[u8; 3]>,
    pub brush: Brush,
    /// Monotonic ID allocator persisted from v3 onward so deleting the final
    /// vector object and reopening cannot cause a later object-ID collision.
    #[serde(
        default = "first_vector_id",
        skip_serializing_if = "is_first_vector_id"
    )]
    pub next_vector_id: u64,
}
fn first_vector_id() -> u64 {
    1
}
fn is_first_vector_id(id: &u64) -> bool {
    *id == 1
}
impl State {
    pub fn new(width: u32, height: u32, dpi: u32) -> Result<Self, PaintError> {
        let s = Self {
            selection: None,
            width,
            height,
            dpi,
            layers: vec![Layer {
                id: 1,
                name: "Paint 1".into(),
                visible: true,
                locked: false,
                opacity: 255,
                blend: Blend::Normal,
                mask: None,
                vector_paths: Vec::new(),
                tiles: BTreeMap::new(),
            }],
            selected: 1,
            background: Some([255; 3]),
            brush: Brush::default(),
            next_vector_id: 1,
        };
        s.validate()?;
        Ok(s)
    }
    pub fn validate(&self) -> Result<(), PaintError> {
        if self.width == 0
            || self.height == 0
            || self.width > 4096
            || self.height > 4096
            || !(1..=9600).contains(&self.dpi)
            || self.layers.is_empty()
            || self.layers.len() > MAX_LAYERS
        {
            return Err(PaintError::Invalid);
        }
        if self.next_vector_id == 0 || self.next_vector_id > u64::from(u32::MAX) + 1 {
            return Err(PaintError::Invalid);
        }
        if !self.brush.validate() {
            return Err(PaintError::Invalid);
        }
        if let Some(selection) = &self.selection {
            selection.validate()?;
            let bounds = selection.bounds();
            if bounds[2] > f64::from(self.width) || bounds[3] > f64::from(self.height) {
                return Err(PaintError::Invalid);
            }
        }
        let mut ids = BTreeSet::new();
        let mut vector_ids = BTreeSet::new();
        let mut total = 0_usize;
        let mut vector_segments = 0_usize;
        let mut vector_work = 0_usize;
        for l in &self.layers {
            if l.name.len() > 256 || l.id == 0 || l.id > u64::from(u32::MAX) || !ids.insert(l.id) {
                return Err(PaintError::Invalid);
            }
            for (&(x, y), t) in &l.tiles {
                if x >= self.width.div_ceil(TILE)
                    || y >= self.height.div_ceil(TILE)
                    || t.len() != TILE_BYTES
                    || t.chunks_exact(4)
                        .any(|p| p[0] > p[3] || p[1] > p[3] || p[2] > p[3])
                {
                    return Err(PaintError::Invalid);
                }
            }
            total += l.tiles.len();
            if let Some(mask) = &l.mask {
                for (&(x, y), tile) in &mask.tiles {
                    if x >= self.width.div_ceil(TILE)
                        || y >= self.height.div_ceil(TILE)
                        || tile.len() != MASK_TILE_BYTES
                    {
                        return Err(PaintError::Invalid);
                    }
                    total += 1;
                }
            }
            for path in &l.vector_paths {
                path.validate()?;
                if path.id >= self.next_vector_id || !vector_ids.insert(path.id) {
                    return Err(PaintError::Invalid);
                }
                vector_segments = vector_segments
                    .checked_add(path.segments.len())
                    .ok_or(PaintError::Limit)?;
                if vector_ids.len() > vector::MAX_VECTOR_PATHS
                    || vector_segments > vector::MAX_VECTOR_SEGMENTS
                {
                    return Err(PaintError::Limit);
                }
            }
            if total > MAX_TILES {
                return Err(PaintError::Limit);
            }
            vector_work = vector_work
                .checked_add(
                    vector::estimated_layer_work(&l.vector_paths, [self.width, self.height])
                        .ok_or(PaintError::Limit)?,
                )
                .filter(|work| *work <= vector::MAX_VECTOR_WORK)
                .ok_or(PaintError::Limit)?;
        }
        if vector_work > vector::MAX_VECTOR_WORK {
            return Err(PaintError::Limit);
        }
        if !ids.contains(&self.selected) {
            return Err(PaintError::Limit);
        }
        Ok(())
    }
    pub fn tile_count(&self) -> usize {
        self.layers
            .iter()
            .map(|layer| layer.tiles.len() + layer.mask.as_ref().map_or(0, |mask| mask.tiles.len()))
            .sum()
    }

    /// Compare a candidate edit with its COW snapshot without reading tile
    /// pixels. Engine edits share unchanged `Arc` tiles, so pointer identity is
    /// sufficient to identify those allocations and avoids a whole-document
    /// byte scan on every property action.
    fn same_snapshot(&self, other: &Self) -> bool {
        self.selection == other.selection
            && self.width == other.width
            && self.height == other.height
            && self.dpi == other.dpi
            && self.selected == other.selected
            && self.background == other.background
            && self.brush == other.brush
            && self.next_vector_id == other.next_vector_id
            && self.layers.len() == other.layers.len()
            && self.layers.iter().zip(&other.layers).all(|(left, right)| {
                left.id == right.id
                    && left.name == right.name
                    && left.visible == right.visible
                    && left.locked == right.locked
                    && left.opacity == right.opacity
                    && left.blend == right.blend
                    && same_mask_snapshot(left.mask.as_ref(), right.mask.as_ref())
                    && left.vector_paths == right.vector_paths
                    && left.tiles.len() == right.tiles.len()
                    && left.tiles.iter().all(|(key, tile)| {
                        right
                            .tiles
                            .get(key)
                            .is_some_and(|other_tile| Arc::ptr_eq(tile, other_tile))
                    })
            })
    }
}
fn same_mask_snapshot(left: Option<&LayerMask>, right: Option<&LayerMask>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            left.enabled == right.enabled
                && left.inverted == right.inverted
                && left.tiles.len() == right.tiles.len()
                && left.tiles.iter().all(|(key, tile)| {
                    right
                        .tiles
                        .get(key)
                        .is_some_and(|other_tile| Arc::ptr_eq(tile, other_tile))
                })
        }
        _ => false,
    }
}
struct Transaction {
    layer: u64,
    brush: Brush,
    mode: StrokeMode,
    symmetry: Symmetry,
    symmetry_transforms: [[f64; 4]; 16],
    symmetry_count: usize,
    smudge_carry: [Option<[u8; 4]>; 16],
    last: StrokeSample,
    remaining: f64,
    stabilizer: stabilization::StrokeStabilizer,
    tiles: TileMap,
    mask_tiles: MaskTileMap,
}
pub struct Engine {
    state: State,
    pub brush: Brush,
    active: BTreeMap<StrokeId, Transaction>,
    symmetry: Symmetry,
    undo: VecDeque<State>,
    redo: VecDeque<State>,
    damage: BTreeSet<TileKey>,
    pub revision: u64,
    pub saved_revision: u64,
}
impl Engine {
    pub fn new(width: u32, height: u32, dpi: u32) -> Result<Self, PaintError> {
        Self::from_state(State::new(width, height, dpi)?)
    }
    pub fn from_state(state: State) -> Result<Self, PaintError> {
        state.validate()?;
        let brush = state.brush.clone();
        let mut e = Self {
            state,
            brush,
            active: BTreeMap::new(),
            symmetry: Symmetry::default(),
            undo: VecDeque::new(),
            redo: VecDeque::new(),
            damage: BTreeSet::new(),
            revision: 0,
            saved_revision: 0,
        };
        e.damage_all();
        Ok(e)
    }
    pub fn sync_brush_settings(&mut self) {
        if self.state.brush != self.brush {
            self.state.brush = self.brush.clone();
            self.revision = self.revision.saturating_add(1);
        }
    }
    pub fn state(&self) -> &State {
        &self.state
    }
    pub fn active_count(&self) -> usize {
        self.active.len()
    }
    pub fn symmetry(&self) -> Symmetry {
        self.symmetry
    }
    /// Set symmetry for strokes that begin afterward. Active strokes retain
    /// their captured settings and can finish or cancel normally.
    pub fn set_symmetry(&mut self, symmetry: Symmetry) -> Result<(), PaintError> {
        if !symmetry.validate() {
            return Err(PaintError::Invalid);
        }
        self.symmetry = symmetry;
        Ok(())
    }
    pub fn undo_count(&self) -> usize {
        self.undo.len()
    }
    pub fn redo_count(&self) -> usize {
        self.redo.len()
    }
    pub fn dirty(&self) -> bool {
        self.revision != self.saved_revision
    }
    pub fn take_damage(&mut self) -> BTreeSet<TileKey> {
        std::mem::take(&mut self.damage)
    }
    pub fn damage_all(&mut self) {
        for y in 0..self.state.height.div_ceil(TILE) {
            for x in 0..self.state.width.div_ceil(TILE) {
                self.damage.insert((x, y));
            }
        }
    }
    fn queue_bytes(queue: &VecDeque<State>) -> usize {
        // Arc identity counts COW allocations once, not once per snapshot/layer.
        let mut allocations = std::collections::HashMap::new();
        let mut vector_bytes = 0usize;
        for s in queue {
            for layer in &s.layers {
                vector_bytes += vector::layer_vectors_bytes(layer);
                for tile in layer.tiles.values() {
                    allocations.insert(Arc::as_ptr(tile), TILE_BYTES);
                }
                if let Some(mask) = &layer.mask {
                    for tile in mask.tiles.values() {
                        allocations.insert(Arc::as_ptr(tile), MASK_TILE_BYTES);
                    }
                }
            }
        }
        allocations.values().sum::<usize>() + vector_bytes
    }
    pub fn history_bytes(&self) -> [usize; 2] {
        [Self::queue_bytes(&self.undo), Self::queue_bytes(&self.redo)]
    }
    fn retain_history(queue: &mut VecDeque<State>, s: State) {
        queue.push_back(s);
        while queue.len() > 32 || Self::queue_bytes(queue) > HISTORY_BYTES {
            queue.pop_front();
        }
    }
    fn changed(&mut self, before: State) {
        Self::retain_history(&mut self.undo, before);
        self.redo.clear();
        self.revision = self.revision.saturating_add(1);
    }
    pub fn cancel_all(&mut self) {
        for t in self.active.values() {
            self.damage.extend(t.tiles.keys());
            self.damage.extend(t.mask_tiles.keys());
        }
        self.active.clear();
    }
    pub fn undo(&mut self) -> bool {
        self.cancel_all();
        if let Some(before) = self.undo.pop_back() {
            Self::retain_history(&mut self.redo, std::mem::replace(&mut self.state, before));
            self.revision = self.revision.saturating_add(1);
            self.damage_all();
            true
        } else {
            false
        }
    }
    pub fn redo(&mut self) -> bool {
        self.cancel_all();
        if let Some(after) = self.redo.pop_back() {
            Self::retain_history(&mut self.undo, std::mem::replace(&mut self.state, after));
            self.revision = self.revision.saturating_add(1);
            self.damage_all();
            true
        } else {
            false
        }
    }
    pub fn edit(
        &mut self,
        operation: impl FnOnce(&mut State) -> Result<(), PaintError>,
    ) -> Result<(), PaintError> {
        self.cancel_all();
        let before = self.state.clone();
        let mut next = before.clone();
        operation(&mut next)?;
        next.validate()?;
        if next.same_snapshot(&before) {
            return Ok(());
        }
        self.state = next;
        self.changed(before);
        self.damage_all();
        Ok(())
    }
    pub fn add_layer(&mut self) -> Result<(), PaintError> {
        self.edit(|s| {
            if s.layers.len() == MAX_LAYERS {
                return Err(PaintError::Limit);
            }
            let id = s.layers.iter().map(|l| l.id).max().unwrap_or(0) + 1;
            s.layers.push(Layer {
                id,
                name: format!("Paint {id}"),
                visible: true,
                locked: false,
                opacity: 255,
                blend: Blend::Normal,
                mask: None,
                vector_paths: Vec::new(),
                tiles: BTreeMap::new(),
            });
            s.selected = id;
            Ok(())
        })
    }
    pub fn duplicate_layer(&mut self) -> Result<(), PaintError> {
        self.edit(|s| {
            let mut l = s
                .layers
                .iter()
                .find(|l| l.id == s.selected)
                .ok_or(PaintError::Invalid)?
                .clone();
            l.id = s.layers.iter().map(|l| l.id).max().unwrap_or(0) + 1;
            l.name = format!("{} copy", l.name).chars().take(256).collect();
            let mut next_vector_id = s.next_vector_id.saturating_sub(1).max(
                s.layers
                    .iter()
                    .flat_map(|layer| layer.vector_paths.iter().map(|path| path.id))
                    .max()
                    .unwrap_or(0),
            );
            for path in &mut l.vector_paths {
                next_vector_id = next_vector_id
                    .checked_add(1)
                    .filter(|id| *id <= u64::from(u32::MAX))
                    .ok_or(PaintError::Limit)?;
                path.id = next_vector_id;
            }
            s.next_vector_id = next_vector_id.checked_add(1).ok_or(PaintError::Limit)?;
            s.selected = l.id;
            s.layers.push(l);
            Ok(())
        })
    }
    pub fn delete_layer(&mut self) -> Result<(), PaintError> {
        self.edit(|s| {
            if s.layers.len() == 1 {
                return Err(PaintError::Invalid);
            }
            s.layers.retain(|l| l.id != s.selected);
            s.selected = s.layers.last().unwrap().id;
            Ok(())
        })
    }
    pub fn clear_layer(&mut self) -> Result<(), PaintError> {
        self.clear_layer_by_id(self.state.selected)
    }
    pub fn clear_layer_by_id(&mut self, id: u64) -> Result<(), PaintError> {
        self.edit(|s| {
            let l = s
                .layers
                .iter_mut()
                .find(|l| l.id == id)
                .ok_or(PaintError::Invalid)?;
            if l.locked {
                return Err(PaintError::Locked);
            }
            l.tiles.clear();
            l.vector_paths.clear();
            Ok(())
        })
    }

    /// Create one editable polyline above raster pixels in the selected layer.
    /// The current brush color, opacity and size become the path's initial
    /// vector stroke style; later raster brush changes do not alter it.
    pub fn add_vector_path(&mut self, points: &[[f64; 2]]) -> Result<u64, PaintError> {
        let layer = self
            .state
            .layers
            .iter()
            .find(|layer| layer.id == self.state.selected)
            .ok_or(PaintError::Invalid)?;
        if layer.locked || !layer.visible {
            return Err(PaintError::Locked);
        }
        let id = self.state.next_vector_id;
        if id > u64::from(u32::MAX) {
            return Err(PaintError::Limit);
        }
        let mut color = [0; 4];
        color[..3].copy_from_slice(&self.brush.color);
        color[3] = (self.brush.opacity * 255.0).round().clamp(0.0, 255.0) as u8;
        let path = vector::VectorPath::from_polyline(id, points, color, self.brush.size)?;
        self.edit(|state| {
            let selected = state.selected;
            let layer = state
                .layers
                .iter_mut()
                .find(|layer| layer.id == selected)
                .ok_or(PaintError::Invalid)?;
            if layer.locked || !layer.visible {
                return Err(PaintError::Locked);
            }
            layer.vector_paths.push(path);
            state.next_vector_id = id.checked_add(1).ok_or(PaintError::Limit)?;
            Ok(())
        })?;
        Ok(id)
    }

    /// Select the topmost visible path within tolerance, in document pixels.
    pub fn hit_test_vector_path(&self, point: [f64; 2], tolerance: f64) -> Option<u64> {
        if !point.iter().all(|value| value.is_finite()) || !tolerance.is_finite() || tolerance < 0.0
        {
            return None;
        }
        for layer in self.state.layers.iter().rev().filter(|layer| layer.visible) {
            for path in layer.vector_paths.iter().rev().filter(|path| path.visible) {
                let allowed = path.width * 0.5 + tolerance;
                if vector::path_distance(path, point) <= allowed {
                    return Some(path.id);
                }
            }
        }
        None
    }

    /// Translate an editable path as one undoable operation. The full path
    /// remains inside document bounds, and hidden/locked owner layers reject it.
    pub fn translate_vector_path(&mut self, id: u64, delta: [f64; 2]) -> Result<(), PaintError> {
        if !delta.iter().all(|value| value.is_finite()) {
            return Err(PaintError::Invalid);
        }
        self.edit(|state| {
            let layer = state
                .layers
                .iter_mut()
                .find(|layer| layer.vector_paths.iter().any(|path| path.id == id))
                .ok_or(PaintError::Invalid)?;
            if layer.locked || !layer.visible {
                return Err(PaintError::Locked);
            }
            let path = layer
                .vector_paths
                .iter_mut()
                .find(|path| path.id == id)
                .ok_or(PaintError::Invalid)?;
            let bounds = path.bounds();
            if bounds[2] - bounds[0] > f64::from(state.width)
                || bounds[3] - bounds[1] > f64::from(state.height)
            {
                return Err(PaintError::Invalid);
            }
            let dx = delta[0].clamp(-bounds[0], f64::from(state.width) - bounds[2]);
            let dy = delta[1].clamp(-bounds[1], f64::from(state.height) - bounds[3]);
            path.translate([dx, dy])
        })
    }

    /// Change a path's immutable-by-default stroke style as one document
    /// transaction. This preserves its editable geometry and invalidates the
    /// owning layer's derived composite and thumbnail data through `edit`.
    pub fn set_vector_path_style(
        &mut self,
        id: u64,
        stroke: [u8; 4],
        width: f64,
    ) -> Result<(), PaintError> {
        if !width.is_finite() || !(0.25..=512.0).contains(&width) {
            return Err(PaintError::Invalid);
        }
        self.edit(|state| {
            let layer = state
                .layers
                .iter_mut()
                .find(|layer| layer.vector_paths.iter().any(|path| path.id == id))
                .ok_or(PaintError::Invalid)?;
            if layer.locked || !layer.visible {
                return Err(PaintError::Locked);
            }
            let path = layer
                .vector_paths
                .iter_mut()
                .find(|path| path.id == id)
                .ok_or(PaintError::Invalid)?;
            path.stroke = stroke;
            path.width = width;
            path.validate()
        })
    }

    /// Move one path anchor as a single history transaction. Adjacent cubic
    /// handles follow the anchor, retaining their tangent offsets.
    pub fn move_vector_path_anchor(
        &mut self,
        id: u64,
        anchor: usize,
        point: [f64; 2],
    ) -> Result<(), PaintError> {
        self.edit(|state| {
            let layer = state
                .layers
                .iter_mut()
                .find(|layer| layer.vector_paths.iter().any(|path| path.id == id))
                .ok_or(PaintError::Invalid)?;
            if layer.locked || !layer.visible {
                return Err(PaintError::Locked);
            }
            let path = layer
                .vector_paths
                .iter_mut()
                .find(|path| path.id == id)
                .ok_or(PaintError::Invalid)?;
            path.move_anchor(anchor, point)
        })
    }

    /// Move one Bézier control handle as a single history transaction.
    pub fn move_vector_path_handle(
        &mut self,
        id: u64,
        anchor: usize,
        handle: vector::VectorHandle,
        point: [f64; 2],
    ) -> Result<(), PaintError> {
        self.edit(|state| {
            let layer = state
                .layers
                .iter_mut()
                .find(|layer| layer.vector_paths.iter().any(|path| path.id == id))
                .ok_or(PaintError::Invalid)?;
            if layer.locked || !layer.visible {
                return Err(PaintError::Locked);
            }
            let path = layer
                .vector_paths
                .iter_mut()
                .find(|path| path.id == id)
                .ok_or(PaintError::Invalid)?;
            path.move_control_handle(anchor, handle, point)
        })
    }

    /// Split the segment after one anchor and return the new anchor index.
    pub fn insert_vector_path_anchor(
        &mut self,
        id: u64,
        after_anchor: usize,
    ) -> Result<usize, PaintError> {
        let selected_index = after_anchor.checked_add(1).ok_or(PaintError::Invalid)?;
        self.edit(|state| {
            let layer = state
                .layers
                .iter_mut()
                .find(|layer| layer.vector_paths.iter().any(|path| path.id == id))
                .ok_or(PaintError::Invalid)?;
            if layer.locked || !layer.visible {
                return Err(PaintError::Locked);
            }
            let path = layer
                .vector_paths
                .iter_mut()
                .find(|path| path.id == id)
                .ok_or(PaintError::Invalid)?;
            path.insert_anchor_after(after_anchor)?;
            Ok(())
        })?;
        Ok(selected_index)
    }

    /// Remove one anchor and return the neighboring anchor selected afterward.
    pub fn remove_vector_path_anchor(
        &mut self,
        id: u64,
        anchor: usize,
    ) -> Result<usize, PaintError> {
        let mut selected_index = 0;
        self.edit(|state| {
            let layer = state
                .layers
                .iter_mut()
                .find(|layer| layer.vector_paths.iter().any(|path| path.id == id))
                .ok_or(PaintError::Invalid)?;
            if layer.locked || !layer.visible {
                return Err(PaintError::Locked);
            }
            let path = layer
                .vector_paths
                .iter_mut()
                .find(|path| path.id == id)
                .ok_or(PaintError::Invalid)?;
            selected_index = path.remove_anchor(anchor)?;
            Ok(())
        })?;
        Ok(selected_index)
    }

    /// Remove one editable path from its visible, unlocked owner layer.
    pub fn remove_vector_path(&mut self, id: u64) -> Result<(), PaintError> {
        self.edit(|state| {
            let layer = state
                .layers
                .iter_mut()
                .find(|layer| layer.vector_paths.iter().any(|path| path.id == id))
                .ok_or(PaintError::Invalid)?;
            if layer.locked || !layer.visible {
                return Err(PaintError::Locked);
            }
            layer.vector_paths.retain(|path| path.id != id);
            Ok(())
        })
    }
    pub fn select(&mut self, id: u64) {
        self.cancel_all();
        if self.state.layers.iter().any(|l| l.id == id) {
            self.state.selected = id;
        }
    }
    pub fn apply(&mut self, c: &StrokeCommand) -> Result<(), PaintError> {
        self.apply_with_mode(c, StrokeMode::Paint)
    }

    /// Apply a command using the requested mode when a transaction begins.
    /// Later commands retain the mode captured by that stroke's BeginStroke.
    pub fn apply_with_mode(
        &mut self,
        c: &StrokeCommand,
        mode: StrokeMode,
    ) -> Result<(), PaintError> {
        if matches!(c, StrokeCommand::BeginStroke { .. })
            && matches!(mode, StrokeMode::ColorMix { strength } if !strength.is_finite() || !(0.0..=1.0).contains(&strength))
        {
            return Err(PaintError::Invalid);
        }
        let candidate = match c {
            StrokeCommand::BeginStroke { stroke_id, sample }
            | StrokeCommand::BreakStrokeSegment { stroke_id, sample }
            | StrokeCommand::UpdateStroke { stroke_id, sample }
            | StrokeCommand::CommitStroke { stroke_id, sample } => Some((*stroke_id, *sample)),
            _ => None,
        };
        if let Some((id, s)) = candidate
            && (!s
                .position_document
                .0
                .iter()
                .all(|n| n.is_finite() && n.abs() <= 1_000_000.0)
                || s.pressure
                    .is_some_and(|p| !p.is_finite() || !(0.0..=1.0).contains(&p))
                || s.tilt_degrees
                    .iter()
                    .flatten()
                    .any(|t| !t.is_finite() || !(-90.0..=90.0).contains(t)))
        {
            if let Some(t) = self.active.remove(&id) {
                self.damage.extend(t.tiles.keys());
                self.damage.extend(t.mask_tiles.keys());
            }
            return Err(PaintError::Invalid);
        }
        match *c {
            StrokeCommand::BeginStroke { stroke_id, sample } => {
                if self.active.contains_key(&stroke_id) {
                    return Ok(());
                }
                // Inversion is an explicit device-level erase gesture and
                // retains precedence over the selected contextual tool.
                let effective_mode = if sample.buttons.eraser || sample.buttons.inverted {
                    if mode == StrokeMode::MaskPaint {
                        StrokeMode::MaskPaint
                    } else {
                        StrokeMode::Paint
                    }
                } else {
                    mode
                };
                if (stroke_mode_needs_exclusive_layer(effective_mode) && !self.active.is_empty())
                    || self
                        .active
                        .values()
                        .any(|transaction| stroke_mode_needs_exclusive_layer(transaction.mode))
                {
                    // A second pointer cannot safely mutate the same layer
                    // while a pixel-replacement transaction owns a private
                    // working copy. Ignore that contact rather than allowing
                    // stale tiles to overwrite another stroke.
                    return Ok(());
                }
                if !self.brush.validate() || self.active.len() >= 64 {
                    return Err(PaintError::Limit);
                }
                let layer = self
                    .state
                    .layers
                    .iter()
                    .find(|l| l.id == self.state.selected)
                    .ok_or(PaintError::Invalid)?;
                if layer.locked || !layer.visible {
                    return Err(PaintError::Locked);
                }
                if effective_mode == StrokeMode::MaskPaint && layer.mask.is_none() {
                    return Err(PaintError::Invalid);
                }
                let mut brush = self.brush.clone();
                if sample.buttons.eraser || sample.buttons.inverted {
                    brush.kind = BrushKind::Eraser;
                }
                let symmetry = self.symmetry;
                let symmetry_count = symmetry.sample_count();
                let mut symmetry_transforms = [[0.0; 4]; 16];
                for (index, transform) in symmetry_transforms
                    .iter_mut()
                    .enumerate()
                    .take(symmetry_count)
                {
                    *transform = symmetry.matrix(index);
                }
                let mut t = Transaction {
                    layer: layer.id,
                    brush,
                    mode: effective_mode,
                    symmetry,
                    symmetry_transforms,
                    symmetry_count,
                    smudge_carry: [None; 16],
                    last: sample,
                    remaining: 0.0,
                    stabilizer: stabilization::StrokeStabilizer::new(
                        sample.position_document.0,
                        sample.time_ms,
                    ),
                    tiles: BTreeMap::new(),
                    mask_tiles: BTreeMap::new(),
                };
                self.dab_with_reset(&mut t, sample, true)?;
                self.active.insert(stroke_id, t);
            }
            StrokeCommand::BreakStrokeSegment { stroke_id, sample } => {
                let Some(mut t) = self.active.remove(&stroke_id) else {
                    return Ok(());
                };
                t.last = sample;
                t.remaining = 0.0;
                t.stabilizer
                    .reset(sample.position_document.0, sample.time_ms);
                let result = self.dab_with_reset(&mut t, sample, true);
                if let Err(error) = result {
                    self.damage.extend(t.tiles.keys());
                    self.damage.extend(t.mask_tiles.keys());
                    return Err(error);
                }
                self.active.insert(stroke_id, t);
            }
            StrokeCommand::UpdateStroke { stroke_id, sample }
            | StrokeCommand::CommitStroke { stroke_id, sample } => {
                let Some(mut t) = self.active.remove(&stroke_id) else {
                    return Ok(());
                };
                let commit = matches!(c, StrokeCommand::CommitStroke { .. });
                if let Err(e) = self.segment(&mut t, sample) {
                    self.damage.extend(t.tiles.keys());
                    self.damage.extend(t.mask_tiles.keys());
                    return Err(e);
                }
                if commit && t.last.position_document.0 != sample.position_document.0 {
                    // A filtered endpoint can lag behind its raw sample. Draw a
                    // short, direct terminal segment so the visible stroke
                    // always reaches the actual released point.
                    let smoothing = t.brush.smoothing;
                    t.brush.smoothing = 0.0;
                    let result = self.segment(&mut t, sample);
                    t.brush.smoothing = smoothing;
                    if let Err(error) = result {
                        self.damage.extend(t.tiles.keys());
                        self.damage.extend(t.mask_tiles.keys());
                        return Err(error);
                    }
                }
                if commit {
                    // Final dab is explicit even when sample spacing has a remainder.
                    if let Err(e) = self.dab(&mut t, sample) {
                        self.damage.extend(t.tiles.keys());
                        self.damage.extend(t.mask_tiles.keys());
                        return Err(e);
                    }
                    let before = self.state.clone();
                    let mut any_change = false;
                    if t.mode == StrokeMode::MaskPaint {
                        let layer = self
                            .state
                            .layers
                            .iter_mut()
                            .find(|layer| layer.id == t.layer)
                            .ok_or(PaintError::Invalid)?;
                        let mask = layer.mask.as_mut().ok_or(PaintError::Invalid)?;
                        for (key, src) in t.mask_tiles {
                            let same = mask
                                .tiles
                                .get(&key)
                                .is_some_and(|existing| existing.as_ref() == src.as_ref())
                                || (!mask.tiles.contains_key(&key)
                                    && src.iter().all(|value| *value == 255));
                            if !same {
                                if src.iter().any(|value| *value != 255) {
                                    mask.tiles.insert(key, src);
                                } else {
                                    mask.tiles.remove(&key);
                                }
                                any_change = true;
                            }
                        }
                    } else {
                        let layer = self
                            .state
                            .layers
                            .iter_mut()
                            .find(|layer| layer.id == t.layer)
                            .ok_or(PaintError::Invalid)?;
                        for (key, src) in t.tiles {
                            if stroke_mode_needs_exclusive_layer(t.mode) {
                                let same = layer
                                    .tiles
                                    .get(&key)
                                    .is_some_and(|existing| existing.as_ref() == src.as_ref())
                                    || (!layer.tiles.contains_key(&key)
                                        && src.iter().all(|&byte| byte == 0));
                                if !same {
                                    if src.iter().any(|&byte| byte != 0) {
                                        layer.tiles.insert(key, src);
                                    } else {
                                        layer.tiles.remove(&key);
                                    }
                                    any_change = true;
                                }
                            } else {
                                any_change = true;
                                let dst = layer
                                    .tiles
                                    .entry(key)
                                    .or_insert_with(|| Arc::new(vec![0; TILE_BYTES]));
                                let dst = Arc::make_mut(dst);
                                for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                                    over(
                                        d,
                                        s,
                                        t.brush.kind == BrushKind::Eraser,
                                        (t.brush.opacity * 255.0).round() as u8,
                                        Blend::Normal,
                                    );
                                }
                            }
                        }
                    }
                    if any_change {
                        self.changed(before);
                    }
                } else {
                    self.active.insert(stroke_id, t);
                }
            }
            StrokeCommand::CancelStroke { stroke_id, .. } => {
                if let Some(t) = self.active.remove(&stroke_id) {
                    self.damage.extend(t.tiles.keys());
                    self.damage.extend(t.mask_tiles.keys());
                }
            }
        }
        Ok(())
    }
    fn segment(&mut self, t: &mut Transaction, mut end: StrokeSample) -> Result<(), PaintError> {
        let a = t.last.position_document.0;
        let b = t
            .stabilizer
            .update(end.position_document.0, end.time_ms, t.brush.smoothing);
        if !b.iter().all(|x| x.is_finite() && x.abs() <= 1_000_000.0) {
            return Err(PaintError::Invalid);
        }
        end.position_document.0 = b;
        let distance = (b[0] - a[0]).hypot(b[1] - a[1]);
        let step = (t.brush.size * t.brush.spacing).max(0.5);
        if distance / step > 16384.0 {
            return Err(PaintError::Limit);
        }
        let mut along = if t.remaining > 0.0 { t.remaining } else { step };
        while along <= distance && distance > 0.0 {
            let f = along / distance;
            let mut p = end;
            p.position_document.0 = [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f];
            p.pressure = match (t.last.pressure, end.pressure) {
                (Some(a), Some(b)) => Some(a + (b - a) * f as f32),
                _ => end.pressure,
            };
            self.dab(t, p)?;
            along += step;
        }
        t.remaining = along - distance;
        t.last = end;
        Ok(())
    }
    fn dab(&mut self, t: &mut Transaction, sample: StrokeSample) -> Result<(), PaintError> {
        self.dab_with_reset(t, sample, false)
    }

    fn dab_with_reset(
        &mut self,
        t: &mut Transaction,
        sample: StrokeSample,
        reset_smudge: bool,
    ) -> Result<(), PaintError> {
        // At most sixteen copies are accepted. A fixed stack buffer prevents
        // per-sample heap allocation in the latency-sensitive dab path.
        let mut visited_positions = [[0.0; 2]; 16];
        let mut visited_count = 0;
        for index in 0..t.symmetry_count {
            let sample = t.symmetry.transform_sample(
                sample,
                t.symmetry_transforms[index],
                self.state.width,
                self.state.height,
            );
            let position = sample.position_document.0;
            let duplicate = visited_positions[..visited_count].iter().any(|other| {
                (other[0] - position[0]).abs() <= 1.0e-7 && (other[1] - position[1]).abs() <= 1.0e-7
            });
            if !duplicate {
                visited_positions[visited_count] = position;
                visited_count += 1;
            }
            match t.mode {
                StrokeMode::Smudge => {
                    self.smudge_dab(t, sample, reset_smudge, index, !duplicate)?
                }
                StrokeMode::ColorMix { .. } if !duplicate => self.color_mix_dab(t, sample)?,
                StrokeMode::ColorMix { .. } => {}
                StrokeMode::MaskPaint if !duplicate => self.mask_dab(t, sample)?,
                StrokeMode::MaskPaint => {}
                StrokeMode::Paint if !duplicate => self.paint_dab(t, sample)?,
                StrokeMode::Paint => {}
            }
        }
        Ok(())
    }

    fn paint_dab(&mut self, t: &mut Transaction, s: StrokeSample) -> Result<(), PaintError> {
        let [x, y] = s.position_document.0;
        if ![x, y].iter().all(|v| v.is_finite()) {
            return Err(PaintError::Invalid);
        }
        let pressure = f64::from(s.pressure.unwrap_or(1.0)).clamp(0.0, 1.0);
        if t.brush.pressure_size && pressure == 0.0 {
            return Ok(());
        }
        let radius =
            (t.brush.size * if t.brush.pressure_size { pressure } else { 1.0 } / 2.0).max(0.25);
        let alpha = t.brush.flow
            * if t.brush.pressure_opacity {
                pressure
            } else {
                1.0
            };
        let minx = (x - radius - 1.0).floor().max(0.0) as u32;
        let miny = (y - radius - 1.0).floor().max(0.0) as u32;
        let maxx = (x + radius + 1.0)
            .ceil()
            .clamp(0.0, f64::from(self.state.width)) as u32;
        let maxy = (y + radius + 1.0)
            .ceil()
            .clamp(0.0, f64::from(self.state.height)) as u32;
        let angle = t.brush.angle
            + if t.brush.tilt {
                match s.tilt_degrees {
                    [Some(x), Some(y)] => {
                        let x = f64::from(x).to_radians().tan();
                        let y = f64::from(y).to_radians().tan();
                        if x.abs() + y.abs() < 0.00001 {
                            0.0
                        } else {
                            y.atan2(x)
                        }
                    }
                    [Some(x), None] => f64::from(x).to_radians(),
                    [None, Some(y)] => f64::from(y).to_radians(),
                    [None, None] => 0.0,
                }
            } else {
                0.0
            };
        let (sin, cos) = angle.sin_cos();
        for yy in miny..maxy {
            for xx in minx..maxx {
                if self
                    .state
                    .selection
                    .as_ref()
                    .is_some_and(|s| !s.contains([f64::from(xx) + 0.5, f64::from(yy) + 0.5]))
                {
                    continue;
                }
                let dx = f64::from(xx) + 0.5 - x;
                let dy = f64::from(yy) + 0.5 - y;
                let shape = if t.brush.kind == BrushKind::Calligraphy {
                    0.28
                } else {
                    1.0
                };
                let d = ((dx * cos + dy * sin).powi(2) + ((-dx * sin + dy * cos) / shape).powi(2))
                    .sqrt()
                    / radius;
                if d >= 1.0 {
                    continue;
                }
                let mut weight = ((1.0 - d) * radius).clamp(0.0, 1.0)
                    * if d <= t.brush.hardness {
                        1.0
                    } else {
                        ((1.0 - d) / (1.0 - t.brush.hardness).max(0.001)).clamp(0.0, 1.0)
                    };
                if matches!(t.brush.kind, BrushKind::Pencil | BrushKind::Textured) {
                    let h = xx.wrapping_mul(73856093) ^ yy.wrapping_mul(19349663);
                    weight *= 0.2 + 0.8 * f64::from(h % 101) / 100.0;
                }
                if t.brush.kind == BrushKind::Wash {
                    weight *= 0.35 + 0.65 * d.powi(2);
                }
                let a = (alpha * weight * 255.0).round().clamp(0.0, 255.0) as u8;
                if a == 0 {
                    continue;
                }
                let key = (xx / TILE, yy / TILE);
                if !t.tiles.contains_key(&key)
                    && self.state.tile_count()
                        + self
                            .active
                            .values()
                            .map(|active| active.tiles.len() + active.mask_tiles.len())
                            .sum::<usize>()
                        + t.tiles.len()
                        + t.mask_tiles.len()
                        >= MAX_TILES
                {
                    return Err(PaintError::Limit);
                }
                let tile = t
                    .tiles
                    .entry(key)
                    .or_insert_with(|| Arc::new(vec![0; TILE_BYTES]));
                let tile = Arc::make_mut(tile);
                let i = ((yy % TILE * TILE + xx % TILE) * 4) as usize;
                let src = [
                    ((u16::from(t.brush.color[0]) * u16::from(a) + 127) / 255) as u8,
                    ((u16::from(t.brush.color[1]) * u16::from(a) + 127) / 255) as u8,
                    ((u16::from(t.brush.color[2]) * u16::from(a) + 127) / 255) as u8,
                    a,
                ];
                over(&mut tile[i..i + 4], &src, false, 255, Blend::Normal);
                self.damage.insert(key);
            }
        }
        Ok(())
    }

    fn mask_dab(&mut self, t: &mut Transaction, s: StrokeSample) -> Result<(), PaintError> {
        let [x, y] = s.position_document.0;
        if !x.is_finite() || !y.is_finite() {
            return Err(PaintError::Invalid);
        }
        let pressure = f64::from(s.pressure.unwrap_or(1.0)).clamp(0.0, 1.0);
        if t.brush.pressure_size && pressure == 0.0 {
            return Ok(());
        }
        let radius =
            (t.brush.size * if t.brush.pressure_size { pressure } else { 1.0 } / 2.0).max(0.25);
        let opacity = t.brush.opacity
            * t.brush.flow
            * if t.brush.pressure_opacity {
                pressure
            } else {
                1.0
            };
        let minx = (x - radius - 1.0).floor().max(0.0) as u32;
        let miny = (y - radius - 1.0).floor().max(0.0) as u32;
        let maxx = (x + radius + 1.0)
            .ceil()
            .clamp(0.0, f64::from(self.state.width)) as u32;
        let maxy = (y + radius + 1.0)
            .ceil()
            .clamp(0.0, f64::from(self.state.height)) as u32;
        let angle = t.brush.angle
            + if t.brush.tilt {
                match s.tilt_degrees {
                    [Some(x), Some(y)] => {
                        let x = f64::from(x).to_radians().tan();
                        let y = f64::from(y).to_radians().tan();
                        if x.abs() + y.abs() < 0.00001 {
                            0.0
                        } else {
                            y.atan2(x)
                        }
                    }
                    [Some(x), None] => f64::from(x).to_radians(),
                    [None, Some(y)] => f64::from(y).to_radians(),
                    [None, None] => 0.0,
                }
            } else {
                0.0
            };
        let (sin, cos) = angle.sin_cos();
        let target = if t.brush.kind == BrushKind::Eraser {
            255
        } else {
            let [red, green, blue] = t.brush.color;
            ((u32::from(red) * 54 + u32::from(green) * 183 + u32::from(blue) * 19 + 128) / 256)
                as u8
        };
        for yy in miny..maxy {
            for xx in minx..maxx {
                if self.state.selection.as_ref().is_some_and(|selection| {
                    !selection.contains([f64::from(xx) + 0.5, f64::from(yy) + 0.5])
                }) {
                    continue;
                }
                let dx = f64::from(xx) + 0.5 - x;
                let dy = f64::from(yy) + 0.5 - y;
                let shape = if t.brush.kind == BrushKind::Calligraphy {
                    0.28
                } else {
                    1.0
                };
                let distance = ((dx * cos + dy * sin).powi(2)
                    + ((-dx * sin + dy * cos) / shape).powi(2))
                .sqrt()
                    / radius;
                if distance >= 1.0 {
                    continue;
                }
                let mut weight = ((1.0 - distance) * radius).clamp(0.0, 1.0)
                    * if distance <= t.brush.hardness {
                        1.0
                    } else {
                        ((1.0 - distance) / (1.0 - t.brush.hardness).max(0.001)).clamp(0.0, 1.0)
                    };
                if matches!(t.brush.kind, BrushKind::Pencil | BrushKind::Textured) {
                    let hash = xx.wrapping_mul(73856093) ^ yy.wrapping_mul(19349663);
                    weight *= 0.2 + 0.8 * f64::from(hash % 101) / 100.0;
                }
                if t.brush.kind == BrushKind::Wash {
                    weight *= 0.35 + 0.65 * distance.powi(2);
                }
                let coverage = (opacity * weight * 255.0).round().clamp(0.0, 255.0) as u32;
                if coverage == 0 {
                    continue;
                }
                let key = (xx / TILE, yy / TILE);
                if !t.mask_tiles.contains_key(&key)
                    && self.state.tile_count()
                        + self
                            .active
                            .values()
                            .map(|active| active.tiles.len() + active.mask_tiles.len())
                            .sum::<usize>()
                        + t.tiles.len()
                        + t.mask_tiles.len()
                        >= MAX_TILES
                {
                    return Err(PaintError::Limit);
                }
                let layer = self
                    .state
                    .layers
                    .iter()
                    .find(|layer| layer.id == t.layer)
                    .ok_or(PaintError::Invalid)?;
                let mask = layer.mask.as_ref().ok_or(PaintError::Invalid)?;
                let tile = t.mask_tiles.entry(key).or_insert_with(|| {
                    mask.tiles
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| Arc::new(vec![255; MASK_TILE_BYTES]))
                });
                let tile = Arc::make_mut(tile);
                let offset = (yy % TILE * TILE + xx % TILE) as usize;
                let previous = u32::from(tile[offset]);
                let next = (previous * (255 - coverage) + u32::from(target) * coverage + 127) / 255;
                tile[offset] = next as u8;
                self.damage.insert(key);
            }
        }
        Ok(())
    }

    fn color_mix_dab(&mut self, t: &mut Transaction, s: StrokeSample) -> Result<(), PaintError> {
        let StrokeMode::ColorMix { strength } = t.mode else {
            return Err(PaintError::Invalid);
        };
        let [x, y] = s.position_document.0;
        if !x.is_finite() || !y.is_finite() {
            return Err(PaintError::Invalid);
        }
        let pressure = f64::from(s.pressure.unwrap_or(1.0)).clamp(0.0, 1.0);
        if t.brush.pressure_size && pressure == 0.0 {
            return Ok(());
        }
        let pressure_load = if t.brush.pressure_opacity {
            pressure
        } else {
            1.0
        };
        let radius =
            (t.brush.size * if t.brush.pressure_size { pressure } else { 1.0 } / 2.0).max(0.25);
        let load = (t.brush.flow * t.brush.opacity * pressure_load).clamp(0.0, 1.0);
        if load <= 0.0 {
            return Ok(());
        }
        let minx = (x - radius - 1.0).floor().max(0.0) as u32;
        let miny = (y - radius - 1.0).floor().max(0.0) as u32;
        let maxx = (x + radius + 1.0)
            .ceil()
            .clamp(0.0, f64::from(self.state.width)) as u32;
        let maxy = (y + radius + 1.0)
            .ceil()
            .clamp(0.0, f64::from(self.state.height)) as u32;
        let angle = t.brush.angle
            + if t.brush.tilt {
                match s.tilt_degrees {
                    [Some(x), Some(y)] => {
                        let x = f64::from(x).to_radians().tan();
                        let y = f64::from(y).to_radians().tan();
                        if x.abs() + y.abs() < 0.00001 {
                            0.0
                        } else {
                            y.atan2(x)
                        }
                    }
                    [Some(x), None] => f64::from(x).to_radians(),
                    [None, Some(y)] => f64::from(y).to_radians(),
                    [None, None] => 0.0,
                }
            } else {
                0.0
            };
        let (sin, cos) = angle.sin_cos();
        let layer = self
            .state
            .layers
            .iter()
            .find(|layer| layer.id == t.layer)
            .ok_or(PaintError::Invalid)?;

        for yy in miny..maxy {
            for xx in minx..maxx {
                if self.state.selection.as_ref().is_some_and(|selection| {
                    !selection.contains([f64::from(xx) + 0.5, f64::from(yy) + 0.5])
                }) {
                    continue;
                }
                let dx = f64::from(xx) + 0.5 - x;
                let dy = f64::from(yy) + 0.5 - y;
                let shape = if t.brush.kind == BrushKind::Calligraphy {
                    0.28
                } else {
                    1.0
                };
                let distance = ((dx * cos + dy * sin).powi(2)
                    + ((-dx * sin + dy * cos) / shape).powi(2))
                .sqrt()
                    / radius;
                let edge = smudge_edge_weight(distance, radius, t.brush.hardness);
                if edge <= 0.0 {
                    continue;
                }
                let mut coverage = edge;
                if matches!(t.brush.kind, BrushKind::Pencil | BrushKind::Textured) {
                    let hash = xx.wrapping_mul(73856093) ^ yy.wrapping_mul(19349663);
                    coverage *= 0.2 + 0.8 * f64::from(hash % 101) / 100.0;
                }
                if t.brush.kind == BrushKind::Wash {
                    coverage *= 0.35 + 0.65 * distance.powi(2);
                }
                let alpha = (load * coverage * 255.0).round().clamp(0.0, 255.0) as u8;
                if alpha == 0 {
                    continue;
                }
                let key = (xx / TILE, yy / TILE);
                let offset = ((yy % TILE * TILE + xx % TILE) * 4) as usize;
                let previous =
                    t.tiles
                        .get(&key)
                        .or_else(|| layer.tiles.get(&key))
                        .map_or([0; 4], |tile| {
                            tile[offset..offset + 4]
                                .try_into()
                                .expect("RGBA pixel has four bytes")
                        });
                let current = if previous[3] == 0 {
                    t.brush.color
                } else {
                    std::array::from_fn(|channel| {
                        ((u16::from(previous[channel]) * 255 + u16::from(previous[3]) / 2)
                            / u16::from(previous[3]))
                        .min(255) as u8
                    })
                };
                let pigment = [
                    mix_byte(current[0], t.brush.color[0], strength),
                    mix_byte(current[1], t.brush.color[1], strength),
                    mix_byte(current[2], t.brush.color[2], strength),
                ];
                let mut next = previous;
                let source = [
                    ((u16::from(pigment[0]) * u16::from(alpha) + 127) / 255) as u8,
                    ((u16::from(pigment[1]) * u16::from(alpha) + 127) / 255) as u8,
                    ((u16::from(pigment[2]) * u16::from(alpha) + 127) / 255) as u8,
                    alpha,
                ];
                over(&mut next, &source, false, 255, Blend::Normal);
                if next == previous {
                    continue;
                }
                if !t.tiles.contains_key(&key)
                    && self.state.tile_count()
                        + self
                            .active
                            .values()
                            .map(|active| active.tiles.len() + active.mask_tiles.len())
                            .sum::<usize>()
                        + t.tiles.len()
                        + t.mask_tiles.len()
                        >= MAX_TILES
                {
                    return Err(PaintError::Limit);
                }
                let tile = t.tiles.entry(key).or_insert_with(|| {
                    layer
                        .tiles
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| Arc::new(vec![0; TILE_BYTES]))
                });
                Arc::make_mut(tile)[offset..offset + 4].copy_from_slice(&next);
                self.damage.insert(key);
            }
        }
        Ok(())
    }

    fn smudge_dab(
        &mut self,
        t: &mut Transaction,
        s: StrokeSample,
        reset_carry: bool,
        path_index: usize,
        write_pixels: bool,
    ) -> Result<(), PaintError> {
        let [x, y] = s.position_document.0;
        if !x.is_finite() || !y.is_finite() {
            return Err(PaintError::Invalid);
        }
        let pressure = f64::from(s.pressure.unwrap_or(1.0)).clamp(0.0, 1.0);
        if t.brush.pressure_size && pressure == 0.0 {
            return Ok(());
        }
        let pressure_strength = if t.brush.pressure_opacity {
            pressure
        } else {
            1.0
        };
        let radius =
            (t.brush.size * if t.brush.pressure_size { pressure } else { 1.0 } / 2.0).max(0.25);
        let minx = (x - radius - 1.0).floor().max(0.0) as u32;
        let miny = (y - radius - 1.0).floor().max(0.0) as u32;
        let maxx = (x + radius + 1.0)
            .ceil()
            .clamp(0.0, f64::from(self.state.width)) as u32;
        let maxy = (y + radius + 1.0)
            .ceil()
            .clamp(0.0, f64::from(self.state.height)) as u32;
        let layer = self
            .state
            .layers
            .iter()
            .find(|layer| layer.id == t.layer)
            .ok_or(PaintError::Invalid)?;

        // Sample premultiplied pixels from this transaction's working copy,
        // falling back to the selected layer. Averaging premultiplied RGBA
        // keeps alpha edges valid while pigment is carried across transparency.
        let mut sum = [0.0; 4];
        let mut total_weight = 0.0;
        for yy in miny..maxy {
            for xx in minx..maxx {
                if self.state.selection.as_ref().is_some_and(|selection| {
                    !selection.contains([f64::from(xx) + 0.5, f64::from(yy) + 0.5])
                }) {
                    continue;
                }
                let dx = f64::from(xx) + 0.5 - x;
                let dy = f64::from(yy) + 0.5 - y;
                let distance = dx.hypot(dy) / radius;
                let weight = smudge_edge_weight(distance, radius, t.brush.hardness);
                if weight <= 0.0 {
                    continue;
                }
                let key = (xx / TILE, yy / TILE);
                let offset = ((yy % TILE * TILE + xx % TILE) * 4) as usize;
                let pixel =
                    t.tiles
                        .get(&key)
                        .or_else(|| layer.tiles.get(&key))
                        .map_or([0; 4], |tile| {
                            tile[offset..offset + 4]
                                .try_into()
                                .expect("RGBA pixel has four bytes")
                        });
                for channel in 0..4 {
                    sum[channel] += f64::from(pixel[channel]) * weight;
                }
                total_weight += weight;
            }
        }
        let sampled = if total_weight > 0.0 {
            sum.map(|channel| (channel / total_weight).round().clamp(0.0, 255.0) as u8)
        } else {
            [0; 4]
        };
        let Some(carry_slot) = t.smudge_carry.get_mut(path_index) else {
            return Err(PaintError::Invalid);
        };
        if reset_carry || carry_slot.is_none() {
            *carry_slot = Some(sampled);
            return Ok(());
        }
        if total_weight == 0.0 {
            return Ok(());
        }

        let mut carry = carry_slot.unwrap_or(sampled);
        let pickup = (t.brush.flow * pressure_strength).clamp(0.0, 1.0);
        for channel in 0..4 {
            carry[channel] = mix_byte(carry[channel], sampled[channel], pickup);
        }
        *carry_slot = Some(carry);
        let strength = (t.brush.opacity * pressure_strength).clamp(0.0, 1.0);
        if strength <= 0.0 {
            return Ok(());
        }
        if !write_pixels {
            return Ok(());
        }

        for yy in miny..maxy {
            for xx in minx..maxx {
                if self.state.selection.as_ref().is_some_and(|selection| {
                    !selection.contains([f64::from(xx) + 0.5, f64::from(yy) + 0.5])
                }) {
                    continue;
                }
                let distance = (f64::from(xx) + 0.5 - x).hypot(f64::from(yy) + 0.5 - y) / radius;
                let amount = strength * smudge_edge_weight(distance, radius, t.brush.hardness);
                if amount <= 0.0 {
                    continue;
                }
                let key = (xx / TILE, yy / TILE);
                let offset = ((yy % TILE * TILE + xx % TILE) * 4) as usize;
                let previous =
                    t.tiles
                        .get(&key)
                        .or_else(|| layer.tiles.get(&key))
                        .map_or([0; 4], |tile| {
                            tile[offset..offset + 4]
                                .try_into()
                                .expect("RGBA pixel has four bytes")
                        });
                let next = std::array::from_fn(|channel| {
                    mix_byte(previous[channel], carry[channel], amount)
                });
                if next == previous {
                    continue;
                }
                if !t.tiles.contains_key(&key)
                    && self.state.tile_count()
                        + self
                            .active
                            .values()
                            .map(|transaction| {
                                transaction.tiles.len() + transaction.mask_tiles.len()
                            })
                            .sum::<usize>()
                        + t.tiles.len()
                        + t.mask_tiles.len()
                        >= MAX_TILES
                {
                    return Err(PaintError::Limit);
                }
                let tile = t.tiles.entry(key).or_insert_with(|| {
                    layer
                        .tiles
                        .get(&key)
                        .cloned()
                        .unwrap_or_else(|| Arc::new(vec![0; TILE_BYTES]))
                });
                Arc::make_mut(tile)[offset..offset + 4].copy_from_slice(&next);
                self.damage.insert(key);
            }
        }
        Ok(())
    }
    pub fn composite_tile(&self, key: TileKey) -> Vec<u8> {
        let mut out = vec![0; TILE_BYTES];
        if let Some(bg) = self.state.background {
            for p in out.chunks_exact_mut(4) {
                p.copy_from_slice(&[bg[0], bg[1], bg[2], 255]);
            }
        }
        for l in self.state.layers.iter().filter(|l| l.visible) {
            let mut tile = l
                .tiles
                .get(&key)
                .map(|v| v.as_ref().clone())
                .unwrap_or_else(|| vec![0; TILE_BYTES]);
            for t in self.active.values().filter(|t| t.layer == l.id) {
                if let Some(src) = t.tiles.get(&key) {
                    if stroke_mode_needs_exclusive_layer(t.mode) {
                        tile.copy_from_slice(src);
                    } else {
                        for (d, s) in tile.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                            over(
                                d,
                                s,
                                t.brush.kind == BrushKind::Eraser,
                                (t.brush.opacity * 255.0).round() as u8,
                                Blend::Normal,
                            );
                        }
                    }
                }
            }
            vector::rasterize_layer_tile(l, key, [self.state.width, self.state.height], &mut tile);
            if let Some(mask) = l.mask.as_ref().filter(|mask| mask.enabled) {
                let provisional = self.active.values().find_map(|transaction| {
                    (transaction.layer == l.id && transaction.mode == StrokeMode::MaskPaint)
                        .then(|| transaction.mask_tiles.get(&key))
                        .flatten()
                });
                let stored = provisional
                    .or_else(|| mask.tiles.get(&key))
                    .map(|values| values.as_slice());
                if let Some(values) = stored {
                    for (index, pixel) in tile.chunks_exact_mut(4).enumerate() {
                        let alpha = if mask.inverted {
                            255 - values[index]
                        } else {
                            values[index]
                        };
                        for channel in pixel {
                            *channel = ((u16::from(*channel) * u16::from(alpha) + 127) / 255) as u8;
                        }
                    }
                } else if mask.inverted {
                    tile.fill(0);
                }
            }
            for (d, s) in out.chunks_exact_mut(4).zip(tile.chunks_exact(4)) {
                over(d, s, false, l.opacity, l.blend);
            }
        }
        out
    }
    pub fn rgba(&self) -> Vec<u8> {
        let mut out = vec![0; (self.state.width * self.state.height * 4) as usize];
        for ty in 0..self.state.height.div_ceil(TILE) {
            for tx in 0..self.state.width.div_ceil(TILE) {
                let tile = self.composite_tile((tx, ty));
                for y in 0..TILE.min(self.state.height - ty * TILE) {
                    for x in 0..TILE.min(self.state.width - tx * TILE) {
                        let src = ((y * TILE + x) * 4) as usize;
                        let dst =
                            (((ty * TILE + y) * self.state.width + tx * TILE + x) * 4) as usize;
                        let a = u32::from(tile[src + 3]);
                        for c in 0..3 {
                            out[dst + c] = if a == 0 {
                                0
                            } else {
                                ((u32::from(tile[src + c]) * 255 + a / 2) / a).min(255) as u8
                            };
                        }
                        out[dst + 3] = a as u8;
                    }
                }
            }
        }
        out
    }

    /// Produce a small, deterministic Home preview without allocating a full-size
    /// flattened canvas. At most 512 × 512 output pixels are allocated.
    pub fn thumbnail_rgba(
        &self,
        max_width: u32,
        max_height: u32,
    ) -> Result<(u32, u32, Vec<u8>), PaintError> {
        if max_width == 0 || max_height == 0 || max_width > 512 || max_height > 512 {
            return Err(PaintError::Limit);
        }
        if self.active_count() > 0 {
            return Err(PaintError::Busy);
        }
        self.state.validate()?;
        let (width, height) = if self.state.width <= max_width && self.state.height <= max_height {
            (self.state.width, self.state.height)
        } else if u64::from(max_width) * u64::from(self.state.height)
            <= u64::from(max_height) * u64::from(self.state.width)
        {
            (
                max_width,
                ((u64::from(self.state.height) * u64::from(max_width))
                    / u64::from(self.state.width))
                .max(1) as u32,
            )
        } else {
            (
                ((u64::from(self.state.width) * u64::from(max_height))
                    / u64::from(self.state.height))
                .max(1) as u32,
                max_height,
            )
        };
        let mut pixels = vec![0; width as usize * height as usize * 4];
        // Downsample by source tile row. Compositing each source pixel directly
        // would visit every editable path and segment once per preview sample;
        // a bounded row cache instead rasterizes each 64×64 tile once and keeps
        // at most one document tile row (1 MiB at the maximum width).
        let tile_columns = self.state.width.div_ceil(TILE) as usize;
        let mut composite_tile_row = Vec::with_capacity(tile_columns);
        let mut cached_tile_y = None;
        for y in 0..height {
            let source_y = (((u64::from(y) * 2 + 1) * u64::from(self.state.height))
                / (u64::from(height) * 2))
                .min(u64::from(self.state.height - 1)) as u32;
            let tile_y = source_y / TILE;
            if cached_tile_y != Some(tile_y) {
                composite_tile_row.clear();
                composite_tile_row.extend(
                    (0..tile_columns).map(|tile_x| self.composite_tile((tile_x as u32, tile_y))),
                );
                cached_tile_y = Some(tile_y);
            }
            for x in 0..width {
                let source_x = (((u64::from(x) * 2 + 1) * u64::from(self.state.width))
                    / (u64::from(width) * 2))
                    .min(u64::from(self.state.width - 1)) as u32;
                let source = (((source_y % TILE) * TILE + source_x % TILE) * 4) as usize;
                let tile = &composite_tile_row[(source_x / TILE) as usize];
                let composite = &tile[source..source + 4];
                let dest = ((y * width + x) * 4) as usize;
                let alpha = u32::from(composite[3]);
                for channel in 0..3 {
                    pixels[dest + channel] = if alpha == 0 {
                        0
                    } else {
                        ((u32::from(composite[channel]) * 255 + alpha / 2) / alpha) as u8
                    };
                }
                pixels[dest + 3] = alpha as u8;
            }
        }
        Ok((width, height, pixels))
    }
}
fn stroke_mode_needs_exclusive_layer(mode: StrokeMode) -> bool {
    matches!(
        mode,
        StrokeMode::Smudge | StrokeMode::ColorMix { .. } | StrokeMode::MaskPaint
    )
}
fn mix_byte(from: u8, to: u8, amount: f64) -> u8 {
    (f64::from(from) + (f64::from(to) - f64::from(from)) * amount.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8
}
fn smudge_edge_weight(distance: f64, radius: f64, hardness: f64) -> f64 {
    if distance >= 1.0 {
        return 0.0;
    }
    let coverage = ((1.0 - distance) * radius).clamp(0.0, 1.0);
    coverage
        * if distance <= hardness {
            1.0
        } else {
            ((1.0 - distance) / (1.0 - hardness).max(0.001)).clamp(0.0, 1.0)
        }
}
fn over(dst: &mut [u8], src: &[u8], erase: bool, opacity: u8, mode: Blend) {
    let sa = u32::from(src[3]) * u32::from(opacity) / 255;
    let da = u32::from(dst[3]);
    if erase {
        for d in dst {
            *d = (u32::from(*d) * (255 - sa) / 255) as u8;
        }
        return;
    }
    for c in 0..3 {
        let sp = u32::from(src[c]) * u32::from(opacity) / 255;
        let dp = u32::from(dst[c]);
        let cs = if sa == 0 { 0 } else { sp * 255 / sa };
        let cd = if da == 0 { 0 } else { dp * 255 / da };
        let blend = match mode {
            Blend::Normal => cs,
            Blend::Multiply => cs * cd / 255,
            Blend::Screen => 255 - (255 - cs) * (255 - cd) / 255,
            Blend::Overlay => {
                if cd < 128 {
                    2 * cs * cd / 255
                } else {
                    255 - 2 * (255 - cs) * (255 - cd) / 255
                }
            }
        };
        dst[c] =
            ((sp * (255 - da) + dp * (255 - sa)) / 255 + blend * sa * da / 65025).min(255) as u8;
    }
    dst[3] = (sa + da * (255 - sa) / 255).min(255) as u8;
}
pub mod recovery;
