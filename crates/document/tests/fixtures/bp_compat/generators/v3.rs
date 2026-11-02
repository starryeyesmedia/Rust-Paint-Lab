use paint_document::{Engine, LayerMask, MASK_TILE_BYTES, State, storage};
use std::{error::Error, fs, path::PathBuf, sync::Arc};

const TILE_BYTES: usize = 64 * 64 * 4;

fn set_pixel(state: &mut State, layer_id: u64, x: u32, y: u32, rgba: [u8; 4]) {
    let layer = state
        .layers
        .iter_mut()
        .find(|layer| layer.id == layer_id)
        .expect("fixture layer exists");
    let tile = layer
        .tiles
        .entry((x / 64, y / 64))
        .or_insert_with(|| Arc::new(vec![0; TILE_BYTES]));
    let offset = (((y % 64) * 64 + x % 64) * 4) as usize;
    Arc::make_mut(tile)[offset..offset + 4].copy_from_slice(&rgba);
}

fn fixture() -> Result<Engine, Box<dyn Error>> {
    let mut engine = Engine::new(64, 64, 300)?;
    engine.add_layer()?;
    engine.edit(|state| {
        state.background = None;
        state.layers[0].name = "Raster base".into();
        state.layers[1].name = "Raster overlay".into();
        for y in 8..20 {
            for x in 4..16 {
                set_pixel(state, 1, x, y, [255, 0, 0, 255]);
            }
            for x in 16..28 {
                set_pixel(state, 1, x, y, [0, 0, 255, 255]);
            }
            for x in 28..36 {
                set_pixel(state, 1, x, y, [64, 0, 64, 128]);
            }
            for x in 44..60 {
                set_pixel(state, 2, x, y, [0, 160, 0, 160]);
            }
        }
        let mut mask = LayerMask::default();
        let mut tile = vec![255; MASK_TILE_BYTES];
        tile[12 * 64 + 8] = 128;
        tile[12 * 64 + 32] = 0;
        mask.tiles.insert((0, 0), Arc::new(tile));
        state.layers[0].mask = Some(mask);
        Ok(())
    })?;
    engine.add_vector_path(&[[16.0, 50.0], [48.0, 50.0]])?;
    Ok(engine)
}

fn main() -> Result<(), Box<dyn Error>> {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: bp_compat_fixture <output.rpl>")?;
    let bytes = storage::encode(fixture()?.state())?;
    assert_eq!(&bytes[..8], b"RPLALP03");
    fs::write(output, bytes)?;
    Ok(())
}
