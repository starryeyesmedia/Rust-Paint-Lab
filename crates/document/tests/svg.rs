use paint_document::{Engine, PaintError, TILE_BYTES, svg};
use std::{fs, path::PathBuf, sync::Arc};

fn temp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("rpl-svg-{}-{name}", std::process::id()))
}

#[test]
fn svg_export_keeps_editable_cubic_and_fill_geometry_with_explicit_omissions() {
    let mut engine = Engine::new(96, 80, 300).unwrap();
    let path_id = engine
        .add_smoothed_vector_path(&[[8.0, 8.0], [40.0, 12.0], [72.0, 64.0]])
        .unwrap();
    engine.set_vector_path_closed(path_id, true).unwrap();
    engine
        .set_vector_path_fill(path_id, Some([40, 120, 220, 192]))
        .unwrap();
    engine
        .edit(|state| {
            let layer = state
                .layers
                .iter_mut()
                .find(|layer| layer.id == state.selected)
                .unwrap();
            layer.name = "Ink <& violet".into();
            layer.opacity = 200;
            layer.blend = paint_document::Blend::Multiply;
            layer.tiles.insert((0, 0), Arc::new(vec![0; TILE_BYTES]));
            layer.mask = Some(paint_document::LayerMask::default());
            Ok(())
        })
        .unwrap();

    let (text, report) = svg::encode(engine.state()).unwrap();
    assert_eq!(report.vector_paths, 1);
    assert_eq!(report.raster_layers_omitted, 1);
    assert_eq!(report.masked_layers_omitted, 1);
    assert_eq!(report.non_normal_blend_layers_omitted, 1);
    assert!(report.background_omitted);
    assert!(report.has_omissions());

    let xml = roxmltree::Document::parse(&text).unwrap();
    let root = xml.root_element();
    assert_eq!(root.tag_name().name(), "svg");
    assert_eq!(root.attribute("viewBox"), Some("0 0 96 80"));
    let layer = root.children().find(|node| node.has_tag_name("g")).unwrap();
    assert_eq!(layer.attribute("data-rpl-layer"), Some("1"));
    assert_eq!(layer.attribute("data-rpl-name"), Some("Ink <& violet"));
    assert_eq!(layer.attribute("opacity"), Some("0.7843137254901961"));
    let path = layer
        .children()
        .find(|node| node.has_tag_name("path"))
        .unwrap();
    assert_eq!(path.attribute("data-rpl-id"), Some("1"));
    assert!(path.attribute("d").unwrap().contains(" C "));
    assert!(path.attribute("d").unwrap().ends_with(" Z"));
    assert_eq!(path.attribute("fill"), Some("#2878dc"));
    assert_eq!(path.attribute("fill-rule"), Some("evenodd"));
    assert_eq!(path.attribute("fill-opacity"), Some("0.7529411764705882"));
    assert_eq!(path.attribute("stroke-linecap"), Some("round"));
    assert_eq!(path.attribute("stroke-linejoin"), Some("round"));
    assert!(!text.contains("<image"), "SVG export is vector-only");
}

#[test]
fn svg_export_writes_without_replacing_existing_files() {
    let mut engine = Engine::new(24, 16, 300).unwrap();
    let path_id = engine.add_vector_path(&[[2.0, 3.0], [18.0, 12.0]]).unwrap();
    let root = temp("export");
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    let output = root.join("editable.svg");

    let report = svg::export(&output, engine.state()).unwrap();
    assert_eq!(report.vector_paths, 1);
    let bytes = fs::read(&output).unwrap();
    assert!(bytes.starts_with(b"<?xml"));
    let reopened = roxmltree::Document::parse(std::str::from_utf8(&bytes).unwrap()).unwrap();
    assert_eq!(
        reopened
            .root_element()
            .children()
            .find(|node| node.has_tag_name("g"))
            .unwrap()
            .children()
            .find(|node| node.has_tag_name("path"))
            .unwrap()
            .attribute("data-rpl-id"),
        Some("1")
    );
    assert!(matches!(
        svg::export(&output, engine.state()),
        Err(PaintError::Conflict)
    ));
    assert_eq!(fs::read(&output).unwrap(), bytes);
    assert!(matches!(
        svg::export(&root.join("editable.png"), engine.state()),
        Err(PaintError::Invalid)
    ));
    assert!(engine.state().layers[0].vector_paths[0].id == path_id);

    fs::remove_dir_all(root).unwrap();
}

#[test]
fn svg_export_reports_solid_background_omission_but_not_transparency() {
    let mut engine = Engine::new(24, 16, 300).unwrap();
    let (_, report) = svg::encode(engine.state()).unwrap();
    assert!(report.background_omitted);
    assert!(report.has_omissions());

    engine
        .edit(|state| {
            state.background = None;
            Ok(())
        })
        .unwrap();
    let (_, report) = svg::encode(engine.state()).unwrap();
    assert!(!report.background_omitted);
    assert!(!report.has_omissions());
}
