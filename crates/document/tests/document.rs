use paint_document::{
    Blend, Engine, PaintError, State,
    brush::{Brush, BrushKind},
    editing::{GradientKind, Selection, SelectionKind, ShapeKind},
    storage::{self, Fault, Store},
};
use paint_input::{Buttons, Tool};
use paint_stroke::{
    CancelReason, StrokeCommand as C, StrokeId, StrokeSample, coordinates::DocumentPoint,
};
fn sample(x: f64, y: f64, p: Option<f32>) -> StrokeSample {
    StrokeSample {
        source_counter: None,
        pointer_id: 1,
        tool: Tool::Pen,
        position_client_px: [x, y],
        position_document: DocumentPoint([x, y]),
        pressure: p,
        tilt_degrees: [Some(0.0), None],
        buttons: Buttons::default(),
        source_time_ms: 0,
        time_ms: 0,
    }
}
fn sample_at(x: f64, y: f64, p: Option<f32>, time_ms: u64) -> StrokeSample {
    let mut s = sample(x, y, p);
    s.source_time_ms = time_ms;
    s.time_ms = time_ms;
    s
}
fn begin(e: &mut Engine, id: u64, x: f64, y: f64) {
    e.apply(&C::BeginStroke {
        stroke_id: StrokeId(id),
        sample: sample(x, y, Some(0.7)),
    })
    .unwrap();
}
fn end(e: &mut Engine, id: u64, x: f64, y: f64) {
    e.apply(&C::CommitStroke {
        stroke_id: StrokeId(id),
        sample: sample(x, y, Some(0.7)),
    })
    .unwrap();
}
fn cancel(e: &mut Engine, id: u64) {
    e.apply(&C::CancelStroke {
        stroke_id: StrokeId(id),
        last: sample(0.0, 0.0, None),
        time_ms: 0,
        reason: CancelReason::RouterInterrupted,
    })
    .unwrap();
}
fn blank(e: &Engine) -> bool {
    e.rgba().chunks_exact(4).all(|p| p == [255, 255, 255, 255])
}
fn path(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("rpl-core-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root.join("art.rpl")
}
#[test]
fn provisional_cancel_never_mutates_authoritative_state_or_history() {
    let mut e = Engine::new(128, 128, 300).unwrap();
    begin(&mut e, 0, 30.0, 30.0);
    assert!(!blank(&e));
    assert_eq!(e.state().tile_count(), 0);
    assert_eq!(e.undo_count(), 0);
    cancel(&mut e, 0);
    assert!(blank(&e));
    assert_eq!(e.state().tile_count(), 0);
    assert!(!e.dirty());
}
#[test]
fn commit_final_sample_undo_redo_are_transactional() {
    let mut e = Engine::new(128, 128, 300).unwrap();
    begin(&mut e, 0, 10.0, 64.0);
    end(&mut e, 0, 110.0, 64.0);
    let pixels = e.rgba();
    assert_ne!(
        &pixels[((64 * 128 + 110) * 4)..((64 * 128 + 110) * 4 + 4)],
        &[255, 255, 255, 255]
    );
    assert_eq!(e.undo_count(), 1);
    assert!(e.undo());
    assert!(blank(&e));
    assert!(e.redo());
    assert_eq!(e.rgba(), pixels);
}

#[test]
fn disconnected_clipped_segment_does_not_rasterize_across_exited_region() {
    let mut e = Engine::new(128, 128, 300).unwrap();
    e.brush.smoothing = 1.0;
    e.apply(&C::BeginStroke {
        stroke_id: StrokeId(7),
        sample: sample(20.0, 64.0, Some(0.8)),
    })
    .unwrap();
    e.apply(&C::BreakStrokeSegment {
        stroke_id: StrokeId(7),
        sample: sample(80.0, 64.0, Some(0.8)),
    })
    .unwrap();
    e.apply(&C::CommitStroke {
        stroke_id: StrokeId(7),
        sample: sample(81.0, 64.0, Some(0.8)),
    })
    .unwrap();
    let rgba = e.rgba();
    let pixel = |x: usize| &rgba[(64 * 128 + x) * 4..(64 * 128 + x) * 4 + 4];
    assert_ne!(pixel(20), &[255; 4]);
    assert_eq!(pixel(50), &[255; 4]);
    assert_ne!(pixel(80), &[255; 4]);
    assert_eq!(e.undo_count(), 1);
}

#[test]
fn adaptive_stabilization_changes_real_raster_path_and_keeps_exact_endpoint() {
    fn paint(smoothing: f64) -> Engine {
        let mut e = Engine::new(160, 128, 300).unwrap();
        e.brush.size = 3.0;
        e.brush.pressure_size = false;
        e.brush.hardness = 1.0;
        e.brush.smoothing = smoothing;
        let id = StrokeId(4);
        e.apply(&C::BeginStroke {
            stroke_id: id,
            sample: sample_at(10.0, 64.0, Some(1.0), 0),
        })
        .unwrap();
        for index in 1..=90 {
            let jitter = if index % 2 == 0 { 3.0 } else { -3.0 };
            e.apply(&C::UpdateStroke {
                stroke_id: id,
                sample: sample_at(index as f64 + 10.0, 64.0 + jitter, Some(1.0), index * 16),
            })
            .unwrap();
        }
        e.apply(&C::CommitStroke {
            stroke_id: id,
            sample: sample_at(110.0, 64.0, Some(1.0), 1456),
        })
        .unwrap();
        e
    }
    let raw = paint(0.0);
    let mut stabilized = paint(0.9);
    let raw_pixels = raw.rgba();
    let smooth_pixels = stabilized.rgba();
    assert_ne!(raw_pixels, smooth_pixels);
    assert_eq!(smooth_pixels, paint(0.9).rgba());
    let endpoint = ((64 * 160 + 110) * 4) as usize;
    assert_ne!(&smooth_pixels[endpoint..endpoint + 4], &[255; 4]);
    assert_eq!(stabilized.undo_count(), 1);
    assert!(stabilized.undo());
    assert!(blank(&stabilized));
}

#[test]
fn linear_and_radial_gradients_rasterize_linear_light_colors_and_roundtrip() {
    let mut linear = Engine::new(5, 5, 300).unwrap();
    linear.brush.color = [0, 0, 0];
    linear
        .draw_gradient(
            GradientKind::Linear,
            [0.5, 2.5],
            [4.5, 2.5],
            [255, 255, 255],
        )
        .unwrap();
    linear
        .draw_gradient(
            GradientKind::Linear,
            [0.5, 2.5],
            [4.5, 2.5],
            [255, 255, 255],
        )
        .unwrap();
    let pixels = linear.rgba();
    let pixel = |image: &[u8], x: usize, y: usize| -> [u8; 4] {
        image[(y * 5 + x) * 4..(y * 5 + x) * 4 + 4]
            .try_into()
            .unwrap()
    };
    assert_eq!(pixel(&pixels, 0, 2), [0, 0, 0, 255]);
    assert_eq!(pixel(&pixels, 2, 2), [188, 188, 188, 255]);
    assert_eq!(pixel(&pixels, 4, 2), [255, 255, 255, 255]);
    assert_eq!(
        linear.undo_count(),
        1,
        "repeating an identical opaque gradient is a no-op"
    );

    let encoded = storage::encode(linear.state()).unwrap();
    let reopened = Engine::from_state(storage::decode(&encoded).unwrap()).unwrap();
    assert_eq!(reopened.rgba(), pixels);
    let png = path("gradient-png").with_extension("png");
    storage::export_png(&png, &linear).unwrap();
    let mut imported = Engine::new(5, 5, 300).unwrap();
    storage::import_png(&png, &mut imported).unwrap();
    assert_eq!(imported.rgba(), pixels);

    let mut radial = Engine::new(5, 5, 300).unwrap();
    radial.brush.color = [0, 0, 0];
    radial
        .draw_gradient(
            GradientKind::Radial,
            [2.5, 2.5],
            [4.5, 2.5],
            [255, 255, 255],
        )
        .unwrap();
    let radial_pixels = radial.rgba();
    assert_eq!(pixel(&radial_pixels, 2, 2), [0, 0, 0, 255]);
    assert_eq!(pixel(&radial_pixels, 4, 2), [255, 255, 255, 255]);
    assert_ne!(pixel(&radial_pixels, 3, 3), pixel(&pixels, 3, 3));

    assert!(radial.undo());
    assert!(blank(&radial));
    assert!(radial.redo());
    assert_eq!(radial.rgba(), radial_pixels);
}

#[test]
fn gradient_obeys_selection_layer_guards_opacity_and_resource_bounds() {
    let mut e = Engine::new(8, 4, 300).unwrap();
    e.brush.color = [0, 0, 0];
    e.set_selection(Some(
        Selection::new(SelectionKind::Rectangle, vec![[2.0, 1.0], [6.0, 3.0]]).unwrap(),
    ))
    .unwrap();
    e.draw_gradient(GradientKind::Linear, [0.5, 1.5], [7.5, 1.5], [255, 0, 0])
        .unwrap();
    let pixels = e.rgba();
    let pixel = |x: usize, y: usize| &pixels[(y * 8 + x) * 4..(y * 8 + x) * 4 + 4];
    assert_eq!(pixel(1, 1), &[255, 255, 255, 255]);
    assert_eq!(pixel(6, 1), &[255, 255, 255, 255]);
    assert_ne!(pixel(3, 1), &[255, 255, 255, 255]);
    assert_eq!(e.state().tile_count(), 1);
    assert!(e.state().tile_count() <= paint_document::MAX_TILES);

    e.edit(|state| {
        state.layers[0].locked = true;
        Ok(())
    })
    .unwrap();
    let before = e.rgba();
    let history = e.undo_count();
    assert!(matches!(
        e.draw_gradient(GradientKind::Radial, [2.5, 2.5], [6.5, 2.5], [0, 0, 255]),
        Err(PaintError::Locked)
    ));
    assert_eq!(e.rgba(), before);
    assert_eq!(e.undo_count(), history);

    let mut hidden = Engine::new(4, 4, 300).unwrap();
    hidden
        .edit(|state| {
            state.layers[0].visible = false;
            Ok(())
        })
        .unwrap();
    let before = hidden.rgba();
    let history = hidden.undo_count();
    assert!(matches!(
        hidden.draw_gradient(GradientKind::Linear, [0.5, 0.5], [3.5, 3.5], [255, 0, 0]),
        Err(PaintError::Locked)
    ));
    assert_eq!(hidden.rgba(), before);
    assert_eq!(hidden.undo_count(), history);

    let mut translucent = Engine::new(4, 2, 300).unwrap();
    translucent.brush.color = [255, 0, 0];
    translucent.brush.opacity = 0.5;
    translucent
        .draw_gradient(GradientKind::Linear, [0.5, 0.5], [3.5, 0.5], [0, 0, 255])
        .unwrap();
    let tile = translucent.state().layers[0].tiles.get(&(0, 0)).unwrap();
    assert_eq!(&tile[..4], &[128, 0, 0, 128]);

    let unchanged = translucent.rgba();
    let history = translucent.undo_count();
    assert!(matches!(
        translucent.draw_gradient(GradientKind::Linear, [1.0, 1.0], [1.0, 1.0], [0, 0, 0]),
        Err(PaintError::Invalid)
    ));
    assert_eq!(translucent.rgba(), unchanged);
    assert_eq!(translucent.undo_count(), history);
}

#[test]
fn gradients_clip_to_ellipse_and_lasso_pixel_centers() {
    let mut ellipse = Engine::new(8, 8, 300).unwrap();
    ellipse.brush.color = [0, 0, 0];
    ellipse
        .set_selection(Some(
            Selection::new(SelectionKind::Ellipse, vec![[1.0, 1.0], [7.0, 7.0]]).unwrap(),
        ))
        .unwrap();
    ellipse
        .draw_gradient(GradientKind::Linear, [0.5, 3.5], [7.5, 3.5], [255, 0, 0])
        .unwrap();
    let pixels = ellipse.rgba();
    let pixel = |image: &[u8], x: usize, y: usize| {
        let offset = (y * 8 + x) * 4;
        [
            image[offset],
            image[offset + 1],
            image[offset + 2],
            image[offset + 3],
        ]
    };
    assert_eq!(pixel(&pixels, 0, 3), [255, 255, 255, 255]);
    assert_ne!(pixel(&pixels, 3, 3), [255, 255, 255, 255]);
    assert_eq!(pixel(&pixels, 7, 3), [255, 255, 255, 255]);

    let mut lasso = Engine::new(8, 8, 300).unwrap();
    lasso.brush.color = [0, 0, 0];
    lasso
        .set_selection(Some(
            Selection::new(
                SelectionKind::Lasso,
                vec![[1.0, 1.0], [7.0, 1.0], [4.0, 7.0]],
            )
            .unwrap(),
        ))
        .unwrap();
    lasso
        .draw_gradient(GradientKind::Radial, [4.0, 1.0], [4.0, 7.0], [255, 0, 0])
        .unwrap();
    let pixels = lasso.rgba();
    assert_eq!(pixel(&pixels, 0, 2), [255, 255, 255, 255]);
    assert_ne!(pixel(&pixels, 4, 2), [255, 255, 255, 255]);
    assert_eq!(pixel(&pixels, 7, 2), [255, 255, 255, 255]);
}

#[test]
fn canceled_stabilized_path_never_reaches_authoritative_tiles() {
    let mut e = Engine::new(128, 128, 300).unwrap();
    e.brush.smoothing = 0.9;
    e.apply(&C::BeginStroke {
        stroke_id: StrokeId(5),
        sample: sample_at(10.0, 64.0, Some(1.0), 0),
    })
    .unwrap();
    e.apply(&C::UpdateStroke {
        stroke_id: StrokeId(5),
        sample: sample_at(80.0, 50.0, Some(1.0), 16),
    })
    .unwrap();
    assert_eq!(e.state().tile_count(), 0);
    cancel(&mut e, 5);
    assert!(blank(&e));
    assert_eq!(e.undo_count(), 0);
}

#[test]
fn line_rectangle_and_ellipse_are_persistent_undoable_raster_edits() {
    let mut e = Engine::new(128, 128, 300).unwrap();
    e.brush.size = 4.0;
    e.brush.pressure_size = false;
    e.brush.color = [220, 30, 80];

    e.draw_shape(ShapeKind::Line, [10.0, 10.0], [50.0, 50.0], false)
        .unwrap();
    e.brush.color = [20, 120, 230];
    e.draw_shape(ShapeKind::Rectangle, [60.0, 10.0], [100.0, 50.0], false)
        .unwrap();
    let line_and_rectangle = e.rgba();
    e.brush.color = [40, 180, 90];
    e.draw_shape(ShapeKind::Ellipse, [10.0, 60.0], [50.0, 100.0], true)
        .unwrap();

    let pixels = e.rgba();
    let pixel = |x: usize, y: usize| &pixels[(y * 128 + x) * 4..(y * 128 + x) * 4 + 4];
    assert!(pixel(30, 30)[0] > 150); // line
    assert!(pixel(60, 30)[2] > 150); // rectangle outline
    assert_eq!(pixel(80, 30), &[255; 4]);
    assert!(pixel(30, 80)[1] > 100); // ellipse fill
    assert_eq!(pixel(12, 62), &[255; 4]); // outside ellipse
    assert_eq!(e.undo_count(), 3);

    let encoded = storage::encode(e.state()).unwrap();
    let reopened = Engine::from_state(storage::decode(&encoded).unwrap()).unwrap();
    assert_eq!(reopened.rgba(), pixels);
    let png = path("shape-png").with_extension("png");
    storage::export_png(&png, &e).unwrap();
    let mut imported = Engine::new(128, 128, 300).unwrap();
    storage::import_png(&png, &mut imported).unwrap();
    assert_eq!(imported.rgba(), pixels);

    assert!(e.undo());
    assert_eq!(e.rgba(), line_and_rectangle);
    assert!(e.redo());
    assert_eq!(e.rgba(), pixels);
}

#[test]
fn shape_respects_layer_state_and_active_selection() {
    let mut e = Engine::new(64, 64, 300).unwrap();
    e.set_selection(Some(
        Selection::new(SelectionKind::Rectangle, vec![[16.0, 16.0], [48.0, 48.0]]).unwrap(),
    ))
    .unwrap();
    e.brush.size = 2.0;
    e.draw_shape(ShapeKind::Line, [0.0, 32.0], [64.0, 32.0], false)
        .unwrap();
    let pixels = e.rgba();
    assert_eq!(&pixels[(32 * 64 + 8) * 4..(32 * 64 + 8) * 4 + 4], &[255; 4]);
    assert_ne!(
        &pixels[(32 * 64 + 32) * 4..(32 * 64 + 32) * 4 + 4],
        &[255; 4]
    );
    e.edit(|state| {
        state.layers[0].locked = true;
        Ok(())
    })
    .unwrap();
    let before = e.rgba();
    assert!(matches!(
        e.draw_shape(ShapeKind::Rectangle, [20.0, 20.0], [40.0, 40.0], false),
        Err(PaintError::Locked)
    ));
    assert_eq!(e.rgba(), before);
}
#[test]
fn concurrent_cancellation_does_not_erase_other_commit() {
    let mut e = Engine::new(128, 128, 300).unwrap();
    begin(&mut e, 1, 20.0, 20.0);
    begin(&mut e, 2, 100.0, 100.0);
    end(&mut e, 1, 30.0, 20.0);
    cancel(&mut e, 2);
    assert_eq!(e.undo_count(), 1);
    assert_ne!(
        &e.rgba()[((20 * 128 + 20) * 4)..((20 * 128 + 20) * 4 + 4)],
        &[255; 4]
    );
    assert_eq!(
        &e.rgba()[((100 * 128 + 100) * 4)..((100 * 128 + 100) * 4 + 4)],
        &[255; 4]
    );
}
#[test]
fn zero_pressure_is_distinct_from_mouse_fallback() {
    let mut e = Engine::new(64, 64, 300).unwrap();
    e.brush.smoothing = 0.8;
    e.apply(&C::BeginStroke {
        stroke_id: StrokeId(0),
        sample: sample(32.0, 32.0, Some(0.0)),
    })
    .unwrap();
    assert!(blank(&e));
    cancel(&mut e, 0);
    e.apply(&C::BeginStroke {
        stroke_id: StrokeId(1),
        sample: sample(32.0, 32.0, None),
    })
    .unwrap();
    assert!(!blank(&e));
}
#[test]
fn presets_are_deterministic_distinct_and_versioned() {
    let mut outputs = std::collections::BTreeSet::new();
    for kind in [
        BrushKind::Round,
        BrushKind::Ink,
        BrushKind::Pencil,
        BrushKind::Marker,
        BrushKind::Airbrush,
        BrushKind::Soft,
        BrushKind::Textured,
        BrushKind::Calligraphy,
        BrushKind::Wash,
    ] {
        let b = Brush::preset(kind);
        assert_eq!(Brush::decode(&b.encode().unwrap()).unwrap(), b);
        let mut a = Engine::new(128, 128, 300).unwrap();
        a.brush = b.clone();
        let mut c = Engine::new(128, 128, 300).unwrap();
        c.brush = b;
        begin(&mut a, 0, 32.0, 64.0);
        end(&mut a, 0, 96.0, 64.0);
        begin(&mut c, 0, 32.0, 64.0);
        end(&mut c, 0, 96.0, 64.0);
        assert_eq!(a.rgba(), c.rgba());
        outputs.insert(a.rgba());
    }
    assert_eq!(outputs.len(), 9);
    assert!(Brush::decode("{\"version\":2,\"brush\":{}}").is_err());
}

#[test]
fn calligraphy_angle_setting_changes_the_rasterized_mark_orientation() {
    fn rasterized_bounds(angle: f64) -> [u32; 4] {
        let mut engine = Engine::new(64, 64, 300).unwrap();
        engine.brush = Brush::preset(BrushKind::Calligraphy);
        engine.brush.size = 24.0;
        engine.brush.color = [20, 30, 90];
        engine.brush.angle = angle;
        engine.brush.smoothing = 0.0;
        begin(&mut engine, 1, 32.0, 32.0);
        end(&mut engine, 1, 32.0, 32.0);

        let mut bounds = [u32::MAX, u32::MAX, 0, 0];
        for (index, pixel) in engine.rgba().chunks_exact(4).enumerate() {
            if pixel[..3] != [255, 255, 255] {
                let x = (index % 64) as u32;
                let y = (index / 64) as u32;
                bounds[0] = bounds[0].min(x);
                bounds[1] = bounds[1].min(y);
                bounds[2] = bounds[2].max(x);
                bounds[3] = bounds[3].max(y);
            }
        }
        assert!(
            bounds[0] != u32::MAX,
            "calligraphy stroke should paint pixels"
        );
        bounds
    }

    let horizontal = rasterized_bounds(0.0);
    let vertical = rasterized_bounds(std::f64::consts::FRAC_PI_2);
    let horizontal_width = horizontal[2] - horizontal[0] + 1;
    let horizontal_height = horizontal[3] - horizontal[1] + 1;
    let vertical_width = vertical[2] - vertical[0] + 1;
    let vertical_height = vertical[3] - vertical[1] + 1;
    assert!(horizontal_width > horizontal_height * 2);
    assert!(vertical_height > vertical_width * 2);
}

#[test]
fn brush_presets_roundtrip_with_bounded_no_clobber_storage() {
    let root = std::env::temp_dir().join(format!("rpl-brush-presets-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("ink.rplbrush");
    let mut brush = Brush::preset(BrushKind::Calligraphy);
    brush.color = [12, 34, 56];
    brush.pressure_opacity = true;

    storage::save_brush_preset(&path, &brush).unwrap();
    assert_eq!(storage::load_brush_preset(&path).unwrap(), brush);
    assert!(matches!(
        storage::save_brush_preset(&path, &Brush::default()),
        Err(PaintError::Conflict)
    ));

    std::fs::write(&path, vec![b' '; 16 * 1024 + 1]).unwrap();
    assert!(matches!(
        storage::load_brush_preset(&path),
        Err(PaintError::Limit)
    ));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn brush_preset_storage_rejects_wrong_extension_and_invalid_settings() {
    let root = std::env::temp_dir().join(format!("rpl-brush-invalid-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    assert!(matches!(
        storage::save_brush_preset(&root.join("brush.json"), &Brush::default()),
        Err(PaintError::Invalid)
    ));
    let path = root.join("bad.rplbrush");
    std::fs::write(&path, "{\"version\":99,\"brush\":{}} ").unwrap();
    assert!(storage::load_brush_preset(&path).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn erasing_and_layer_properties_are_undoable() {
    let mut e = Engine::new(64, 64, 300).unwrap();
    e.edit(|s| {
        s.background = None;
        Ok(())
    })
    .unwrap();
    begin(&mut e, 0, 32.0, 32.0);
    end(&mut e, 0, 32.0, 32.0);
    let painted = e.rgba();
    e.brush = Brush::preset(BrushKind::Eraser);
    begin(&mut e, 1, 32.0, 32.0);
    end(&mut e, 1, 32.0, 32.0);
    assert!(e.rgba()[(32 * 64 + 32) * 4 + 3] < painted[(32 * 64 + 32) * 4 + 3]);
    assert!(e.undo());
    assert_eq!(e.rgba(), painted);
    e.edit(|s| {
        s.layers[0].visible = false;
        Ok(())
    })
    .unwrap();
    assert!(e.rgba().iter().all(|p| *p == 0));
    assert!(e.undo());
    assert_eq!(e.rgba(), painted);
}
#[test]
fn layer_creation_duplicate_lock_reorder_delete_and_clear() {
    let mut e = Engine::new(64, 64, 300).unwrap();
    begin(&mut e, 0, 20.0, 20.0);
    end(&mut e, 0, 30.0, 20.0);
    e.duplicate_layer().unwrap();
    assert_eq!(e.state().layers.len(), 2);
    e.edit(|s| {
        s.layers.swap(0, 1);
        s.layers[0].opacity = 128;
        s.layers[0].blend = Blend::Multiply;
        s.layers[0].name = "Ink".into();
        Ok(())
    })
    .unwrap();
    e.edit(|s| {
        for l in &mut s.layers {
            l.locked = true;
        }
        Ok(())
    })
    .unwrap();
    assert!(matches!(
        e.apply(&C::BeginStroke {
            stroke_id: StrokeId(2),
            sample: sample(30.0, 30.0, None)
        }),
        Err(PaintError::Locked)
    ));
    assert!(e.clear_layer().is_err());
    e.delete_layer().unwrap();
    assert_eq!(e.state().layers.len(), 1);
    assert!(e.undo());
    assert_eq!(e.state().layers.len(), 2);
}

#[test]
fn clearing_a_targeted_layer_preserves_selection_and_undo_restores_its_pixels() {
    let mut e = Engine::new(64, 64, 300).unwrap();
    e.brush.color = [230, 35, 80];
    begin(&mut e, 0, 18.0, 22.0);
    end(&mut e, 0, 30.0, 22.0);
    let lower_id = e.state().selected;

    e.add_layer().unwrap();
    e.brush.color = [30, 90, 230];
    begin(&mut e, 1, 40.0, 42.0);
    end(&mut e, 1, 48.0, 42.0);
    let selected_id = e.state().selected;
    let before = e.rgba();
    let lower_pixels = e
        .state()
        .layers
        .iter()
        .find(|layer| layer.id == lower_id)
        .unwrap()
        .tiles
        .len();
    assert!(lower_pixels > 0);

    e.clear_layer_by_id(lower_id).unwrap();
    assert_eq!(e.state().selected, selected_id);
    assert!(
        e.state()
            .layers
            .iter()
            .find(|layer| layer.id == lower_id)
            .unwrap()
            .tiles
            .is_empty()
    );
    assert!(e.undo());
    assert_eq!(e.state().selected, selected_id);
    assert_eq!(e.rgba(), before);

    e.edit(|state| {
        state.layers[0].locked = true;
        Ok(())
    })
    .unwrap();
    assert!(matches!(
        e.clear_layer_by_id(lower_id),
        Err(PaintError::Locked)
    ));
}
#[test]
fn layer_opacity_and_blend_modes_have_expected_pixels_through_native_and_png_roundtrips() {
    use std::{collections::BTreeMap, sync::Arc};

    fn fixture(mode: Blend, opacity: u8) -> Engine {
        let mut state = State::new(64, 64, 300).unwrap();
        state.background = None;
        let pixel_index = ((19 * 64 + 17) * 4) as usize;
        let mut bottom = vec![0; paint_document::TILE_BYTES];
        bottom[pixel_index..pixel_index + 4].copy_from_slice(&[80, 120, 180, 255]);
        state.layers[0].tiles.insert((0, 0), Arc::new(bottom));

        let mut top = state.layers[0].clone();
        top.id = 2;
        top.name = "Blend sample".into();
        let mut pixels = vec![0; paint_document::TILE_BYTES];
        pixels[pixel_index..pixel_index + 4].copy_from_slice(&[100, 50, 25, 128]);
        top.tiles = BTreeMap::from([((0, 0), Arc::new(pixels))]);
        top.opacity = opacity;
        top.blend = mode;
        state.layers.push(top);
        state.selected = 2;
        Engine::from_state(state).unwrap()
    }

    let samples = [
        (Blend::Normal, [108, 113, 145, 255]),
        (Blend::Multiply, [74, 100, 142, 255]),
        (Blend::Screen, [113, 132, 182, 255]),
        (Blend::Overlay, [90, 112, 167, 255]),
    ];
    let mut actual = Vec::new();
    for (mode, expected) in samples {
        let engine = fixture(mode, 128);
        let offset = (19 * 64 + 17) * 4;
        let pixels = engine.rgba();
        let sample: [u8; 4] = pixels[offset..offset + 4].try_into().unwrap();
        assert_eq!(sample, expected, "{mode:?} layer output");
        assert!(
            pixels.chunks_exact(4).all(|pixel| {
                pixel[0] <= pixel[3] && pixel[1] <= pixel[3] && pixel[2] <= pixel[3]
            })
        );
        actual.push(sample);
        engine.state().validate().unwrap();

        let native = path(&format!("blend-roundtrip-{mode:?}"));
        let mut store = Store::new(native.clone());
        store.save(engine.state()).unwrap();
        let (_store, state) = Store::open(native.clone()).unwrap();
        let reopened = Engine::from_state(state).unwrap();
        assert_eq!(&reopened.rgba()[offset..offset + 4], &expected);

        let png = native.with_extension("png");
        storage::export_png(&png, &reopened).unwrap();
        let mut imported = Engine::new(64, 64, 300).unwrap();
        imported
            .edit(|state| {
                state.background = None;
                Ok(())
            })
            .unwrap();
        storage::import_png(&png, &mut imported).unwrap();
        assert_eq!(&imported.rgba()[offset..offset + 4], &expected);
    }
    actual.sort_unstable();
    actual.dedup();
    assert_eq!(actual.len(), 4, "each blend mode must visibly differ");

    let transparent = fixture(Blend::Overlay, 0);
    let offset = (19 * 64 + 17) * 4;
    assert_eq!(
        &transparent.rgba()[offset..offset + 4],
        &[80, 120, 180, 255],
        "zero layer opacity leaves the underlying layer unchanged"
    );
}
#[test]
fn sparse_4096_document_is_not_allocated_until_painted_and_dirty_tiles_are_local() {
    let mut e = Engine::new(4096, 4096, 300).unwrap();
    e.take_damage();
    assert_eq!(e.state().tile_count(), 0);
    begin(&mut e, 0, 100.0, 100.0);
    end(&mut e, 0, 110.0, 110.0);
    assert!(e.state().tile_count() < 10);
    assert!(e.take_damage().len() < 10);
}
#[test]
fn history_is_bounded_and_invalid_edits_do_not_replace_document() {
    let mut e = Engine::new(64, 64, 300).unwrap();
    for _ in 0..100 {
        e.edit(|s| {
            s.background = Some([100; 3]);
            Ok(())
        })
        .unwrap();
    }
    assert!(e.undo_count() <= 32);
    assert!(
        e.edit(|s| {
            s.width = u32::MAX;
            Ok(())
        })
        .is_err()
    );
    assert_eq!(e.state().width, 64);
    assert!(State::new(0, 1, 300).is_err());
    assert!(State::new(4097, 64, 300).is_err());
}
#[test]
fn editable_roundtrip_checksums_and_corruption_limits() {
    let mut e = Engine::new(128, 128, 300).unwrap();
    begin(&mut e, 0, 10.0, 30.0);
    end(&mut e, 0, 100.0, 80.0);
    e.add_layer().unwrap();
    let bytes = storage::encode(e.state()).unwrap();
    let loaded = Engine::from_state(storage::decode(&bytes).unwrap()).unwrap();
    assert_eq!(e.rgba(), loaded.rgba());
    assert_eq!(loaded.state().layers.len(), 2);
    for n in [0, 8, 12, 32, bytes.len() - 1] {
        assert!(storage::decode(&bytes[..n]).is_err());
    }
    let mut bad = bytes.clone();
    bad[40] ^= 1;
    assert!(storage::decode(&bad).is_err());
    assert!(storage::decode(&vec![0; 1024]).is_err());
}
#[test]
fn interrupted_saves_preserve_previous_valid_artwork_and_backup() {
    for fault in [Fault::AfterWrite, Fault::BeforeReplace] {
        let p = path(if fault == Fault::AfterWrite {
            "after"
        } else {
            "before"
        });
        let mut store = Store::new(p.clone());
        let mut e = Engine::new(64, 64, 300).unwrap();
        store.save(e.state()).unwrap();
        let old = std::fs::read(&p).unwrap();
        begin(&mut e, 0, 20.0, 20.0);
        end(&mut e, 0, 40.0, 40.0);
        assert!(store.save_fault(e.state(), fault).is_err());
        assert_eq!(std::fs::read(&p).unwrap(), old);
        storage::load(&p).unwrap();
        store.save(e.state()).unwrap();
        assert_eq!(
            storage::load(&p).unwrap().tile_count(),
            e.state().tile_count()
        );
        assert_eq!(std::fs::read(p.with_file_name("art.rpl.bak")).unwrap(), old);
    }
}
#[test]
fn save_conflicts_are_not_silently_overwritten() {
    let p = path("conflict");
    let mut store = Store::new(p.clone());
    let e = Engine::new(64, 64, 300).unwrap();
    store.save(e.state()).unwrap();
    let mut stranger = Store::new(p);
    assert!(matches!(
        stranger.save(e.state()),
        Err(PaintError::Conflict)
    ));
}
#[test]
fn png_export_import_keeps_dimensions_alpha_and_visible_pixels() {
    let p = path("png").with_extension("png");
    let mut e = Engine::new(64, 64, 300).unwrap();
    begin(&mut e, 0, 10.0, 20.0);
    end(&mut e, 0, 50.0, 40.0);
    storage::export_png(&p, &e).unwrap();
    let mut loaded = Engine::new(64, 64, 300).unwrap();
    storage::import_png(&p, &mut loaded).unwrap();
    assert_eq!(e.rgba(), loaded.rgba());
    assert_eq!(loaded.state().layers.len(), 2);
}

#[test]
fn recovery_background_roundtrip_skips_active_owners_and_joins_latest_checkpoint() {
    let base = path("recovery");
    let dir = base.parent().unwrap();
    let mut e = Engine::new(64, 64, 300).unwrap();
    let recovery = paint_document::recovery::Recovery::start(dir).unwrap();
    let saved = recovery.path.clone();
    assert!(paint_document::recovery::available(dir).is_empty());
    begin(&mut e, 0, 20.0, 20.0);
    end(&mut e, 0, 40.0, 40.0);
    recovery.flush(e.state().clone()).unwrap();
    drop(recovery);
    assert!(paint_document::recovery::available(dir).contains(&saved));
    assert_eq!(
        storage::load(&saved).unwrap().tile_count(),
        e.state().tile_count()
    );
}

#[test]
fn stroke_opacity_caps_accumulation_while_flow_builds_up() {
    let mut capped = Engine::new(64, 64, 300).unwrap();
    capped
        .edit(|s| {
            s.background = None;
            Ok(())
        })
        .unwrap();
    capped.brush.pressure_size = false;
    capped.brush.pressure_opacity = false;
    capped.brush.opacity = 0.25;
    capped.brush.flow = 1.0;
    begin(&mut capped, 1, 32.0, 32.0);
    end(&mut capped, 1, 32.0, 32.0);
    let mut flow = Engine::new(64, 64, 300).unwrap();
    flow.edit(|s| {
        s.background = None;
        Ok(())
    })
    .unwrap();
    flow.brush = capped.brush.clone();
    flow.brush.opacity = 1.0;
    flow.brush.flow = 0.25;
    begin(&mut flow, 1, 32.0, 32.0);
    end(&mut flow, 1, 32.0, 32.0);
    let i = (32 * 64 + 32) * 4 + 3;
    assert!(capped.rgba()[i] <= 64);
    assert!(flow.rgba()[i] > capped.rgba()[i]);
}
#[test]
fn smoothing_connects_to_the_final_unfiltered_endpoint() {
    let mut e = Engine::new(128, 64, 300).unwrap();
    e.brush.smoothing = 1.0;
    e.brush.size = 4.0;
    e.brush.pressure_size = false;
    begin(&mut e, 1, 10.0, 32.0);
    end(&mut e, 1, 110.0, 32.0);
    for x in 10..110 {
        assert_ne!(
            &e.rgba()[(32 * 128 + x) * 4..(32 * 128 + x) * 4 + 4],
            &[255; 4],
            "stroke has a horizontal gap at x={x}"
        );
    }
}

#[test]
fn history_accounts_shared_tile_allocations_once() {
    let mut e = Engine::new(128, 128, 300).unwrap();
    begin(&mut e, 1, 32.0, 32.0);
    end(&mut e, 1, 32.0, 32.0);
    for opacity in 0..40 {
        e.edit(|s| {
            s.layers[0].opacity = opacity + 1;
            Ok(())
        })
        .unwrap();
    }
    assert_eq!(e.undo_count(), 32);
    assert_eq!(
        e.history_bytes()[0],
        e.state().tile_count() * paint_document::TILE_BYTES
    );
    assert!(
        e.history_bytes()
            .iter()
            .all(|n| *n <= paint_document::HISTORY_BYTES)
    );
    let saved = e.rgba();
    for _ in 0..32 {
        assert!(e.undo());
    }
    for _ in 0..32 {
        assert!(e.redo());
    }
    assert_eq!(e.rgba(), saved);
}
#[test]
fn checksum_valid_malformed_metadata_and_mutated_inputs_are_rejected_without_panics() {
    use sha2::{Digest, Sha256};
    let bytes = storage::encode(&State::new(64, 64, 300).unwrap()).unwrap();
    let n = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    let metadata: serde_json::Value = serde_json::from_slice(&bytes[12..12 + n]).unwrap();
    for (key, value) in [
        ("width", serde_json::json!(4294967295u64)),
        ("height", serde_json::json!(0)),
        ("layers", serde_json::json!([])),
        ("selected", serde_json::json!(99)),
        ("future_feature", serde_json::json!(true)),
    ] {
        let mut m = metadata.clone();
        m[key] = value;
        let m = serde_json::to_vec(&m).unwrap();
        let mut bad = b"RPLALP01".to_vec();
        bad.extend_from_slice(&(m.len() as u32).to_le_bytes());
        bad.extend(m);
        bad.extend_from_slice(&0u32.to_le_bytes());
        bad.extend(Sha256::digest(&bad));
        assert!(storage::decode(&bad).is_err());
    }
    // Deterministic mutation corpus exercises both checksum and structural guards.
    for i in 0..512 {
        let mut bad = bytes.clone();
        let len = bad.len();
        bad[i % len] ^= (i as u8).wrapping_add(1);
        let _ = storage::decode(&bad);
    }
}
#[test]
fn png_refuses_overwriting_artwork_or_an_existing_export() {
    let p = path("png-protect");
    let e = Engine::new(64, 64, 300).unwrap();
    let mut store = Store::new(p.clone());
    store.save(e.state()).unwrap();
    let old = std::fs::read(&p).unwrap();
    assert!(storage::export_png(&p, &e).is_err());
    assert_eq!(std::fs::read(&p).unwrap(), old);
    let png = p.with_extension("png");
    storage::export_png(&png, &e).unwrap();
    let old = std::fs::read(&png).unwrap();
    assert!(storage::export_png(&png, &e).is_err());
    assert_eq!(std::fs::read(png).unwrap(), old);
}
#[test]
fn cooperative_writer_lock_preserves_existing_project() {
    let p = path("writer-lock");
    let mut store = Store::new(p.clone());
    let e = Engine::new(64, 64, 300).unwrap();
    store.save(e.state()).unwrap();
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(p.with_file_name("art.rpl.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(matches!(store.save(e.state()), Err(PaintError::Busy)));
    drop(lock);
    store.save(e.state()).unwrap();
}

#[test]
fn open_tracks_actual_bytes_of_valid_noncanonical_metadata() {
    use sha2::{Digest, Sha256};
    let p = path("noncanonical");
    let data = storage::encode(&State::new(64, 64, 300).unwrap()).unwrap();
    let n = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    let metadata: serde_json::Value = serde_json::from_slice(&data[12..12 + n]).unwrap();
    let metadata = serde_json::to_vec_pretty(&metadata).unwrap();
    let mut alternate = b"RPLALP01".to_vec();
    alternate.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    alternate.extend(metadata);
    alternate.extend_from_slice(&0u32.to_le_bytes());
    alternate.extend(Sha256::digest(&alternate));
    std::fs::write(&p, alternate).unwrap();
    let (mut store, state) = Store::open(p).unwrap();
    store.save(&state).unwrap();
}

#[test]
fn independent_tilt_axes_rotate_calligraphy_and_unsupported_tilt_is_valid() {
    let render = |tilt| {
        let mut e = Engine::new(64, 64, 300).unwrap();
        e.brush = Brush::preset(BrushKind::Calligraphy);
        let mut s = sample(32.0, 32.0, Some(1.0));
        s.tilt_degrees = tilt;
        e.apply(&C::BeginStroke {
            stroke_id: StrokeId(1),
            sample: s,
        })
        .unwrap();
        e.rgba()
    };
    assert_ne!(
        render([Some(30.0), Some(0.0)]),
        render([Some(0.0), Some(30.0)])
    );
    assert_ne!(render([None, None]), render([None, Some(30.0)]));
}

#[test]
fn brush_setting_changes_become_dirty_revisioned_document_metadata_once() {
    let mut engine = Engine::new(64, 64, 300).unwrap();
    assert!(!engine.dirty());
    engine.brush.size = 37.0;
    engine.sync_brush_settings();
    assert_eq!(engine.revision, 1);
    assert!(engine.dirty());
    assert_eq!(engine.state().brush.size, 37.0);
    engine.sync_brush_settings();
    assert_eq!(engine.revision, 1);
    engine.saved_revision = engine.revision;
    assert!(!engine.dirty());
}

#[test]
fn no_op_document_edit_does_not_pollute_history_or_mark_artwork_dirty() {
    let mut engine = Engine::new(64, 64, 300).unwrap();
    let revision = engine.revision;
    let undo_count = engine.undo_count();
    let saved = engine.state().clone();

    engine.edit(|_| Ok(())).unwrap();

    assert_eq!(engine.state(), &saved);
    assert_eq!(engine.revision, revision);
    assert_eq!(engine.undo_count(), undo_count);
    assert!(!engine.dirty());
}

#[test]
fn home_thumbnail_is_deterministic_transparent_and_bounded() {
    let mut state = State::new(4096, 2048, 300).unwrap();
    state.background = None;
    let engine = Engine::from_state(state).unwrap();
    let (width, height, pixels) = engine.thumbnail_rgba(256, 160).unwrap();
    assert_eq!((width, height), (256, 128));
    assert_eq!(pixels.len(), width as usize * height as usize * 4);
    assert!(pixels.chunks_exact(4).all(|pixel| pixel == [0, 0, 0, 0]));
    assert_eq!(
        engine.thumbnail_rgba(256, 160).unwrap(),
        (width, height, pixels)
    );
    assert!(engine.thumbnail_rgba(0, 160).is_err());
    assert!(engine.thumbnail_rgba(513, 160).is_err());
    let solid = Engine::new(12, 6, 96).unwrap();
    let (width, height, pixels) = solid.thumbnail_rgba(256, 160).unwrap();
    assert_eq!((width, height), (12, 6));
    assert!(
        pixels
            .chunks_exact(4)
            .all(|pixel| pixel == [255, 255, 255, 255])
    );
    let mut active = Engine::new(32, 32, 96).unwrap();
    active
        .apply(&C::BeginStroke {
            stroke_id: StrokeId(88),
            sample: sample(12.0, 12.0, Some(0.5)),
        })
        .unwrap();
    assert!(matches!(
        active.thumbnail_rgba(64, 64),
        Err(PaintError::Busy)
    ));
}
