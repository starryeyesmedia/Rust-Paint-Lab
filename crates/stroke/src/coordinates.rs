//! Axis-aligned client-to-document mapping; no DPI, GPU, or window dependency.
use std::{error::Error, fmt};

/// Document units, with +x right and +y down. One unit is one pixel at zoom 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DocumentPoint(pub [f64; 2]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransformError {
    InvalidOrigin,
    InvalidZoom,
    InvalidPosition,
    NonFiniteResult,
}
impl fmt::Display for TransformError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid viewport mapping: {self:?}")
    }
}
impl Error for TransformError {}

/// `origin_client_px` is the physical client location of document (0,0).
/// `zoom` is physical client pixels per document unit, strictly positive.
/// document = (client - origin_client_px) / zoom. Rotation is not supported.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    origin_client_px: [f64; 2],
    zoom: f64,
}
impl Default for Viewport {
    fn default() -> Self {
        Self {
            origin_client_px: [0.0; 2],
            zoom: 1.0,
        }
    }
}
impl Viewport {
    pub fn new(origin_client_px: [f64; 2], zoom: f64) -> Result<Self, TransformError> {
        if !origin_client_px.iter().all(|v| v.is_finite()) {
            return Err(TransformError::InvalidOrigin);
        }
        if !zoom.is_finite() || zoom <= 0.0 {
            return Err(TransformError::InvalidZoom);
        }
        Ok(Self {
            origin_client_px,
            zoom,
        })
    }
    pub fn to_document(self, client_px: [f64; 2]) -> Result<DocumentPoint, TransformError> {
        if !client_px.iter().all(|v| v.is_finite()) {
            return Err(TransformError::InvalidPosition);
        }
        let point = std::array::from_fn(|i| (client_px[i] - self.origin_client_px[i]) / self.zoom);
        if !point.iter().all(|v| v.is_finite()) {
            return Err(TransformError::NonFiniteResult);
        }
        Ok(DocumentPoint(point))
    }
}
