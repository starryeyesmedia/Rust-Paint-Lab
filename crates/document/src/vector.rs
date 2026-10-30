//! Bounded editable vector paths stored alongside each raster layer.
//!
//! This first format supports stroked line and cubic Bézier paths. Paths stay
//! editable in native projects; raster output is generated only when a layer is
//! composited or exported.
use crate::{Blend, Layer, PaintError, TILE, TILE_BYTES};
use serde::{Deserialize, Serialize};

pub const MAX_VECTOR_PATHS: usize = 1_024;
pub const MAX_VECTOR_SEGMENTS: usize = 8_192;
pub const MAX_VECTOR_SEGMENTS_PER_PATH: usize = 1_024;
/// Maximum estimated per-composite pixel-distance checks across all paths.
/// The estimate sums each flattened segment's clipped document-space bounds.
pub const MAX_VECTOR_WORK: usize = 32_000_000;
pub const MAX_VECTOR_COORDINATE: f64 = 16_384.0;
const CUBIC_STEPS: usize = 16;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VectorPath {
    pub id: u64,
    pub visible: bool,
    pub start: [f64; 2],
    pub segments: Vec<VectorSegment>,
    /// Straight-alpha sRGB stroke color.
    pub stroke: [u8; 4],
    /// Width in document pixels.
    pub width: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum VectorSegment {
    LineTo {
        point: [f64; 2],
    },
    CubicTo {
        control1: [f64; 2],
        control2: [f64; 2],
        point: [f64; 2],
    },
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VectorHandle {
    Incoming,
    Outgoing,
}

impl VectorPath {
    pub fn from_polyline(
        id: u64,
        points: &[[f64; 2]],
        stroke: [u8; 4],
        width: f64,
    ) -> Result<Self, PaintError> {
        if points.len() < 2 || points.len() > MAX_VECTOR_SEGMENTS_PER_PATH + 1 {
            return Err(PaintError::Limit);
        }
        let path = Self {
            id,
            visible: true,
            start: points[0],
            segments: points[1..]
                .iter()
                .copied()
                .map(|point| VectorSegment::LineTo { point })
                .collect(),
            stroke,
            width,
        };
        path.validate()?;
        Ok(path)
    }

    /// Create an editable Catmull–Rom-smoothed path from ordered pointer
    /// samples. Every interval remains a native cubic segment; the first and
    /// last anchors are exact, and adjacent curves share the same tangent at
    /// each interior anchor. This is deterministic and bounded by the same
    /// point limit as a polyline.
    pub fn from_smoothed_polyline(
        id: u64,
        points: &[[f64; 2]],
        stroke: [u8; 4],
        width: f64,
    ) -> Result<Self, PaintError> {
        if points.len() < 2 || points.len() > MAX_VECTOR_SEGMENTS_PER_PATH + 1 {
            return Err(PaintError::Limit);
        }

        let mut segments = Vec::with_capacity(points.len() - 1);
        for index in 0..points.len() - 1 {
            let previous = points[index.saturating_sub(1)];
            let start = points[index];
            let end = points[index + 1];
            let next = points.get(index + 2).copied().unwrap_or(end);
            let control1 = [
                start[0] + (end[0] - previous[0]) / 6.0,
                start[1] + (end[1] - previous[1]) / 6.0,
            ];
            let control2 = [
                end[0] - (next[0] - start[0]) / 6.0,
                end[1] - (next[1] - start[1]) / 6.0,
            ];
            segments.push(VectorSegment::CubicTo {
                control1,
                control2,
                point: end,
            });
        }

        let path = Self {
            id,
            visible: true,
            start: points[0],
            segments,
            stroke,
            width,
        };
        path.validate()?;
        Ok(path)
    }

    pub fn validate(&self) -> Result<(), PaintError> {
        if self.id == 0
            || self.id > u64::from(u32::MAX)
            || self.segments.is_empty()
            || self.segments.len() > MAX_VECTOR_SEGMENTS_PER_PATH
            || !self.width.is_finite()
            || !(0.25..=512.0).contains(&self.width)
            || !valid_point(self.start)
        {
            return Err(PaintError::Invalid);
        }
        let mut closed = false;
        for segment in &self.segments {
            if closed {
                return Err(PaintError::Invalid);
            }
            match segment {
                VectorSegment::LineTo { point } => {
                    if !valid_point(*point) {
                        return Err(PaintError::Invalid);
                    }
                }
                VectorSegment::CubicTo {
                    control1,
                    control2,
                    point,
                } => {
                    if !valid_point(*control1) || !valid_point(*control2) || !valid_point(*point) {
                        return Err(PaintError::Invalid);
                    }
                }
                VectorSegment::Close => closed = true,
            }
        }
        Ok(())
    }

    /// Return the editable anchors in path order. Cubic control handles are
    /// intentionally excluded; they are adjusted with their adjacent anchors.
    pub fn anchor_points(&self) -> Vec<[f64; 2]> {
        let mut points = Vec::with_capacity(self.segments.len() + 1);
        points.push(self.start);
        points.extend(self.segments.iter().filter_map(|segment| match segment {
            VectorSegment::LineTo { point } | VectorSegment::CubicTo { point, .. } => Some(*point),
            VectorSegment::Close => None,
        }));
        points
    }

    /// Move one anchor and carry the adjacent cubic handles with it. The edit
    /// is applied to a clone first so invalid coordinates leave the path intact.
    pub fn move_anchor(&mut self, index: usize, point: [f64; 2]) -> Result<(), PaintError> {
        if !valid_point(point) {
            return Err(PaintError::Invalid);
        }
        let old = self
            .anchor_points()
            .get(index)
            .copied()
            .ok_or(PaintError::Invalid)?;
        let delta = [point[0] - old[0], point[1] - old[1]];
        let mut candidate = self.clone();
        if index == 0 {
            candidate.start = point;
        } else {
            let segment = candidate
                .segments
                .get_mut(index - 1)
                .ok_or(PaintError::Invalid)?;
            match segment {
                VectorSegment::LineTo { point: endpoint } => *endpoint = point,
                VectorSegment::CubicTo {
                    control2,
                    point: endpoint,
                    ..
                } => {
                    translate_point(control2, delta);
                    *endpoint = point;
                }
                VectorSegment::Close => return Err(PaintError::Invalid),
            }
        }
        if let Some(VectorSegment::CubicTo { control1, .. }) = candidate.segments.get_mut(index) {
            translate_point(control1, delta);
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Return a cubic control handle adjacent to an anchor. Incoming handles
    /// belong to the segment ending at the anchor; outgoing handles belong to
    /// the segment starting at it.
    pub fn control_handle(&self, anchor: usize, handle: VectorHandle) -> Option<[f64; 2]> {
        let segment_index = match handle {
            VectorHandle::Incoming => anchor.checked_sub(1)?,
            VectorHandle::Outgoing => anchor,
        };
        match self.segments.get(segment_index)? {
            VectorSegment::CubicTo {
                control1, control2, ..
            } => Some(match handle {
                VectorHandle::Incoming => *control2,
                VectorHandle::Outgoing => *control1,
            }),
            VectorSegment::LineTo { .. } | VectorSegment::Close => None,
        }
    }

    /// Move one control point without changing its anchors or the other handle.
    pub fn move_control_handle(
        &mut self,
        anchor: usize,
        handle: VectorHandle,
        point: [f64; 2],
    ) -> Result<(), PaintError> {
        if !valid_point(point) {
            return Err(PaintError::Invalid);
        }
        let segment_index = match handle {
            VectorHandle::Incoming => anchor.checked_sub(1).ok_or(PaintError::Invalid)?,
            VectorHandle::Outgoing => anchor,
        };
        let mut candidate = self.clone();
        let segment = candidate
            .segments
            .get_mut(segment_index)
            .ok_or(PaintError::Invalid)?;
        match (segment, handle) {
            (VectorSegment::CubicTo { control1, .. }, VectorHandle::Outgoing) => *control1 = point,
            (VectorSegment::CubicTo { control2, .. }, VectorHandle::Incoming) => *control2 = point,
            (VectorSegment::LineTo { .. } | VectorSegment::Close, _) => {
                return Err(PaintError::Invalid);
            }
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Convert a straight segment to a cubic with collinear one-third handles.
    /// The initial curve is visually the same line, after which either handle
    /// can be edited independently.
    pub fn convert_line_to_cubic(&mut self, after_anchor: usize) -> Result<(), PaintError> {
        let start = self
            .anchor_points()
            .get(after_anchor)
            .copied()
            .ok_or(PaintError::Invalid)?;
        let segment = self
            .segments
            .get(after_anchor)
            .cloned()
            .ok_or(PaintError::Invalid)?;
        let VectorSegment::LineTo { point } = segment else {
            return Err(PaintError::Invalid);
        };
        let mut candidate = self.clone();
        candidate.segments[after_anchor] = VectorSegment::CubicTo {
            control1: lerp(start, point, 1.0 / 3.0),
            control2: lerp(start, point, 2.0 / 3.0),
            point,
        };
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Insert an anchor halfway along the segment following `index`. Cubic
    /// segments are split with De Casteljau's algorithm, retaining the same
    /// curve while making the split point independently editable.
    pub fn insert_anchor_after(&mut self, index: usize) -> Result<usize, PaintError> {
        if self.segments.len() >= MAX_VECTOR_SEGMENTS_PER_PATH {
            return Err(PaintError::Limit);
        }
        let anchors = self.anchor_points();
        let start = anchors.get(index).copied().ok_or(PaintError::Invalid)?;
        if index + 1 >= anchors.len() {
            return Err(PaintError::Invalid);
        }
        let segment = self
            .segments
            .get(index)
            .cloned()
            .ok_or(PaintError::Invalid)?;
        let (first, second) = match segment {
            VectorSegment::LineTo { point } => {
                let middle = midpoint(start, point);
                (
                    VectorSegment::LineTo { point: middle },
                    VectorSegment::LineTo { point },
                )
            }
            VectorSegment::CubicTo {
                control1,
                control2,
                point,
            } => {
                let q0 = midpoint(start, control1);
                let q1 = midpoint(control1, control2);
                let q2 = midpoint(control2, point);
                let r0 = midpoint(q0, q1);
                let r1 = midpoint(q1, q2);
                let middle = midpoint(r0, r1);
                (
                    VectorSegment::CubicTo {
                        control1: q0,
                        control2: r0,
                        point: middle,
                    },
                    VectorSegment::CubicTo {
                        control1: r1,
                        control2: q2,
                        point,
                    },
                )
            }
            VectorSegment::Close => return Err(PaintError::Invalid),
        };
        let mut candidate = self.clone();
        candidate.segments.splice(index..=index, [first, second]);
        candidate.validate()?;
        *self = candidate;
        Ok(index + 1)
    }

    /// Whether this anchor can be removed without approximating adjacent
    /// curves. Interior removal currently joins two line segments exactly;
    /// endpoint removal drops one adjacent segment.
    pub fn can_remove_anchor(&self, index: usize) -> bool {
        let anchors = self.anchor_points();
        if anchors.len() <= 2 || index >= anchors.len() {
            return false;
        }
        if index == 0 {
            return matches!(
                self.segments.first(),
                Some(VectorSegment::LineTo { .. } | VectorSegment::CubicTo { .. })
            );
        }
        if index + 1 == anchors.len() {
            return matches!(
                self.segments.get(index - 1),
                Some(VectorSegment::LineTo { .. } | VectorSegment::CubicTo { .. })
            );
        }
        matches!(
            (self.segments.get(index - 1), self.segments.get(index)),
            (
                Some(VectorSegment::LineTo { .. }),
                Some(VectorSegment::LineTo { .. })
            )
        )
    }

    /// Remove an anchor without changing neighboring line geometry. Interior
    /// anchors between cubic segments are rejected rather than silently
    /// replacing a curve with an approximation.
    pub fn remove_anchor(&mut self, index: usize) -> Result<usize, PaintError> {
        if !self.can_remove_anchor(index) {
            return Err(PaintError::Invalid);
        }
        let anchors = self.anchor_points();
        let mut candidate = self.clone();
        if index == 0 {
            candidate.start = match candidate.segments.remove(0) {
                VectorSegment::LineTo { point } | VectorSegment::CubicTo { point, .. } => point,
                VectorSegment::Close => return Err(PaintError::Invalid),
            };
        } else if index + 1 == anchors.len() {
            candidate.segments.remove(index - 1);
        } else {
            let point = match candidate.segments.get(index) {
                Some(VectorSegment::LineTo { point }) => *point,
                _ => return Err(PaintError::Invalid),
            };
            candidate.segments[index - 1] = VectorSegment::LineTo { point };
            candidate.segments.remove(index);
        }
        candidate.validate()?;
        *self = candidate;
        Ok(index.saturating_sub(1).min(anchors.len() - 2))
    }

    pub fn translate(&mut self, delta: [f64; 2]) -> Result<(), PaintError> {
        if !delta.iter().all(|value| value.is_finite()) {
            return Err(PaintError::Invalid);
        }
        let mut candidate = self.clone();
        translate_point(&mut candidate.start, delta);
        for segment in &mut candidate.segments {
            match segment {
                VectorSegment::LineTo { point } => translate_point(point, delta),
                VectorSegment::CubicTo {
                    control1,
                    control2,
                    point,
                } => {
                    translate_point(control1, delta);
                    translate_point(control2, delta);
                    translate_point(point, delta);
                }
                VectorSegment::Close => {}
            }
        }
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    /// Uniformly scale and rotate a path around the center of its rendered
    /// bounds. The stroke width follows the scale so the result behaves like
    /// one vector object rather than a geometry-only edit.
    pub fn transform_uniform(
        &mut self,
        scale: f64,
        rotation_degrees: f64,
    ) -> Result<(), PaintError> {
        if !scale.is_finite()
            || !(0.1..=10.0).contains(&scale)
            || !rotation_degrees.is_finite()
            || !(-360.0..=360.0).contains(&rotation_degrees)
        {
            return Err(PaintError::Invalid);
        }
        let bounds = self.bounds();
        let center = [(bounds[0] + bounds[2]) * 0.5, (bounds[1] + bounds[3]) * 0.5];
        let (sin, cos) = rotation_degrees.to_radians().sin_cos();
        let transform_point = |point: &mut [f64; 2]| {
            let x = (point[0] - center[0]) * scale;
            let y = (point[1] - center[1]) * scale;
            point[0] = center[0] + x * cos - y * sin;
            point[1] = center[1] + x * sin + y * cos;
        };
        let mut candidate = self.clone();
        transform_point(&mut candidate.start);
        for segment in &mut candidate.segments {
            match segment {
                VectorSegment::LineTo { point } => transform_point(point),
                VectorSegment::CubicTo {
                    control1,
                    control2,
                    point,
                } => {
                    transform_point(control1);
                    transform_point(control2);
                    transform_point(point);
                }
                VectorSegment::Close => {}
            }
        }
        candidate.width *= scale;
        candidate.validate()?;
        *self = candidate;
        Ok(())
    }

    pub fn bounds(&self) -> [f64; 4] {
        let mut bounds = [self.start[0], self.start[1], self.start[0], self.start[1]];
        for_each_flat_segment(self, |a, b| {
            bounds[0] = bounds[0].min(a[0]).min(b[0]);
            bounds[1] = bounds[1].min(a[1]).min(b[1]);
            bounds[2] = bounds[2].max(a[0]).max(b[0]);
            bounds[3] = bounds[3].max(a[1]).max(b[1]);
        });
        bounds
    }

    /// Cheap conservative bounds using the convex hull of cubic control points.
    /// This is used to skip tile raster work before flattening path segments.
    pub fn control_bounds(&self) -> [f64; 4] {
        let mut bounds = [self.start[0], self.start[1], self.start[0], self.start[1]];
        let mut include = |point: [f64; 2]| {
            bounds[0] = bounds[0].min(point[0]);
            bounds[1] = bounds[1].min(point[1]);
            bounds[2] = bounds[2].max(point[0]);
            bounds[3] = bounds[3].max(point[1]);
        };
        for segment in &self.segments {
            match segment {
                VectorSegment::LineTo { point } => include(*point),
                VectorSegment::CubicTo {
                    control1,
                    control2,
                    point,
                } => {
                    include(*control1);
                    include(*control2);
                    include(*point);
                }
                VectorSegment::Close => {}
            }
        }
        bounds
    }
}

fn valid_point(point: [f64; 2]) -> bool {
    point
        .iter()
        .all(|value| value.is_finite() && value.abs() <= MAX_VECTOR_COORDINATE)
}

fn midpoint(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
}

fn lerp(a: [f64; 2], b: [f64; 2], t: f64) -> [f64; 2] {
    [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t]
}

fn translate_point(point: &mut [f64; 2], delta: [f64; 2]) {
    point[0] += delta[0];
    point[1] += delta[1];
}

fn cubic(a: f64, b: f64, c: f64, d: f64, t: f64) -> f64 {
    let inverse = 1.0 - t;
    inverse * inverse * inverse * a
        + 3.0 * inverse * inverse * t * b
        + 3.0 * inverse * t * t * c
        + t * t * t * d
}

fn for_each_flat_segment(path: &VectorPath, mut visit: impl FnMut([f64; 2], [f64; 2])) {
    let mut current = path.start;
    for segment in &path.segments {
        match segment {
            VectorSegment::LineTo { point } => {
                visit(current, *point);
                current = *point;
            }
            VectorSegment::CubicTo {
                control1,
                control2,
                point,
            } => {
                let start = current;
                for step in 1..=CUBIC_STEPS {
                    let t = step as f64 / CUBIC_STEPS as f64;
                    let next = [
                        cubic(start[0], control1[0], control2[0], point[0], t),
                        cubic(start[1], control1[1], control2[1], point[1], t),
                    ];
                    visit(current, next);
                    current = next;
                }
                current = *point;
            }
            VectorSegment::Close => {
                visit(current, path.start);
                current = path.start;
            }
        }
    }
}

fn distance_to_segment(point: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let delta = [b[0] - a[0], b[1] - a[1]];
    let length_squared = delta[0] * delta[0] + delta[1] * delta[1];
    let amount = if length_squared <= f64::EPSILON {
        0.0
    } else {
        (((point[0] - a[0]) * delta[0] + (point[1] - a[1]) * delta[1]) / length_squared)
            .clamp(0.0, 1.0)
    };
    (point[0] - (a[0] + delta[0] * amount)).hypot(point[1] - (a[1] + delta[1] * amount))
}

pub fn path_distance(path: &VectorPath, point: [f64; 2]) -> f64 {
    let mut nearest = f64::INFINITY;
    for_each_flat_segment(path, |a, b| {
        nearest = nearest.min(distance_to_segment(point, a, b));
    });
    nearest
}

/// Flatten a path deterministically for lightweight selection overlays. Raster
/// compositing uses the same subdivision rule without allocating this vector.
pub fn flattened_points(path: &VectorPath) -> Vec<[f64; 2]> {
    let mut points = vec![path.start];
    for_each_flat_segment(path, |_, end| points.push(end));
    points
}

/// Conservative upper bound for the number of pixel distance checks required
/// to render a layer's visible paths once. It mirrors the renderer's per-segment
/// expanded bounding rectangles and is evaluated during state validation.
pub fn estimated_layer_work(paths: &[VectorPath], document: [u32; 2]) -> Option<usize> {
    let mut work = 0usize;
    for path in paths.iter().filter(|path| path.visible) {
        let radius = path.width * 0.5 + 0.5;
        for_each_flat_segment(path, |a, b| {
            if work > MAX_VECTOR_WORK {
                return;
            }
            let min_x = (a[0].min(b[0]) - radius).floor().max(0.0);
            let min_y = (a[1].min(b[1]) - radius).floor().max(0.0);
            let max_x = (a[0].max(b[0]) + radius).ceil().min(f64::from(document[0]));
            let max_y = (a[1].max(b[1]) + radius).ceil().min(f64::from(document[1]));
            if min_x >= max_x || min_y >= max_y {
                return;
            }
            let area = ((max_x - min_x) as usize).checked_mul((max_y - min_y) as usize);
            work = area
                .and_then(|area| work.checked_add(area))
                .unwrap_or(usize::MAX);
        });
    }
    (work <= MAX_VECTOR_WORK).then_some(work)
}

pub fn sample_path(path: &VectorPath, point: [f64; 2], dst: &mut [u8; 4]) {
    let distance = path_distance(path, point);
    let coverage = (path.width * 0.5 + 0.5 - distance).clamp(0.0, 1.0);
    blend_coverage(path.stroke, coverage, dst);
}

pub fn rasterize_paths_at_pixel(paths: &[VectorPath], point: [f64; 2], dst: &mut [u8; 4]) {
    for path in paths.iter().filter(|path| path.visible) {
        sample_path(path, point, dst);
    }
}

/// Composite editable strokes into one premultiplied document-space tile.
/// Temporary coverage is a fixed 64×64 stack buffer; path data is never retained
/// as a duplicate raster layer or written back to authoritative tiles.
pub fn rasterize_layer_tile(layer: &Layer, key: (u32, u32), document: [u32; 2], tile: &mut [u8]) {
    if layer.vector_paths.is_empty()
        || tile.len() != TILE_BYTES
        || key.0 >= document[0].div_ceil(TILE)
        || key.1 >= document[1].div_ceil(TILE)
    {
        return;
    }
    let origin = [f64::from(key.0 * TILE), f64::from(key.1 * TILE)];
    let right = (origin[0] + f64::from(TILE)).min(f64::from(document[0]));
    let bottom = (origin[1] + f64::from(TILE)).min(f64::from(document[1]));
    if origin[0] >= right || origin[1] >= bottom {
        return;
    }

    let mut coverage = [0.0_f32; (TILE * TILE) as usize];
    for path in layer.vector_paths.iter().filter(|path| path.visible) {
        let radius = path.width * 0.5 + 0.5;
        let bounds = path.control_bounds();
        if bounds[2] + radius < origin[0]
            || bounds[0] - radius >= right
            || bounds[3] + radius < origin[1]
            || bounds[1] - radius >= bottom
        {
            continue;
        }
        coverage.fill(0.0);
        for_each_flat_segment(path, |a, b| {
            let min_x = (a[0].min(b[0]) - radius - origin[0]).floor().max(0.0) as u32;
            let min_y = (a[1].min(b[1]) - radius - origin[1]).floor().max(0.0) as u32;
            let max_x = (a[0].max(b[0]) + radius - origin[0])
                .ceil()
                .min(right - origin[0]) as u32;
            let max_y = (a[1].max(b[1]) + radius - origin[1])
                .ceil()
                .min(bottom - origin[1]) as u32;
            if min_x >= max_x || min_y >= max_y {
                return;
            }
            for y in min_y..max_y.min(TILE) {
                for x in min_x..max_x.min(TILE) {
                    let point = [
                        origin[0] + f64::from(x) + 0.5,
                        origin[1] + f64::from(y) + 0.5,
                    ];
                    let amount = (radius - distance_to_segment(point, a, b)).clamp(0.0, 1.0) as f32;
                    let index = (y * TILE + x) as usize;
                    coverage[index] = coverage[index].max(amount);
                }
            }
        });
        for (index, amount) in coverage.iter().copied().enumerate() {
            if amount > 0.0 {
                let offset = index * 4;
                let dst: &mut [u8; 4] = (&mut tile[offset..offset + 4]).try_into().unwrap();
                blend_coverage(path.stroke, f64::from(amount), dst);
            }
        }
    }
}

fn blend_coverage(color: [u8; 4], coverage: f64, dst: &mut [u8; 4]) {
    let alpha = (f64::from(color[3]) * coverage).round().clamp(0.0, 255.0) as u8;
    if alpha == 0 {
        return;
    }
    let source = [
        ((u16::from(color[0]) * u16::from(alpha) + 127) / 255) as u8,
        ((u16::from(color[1]) * u16::from(alpha) + 127) / 255) as u8,
        ((u16::from(color[2]) * u16::from(alpha) + 127) / 255) as u8,
        alpha,
    ];
    crate::over(dst, &source, false, 255, Blend::Normal);
}

pub fn layer_vectors_bytes(layer: &Layer) -> usize {
    layer
        .vector_paths
        .iter()
        .map(|path| {
            std::mem::size_of::<VectorPath>()
                + path.segments.capacity() * std::mem::size_of::<VectorSegment>()
        })
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cubic_path_has_deterministic_distance_and_keeps_control_points_editable() {
        let path = VectorPath {
            id: 1,
            visible: true,
            start: [0.0, 0.0],
            segments: vec![VectorSegment::CubicTo {
                control1: [10.0, 0.0],
                control2: [10.0, 10.0],
                point: [20.0, 10.0],
            }],
            stroke: [120, 40, 220, 255],
            width: 2.0,
        };
        path.validate().unwrap();
        let bounds = path.control_bounds();
        for point in flattened_points(&path) {
            assert!(point[0] >= bounds[0] && point[0] <= bounds[2]);
            assert!(point[1] >= bounds[1] && point[1] <= bounds[3]);
        }
        assert_eq!(bounds, [0.0, 0.0, 20.0, 10.0]);
        let first = path_distance(&path, [10.0, 5.0]);
        assert_eq!(first, path_distance(&path, [10.0, 5.0]));
        assert!(first < 3.0, "distance={first}");
    }

    #[test]
    fn smoothed_paths_keep_exact_anchors_and_share_c1_tangents() {
        let points = [[1.25, 2.5], [8.0, 12.0], [21.0, 5.0], [31.5, 18.25]];
        let path =
            VectorPath::from_smoothed_polyline(7, &points, [180, 60, 240, 192], 3.5).unwrap();
        let repeated =
            VectorPath::from_smoothed_polyline(7, &points, [180, 60, 240, 192], 3.5).unwrap();
        assert_eq!(path, repeated);
        assert_eq!(path.start, points[0]);
        assert_eq!(path.anchor_points(), points);
        assert_eq!(path.segments.len(), points.len() - 1);

        for index in 1..points.len() - 1 {
            let VectorSegment::CubicTo {
                control2: incoming,
                point: anchor,
                ..
            } = &path.segments[index - 1]
            else {
                panic!("smoothed path segment must remain cubic");
            };
            let VectorSegment::CubicTo {
                control1: outgoing, ..
            } = &path.segments[index]
            else {
                panic!("smoothed path segment must remain cubic");
            };
            for axis in 0..2 {
                let before = 3.0 * (anchor[axis] - incoming[axis]);
                let after = 3.0 * (outgoing[axis] - anchor[axis]);
                assert!((before - after).abs() <= 1e-12, "knot {index}, axis {axis}");
            }
        }
    }

    #[test]
    fn smoothed_path_rejects_invalid_and_unbounded_samples() {
        assert!(matches!(
            VectorPath::from_smoothed_polyline(1, &[[0.0, 0.0]], [0; 4], 2.0),
            Err(PaintError::Limit)
        ));
        let too_many = vec![[1.0, 1.0]; MAX_VECTOR_SEGMENTS_PER_PATH + 2];
        assert!(matches!(
            VectorPath::from_smoothed_polyline(1, &too_many, [0; 4], 2.0),
            Err(PaintError::Limit)
        ));
        assert!(matches!(
            VectorPath::from_smoothed_polyline(1, &[[0.0, 0.0], [f64::NAN, 1.0]], [0; 4], 2.0),
            Err(PaintError::Invalid)
        ));
        assert!(matches!(
            VectorPath::from_smoothed_polyline(1, &[[0.0, 0.0], [1.0, 1.0]], [0; 4], f64::INFINITY),
            Err(PaintError::Invalid)
        ));
    }

    #[test]
    fn tile_rasterization_is_antialiased_bounded_and_keeps_vector_separate() {
        fn pixel(tile: &[u8], x: usize, y: usize) -> &[u8] {
            &tile[(y * TILE as usize + x) * 4..][..4]
        }

        let mut layer = Layer {
            id: 1,
            name: "Paint 1".into(),
            visible: true,
            locked: false,
            opacity: 255,
            blend: Blend::Normal,
            mask: None,
            vector_paths: vec![
                VectorPath::from_polyline(1, &[[2.0, 2.5], [12.0, 2.5]], [200, 50, 100, 255], 2.0)
                    .unwrap(),
            ],
            tiles: Default::default(),
        };
        let mut tile = vec![0; TILE_BYTES];
        rasterize_layer_tile(&layer, (0, 0), [16, 16], &mut tile);
        assert_eq!(pixel(&tile, 6, 2), &[200, 50, 100, 255]);
        assert!(pixel(&tile, 6, 1)[3] > 0 && pixel(&tile, 6, 1)[3] < 255);
        assert_eq!(pixel(&tile, 6, 5), &[0, 0, 0, 0]);
        assert!(layer.tiles.is_empty(), "raster output must remain derived");

        layer.vector_paths[0].visible = false;
        tile.fill(0);
        rasterize_layer_tile(&layer, (0, 0), [16, 16], &mut tile);
        assert_eq!(pixel(&tile, 6, 2), &[0, 0, 0, 0]);
    }
}
