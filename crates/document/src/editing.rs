//! Pixel-center selections and bounded nearest-neighbor raster edits.
use crate::{Blend, Engine, PaintError, State, TILE, TILE_BYTES, TileMap, over};
use std::sync::Arc;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SelectionKind {
    Rectangle,
    Ellipse,
    Lasso,
}

/// Persistent raster shape primitives. This is distinct from a selection: the
/// engine rasterizes each completed primitive into the selected layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeKind {
    Line,
    Rectangle,
    Ellipse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GradientKind {
    Linear,
    Radial,
}

/// Small lookup table for deterministic linear-light sRGB interpolation.
/// Reusing the ramp keeps large 4096px document operations practical.
#[derive(Debug, Clone)]
pub struct GradientRamp {
    colors: Vec<[u8; 3]>,
}

impl GradientRamp {
    pub fn new(start: [u8; 3], end: [u8; 3]) -> Self {
        let colors = (0..=1024)
            .map(|step| interpolate_srgb_linear(start, end, step as f64 / 1024.0))
            .collect();
        Self { colors }
    }

    pub fn sample(&self, amount: f64) -> [u8; 3] {
        if !amount.is_finite() {
            return self.colors[0];
        }
        let index = (amount.clamp(0.0, 1.0) * (self.colors.len() - 1) as f64).round() as usize;
        self.colors[index]
    }
}

/// Interpolate sRGB stops in linear light, then encode the result as sRGB
/// bytes. Alpha/brush opacity is applied separately by the raster compositor.
pub fn interpolate_srgb_linear(start: [u8; 3], end: [u8; 3], amount: f64) -> [u8; 3] {
    if !amount.is_finite() {
        return start;
    }
    let amount = amount.clamp(0.0, 1.0);
    let decode = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    let encode = |value: f64| {
        let value = value.clamp(0.0, 1.0);
        let srgb = if value <= 0.0031308 {
            value * 12.92
        } else {
            1.055 * value.powf(1.0 / 2.4) - 0.055
        };
        (srgb * 255.0).round().clamp(0.0, 255.0) as u8
    };
    std::array::from_fn(|channel| {
        let a = decode(start[channel]);
        let b = decode(end[channel]);
        encode(a + (b - a) * amount)
    })
}

pub fn selection_row_spans(selection: Option<&Selection>, y: f64, width: u32) -> Vec<(u32, u32)> {
    let Some(selection) = selection else {
        return vec![(0, width)];
    };
    let [left, top, right, bottom] = selection.bounds();
    if y < top || y >= bottom {
        return Vec::new();
    }
    let span = |left: f64, right: f64| {
        let start = (left - 0.5).ceil().clamp(0.0, f64::from(width)) as u32;
        let end = (right - 0.5).ceil().clamp(0.0, f64::from(width)) as u32;
        (start < end).then_some((start, end))
    };
    match selection.kind {
        SelectionKind::Rectangle => span(left, right).into_iter().collect(),
        SelectionKind::Ellipse => {
            let [cx, cy] = [(left + right) / 2.0, (top + bottom) / 2.0];
            let [rx, ry] = [(right - left) / 2.0, (bottom - top) / 2.0];
            let vertical = (y - cy) / ry;
            if vertical.abs() > 1.0 {
                return Vec::new();
            }
            let half_width = rx * (1.0 - vertical * vertical).max(0.0).sqrt();
            span(cx - half_width, cx + half_width).into_iter().collect()
        }
        SelectionKind::Lasso => {
            let mut intersections = Vec::with_capacity(selection.points.len());
            let mut a = selection.points[selection.points.len() - 1];
            for &b in &selection.points {
                if (a[1] > y) != (b[1] > y) {
                    intersections.push((b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0]);
                }
                a = b;
            }
            intersections.sort_by(f64::total_cmp);
            intersections
                .chunks_exact(2)
                .filter_map(|pair| span(pair[0], pair[1]))
                .collect()
        }
    }
}
pub const MAX_SELECTION_POINTS: usize = 1024;
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    kind: SelectionKind,
    points: Vec<[f64; 2]>,
    bounds: [f64; 4],
}
impl Selection {
    pub fn new(kind: SelectionKind, points: Vec<[f64; 2]>) -> Result<Self, PaintError> {
        let bounds = points
            .iter()
            .fold([4096.0_f64, 4096.0_f64, 0.0_f64, 0.0_f64], |a, p| {
                [
                    a[0].min(p[0]),
                    a[1].min(p[1]),
                    a[2].max(p[0]),
                    a[3].max(p[1]),
                ]
            });
        let s = Self {
            kind,
            points,
            bounds,
        };
        s.validate()?;
        Ok(s)
    }
    pub fn validate(&self) -> Result<(), PaintError> {
        if self.points.len() > MAX_SELECTION_POINTS
            || self.points.len()
                < if self.kind == SelectionKind::Lasso {
                    3
                } else {
                    2
                }
            || (self.kind != SelectionKind::Lasso && self.points.len() != 2)
            || !self
                .points
                .iter()
                .flatten()
                .all(|n| n.is_finite() && (0.0..=4096.0).contains(n))
        {
            return Err(PaintError::Invalid);
        }
        let [x, y, r, b] = self.bounds();
        if r <= x || b <= y {
            return Err(PaintError::Invalid);
        }
        Ok(())
    }
    pub fn kind(&self) -> SelectionKind {
        self.kind
    }
    pub fn points(&self) -> &[[f64; 2]] {
        &self.points
    }
    pub fn bounds(&self) -> [f64; 4] {
        self.bounds
    }
    pub fn contains(&self, p: [f64; 2]) -> bool {
        let [x, y, r, b] = self.bounds();
        if p[0] < x || p[1] < y || p[0] >= r || p[1] >= b {
            return false;
        }
        match self.kind {
            SelectionKind::Rectangle => true,
            SelectionKind::Ellipse => {
                ((p[0] - (x + r) / 2.0) / ((r - x) / 2.0)).powi(2)
                    + ((p[1] - (y + b) / 2.0) / ((b - y) / 2.0)).powi(2)
                    <= 1.0
            }
            SelectionKind::Lasso => {
                let mut inside = false;
                let mut a = self.points[self.points.len() - 1];
                for &b in &self.points {
                    if (a[1] > p[1]) != (b[1] > p[1])
                        && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
                    {
                        inside = !inside;
                    }
                    a = b;
                }
                inside
            }
        }
    }
}
fn read(tiles: &TileMap, x: u32, y: u32) -> [u8; 4] {
    let Some(t) = tiles.get(&(x / TILE, y / TILE)) else {
        return [0; 4];
    };
    let i = ((y % TILE * TILE + x % TILE) * 4) as usize;
    t[i..i + 4].try_into().unwrap()
}
fn write(
    tiles: &mut TileMap,
    x: u32,
    y: u32,
    p: [u8; 4],
    count: &mut usize,
) -> Result<(), PaintError> {
    let key = (x / TILE, y / TILE);
    if let std::collections::btree_map::Entry::Vacant(entry) = tiles.entry(key) {
        if p == [0; 4] {
            return Ok(());
        }
        if *count >= crate::MAX_TILES {
            return Err(PaintError::Limit);
        }
        *count += 1;
        entry.insert(Arc::new(vec![0; TILE_BYTES]));
    }
    let t = Arc::make_mut(tiles.get_mut(&key).unwrap());
    let i = ((y % TILE * TILE + x % TILE) * 4) as usize;
    t[i..i + 4].copy_from_slice(&p);
    Ok(())
}
fn mask(s: &State) -> Vec<bool> {
    let mut mask = vec![s.selection.is_none(); (s.width * s.height) as usize];
    let Some(selection) = &s.selection else {
        return mask;
    };
    let [left, top, right, bottom] = selection.bounds();
    for y in (top.floor() as u32)..(bottom.ceil() as u32).min(s.height) {
        if selection.kind == SelectionKind::Lasso {
            let py = f64::from(y) + 0.5;
            let mut intersections = Vec::with_capacity(selection.points.len());
            let mut a = selection.points[selection.points.len() - 1];
            for &b in &selection.points {
                if (a[1] > py) != (b[1] > py) {
                    intersections.push((b[0] - a[0]) * (py - a[1]) / (b[1] - a[1]) + a[0]);
                }
                a = b;
            }
            intersections.sort_by(f64::total_cmp);
            for pair in intersections.chunks_exact(2) {
                let start = (pair[0] - 0.5).ceil().clamp(0.0, f64::from(s.width)) as u32;
                let end = (pair[1] - 0.5).ceil().clamp(0.0, f64::from(s.width)) as u32;
                for x in start..end {
                    mask[(y * s.width + x) as usize] = true;
                }
            }
        } else {
            for x in (left.floor() as u32)..(right.ceil() as u32).min(s.width) {
                mask[(y * s.width + x) as usize] =
                    selection.contains([f64::from(x) + 0.5, f64::from(y) + 0.5]);
            }
        }
    }
    mask
}
impl Engine {
    /// Copy the selected pixels from the active, visible, unlocked layer into a
    /// new normal layer immediately above it. The source and selection remain
    /// unchanged; the new layer becomes active. This is one bounded document
    /// transaction and never changes the native format.
    pub fn copy_selection_to_new_layer(&mut self) -> Result<u64, PaintError> {
        let mut created_id = None;
        self.edit(|state| {
            if state.layers.len() >= crate::MAX_LAYERS {
                return Err(PaintError::Limit);
            }
            let selection = state.selection.clone().ok_or(PaintError::Invalid)?;
            let source_index = state
                .layers
                .iter()
                .position(|layer| layer.id == state.selected)
                .ok_or(PaintError::Invalid)?;
            let source = &state.layers[source_index];
            if source.locked || !source.visible {
                return Err(PaintError::Locked);
            }
            let id = state
                .layers
                .iter()
                .map(|layer| layer.id)
                .max()
                .unwrap_or(0)
                .checked_add(1)
                .filter(|id| *id <= u64::from(u32::MAX))
                .ok_or(PaintError::Limit)?;
            let mut name = format!("{} copy", source.name);
            while name.len() > 256 {
                name.pop();
            }

            let mut tiles = TileMap::new();
            let mut total_tiles = state.tile_count();
            let mut copied_pixels = 0usize;
            let [_, top, _, bottom] = selection.bounds();
            let first_row = (top - 0.5).ceil().clamp(0.0, f64::from(state.height)) as u32;
            let end_row = (bottom - 0.5).ceil().clamp(0.0, f64::from(state.height)) as u32;
            for y in first_row..end_row {
                for (left, right) in
                    selection_row_spans(Some(&selection), f64::from(y) + 0.5, state.width)
                {
                    for x in left..right {
                        let pixel = read(&source.tiles, x, y);
                        if pixel[3] == 0 {
                            continue;
                        }
                        write(&mut tiles, x, y, pixel, &mut total_tiles)?;
                        copied_pixels += 1;
                    }
                }
            }
            if copied_pixels == 0 {
                return Err(PaintError::Invalid);
            }
            let copied_layer = crate::Layer {
                id,
                name,
                visible: true,
                locked: false,
                opacity: 255,
                blend: Blend::Normal,
                tiles,
            };
            state.layers.insert(source_index + 1, copied_layer);
            state.selected = id;
            created_id = Some(id);
            Ok(())
        })?;
        created_id.ok_or(PaintError::Invalid)
    }

    /// Apply one raster gradient to the active layer as a single undoable edit.
    /// The first stop is the active brush color, the second stop is supplied by
    /// the workspace, and brush opacity composites the gradient over existing
    /// layer pixels. Selection clipping follows pixel-center geometry.
    pub fn draw_gradient(
        &mut self,
        kind: GradientKind,
        start: [f64; 2],
        end: [f64; 2],
        end_color: [u8; 3],
    ) -> Result<(), PaintError> {
        let brush = self.brush.clone();
        if !brush.validate()
            || !start
                .iter()
                .chain(end.iter())
                .all(|value| value.is_finite())
            || start == end
        {
            return Err(PaintError::Invalid);
        }
        let alpha = (brush.opacity * 255.0).round().clamp(0.0, 255.0) as u8;
        if alpha == 0 {
            return Ok(());
        }
        let start_color = brush.color;
        let ramp = GradientRamp::new(start_color, end_color);
        self.cancel_all();
        let before = self.state.clone();
        let mut next = before.clone();
        let mut painted = false;
        {
            let state = &mut next;
            let width = state.width;
            let height = state.height;
            let doc_width = f64::from(width);
            let doc_height = f64::from(height);
            if start[0] < 0.0
                || start[0] > doc_width
                || start[1] < 0.0
                || start[1] > doc_height
                || end[0] < 0.0
                || end[0] > doc_width
                || end[1] < 0.0
                || end[1] > doc_height
            {
                return Err(PaintError::Invalid);
            }
            let selected_id = state.selected;
            let mut tile_count = state.tile_count();
            let selection = state.selection.clone();
            let (top, bottom) = selection.as_ref().map_or((0.0, doc_height), |selection| {
                let [_, top, _, bottom] = selection.bounds();
                (top, bottom)
            });
            let first_y = (top - 0.5).ceil().clamp(0.0, doc_height) as u32;
            let end_y = (bottom - 0.5).ceil().clamp(0.0, doc_height) as u32;
            let layer = state
                .layers
                .iter_mut()
                .find(|layer| layer.id == selected_id)
                .ok_or(PaintError::Invalid)?;
            if layer.locked || !layer.visible {
                return Err(PaintError::Locked);
            }

            for y in first_y..end_y {
                let py = f64::from(y) + 0.5;
                for (left, right) in selection_row_spans(selection.as_ref(), py, width) {
                    for x in left..right {
                        let point = [f64::from(x) + 0.5, py];
                        let amount = gradient_parameter(kind, start, end, point);
                        let color = ramp.sample(amount);
                        let source = [
                            ((u16::from(color[0]) * u16::from(alpha) + 127) / 255) as u8,
                            ((u16::from(color[1]) * u16::from(alpha) + 127) / 255) as u8,
                            ((u16::from(color[2]) * u16::from(alpha) + 127) / 255) as u8,
                            alpha,
                        ];
                        let previous = read(&layer.tiles, x, y);
                        let mut result = previous;
                        over(&mut result, &source, false, 255, Blend::Normal);
                        if result != previous {
                            write(&mut layer.tiles, x, y, result, &mut tile_count)?;
                            painted = true;
                        }
                    }
                }
            }
        }
        next.validate()?;
        if !painted {
            return Ok(());
        }
        self.state = next;
        self.changed(before);
        self.damage_all();
        Ok(())
    }

    /// Rasterize one completed shape as a single undoable document edit. Shape
    /// previews belong to the workspace and never enter document pixels before
    /// this method is called.
    pub fn draw_shape(
        &mut self,
        kind: ShapeKind,
        start: [f64; 2],
        end: [f64; 2],
        filled: bool,
    ) -> Result<(), PaintError> {
        let brush = self.brush.clone();
        if !brush.validate()
            || !start
                .iter()
                .chain(end.iter())
                .all(|value| value.is_finite())
        {
            return Err(PaintError::Invalid);
        }
        self.edit(move |state| {
            let width = f64::from(state.width);
            let height = f64::from(state.height);
            if start[0] < 0.0
                || start[0] > width
                || start[1] < 0.0
                || start[1] > height
                || end[0] < 0.0
                || end[0] > width
                || end[1] < 0.0
                || end[1] > height
            {
                return Err(PaintError::Invalid);
            }
            if kind != ShapeKind::Line && (start[0] == end[0] || start[1] == end[1]) {
                return Err(PaintError::Invalid);
            }
            if kind == ShapeKind::Line && start == end {
                return Err(PaintError::Invalid);
            }
            let selected = state.selected;
            let all_tiles = state.tile_count();
            let selection = state.selection.clone();
            let layer = state
                .layers
                .iter_mut()
                .find(|layer| layer.id == selected)
                .ok_or(PaintError::Invalid)?;
            if layer.locked || !layer.visible {
                return Err(PaintError::Locked);
            }

            let half = brush.size / 2.0;
            let margin = if kind == ShapeKind::Line || !filled {
                half + 1.0
            } else {
                1.0
            };
            let bounds = [
                start[0].min(end[0]),
                start[1].min(end[1]),
                start[0].max(end[0]),
                start[1].max(end[1]),
            ];
            let left = (bounds[0] - margin).floor().max(0.0) as u32;
            let top = (bounds[1] - margin).floor().max(0.0) as u32;
            let right = (bounds[2] + margin).ceil().min(width) as u32;
            let bottom = (bounds[3] + margin).ceil().min(height) as u32;
            let mut updates = TileMap::new();
            let mut tile_count = all_tiles;
            let mut painted = false;
            let base_alpha = brush.opacity * 255.0;
            for y in top..bottom {
                for x in left..right {
                    let point = [f64::from(x) + 0.5, f64::from(y) + 0.5];
                    if selection
                        .as_ref()
                        .is_some_and(|selection| !selection.contains(point))
                    {
                        continue;
                    }
                    let coverage = shape_coverage(kind, start, end, point, half, filled);
                    let alpha = (base_alpha * coverage).round().clamp(0.0, 255.0) as u8;
                    if alpha == 0 {
                        continue;
                    }
                    let key = (x / TILE, y / TILE);
                    if let std::collections::btree_map::Entry::Vacant(entry) = updates.entry(key) {
                        if !layer.tiles.contains_key(&key) {
                            if tile_count >= crate::MAX_TILES {
                                return Err(PaintError::Limit);
                            }
                            tile_count += 1;
                        }
                        let tile = layer
                            .tiles
                            .get(&key)
                            .map(|tile| tile.as_ref().clone())
                            .unwrap_or_else(|| vec![0; TILE_BYTES]);
                        entry.insert(Arc::new(tile));
                    }
                    let tile = Arc::make_mut(updates.get_mut(&key).expect("inserted tile"));
                    let offset = ((y % TILE * TILE + x % TILE) * 4) as usize;
                    let source = [
                        ((u16::from(brush.color[0]) * u16::from(alpha) + 127) / 255) as u8,
                        ((u16::from(brush.color[1]) * u16::from(alpha) + 127) / 255) as u8,
                        ((u16::from(brush.color[2]) * u16::from(alpha) + 127) / 255) as u8,
                        alpha,
                    ];
                    over(
                        &mut tile[offset..offset + 4],
                        &source,
                        false,
                        255,
                        Blend::Normal,
                    );
                    painted = true;
                }
            }
            if !painted {
                return Err(PaintError::Invalid);
            }
            layer.tiles.extend(updates);
            Ok(())
        })
    }

    pub fn set_selection(&mut self, selection: Option<Selection>) -> Result<(), PaintError> {
        if let Some(s) = &selection {
            s.validate()?;
            let b = s.bounds();
            if b[2] > f64::from(self.state.width) || b[3] > f64::from(self.state.height) {
                return Err(PaintError::Invalid);
            }
        }
        self.cancel_all();
        self.state.selection = selection;
        Ok(())
    }
    pub fn fill_selection(&mut self, erase: bool) -> Result<(), PaintError> {
        if !self.brush.validate() {
            return Err(PaintError::Invalid);
        }
        let color = self.brush.color;
        let alpha = (self.brush.opacity * 255.0).round() as u8;
        self.edit(|s| {
            let index = s
                .layers
                .iter()
                .position(|l| l.id == s.selected)
                .ok_or(PaintError::Invalid)?;
            if s.layers[index].locked {
                return Err(PaintError::Locked);
            }
            let mut tiles = s.layers[index].tiles.clone();
            let mut count = s.tile_count();
            let mask = mask(s);
            let src = [
                (u16::from(color[0]) * u16::from(alpha) / 255) as u8,
                (u16::from(color[1]) * u16::from(alpha) / 255) as u8,
                (u16::from(color[2]) * u16::from(alpha) / 255) as u8,
                alpha,
            ];
            for y in 0..s.height {
                for x in 0..s.width {
                    if mask[(y * s.width + x) as usize] {
                        let mut p = read(&tiles, x, y);
                        if erase {
                            p = [0; 4]
                        } else {
                            over(&mut p, &src, false, 255, Blend::Normal);
                        }
                        write(&mut tiles, x, y, p, &mut count)?;
                    }
                }
            }
            tiles.retain(|_, t| t.iter().any(|v| *v != 0));
            s.layers[index].tiles = tiles;
            Ok(())
        })
    }
    pub fn eyedropper(&self, position: [f64; 2]) -> Result<[u8; 3], PaintError> {
        if !position.iter().all(|n| n.is_finite())
            || position[0] < 0.0
            || position[1] < 0.0
            || position[0] >= f64::from(self.state.width)
            || position[1] >= f64::from(self.state.height)
        {
            return Err(PaintError::Invalid);
        }
        let x = position[0] as u32;
        let y = position[1] as u32;
        let tile = self.composite_tile((x / TILE, y / TILE));
        let i = ((y % TILE * TILE + x % TILE) * 4) as usize;
        let a = u32::from(tile[i + 3]);
        if a == 0 {
            return Err(PaintError::Invalid);
        }
        Ok([0, 1, 2].map(|c| (u32::from(tile[i + c]) * 255 / a).min(255) as u8))
    }
    /// Affine source -> destination; [a,b,c,d,tx,ty]. Nearest neighbor, clipped to
    /// the document. The original selected pixels are cleared, then transformed
    /// pixels composite over unselected content. One undo transaction.
    pub fn transform_selection(&mut self, m: [f64; 6]) -> Result<(), PaintError> {
        if !m.iter().all(|n| n.is_finite() && n.abs() <= 8192.0) {
            return Err(PaintError::Invalid);
        }
        let det = m[0] * m[3] - m[1] * m[2];
        if det.abs() < 0.0001 || m[..4].iter().any(|n| n.abs() > 16.0) {
            return Err(PaintError::Invalid);
        }
        self.edit(|s| {
            let index = s
                .layers
                .iter()
                .position(|l| l.id == s.selected)
                .ok_or(PaintError::Invalid)?;
            if s.layers[index].locked {
                return Err(PaintError::Locked);
            }
            let original = s.layers[index].tiles.clone();
            let mut out = original.clone();
            let mut count = s.tile_count();
            let mask = mask(s);
            for y in 0..s.height {
                for x in 0..s.width {
                    if mask[(y * s.width + x) as usize] {
                        write(&mut out, x, y, [0; 4], &mut count)?;
                    }
                }
            }
            for y in 0..s.height {
                for x in 0..s.width {
                    let dx = f64::from(x) + 0.5 - m[4];
                    let dy = f64::from(y) + 0.5 - m[5];
                    let sx = (m[3] * dx - m[2] * dy) / det;
                    let sy = (-m[1] * dx + m[0] * dy) / det;
                    if sx < 0.0 || sy < 0.0 || sx >= f64::from(s.width) || sy >= f64::from(s.height)
                    {
                        continue;
                    }
                    let (sx, sy) = (sx.floor() as u32, sy.floor() as u32);
                    if mask[(sy * s.width + sx) as usize] {
                        let src = read(&original, sx, sy);
                        if src[3] > 0 {
                            let mut p = read(&out, x, y);
                            over(&mut p, &src, false, 255, Blend::Normal);
                            write(&mut out, x, y, p, &mut count)?;
                        }
                    }
                }
            }
            out.retain(|_, t| t.iter().any(|v| *v != 0));
            s.layers[index].tiles = out;
            s.selection = None;
            Ok(())
        })
    }
    pub fn crop_selection(&mut self) -> Result<(), PaintError> {
        self.edit(|s| {
            let bounds = s.selection.as_ref().ok_or(PaintError::Invalid)?.bounds();
            let [x, y, r, b] = bounds.map(|v| v.floor() as u32);
            if r <= x || b <= y || r > s.width || b > s.height {
                return Err(PaintError::Invalid);
            }
            let mut count = 0;
            for l in &mut s.layers {
                let mut out = TileMap::new();
                for yy in y..b {
                    for xx in x..r {
                        write(&mut out, xx - x, yy - y, read(&l.tiles, xx, yy), &mut count)?;
                    }
                }
                l.tiles = out;
            }
            s.width = r - x;
            s.height = b - y;
            s.selection = None;
            Ok(())
        })
    }
}

pub fn gradient_parameter(
    kind: GradientKind,
    start: [f64; 2],
    end: [f64; 2],
    point: [f64; 2],
) -> f64 {
    let delta = [end[0] - start[0], end[1] - start[1]];
    let length_squared = delta[0].mul_add(delta[0], delta[1] * delta[1]);
    if length_squared <= f64::EPSILON
        || !start
            .iter()
            .chain(end.iter())
            .chain(point.iter())
            .all(|value| value.is_finite())
    {
        return 0.0;
    }
    match kind {
        GradientKind::Linear => {
            ((point[0] - start[0]) * delta[0] + (point[1] - start[1]) * delta[1]) / length_squared
        }
        GradientKind::Radial => {
            (point[0] - start[0]).hypot(point[1] - start[1]) / length_squared.sqrt()
        }
    }
    .clamp(0.0, 1.0)
}

fn shape_coverage(
    kind: ShapeKind,
    start: [f64; 2],
    end: [f64; 2],
    point: [f64; 2],
    half_width: f64,
    filled: bool,
) -> f64 {
    let (left, top, right, bottom) = (
        start[0].min(end[0]),
        start[1].min(end[1]),
        start[0].max(end[0]),
        start[1].max(end[1]),
    );
    match kind {
        ShapeKind::Line => {
            let distance = segment_distance(point, start, end);
            (half_width + 0.5 - distance).clamp(0.0, 1.0)
        }
        ShapeKind::Rectangle => {
            if filled {
                let inside = (point[0] - left)
                    .min(right - point[0])
                    .min(point[1] - top)
                    .min(bottom - point[1]);
                (inside + 0.5).clamp(0.0, 1.0)
            } else {
                let corners = [[left, top], [right, top], [right, bottom], [left, bottom]];
                let distance = (0..4)
                    .map(|index| segment_distance(point, corners[index], corners[(index + 1) % 4]))
                    .fold(f64::INFINITY, f64::min);
                (half_width + 0.5 - distance).clamp(0.0, 1.0)
            }
        }
        ShapeKind::Ellipse => {
            let center = [(left + right) / 2.0, (top + bottom) / 2.0];
            let radii = [(right - left) / 2.0, (bottom - top) / 2.0];
            let delta = [point[0] - center[0], point[1] - center[1]];
            let normalized = (delta[0] / radii[0]).hypot(delta[1] / radii[1]);
            let gradient =
                (delta[0] / (radii[0] * radii[0])).hypot(delta[1] / (radii[1] * radii[1]));
            let signed_distance = if gradient > f64::EPSILON {
                (normalized - 1.0) / gradient
            } else {
                -radii[0].min(radii[1])
            };
            if filled {
                (0.5 - signed_distance).clamp(0.0, 1.0)
            } else {
                (half_width + 0.5 - signed_distance.abs()).clamp(0.0, 1.0)
            }
        }
    }
}

fn segment_distance(point: [f64; 2], start: [f64; 2], end: [f64; 2]) -> f64 {
    let delta = [end[0] - start[0], end[1] - start[1]];
    let length_squared = delta[0].mul_add(delta[0], delta[1] * delta[1]);
    if length_squared <= f64::EPSILON {
        return (point[0] - start[0]).hypot(point[1] - start[1]);
    }
    let t = (((point[0] - start[0]) * delta[0] + (point[1] - start[1]) * delta[1])
        / length_squared)
        .clamp(0.0, 1.0);
    (point[0] - (start[0] + t * delta[0])).hypot(point[1] - (start[1] + t * delta[1]))
}
