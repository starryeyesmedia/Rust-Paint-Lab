//! Pixel-center selections and bounded nearest-neighbor raster edits.
use crate::{Blend, Engine, PaintError, State, TILE, TILE_BYTES, TileMap, over};
use std::sync::Arc;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionKind {
    Rectangle,
    Ellipse,
    Lasso,
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
