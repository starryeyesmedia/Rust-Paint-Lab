use paint_document::{
    Blend, Engine, Layer, StrokeMode, Symmetry, SymmetryMode, TILE, TILE_BYTES,
    brush::{Brush, BrushKind},
    editing::{Selection, SelectionKind},
};
use paint_input::{Buttons, Tool};
use paint_stroke::{StrokeCommand as C, StrokeId, StrokeSample, coordinates::DocumentPoint};
use std::sync::Arc;

fn sample(x: f64, y: f64, pressure: f32, time_ms: u64) -> StrokeSample {
    StrokeSample {
        source_counter: None,
        pointer_id: 7,
        tool: Tool::Pen,
        position_client_px: [x, y],
        position_document: DocumentPoint([x, y]),
        pressure: Some(pressure),
        tilt_degrees: [Some(14.0), Some(-9.0)],
        buttons: Buttons {
            primary: true,
            ..Buttons::default()
        },
        source_time_ms: time_ms,
        time_ms,
    }
}

fn draw(engine: &mut Engine, id: u64, points: &[(f64, f64, f32)], mode: StrokeMode) {
    let (x, y, pressure) = points[0];
    engine
        .apply_with_mode(
            &C::BeginStroke {
                stroke_id: StrokeId(id),
                sample: sample(x, y, pressure, 0),
            },
            mode,
        )
        .unwrap();
    for (index, &(x, y, pressure)) in points[1..points.len() - 1].iter().enumerate() {
        engine
            .apply_with_mode(
                &C::UpdateStroke {
                    stroke_id: StrokeId(id),
                    sample: sample(x, y, pressure, ((index + 1) * 4) as u64),
                },
                mode,
            )
            .unwrap();
    }
    let &(x, y, pressure) = points.last().unwrap();
    engine
        .apply_with_mode(
            &C::CommitStroke {
                stroke_id: StrokeId(id),
                sample: sample(x, y, pressure, (points.len() * 4) as u64),
            },
            mode,
        )
        .unwrap();
}

fn digest(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn rendered_fixture(size: f64, operation: usize) -> u64 {
    let mut engine = Engine::new(256, 256, 300).unwrap();
    engine
        .edit(|state| {
            state.background = None;
            let mut lower_tile = vec![0; TILE_BYTES];
            for y in 0..TILE {
                for x in 0..TILE {
                    let index = ((y * TILE + x) * 4) as usize;
                    let document_x = TILE + x;
                    let document_y = TILE + y;
                    lower_tile[index..index + 4].copy_from_slice(&[
                        ((document_x * 3 + document_y * 7 + 19) % 256) as u8,
                        ((document_x * 11 + document_y * 5 + 37) % 256) as u8,
                        ((document_x * 13 + document_y * 17 + 71) % 256) as u8,
                        255,
                    ]);
                }
            }
            state.layers.push(Layer {
                id: 2,
                name: "Patterned lower raster".into(),
                visible: true,
                locked: false,
                opacity: 255,
                blend: Blend::Normal,
                mask: None,
                vector_paths: Vec::new(),
                tiles: [((1, 1), Arc::new(lower_tile))].into_iter().collect(),
            });
            state.layers.push(Layer {
                id: 3,
                name: "Sparse empty raster".into(),
                visible: true,
                locked: false,
                opacity: 255,
                blend: Blend::Normal,
                mask: None,
                vector_paths: Vec::new(),
                tiles: Default::default(),
            });
            Ok(())
        })
        .unwrap();
    let mut base = Brush::preset(BrushKind::Round);
    base.size = 22.0;
    base.pressure_size = false;
    base.pressure_opacity = false;
    base.color = [185, 42, 96];
    engine.brush = base;
    draw(
        &mut engine,
        1,
        &[(26.0, 72.0, 0.7), (225.0, 183.0, 0.7)],
        StrokeMode::Paint,
    );

    engine
        .set_selection(Some(
            Selection::new(SelectionKind::Rectangle, vec![[18.0, 22.0], [238.0, 234.0]]).unwrap(),
        ))
        .unwrap();
    engine
        .set_symmetry(Symmetry {
            mode: SymmetryMode::VerticalAndHorizontal,
            axes: 8,
            center: [0.5, 0.5],
        })
        .unwrap();

    if operation == 4 {
        engine.add_layer_mask(1).unwrap();
    }
    let mut brush = Brush::preset(if operation == 1 {
        BrushKind::Eraser
    } else {
        BrushKind::Round
    });
    brush.size = size;
    brush.spacing = 0.15;
    brush.opacity = 0.73;
    brush.flow = 0.81;
    brush.hardness = 0.64;
    brush.pressure_size = true;
    brush.pressure_opacity = true;
    brush.tilt = true;
    brush.color = [34, 188, 224];
    engine.brush = brush;
    let mode = match operation {
        2 => StrokeMode::Smudge,
        3 => StrokeMode::ColorMix { strength: 0.55 },
        4 => StrokeMode::MaskPaint,
        _ => StrokeMode::Paint,
    };
    draw(
        &mut engine,
        2,
        &[
            (34.0, 78.0, 0.22),
            (78.0, 117.0, 0.55),
            (144.0, 92.0, 0.83),
            (220.0, 172.0, 0.61),
        ],
        mode,
    );
    digest(&engine.rgba())
}

#[test]
fn large_brush_modes_keep_baseline_pixels_across_sizes() {
    let mut actual = Vec::new();
    for size in [16.0, 64.0, 128.0, 256.0] {
        for operation in 0..5 {
            actual.push(rendered_fixture(size, operation));
        }
    }
    // Captured from main at 1559122943e8b6e9123b653a555f72e7a894d3e6.
    // Each group of five is Paint, Eraser, Smudge, Color Mixer, and mask paint.
    let baseline = [
        0x7d3e_7948_e825_b17f,
        0xcf59_0122_5fd0_4c17,
        0xbb4c_ccfc_f24a_498b,
        0x4db2_497b_009d_f4fa,
        0xc904_87c4_73dc_0b9e,
        0x5d1c_d5b9_d614_4e96,
        0x1c60_248d_a8a9_2182,
        0xc5b3_2de4_5847_ae69,
        0x35be_152e_cf45_e859,
        0xc974_aaab_d05d_f622,
        0x169b_ce1c_ef9e_4d30,
        0xecce_c5f3_498b_824b,
        0xb0b1_b119_a1ee_531c,
        0x3b22_433d_e835_31e1,
        0xd1d2_19ed_4840_a012,
        0x2a58_8042_7bdd_0701,
        0xf1dc_18d7_3632_97dc,
        0x5cc4_13cc_907c_ebee,
        0x8dd0_b62f_0c21_c8d2,
        0x2ba0_4644_f47d_9c92,
    ];
    assert_eq!(
        actual, baseline,
        "rendered pixels changed from the main baseline"
    );
}
