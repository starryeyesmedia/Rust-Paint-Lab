use paint_document::brush::{Brush, BrushKind};
use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: bp_brush_fixture <output.rplbrush>")?;
    let mut brush = Brush::preset(BrushKind::Ink);
    brush.size = 9.0;
    brush.opacity = 0.65;
    brush.flow = 0.4;
    brush.pressure_size = false;
    brush.pressure_opacity = true;
    brush.color = [9, 101, 233];
    fs::write(output, brush.encode()?)?;
    Ok(())
}
