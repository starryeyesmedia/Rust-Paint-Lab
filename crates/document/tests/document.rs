use paint_document::{
    Blend, Engine, PaintError, State,
    brush::{Brush, BrushKind},
    storage::{self, Fault, Store},
};
use paint_input::{Buttons, Tool};
use paint_stroke::{
    CancelReason, StrokeCommand as C, StrokeId, StrokeSample, coordinates::DocumentPoint,
};
fn sample(x: f64, y: f64, p: Option<f32>) -> StrokeSample {
    StrokeSample {
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
fn blend_modes_and_transparent_alpha_preserve_invariants() {
    for mode in [
        Blend::Normal,
        Blend::Multiply,
        Blend::Screen,
        Blend::Overlay,
    ] {
        let mut e = Engine::new(64, 64, 300).unwrap();
        e.edit(|s| {
            s.background = None;
            Ok(())
        })
        .unwrap();
        begin(&mut e, 0, 32.0, 32.0);
        end(&mut e, 0, 32.0, 32.0);
        e.add_layer().unwrap();
        e.brush.color = [240, 50, 20];
        e.brush.opacity = 0.5;
        begin(&mut e, 1, 32.0, 32.0);
        end(&mut e, 1, 32.0, 32.0);
        e.edit(|s| {
            s.layers[1].blend = mode;
            Ok(())
        })
        .unwrap();
        e.state().validate().unwrap();
        let tile = e.composite_tile((0, 0));
        assert!(
            tile.chunks_exact(4)
                .all(|p| p[0] <= p[3] && p[1] <= p[3] && p[2] <= p[3])
        );
    }
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
            &[255; 4]
        );
    }
}
