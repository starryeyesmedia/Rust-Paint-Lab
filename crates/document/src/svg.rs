//! Bounded SVG output for editable vector paths.
//!
//! This exports the visible vector subset only. Raster tiles and masks remain
//! in the native project or PNG export and are deliberately not flattened into
//! the SVG file.

use crate::{Blend, PaintError, State, vector::VectorSegment};
#[cfg(unix)]
use std::fs::File;
use std::{
    fmt::Write as _,
    fs::{self, OpenOptions},
    io::Write as _,
    path::Path,
    time::{SystemTime, UNIX_EPOCH},
};

pub const MAX_SVG_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SvgExportReport {
    pub vector_paths: usize,
    pub raster_layers_omitted: usize,
    pub masked_layers_omitted: usize,
    pub non_normal_blend_layers_omitted: usize,
    pub background_omitted: bool,
}

impl SvgExportReport {
    pub fn has_omissions(self) -> bool {
        self.raster_layers_omitted != 0
            || self.masked_layers_omitted != 0
            || self.non_normal_blend_layers_omitted != 0
            || self.background_omitted
    }
}

/// Encode visible vector paths as ordinary editable SVG `<path>` elements.
/// The result retains path order, line/cubic geometry, stroke/fill colors and
/// path visibility. Raster artwork and masks are not embedded.
pub fn encode(state: &State) -> Result<(String, SvgExportReport), PaintError> {
    state.validate()?;
    let mut report = SvgExportReport {
        background_omitted: state.background.is_some(),
        ..SvgExportReport::default()
    };
    let mut output = format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">\n",
        state.width, state.height, state.width, state.height
    );
    for layer in state.layers.iter().filter(|layer| layer.visible) {
        if !layer.tiles.is_empty() {
            report.raster_layers_omitted += 1;
        }
        if layer.mask.is_some() {
            report.masked_layers_omitted += 1;
        }
        if layer.blend != Blend::Normal {
            report.non_normal_blend_layers_omitted += 1;
        }
        if layer.vector_paths.is_empty() {
            continue;
        }
        write!(
            output,
            "<g data-rpl-layer=\"{}\" data-rpl-name=\"{}\"",
            layer.id,
            escape_attribute(&layer.name)
        )
        .map_err(|_| PaintError::Invalid)?;
        if layer.opacity != 255 {
            write!(output, " opacity=\"{}\"", f64::from(layer.opacity) / 255.0)
                .map_err(|_| PaintError::Invalid)?;
        }
        output.push_str(">\n");
        for path in &layer.vector_paths {
            report.vector_paths += 1;
            write!(output, "<path data-rpl-id=\"{}\" d=\"", path.id)
                .map_err(|_| PaintError::Invalid)?;
            write_path_data(&mut output, path)?;
            output.push_str("\" ");
            write_color_attribute(&mut output, "fill", path.fill)?;
            if path.fill.is_some() {
                output.push_str(" fill-rule=\"evenodd\"");
            }
            output.push(' ');
            write_color_attribute(&mut output, "stroke", Some(path.stroke))?;
            write!(
                output,
                " stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"",
                path.width
            )
            .map_err(|_| PaintError::Invalid)?;
            if !path.visible {
                output.push_str(" display=\"none\"");
            }
            output.push_str("/>\n");
            if output.len() > MAX_SVG_BYTES {
                return Err(PaintError::Limit);
            }
        }
        output.push_str("</g>\n");
    }
    output.push_str("</svg>\n");
    if output.len() > MAX_SVG_BYTES {
        return Err(PaintError::Limit);
    }
    Ok((output, report))
}

/// Write an SVG without replacing an existing file. A sibling temporary is
/// synchronized before an atomic no-clobber hard-link publishes the result.
pub fn export(path: &Path, state: &State) -> Result<SvgExportReport, PaintError> {
    if path.extension().is_none_or(|extension| extension != "svg") {
        return Err(PaintError::Invalid);
    }
    let (svg, report) = encode(state)?;
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let file_name = path.file_name().ok_or(PaintError::Invalid)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = parent.join(format!(
        ".{}.{}.{}.tmp",
        file_name.to_string_lossy(),
        std::process::id(),
        nonce
    ));
    let mut temporary_created = false;
    let result = (|| {
        if path.exists() {
            return Err(PaintError::Conflict);
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        temporary_created = true;
        file.write_all(svg.as_bytes())?;
        file.sync_all()?;
        fs::hard_link(&temporary, path)?;
        #[cfg(unix)]
        File::open(parent)?.sync_all()?;
        Ok(())
    })();
    if temporary_created && temporary.exists() {
        let _ = fs::remove_file(temporary);
    }
    result?;
    Ok(report)
}

fn write_path_data(
    output: &mut String,
    path: &crate::vector::VectorPath,
) -> Result<(), PaintError> {
    write!(output, "M {} {}", path.start[0], path.start[1]).map_err(|_| PaintError::Invalid)?;
    for segment in &path.segments {
        match segment {
            VectorSegment::LineTo { point } => {
                write!(output, " L {} {}", point[0], point[1]).map_err(|_| PaintError::Invalid)?;
            }
            VectorSegment::CubicTo {
                control1,
                control2,
                point,
            } => {
                write!(
                    output,
                    " C {} {}, {} {}, {} {}",
                    control1[0], control1[1], control2[0], control2[1], point[0], point[1]
                )
                .map_err(|_| PaintError::Invalid)?;
            }
            VectorSegment::Close => output.push_str(" Z"),
        }
    }
    Ok(())
}

fn write_color_attribute(
    output: &mut String,
    name: &str,
    color: Option<[u8; 4]>,
) -> Result<(), PaintError> {
    let Some(color) = color else {
        write!(output, "{name}=\"none\"").map_err(|_| PaintError::Invalid)?;
        return Ok(());
    };
    if color[3] == 0 {
        write!(output, "{name}=\"none\"").map_err(|_| PaintError::Invalid)?;
        return Ok(());
    }
    write!(
        output,
        "{name}=\"#{:02x}{:02x}{:02x}\"",
        color[0], color[1], color[2]
    )
    .map_err(|_| PaintError::Invalid)?;
    if color[3] != 255 {
        write!(
            output,
            " {name}-opacity=\"{}\"",
            f64::from(color[3]) / 255.0
        )
        .map_err(|_| PaintError::Invalid)?;
    }
    Ok(())
}

fn escape_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\'', "&apos;")
}
