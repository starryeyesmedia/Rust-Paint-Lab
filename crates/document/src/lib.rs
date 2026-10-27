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
use brush::{Brush, BrushKind};
use paint_stroke::{StrokeCommand, StrokeId, StrokeSample};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::Arc,
};
pub const TILE: u32 = 64;
pub const TILE_BYTES: usize = (TILE * TILE * 4) as usize;
pub const MAX_TILES: usize = 8192; // 128 MiB authoritative + provisional tile budget.
pub const HISTORY_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_LAYERS: usize = 32;
pub type TileKey = (u32, u32);
pub type TileMap = BTreeMap<TileKey, Arc<Vec<u8>>>;
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Layer {
    pub id: u64,
    pub name: String,
    pub visible: bool,
    pub locked: bool,
    pub opacity: u8,
    pub blend: Blend,
    #[serde(skip)]
    pub tiles: TileMap,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
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
                tiles: BTreeMap::new(),
            }],
            selected: 1,
            background: Some([255; 3]),
            brush: Brush::default(),
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
        let mut total = 0;
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
        }
        if !ids.contains(&self.selected) || total > MAX_TILES {
            return Err(PaintError::Limit);
        }
        Ok(())
    }
    pub fn tile_count(&self) -> usize {
        self.layers.iter().map(|l| l.tiles.len()).sum()
    }
}
struct Transaction {
    layer: u64,
    brush: Brush,
    last: StrokeSample,
    remaining: f64,
    stabilizer: stabilization::StrokeStabilizer,
    tiles: TileMap,
}
pub struct Engine {
    state: State,
    pub brush: Brush,
    active: BTreeMap<StrokeId, Transaction>,
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
        let mut allocations = std::collections::HashSet::new();
        for s in queue {
            for tile in s.layers.iter().flat_map(|l| l.tiles.values()) {
                allocations.insert(Arc::as_ptr(tile));
            }
        }
        allocations.len() * TILE_BYTES
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
            }
            return Err(PaintError::Invalid);
        }
        match *c {
            StrokeCommand::BeginStroke { stroke_id, sample } => {
                if self.active.contains_key(&stroke_id) {
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
                let mut brush = self.brush.clone();
                if sample.buttons.eraser || sample.buttons.inverted {
                    brush.kind = BrushKind::Eraser;
                }
                let mut t = Transaction {
                    layer: layer.id,
                    brush,
                    last: sample,
                    remaining: 0.0,
                    stabilizer: stabilization::StrokeStabilizer::new(
                        sample.position_document.0,
                        sample.time_ms,
                    ),
                    tiles: BTreeMap::new(),
                };
                self.dab(&mut t, sample)?;
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
                if let Err(error) = self.dab(&mut t, sample) {
                    self.damage.extend(t.tiles.keys());
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
                        return Err(error);
                    }
                }
                if commit {
                    // Final dab is explicit even when sample spacing has a remainder.
                    if let Err(e) = self.dab(&mut t, sample) {
                        self.damage.extend(t.tiles.keys());
                        return Err(e);
                    }
                    let before = self.state.clone();
                    let l = self
                        .state
                        .layers
                        .iter_mut()
                        .find(|l| l.id == t.layer)
                        .ok_or(PaintError::Invalid)?;
                    for (key, src) in t.tiles {
                        let dst = l
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
                    self.changed(before);
                } else {
                    self.active.insert(stroke_id, t);
                }
            }
            StrokeCommand::CancelStroke { stroke_id, .. } => {
                if let Some(t) = self.active.remove(&stroke_id) {
                    self.damage.extend(t.tiles.keys());
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
    fn dab(&mut self, t: &mut Transaction, s: StrokeSample) -> Result<(), PaintError> {
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
                        + self.active.values().map(|t| t.tiles.len()).sum::<usize>()
                        + t.tiles.len()
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
        for y in 0..height {
            let source_y = (((u64::from(y) * 2 + 1) * u64::from(self.state.height))
                / (u64::from(height) * 2))
                .min(u64::from(self.state.height - 1)) as u32;
            for x in 0..width {
                let source_x = (((u64::from(x) * 2 + 1) * u64::from(self.state.width))
                    / (u64::from(width) * 2))
                    .min(u64::from(self.state.width - 1)) as u32;
                let key = (source_x / TILE, source_y / TILE);
                let source = (((source_y % TILE) * TILE + source_x % TILE) * 4) as usize;
                let mut composite = [0u8; 4];
                if let Some(background) = self.state.background {
                    composite.copy_from_slice(&[background[0], background[1], background[2], 255]);
                }
                for layer in self.state.layers.iter().filter(|layer| layer.visible) {
                    if let Some(tile) = layer.tiles.get(&key) {
                        over(
                            &mut composite,
                            &tile[source..source + 4],
                            false,
                            layer.opacity,
                            layer.blend,
                        );
                    }
                }
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
