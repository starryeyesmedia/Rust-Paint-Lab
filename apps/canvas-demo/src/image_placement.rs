//! Pending raster-image placement state and source-to-document transforms.

use egui::{Context, TextureHandle, TextureOptions};
use std::path::Path;

pub const MAX_PLACED_IMAGE_BYTES: usize = 64 * 1024 * 1024;

pub struct PendingImagePlacement {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub name: String,
    pub center: [f64; 2],
    pub scale: f64,
    pub angle_degrees: f64,
    pub texture: TextureHandle,
}

impl PendingImagePlacement {
    pub fn load(
        ctx: &Context,
        path: &Path,
        document: [u32; 2],
    ) -> Result<Self, paint_document::PaintError> {
        let (width, height, rgba) = paint_document::storage::decode_placement_image(path)?;
        if rgba.len() > MAX_PLACED_IMAGE_BYTES {
            return Err(paint_document::PaintError::Limit);
        }
        let name: String = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .filter(|stem| !stem.trim().is_empty())
            .unwrap_or("Placed image")
            .chars()
            .filter(|character| !character.is_control())
            .take(240)
            .collect();
        let preview_scale = (2048.0 / width.max(height) as f32).min(1.0);
        let preview_size = [
            ((width as f32 * preview_scale).round() as usize).max(1),
            ((height as f32 * preview_scale).round() as usize).max(1),
        ];
        let mut preview_pixels = Vec::with_capacity(preview_size[0] * preview_size[1]);
        for y in 0..preview_size[1] {
            for x in 0..preview_size[0] {
                let source_x =
                    ((x as f64 + 0.5) * f64::from(width) / preview_size[0] as f64).floor() as u32;
                let source_y =
                    ((y as f64 + 0.5) * f64::from(height) / preview_size[1] as f64).floor() as u32;
                let offset =
                    ((source_y.min(height - 1) * width + source_x.min(width - 1)) * 4) as usize;
                preview_pixels.push(egui::Color32::from_rgba_unmultiplied(
                    rgba[offset],
                    rgba[offset + 1],
                    rgba[offset + 2],
                    rgba[offset + 3],
                ));
            }
        }
        let texture = ctx.load_texture(
            format!("pending placed image: {name}"),
            egui::ColorImage::new(preview_size, preview_pixels),
            TextureOptions::LINEAR,
        );
        let scale = fit_scale([width, height], document, false) * 0.9;
        Ok(Self {
            width,
            height,
            rgba,
            name,
            center: [f64::from(document[0]) / 2.0, f64::from(document[1]) / 2.0],
            scale,
            angle_degrees: 0.0,
            texture,
        })
    }

    pub fn transform(&self) -> [f64; 6] {
        let (sin, cos) = self.angle_degrees.to_radians().sin_cos();
        let a = cos * self.scale;
        let b = sin * self.scale;
        let c = -sin * self.scale;
        let d = cos * self.scale;
        [
            a,
            b,
            c,
            d,
            self.center[0] - a * f64::from(self.width) / 2.0 - c * f64::from(self.height) / 2.0,
            self.center[1] - b * f64::from(self.width) / 2.0 - d * f64::from(self.height) / 2.0,
        ]
    }

    pub fn corners(&self) -> [[f64; 2]; 4] {
        let transform = self.transform();
        [
            [0.0, 0.0],
            [f64::from(self.width), 0.0],
            [f64::from(self.width), f64::from(self.height)],
            [0.0, f64::from(self.height)],
        ]
        .map(|[x, y]| {
            [
                transform[0] * x + transform[2] * y + transform[4],
                transform[1] * x + transform[3] * y + transform[5],
            ]
        })
    }

    pub fn fit(&mut self, document: [u32; 2]) {
        self.scale = fit_scale_rotated(
            [self.width, self.height],
            document,
            false,
            self.angle_degrees,
        );
        self.center_document(document);
    }

    pub fn fill(&mut self, document: [u32; 2]) {
        self.scale = fit_scale_rotated(
            [self.width, self.height],
            document,
            true,
            self.angle_degrees,
        );
        self.center_document(document);
    }

    pub fn reset(&mut self, document: [u32; 2]) {
        self.scale = 1.0;
        self.angle_degrees = 0.0;
        self.center_document(document);
    }

    fn center_document(&mut self, document: [u32; 2]) {
        self.center = [f64::from(document[0]) / 2.0, f64::from(document[1]) / 2.0];
    }

    pub fn contains_document_point(&self, point: [f64; 2]) -> bool {
        let transform = self.transform();
        let determinant = transform[0] * transform[3] - transform[1] * transform[2];
        let dx = point[0] - transform[4];
        let dy = point[1] - transform[5];
        let source_x = (transform[3] * dx - transform[2] * dy) / determinant;
        let source_y = (-transform[1] * dx + transform[0] * dy) / determinant;
        source_x >= 0.0
            && source_y >= 0.0
            && source_x < f64::from(self.width)
            && source_y < f64::from(self.height)
    }
}

pub fn fit_scale(image: [u32; 2], document: [u32; 2], fill: bool) -> f64 {
    fit_scale_rotated(image, document, fill, 0.0)
}

pub fn fit_scale_rotated(
    image: [u32; 2],
    document: [u32; 2],
    fill: bool,
    angle_degrees: f64,
) -> f64 {
    if image.contains(&0) || document.contains(&0) {
        return 1.0;
    }
    if !angle_degrees.is_finite() {
        return 1.0;
    }
    let (sin, cos) = angle_degrees.to_radians().sin_cos();
    let [width, height] = [
        f64::from(image[0]) * cos.abs() + f64::from(image[1]) * sin.abs(),
        f64::from(image[0]) * sin.abs() + f64::from(image[1]) * cos.abs(),
    ];
    let horizontal = f64::from(document[0]) / width;
    let vertical = f64::from(document[1]) / height;
    if fill {
        horizontal.max(vertical)
    } else {
        horizontal.min(vertical)
    }
    .clamp(0.01, 32.0)
}

#[cfg(test)]
mod tests {
    use super::{fit_scale, fit_scale_rotated};

    #[test]
    fn fit_and_fill_keep_aspect_ratio_and_cover_opposite_axes() {
        assert_eq!(fit_scale([400, 200], [200, 200], false), 0.5);
        assert_eq!(fit_scale([400, 200], [200, 200], true), 1.0);
    }

    #[test]
    fn invalid_dimensions_return_safe_unit_scale() {
        assert_eq!(fit_scale([0, 200], [200, 200], false), 1.0);
    }

    #[test]
    fn fit_and_fill_account_for_the_current_rotation() {
        assert_eq!(fit_scale_rotated([400, 200], [300, 200], false, 90.0), 0.5);
        assert!((fit_scale_rotated([400, 200], [300, 200], true, 90.0) - 1.5).abs() < 1e-12);
    }
}
