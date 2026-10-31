use paint_document::{Blend, Engine, openraster};
use std::io::{Read, Write};
fn path(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("rpl-ora-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("test.ora")
}
#[test]
fn layered_roundtrip_keeps_order_names_visibility_opacity_blend_and_pixels() {
    let mut e = Engine::new(32, 32, 300).unwrap();
    e.edit(|s| {
        s.background = None;
        Ok(())
    })
    .unwrap();
    e.brush.color = [220, 40, 20];
    e.fill_selection(false).unwrap();
    e.add_layer().unwrap();
    e.brush.color = [20, 150, 240];
    e.fill_selection(false).unwrap();
    e.edit(|s| {
        s.layers[0].name = "A & <\"B\">".into();
        s.layers[1].opacity = 128;
        s.layers[1].blend = Blend::Multiply;
        Ok(())
    })
    .unwrap();
    let p = path("roundtrip");
    openraster::export(&p, &e).unwrap();
    let state = openraster::load(&p).unwrap();
    assert_eq!(state.layers.len(), 2);
    for (a, b) in e.state().layers.iter().zip(&state.layers) {
        assert_eq!(
            (&a.name, a.visible, a.opacity, a.blend),
            (&b.name, b.visible, b.opacity, b.blend)
        );
    }
    assert_eq!(Engine::from_state(state).unwrap().rgba(), e.rgba());
    assert!(openraster::export(&p, &e).is_err());
}
#[test]
fn background_becomes_separate_editable_bottom_layer() {
    let e = Engine::new(32, 32, 144).unwrap();
    let p = path("background");
    openraster::export(&p, &e).unwrap();
    let state = openraster::load(&p).unwrap();
    assert_eq!(state.dpi, 144);
    assert_eq!(state.layers.len(), 2);
    assert!(state.background.is_none());
    assert_eq!(Engine::from_state(state).unwrap().rgba(), e.rgba());
}
fn rewrite_stack(path: &std::path::Path, xml: &str) {
    let f = std::fs::File::open(path).unwrap();
    let mut archive = zip::ZipArchive::new(f).unwrap();
    let mut entries = Vec::new();
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).unwrap();
        let name = file.name().unwrap().into_owned();
        let mut data = Vec::new();
        file.read_to_end(&mut data).unwrap();
        entries.push((name, data));
    }
    drop(archive);
    let f = std::fs::File::create(path).unwrap();
    let mut writer = zip::ZipWriter::new(f);
    for (name, data) in entries {
        writer
            .start_file(
                &name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        writer
            .write_all(if name == "stack.xml" {
                xml.as_bytes()
            } else {
                &data
            })
            .unwrap();
    }
    writer.finish().unwrap();
}
#[test]
fn unsupported_groups_offsets_and_blends_fail_instead_of_flattening() {
    let e = Engine::new(32, 32, 300).unwrap();
    for (name, layer) in [
        ("group", "<stack/>"),
        ("offset", "<layer src=\"data/layer0.png\" x=\"1\"/>"),
        (
            "blend",
            "<layer src=\"data/layer0.png\" composite-op=\"svg:color-dodge\"/>",
        ),
        ("traversal", "<layer src=\"data/../other.png\"/>"),
    ] {
        let p = path(name);
        openraster::export(&p, &e).unwrap();
        rewrite_stack(
            &p,
            &format!("<image w=\"32\" h=\"32\"><stack>{layer}</stack></image>"),
        );
        assert!(openraster::load(&p).is_err());
    }
}
#[test]
fn corrupt_zip_and_unbounded_directory_counts_are_refused() {
    let e = Engine::new(32, 32, 300).unwrap();
    let p = path("malformed");
    openraster::export(&p, &e).unwrap();
    let bytes = std::fs::read(&p).unwrap();
    for n in [0, 1, 10, bytes.len() - 1] {
        std::fs::write(&p, &bytes[..n]).unwrap();
        assert!(openraster::load(&p).is_err());
    }
    let mut bad = bytes.clone();
    let n = bad.len();
    bad[n - 12..n - 10].copy_from_slice(&65535u16.to_le_bytes());
    std::fs::write(&p, &bad).unwrap();
    assert!(openraster::load(&p).is_err());
}
#[test]
fn unsafe_xml_and_invalid_dimensions_are_rejected() {
    let e = Engine::new(32, 32, 300).unwrap();
    for (name, xml) in [
        (
            "dtd",
            "<!DOCTYPE image [<!ENTITY a SYSTEM 'file:///etc/passwd'>]><image w=\"32\" h=\"32\"><stack>&a;</stack></image>",
        ),
        (
            "size",
            "<image w=\"4294967295\" h=\"32\"><stack><layer src=\"data/layer0.png\"/></stack></image>",
        ),
    ] {
        let p = path(name);
        openraster::export(&p, &e).unwrap();
        rewrite_stack(&p, xml);
        assert!(openraster::load(&p).is_err());
    }
}

#[test]
fn alpha_conversion_roundtrip_preserves_premultiplied_channels() {
    let mut e = Engine::new(64, 64, 300).unwrap();
    e.edit(|s| {
        s.background = None;
        let mut tile = vec![0; paint_document::TILE_BYTES];
        for a in 0..256usize {
            tile[a * 4] = (a * 127 / 255) as u8;
            tile[a * 4 + 1] = (a * 200 / 255) as u8;
            tile[a * 4 + 3] = a as u8;
        }
        s.layers[0].tiles.insert((0, 0), std::sync::Arc::new(tile));
        Ok(())
    })
    .unwrap();
    let p = path("alpha");
    openraster::export(&p, &e).unwrap();
    let state = openraster::load(&p).unwrap();
    assert_eq!(state.layers[0].tiles, e.state().layers[0].tiles);
}
#[test]
fn invalid_xml_names_and_excessive_layers_are_not_published() {
    let mut e = Engine::new(32, 32, 300).unwrap();
    e.edit(|s| {
        s.layers[0].name = "invalid\0name".into();
        Ok(())
    })
    .unwrap();
    let p = path("xmlname");
    assert!(openraster::export(&p, &e).is_err());
    assert!(!p.exists());
    let mut many = Engine::new(32, 32, 300).unwrap();
    for _ in 1..paint_document::MAX_LAYERS {
        many.add_layer().unwrap();
    }
    assert!(openraster::export(&p, &many).is_err());
}
