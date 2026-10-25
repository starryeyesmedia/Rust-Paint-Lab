//! Functional alpha controls. No widget is presented without an implemented action.
use crate::{
    gpu::{Failure, Renderer},
    interaction::{CanvasSession, MouseEvent},
    scene::CanvasMapping,
};
use paint_document::{
    Blend, Engine,
    brush::{Brush, BrushKind},
    editing::{Selection, SelectionKind},
    recovery::Recovery,
    storage::{self, Store},
};
use std::{path::PathBuf, time::Instant};
use winit::window::Window;

struct ReferenceImage {
    layout: crate::workspace::ReferenceLayout,
    texture: egui::TextureHandle,
    source_size: [u32; 2],
    viewport: Option<egui::Rect>,
    touches: crate::navigation::TouchNavigation,
}

#[derive(Clone)]
struct RecentProjectCard {
    path: PathBuf,
    bytes: Option<u64>,
    dimensions: Option<(u32, u32, u32)>,
    opened_unix_seconds: u64,
}

type AppearanceKey = (
    crate::workspace::AppearanceMode,
    crate::workspace::AccentScheme,
    Option<[u8; 3]>,
    bool,
    bool,
    u32,
);

fn recent_project_cards(recent: &paint_document::recent::Recent) -> Vec<RecentProjectCard> {
    recent
        .paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let metadata = std::fs::metadata(path)
                .ok()
                .filter(|metadata| metadata.is_file());
            RecentProjectCard {
                path: path.clone(),
                bytes: metadata.map(|metadata| metadata.len()),
                dimensions: storage::peek_dimensions(path),
                opened_unix_seconds: recent.opened_unix_seconds.get(index).copied().unwrap_or(0),
            }
        })
        .collect()
}

fn recent_opened_label(timestamp: u64) -> String {
    if timestamp == 0 {
        return "Opened date unavailable".into();
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    let days = now.saturating_sub(timestamp) / 86_400;
    match days {
        0 => "Opened today".into(),
        1 => "Opened yesterday".into(),
        days => format!("Opened {days} days ago"),
    }
}

const MAX_REFERENCE_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REFERENCE_SOURCE_EDGE: u32 = 16_384;
const MAX_REFERENCE_DECODE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_REFERENCE_PREVIEW_EDGE: u32 = 1_448;
const MAX_REFERENCE_TEXTURE_BYTES: usize = 8 * 1024 * 1024;

fn load_reference_texture(
    context: &egui::Context,
    layout: crate::workspace::ReferenceLayout,
) -> Result<ReferenceImage, String> {
    let metadata = std::fs::metadata(&layout.path).map_err(|e| e.to_string())?;
    if metadata.len() > MAX_REFERENCE_FILE_BYTES {
        return Err(
            "reference file exceeds 32 MiB; resize or export a smaller JPEG/PNG and try again"
                .into(),
        );
    }
    let mut reader = image::ImageReader::open(&layout.path)
        .and_then(|reader| reader.with_guessed_format())
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(MAX_REFERENCE_SOURCE_EDGE);
    limits.max_image_height = Some(MAX_REFERENCE_SOURCE_EDGE);
    limits.max_alloc = Some(MAX_REFERENCE_DECODE_BYTES);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|error| match error {
        image::ImageError::Limits(_) => format!(
            "Reference exceeds safe decode limits ({} px per edge / {} MiB source memory); resize it or export a smaller JPEG/PNG",
            MAX_REFERENCE_SOURCE_EDGE,
            MAX_REFERENCE_DECODE_BYTES / (1024 * 1024)
        ),
        error => error.to_string(),
    })?;
    let source_size = [decoded.width(), decoded.height()];
    if source_size.contains(&0) {
        return Err("Reference image has an empty dimension".into());
    }
    let preview = if source_size[0] > MAX_REFERENCE_PREVIEW_EDGE
        || source_size[1] > MAX_REFERENCE_PREVIEW_EDGE
    {
        decoded.thumbnail(MAX_REFERENCE_PREVIEW_EDGE, MAX_REFERENCE_PREVIEW_EDGE)
    } else {
        decoded
    };
    let rgba = preview.into_rgba8();
    let (width, height) = (rgba.width() as usize, rgba.height() as usize);
    let decoded_bytes = width
        .checked_mul(height)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| "Reference preview dimensions overflow the safe limit".to_string())?;
    if decoded_bytes > MAX_REFERENCE_TEXTURE_BYTES {
        return Err("Reference preview exceeds the 8 MiB GPU texture budget".into());
    }
    let image = egui::ColorImage::from_rgba_unmultiplied([width, height], rgba.as_raw());
    let name = layout
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("reference image");
    let texture = context.load_texture(name, image, egui::TextureOptions::LINEAR);
    Ok(ReferenceImage {
        layout,
        texture,
        source_size,
        viewport: None,
        touches: Default::default(),
    })
}

fn add_reference_path(
    context: &egui::Context,
    references: &mut Vec<ReferenceImage>,
    path: PathBuf,
) -> Result<(), String> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "reference image".into());
    if references.len() >= crate::workspace::MAX_REFERENCES {
        return Err(format!(
            "{name}: the reference board is full ({} images maximum)",
            crate::workspace::MAX_REFERENCES
        ));
    }
    if references
        .iter()
        .any(|reference| reference.layout.path == path)
    {
        return Err(format!(
            "{name}: this image is already on the reference board"
        ));
    }
    let index = references.len();
    let layout = crate::workspace::ReferenceLayout {
        path,
        position: [40.0 + index as f32 * 28.0, 80.0 + index as f32 * 24.0],
        size: [360.0, 300.0],
        opacity: 0.72,
        view_zoom: 1.0,
        view_pan: [0.0, 0.0],
        position_locked: false,
    };
    load_reference_texture(context, layout)
        .map(|reference| references.push(reference))
        .map_err(|error| format!("{name}: {error}"))
}

fn reference_contains_client(
    layout: &crate::workspace::ReferenceLayout,
    client_px: [f64; 2],
    pixels_per_point: f32,
) -> bool {
    if !pixels_per_point.is_finite() || pixels_per_point <= 0.0 {
        return false;
    }
    let point = [
        client_px[0] / f64::from(pixels_per_point),
        client_px[1] / f64::from(pixels_per_point),
    ];
    point.iter().all(|value| value.is_finite())
        && layout.position.iter().all(|value| value.is_finite())
        && layout
            .size
            .iter()
            .all(|value| value.is_finite() && *value > 0.0)
        && point[0] >= f64::from(layout.position[0])
        && point[1] >= f64::from(layout.position[1])
        && point[0] <= f64::from(layout.position[0] + layout.size[0])
        && point[1] <= f64::from(layout.position[1] + layout.size[1])
}

fn reference_image_rect(
    viewport: egui::Rect,
    source_size: egui::Vec2,
    zoom: f32,
    pan: [f32; 2],
) -> Option<egui::Rect> {
    if !viewport.is_finite()
        || viewport.width() <= 0.0
        || viewport.height() <= 0.0
        || !source_size.is_finite()
        || source_size.x <= 0.0
        || source_size.y <= 0.0
        || !zoom.is_finite()
        || !pan.iter().all(|value| value.is_finite())
    {
        return None;
    }
    let fit = (viewport.width() / source_size.x).min(viewport.height() / source_size.y);
    let size = source_size
        * fit
        * zoom.clamp(
            crate::workspace::MIN_REFERENCE_ZOOM,
            crate::workspace::MAX_REFERENCE_ZOOM,
        );
    let center = viewport.center()
        + egui::vec2(
            pan[0].clamp(-10000.0, 10000.0),
            pan[1].clamp(-10000.0, 10000.0),
        );
    Some(egui::Rect::from_center_size(center, size))
}

fn clamp_reference_position(
    position: [f32; 2],
    size: [f32; 2],
    viewport: egui::Rect,
) -> egui::Pos2 {
    let max_x = (viewport.max.x - size[0].min(viewport.width())).max(viewport.min.x);
    let max_y = (viewport.max.y - size[1].min(viewport.height())).max(viewport.min.y);
    egui::pos2(
        position[0].clamp(viewport.min.x, max_x),
        position[1].clamp(viewport.min.y, max_y),
    )
}

fn centered_reference_position(size: [f32; 2], viewport: egui::Rect) -> egui::Pos2 {
    clamp_reference_position(
        [
            viewport.center().x - size[0] * 0.5,
            viewport.center().y - size[1] * 0.5,
        ],
        size,
        viewport,
    )
}

fn reference_zoom_about(
    layout: &mut crate::workspace::ReferenceLayout,
    viewport: egui::Rect,
    from: egui::Pos2,
    to: egui::Pos2,
    ratio: f32,
) -> bool {
    if !ratio.is_finite() || ratio <= 0.0 || !from.is_finite() || !to.is_finite() {
        return false;
    }
    let old_zoom = layout.view_zoom;
    let next_zoom = (old_zoom * ratio).clamp(
        crate::workspace::MIN_REFERENCE_ZOOM,
        crate::workspace::MAX_REFERENCE_ZOOM,
    );
    if !old_zoom.is_finite() || old_zoom <= 0.0 {
        return false;
    }
    let ratio = next_zoom / old_zoom;
    let origin = viewport.center() + egui::vec2(layout.view_pan[0], layout.view_pan[1]);
    let next_origin = to - (from - origin) * ratio;
    let next_pan = next_origin - viewport.center();
    if !next_pan.is_finite() {
        return false;
    }
    let changed = layout.view_zoom != next_zoom || layout.view_pan != [next_pan.x, next_pan.y];
    layout.view_zoom = next_zoom;
    layout.view_pan = [
        next_pan.x.clamp(-10000.0, 10000.0),
        next_pan.y.clamp(-10000.0, 10000.0),
    ];
    changed
}

#[cfg(test)]
mod reference_tests {
    use super::*;

    #[test]
    fn reference_window_bounds_block_canvas_input_in_physical_pixels() {
        let layout = crate::workspace::ReferenceLayout {
            path: PathBuf::from("reference.png"),
            position: [10.0, 20.0],
            size: [100.0, 80.0],
            opacity: 0.7,
            view_zoom: 1.0,
            view_pan: [0.0, 0.0],
            position_locked: false,
        };
        assert!(reference_contains_client(&layout, [220.0, 100.0], 2.0));
        assert!(!reference_contains_client(&layout, [19.0, 100.0], 2.0));
        assert!(!reference_contains_client(&layout, [0.0, 0.0], 0.0));
        assert!(!reference_contains_client(&layout, [f64::NAN, 0.0], 1.0));
    }

    #[test]
    fn reference_image_zoom_and_pan_map_to_bounded_viewport_geometry() {
        let viewport = egui::Rect::from_min_size(egui::pos2(20.0, 30.0), egui::vec2(200.0, 100.0));
        let fitted =
            reference_image_rect(viewport, egui::vec2(100.0, 50.0), 1.0, [0.0, 0.0]).unwrap();
        assert_eq!(fitted, viewport);
        let zoomed =
            reference_image_rect(viewport, egui::vec2(100.0, 50.0), 2.0, [10.0, -5.0]).unwrap();
        assert_eq!(zoomed.size(), egui::vec2(400.0, 200.0));
        assert_eq!(zoomed.center(), viewport.center() + egui::vec2(10.0, -5.0));
        assert!(reference_image_rect(viewport, egui::Vec2::ZERO, 1.0, [0.0, 0.0]).is_none());
        assert!(
            reference_image_rect(viewport, egui::vec2(1.0, 1.0), f32::NAN, [0.0, 0.0]).is_none()
        );
    }

    #[test]
    fn offscreen_reference_panels_are_clamped_to_the_current_viewport() {
        let viewport = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1000.0, 700.0));
        assert_eq!(
            clamp_reference_position([50_000.0, -50_000.0], [360.0, 300.0], viewport),
            egui::pos2(640.0, 0.0)
        );
        assert_eq!(
            clamp_reference_position([-50_000.0, 50_000.0], [1200.0, 900.0], viewport),
            egui::Pos2::ZERO
        );
        assert_eq!(
            centered_reference_position([360.0, 300.0], viewport),
            egui::pos2(320.0, 200.0)
        );
    }

    #[test]
    fn reference_gesture_keeps_the_focal_artwork_point_under_the_fingers() {
        let viewport = egui::Rect::from_min_size(egui::pos2(10.0, 20.0), egui::vec2(200.0, 100.0));
        let mut layout = crate::workspace::ReferenceLayout {
            path: PathBuf::from("reference.png"),
            position: [0.0, 0.0],
            size: [360.0, 300.0],
            opacity: 0.7,
            view_zoom: 1.0,
            view_pan: [0.0, 0.0],
            position_locked: false,
        };
        let from = egui::pos2(130.0, 70.0);
        let to = egui::pos2(140.0, 75.0);
        assert!(reference_zoom_about(&mut layout, viewport, from, to, 2.0));
        let new_origin = viewport.center() + egui::vec2(layout.view_pan[0], layout.view_pan[1]);
        let old_relative = from - viewport.center();
        assert!((new_origin.x + old_relative.x * layout.view_zoom - to.x).abs() < 0.001);
        assert!((new_origin.y + old_relative.y * layout.view_zoom - to.y).abs() < 0.001);
        assert!(!reference_zoom_about(
            &mut layout,
            viewport,
            from,
            to,
            f32::NAN
        ));
    }

    #[test]
    fn external_reference_decodes_into_a_ui_texture() {
        let path = std::env::temp_dir().join(format!(
            "rpl-reference-{}-{}.png",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let image = image::RgbaImage::from_pixel(2, 3, image::Rgba([240, 16, 120, 255]));
        image.save(&path).unwrap();
        let loaded = load_reference_texture(
            &egui::Context::default(),
            crate::workspace::ReferenceLayout {
                path: path.clone(),
                position: [0.0, 0.0],
                size: [300.0, 300.0],
                opacity: 0.5,
                view_zoom: 1.0,
                view_pan: [0.0, 0.0],
                position_locked: false,
            },
        )
        .unwrap();
        assert_eq!(loaded.texture.size(), [2, 3]);
        assert_eq!(loaded.layout.path, path);
        std::fs::remove_file(path).unwrap();
    }

    #[test]
    fn reference_board_adds_jpeg_and_downscales_photos_with_partial_failure_feedback() {
        let dir = std::env::temp_dir().join(format!(
            "rpl-reference-import-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let large_jpeg = dir.join("phone-photo.jpg");
        image::RgbImage::from_pixel(2400, 1800, image::Rgb([120, 80, 200]))
            .save(&large_jpeg)
            .unwrap();
        let missing = dir.join("missing.png");
        let mut references = Vec::new();
        let context = egui::Context::default();

        add_reference_path(&context, &mut references, large_jpeg.clone()).unwrap();
        let reference = &references[0];
        assert_eq!(reference.source_size, [2400, 1800]);
        assert!(reference.texture.size()[0] <= MAX_REFERENCE_PREVIEW_EDGE as usize);
        assert!(reference.texture.size()[1] <= MAX_REFERENCE_PREVIEW_EDGE as usize);
        assert_eq!(references[0].layout.path, large_jpeg);

        let error = add_reference_path(&context, &mut references, missing).unwrap_err();
        assert!(error.contains("missing.png"));
        assert_eq!(
            references.len(),
            1,
            "successful imports survive later errors"
        );
        let duplicate = references[0].layout.path.clone();
        assert!(
            add_reference_path(&context, &mut references, duplicate)
                .unwrap_err()
                .contains("already on the reference board")
        );

        std::fs::remove_dir_all(dir).unwrap();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LassoAppend {
    Unchanged,
    Added,
    LimitReached,
}

fn append_lasso_point(points: &mut Vec<[f64; 2]>, position: [f64; 2]) -> LassoAppend {
    let Some(last) = points.last().copied() else {
        return LassoAppend::LimitReached;
    };
    if (position[0] - last[0]).hypot(position[1] - last[1]) < 2.0 {
        return LassoAppend::Unchanged;
    }
    if points.len() >= paint_document::editing::MAX_SELECTION_POINTS {
        return LassoAppend::LimitReached;
    }
    points.push(position);
    LassoAppend::Added
}

fn native_project_path(path: &std::path::Path) -> PathBuf {
    path.with_extension("rpl")
}

fn brush_display_name(kind: BrushKind) -> &'static str {
    match kind {
        BrushKind::Round => "Hard round",
        BrushKind::Ink => "Ink pen",
        BrushKind::Pencil => "Pencil",
        BrushKind::Marker => "Marker",
        BrushKind::Airbrush => "Airbrush",
        BrushKind::Soft => "Soft brush",
        BrushKind::Textured => "Textured brush",
        BrushKind::Calligraphy => "Calligraphy",
        BrushKind::Eraser => "Eraser",
        BrushKind::Wash => "Wash · experimental",
    }
}

#[derive(Clone)]
enum Action {
    Fill(bool),
    Deselect,
    Transform([f64; 6]),
    Crop,
    New,
    Open,
    Save,
    SaveAs,
    RetryClose,
    SaveBrush,
    LoadBrush,
    Export,
    Import,
    ImportOra,
    ExportOra,
    Recover(PathBuf),
    Recent(PathBuf),
    RemoveRecovery(PathBuf),
    RemoveDraft(PathBuf),
    AddReference(PathBuf),
    Undo,
    Redo,
    Add,
    Duplicate,
    Delete,
    Clear,
    Select(u64),
    Move(i32),
    Layer(u64, String, bool, bool, u8, Blend),
}
#[derive(Clone, Copy)]
enum FileRequest {
    OpenProject,
    SaveProjectAs,
    SaveBrush,
    LoadBrush,
    ExportPng,
    ImportPng,
    ImportOra,
    ExportOra,
    AddReferences,
}
impl FileRequest {
    fn action(self) -> Option<Action> {
        Some(match self {
            Self::OpenProject => Action::Open,
            Self::SaveProjectAs => Action::SaveAs,
            Self::SaveBrush => Action::SaveBrush,
            Self::LoadBrush => Action::LoadBrush,
            Self::ExportPng => Action::Export,
            Self::ImportPng => Action::Import,
            Self::ImportOra => Action::ImportOra,
            Self::ExportOra => Action::ExportOra,
            Self::AddReferences => return None,
        })
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum EditTool {
    Brush,
    Rectangle,
    Ellipse,
    Lasso,
    Eyedropper,
}
pub struct Studio {
    pub context: egui::Context,
    pub input: egui_winit::State,
    painter: egui_wgpu::Renderer,
    pub mapping: Option<CanvasMapping>,
    path: String,
    store: Option<Store>,
    status: String,
    new_name: String,
    new_width: f64,
    new_height: f64,
    new_dpi: u32,
    new_unit: crate::document_setup::DocumentUnit,
    new_background: [u8; 3],
    new_transparent: bool,
    recent_document_sizes: Vec<crate::workspace::RecentDocumentSize>,
    recent_brush_colors: Vec<[u8; 3]>,
    observed_brush_color: Option<[u8; 3]>,
    hex_color: String,
    hex_color_for: Option<[u8; 3]>,
    new_dialog: bool,
    picker: Option<FileRequest>,
    manual_file_dialog: bool,
    manual_file_action: Option<Action>,
    manual_file_submit: bool,
    test_path_dialog: bool,
    confirm: Option<Action>,
    pub close_requested: bool,
    pub close_ready: bool,
    recovery: Option<Recovery>,
    recovery_files: Vec<PathBuf>,
    recovery_manager: bool,
    recovery_manager_from_home: bool,
    drafts: Vec<PathBuf>,
    recent: paint_document::recent::Recent,
    recent_cards: Vec<RecentProjectCard>,
    recent_path: Option<PathBuf>,
    cleanup: Option<PathBuf>,
    draft_cleanup: Option<PathBuf>,
    last_submitted: u64,
    autosave_schedule: crate::autosave::AutosaveDebounce,
    recovery_saving: bool,
    appearance_mode: crate::workspace::AppearanceMode,
    accent_scheme: crate::workspace::AccentScheme,
    custom_accent_rgb: Option<[u8; 3]>,
    custom_accent_hex: String,
    system_dark: bool,
    appearance_cache: Option<AppearanceKey>,
    high_contrast: bool,
    ui_scale: f32,
    palette_dock: crate::workspace::PaletteDock,
    palette_collapsed: bool,
    handedness: crate::workspace::Handedness,
    palette_position: [f32; 2],
    navigation: crate::navigation::Navigation,
    touches: crate::navigation::TouchNavigation,
    touch_routes: crate::navigation::TouchTargetRouter,
    control_held: bool,
    workspace_needs_save: bool,
    pan_cursor: Option<[f64; 2]>,
    workspace_center: [f64; 2],
    tool: EditTool,
    edit_pointer: Option<u64>,
    edit_points: Vec<[f64; 2]>,
    transform_offset: [f64; 2],
    transform_scale: f64,
    transform_angle: f64,
    pen_ui: std::collections::BTreeSet<u64>,
    mouse_position: Option<[f64; 2]>,
    ui_drag: bool,
    pub last_upload_us: u128,
    pub last_frame_us: u128,
    preferences_path: Option<PathBuf>,
    references: Vec<ReferenceImage>,
    brush_panel_width: f32,
    show_controls: bool,
    inspector_tab: crate::workspace::InspectorTab,
    welcome: bool,
    new_from_welcome: bool,
    picker_from_welcome: bool,
}
impl Studio {
    pub fn new(g: &Renderer, window: &Window, format: wgpu::TextureFormat) -> Self {
        let context = egui::Context::default();
        crate::ui_tokens::install(&context);
        let input = egui_winit::State::new(
            context.clone(),
            egui::ViewportId::ROOT,
            window,
            Some(window.scale_factor() as f32),
            window.theme(),
            Some(g.device.limits().max_texture_dimension_2d as usize),
        );
        let dir = paint_document::recovery::directory();
        let files = dir
            .as_ref()
            .map(|p| paint_document::recovery::available(p))
            .unwrap_or_default();
        let recovery = dir.as_ref().ok().and_then(|p| Recovery::start(p).ok());
        let recent_path = dir
            .as_ref()
            .ok()
            .and_then(|p| p.parent())
            .map(|p| p.join("recent.json"));
        let recent = recent_path
            .as_ref()
            .and_then(|p| paint_document::recent::Recent::load(p).ok())
            .unwrap_or_default();
        let recent_cards = recent_project_cards(&recent);
        let preferences_path = dir
            .as_ref()
            .ok()
            .and_then(|p| p.parent())
            .map(|p| p.join("workspace.json"));
        let test_path_dialog = std::env::var_os("RPL_TEST_PATH_DIALOG").is_some();
        let new_name = if test_path_dialog {
            std::env::var("RPL_TEST_DOCUMENT_NAME").unwrap_or_else(|_| "Untitled".into())
        } else {
            "Untitled".into()
        };
        let preferences = preferences_path
            .as_ref()
            .and_then(|p| crate::workspace::load(p))
            .unwrap_or_default();
        let document_setup = preferences.new_document.clone();
        let recent_document_sizes = preferences.recent_document_sizes.clone();
        let recent_brush_colors = preferences.recent_brush_colors.clone();
        let references = preferences
            .references
            .iter()
            .cloned()
            .filter_map(|layout| load_reference_texture(&context, layout).ok())
            .collect();
        let appearance_mode = preferences.effective_appearance_mode();
        let custom_accent_rgb = preferences.custom_accent_rgb;
        let custom_accent_hex = custom_accent_rgb
            .map(crate::color_controls::format_hex_rgb)
            .unwrap_or_else(|| "#A78BFA".into());
        let status = if recovery.is_some() {
            "Ready — local recovery enabled".into()
        } else {
            "Recovery unavailable: choose a writable project path and save frequently".into()
        };
        Self {
            context,
            input,
            painter: egui_wgpu::Renderer::new(&g.device, format, Default::default()),
            mapping: None,
            path: "artwork.rpl".into(),
            store: None,
            status,
            new_name,
            new_width: document_setup.width,
            new_height: document_setup.height,
            new_dpi: document_setup.dpi,
            new_unit: document_setup.unit,
            new_background: document_setup.background,
            new_transparent: document_setup.transparent,
            recent_document_sizes,
            recent_brush_colors,
            observed_brush_color: None,
            hex_color: String::new(),
            hex_color_for: None,
            new_dialog: false,
            picker: None,
            manual_file_dialog: false,
            manual_file_action: None,
            manual_file_submit: false,
            test_path_dialog,
            confirm: None,
            close_requested: false,
            close_ready: false,
            recovery,
            recovery_files: files,
            recovery_manager: false,
            recovery_manager_from_home: false,
            drafts: crate::drafts::list(),
            recent,
            recent_cards,
            recent_path,
            cleanup: None,
            draft_cleanup: None,
            last_submitted: 0,
            autosave_schedule: Default::default(),
            recovery_saving: false,
            appearance_mode,
            accent_scheme: preferences.accent_scheme,
            custom_accent_rgb,
            custom_accent_hex,
            system_dark: window
                .theme()
                .map(|theme| theme == winit::window::Theme::Dark)
                .unwrap_or(preferences.dark_theme),
            appearance_cache: None,
            high_contrast: preferences.high_contrast,
            ui_scale: preferences.ui_scale,
            palette_dock: preferences.palette_dock,
            palette_collapsed: preferences.palette_collapsed,
            handedness: preferences.handedness,
            palette_position: preferences.palette_position,
            navigation: crate::navigation::Navigation {
                zoom: preferences.canvas_zoom,
                pan: preferences.canvas_pan,
            },
            touches: Default::default(),
            touch_routes: Default::default(),
            control_held: false,
            workspace_needs_save: false,
            pan_cursor: None,
            workspace_center: [0.0; 2],
            tool: EditTool::Brush,
            edit_pointer: None,
            edit_points: Vec::new(),
            transform_offset: [0.0; 2],
            transform_scale: 1.0,
            transform_angle: 0.0,
            pen_ui: std::collections::BTreeSet::new(),
            mouse_position: None,
            ui_drag: false,
            last_upload_us: 0,
            last_frame_us: 0,
            preferences_path,
            brush_panel_width: preferences.brush_panel_width,
            show_controls: preferences.show_controls,
            inspector_tab: preferences.inspector_tab,
            references,
            welcome: true,
            new_from_welcome: false,
            picker_from_welcome: false,
        }
    }

    pub fn init_accesskit(
        &mut self,
        event_loop: &winit::event_loop::ActiveEventLoop,
        window: &Window,
        proxy: winit::event_loop::EventLoopProxy<
            impl From<egui_winit::accesskit_winit::Event> + Send + 'static,
        >,
    ) {
        self.input.init_accesskit(event_loop, window, proxy);
    }

    pub fn accesskit_action_request(&mut self, request: egui::accesskit::ActionRequest) {
        self.input.on_accesskit_action_request(request);
    }
    fn reference_under_client(&self, client_px: [f64; 2]) -> bool {
        self.reference_index_under_client(client_px).is_some()
    }
    fn reference_index_under_client(&self, client_px: [f64; 2]) -> Option<usize> {
        let pixels_per_point = self.context.pixels_per_point();
        self.references
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, reference)| {
                reference_contains_client(&reference.layout, client_px, pixels_per_point)
                    .then_some(index)
            })
    }
    fn reference_viewport_under_client(&self, client_px: [f64; 2]) -> Option<usize> {
        let scale = f64::from(self.context.pixels_per_point());
        if !scale.is_finite() || scale <= 0.0 {
            return None;
        }
        let point = egui::pos2((client_px[0] / scale) as f32, (client_px[1] / scale) as f32);
        self.references
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, reference)| {
                reference
                    .viewport
                    .is_some_and(|rect| rect.contains(point))
                    .then_some(index)
            })
    }
    fn clear_touch_routes(&mut self) {
        self.touch_routes.clear();
        self.touches.clear();
        for reference in &mut self.references {
            reference.touches.clear();
        }
    }
    pub fn window_event(
        &mut self,
        window: &Window,
        event: &winit::event::WindowEvent,
        session: &mut CanvasSession,
    ) -> (bool, bool) {
        use winit::event::{ElementState, WindowEvent as E};
        if let E::ModifiersChanged(modifiers) = event {
            self.control_held = modifiers.state().control_key();
        }
        if let E::ThemeChanged(theme) = event {
            self.system_dark = *theme == winit::window::Theme::Dark;
            self.appearance_cache = None;
            window.request_redraw();
            return (true, true);
        }
        // The deterministic path-entry dialog is only enabled by the Xvfb
        // workflow harness. Latch its submit key at the winit boundary: egui
        // text editing and platform shortcut handling can otherwise consume
        // Enter before the modal observes it.
        let test_dialog_enter = self.test_path_dialog
            && self.manual_file_dialog
            && matches!(
                event,
                E::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed
                        && (matches!(
                            event.physical_key,
                            winit::keyboard::PhysicalKey::Code(
                                winit::keyboard::KeyCode::Enter
                                    | winit::keyboard::KeyCode::NumpadEnter
                            )
                        )
                        || event.logical_key
                            == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Enter))
            );
        if test_dialog_enter {
            eprintln!("TEST_PATH_DIALOG: Enter received at winit boundary");
            self.manual_file_submit = true;
            window.request_redraw();
            return (true, true);
        }
        if let E::CursorMoved { position, .. } = event {
            self.mouse_position = Some([position.x, position.y]);
        }
        if matches!(
            event,
            E::Focused(false) | E::CursorLeft { .. } | E::Resized(_) | E::ScaleFactorChanged { .. }
        ) {
            self.clear_touch_routes();
            self.pan_cursor = None;
        }
        if let E::Touch(touch) = event {
            if session.live().active() {
                self.clear_touch_routes();
                return (false, false);
            }
            let p = [touch.location.x, touch.location.y];
            let phase = match touch.phase {
                winit::event::TouchPhase::Started => crate::navigation::TouchPhase::Start,
                winit::event::TouchPhase::Moved => crate::navigation::TouchPhase::Move,
                winit::event::TouchPhase::Ended => crate::navigation::TouchPhase::End,
                winit::event::TouchPhase::Cancelled => crate::navigation::TouchPhase::Cancel,
            };
            let routed = if phase == crate::navigation::TouchPhase::Start {
                let target = if let Some(index) = self.reference_viewport_under_client(p) {
                    Some(crate::navigation::TouchTarget::Reference(
                        self.references[index].layout.path.clone(),
                    ))
                } else if !self.blocks_canvas()
                    && session.mode() == crate::interaction::Mode::Live
                    && self
                        .mapping
                        .is_some_and(|mapping| mapping.contains_client(p))
                    && !self.reference_under_client(p)
                {
                    Some(crate::navigation::TouchTarget::Canvas)
                } else {
                    None
                };
                match target {
                    Some(target) => Some(self.touch_routes.start(touch.id, target)),
                    None if self.touch_routes.active_target().is_some() => Some(
                        self.touch_routes
                            .start(touch.id, crate::navigation::TouchTarget::Suppressed),
                    ),
                    None => None,
                }
            } else {
                self.touch_routes.target(touch.id)
            };
            if let Some(target) = routed {
                let scale = f64::from(self.context.pixels_per_point());
                match target {
                    crate::navigation::TouchTarget::Canvas => {
                        if let Some(pinch) = self.touches.event(touch.id, phase, p) {
                            self.workspace_needs_save |= self.navigation.pinch(
                                self.workspace_center,
                                pinch.from,
                                pinch.to,
                                pinch.ratio,
                            );
                        }
                    }
                    crate::navigation::TouchTarget::Reference(path) => {
                        if let Some(reference) = self
                            .references
                            .iter_mut()
                            .find(|reference| reference.layout.path == path)
                        {
                            let point = [p[0] / scale, p[1] / scale];
                            if let Some(pinch) = reference.touches.event(touch.id, phase, point)
                                && let Some(viewport) = reference.viewport
                            {
                                let from = egui::pos2(pinch.from[0] as f32, pinch.from[1] as f32);
                                let to = egui::pos2(pinch.to[0] as f32, pinch.to[1] as f32);
                                self.workspace_needs_save |= reference_zoom_about(
                                    &mut reference.layout,
                                    viewport,
                                    from,
                                    to,
                                    pinch.ratio as f32,
                                );
                            }
                        }
                    }
                    crate::navigation::TouchTarget::Suppressed => {}
                }
                if matches!(
                    phase,
                    crate::navigation::TouchPhase::End | crate::navigation::TouchPhase::Cancel
                ) {
                    self.touch_routes.finish(touch.id);
                }
                return (true, true);
            }
        }
        let canvas = self.mouse_position.is_some_and(|p| {
            self.mapping.is_some_and(|m| m.contains_client(p)) && !self.reference_under_client(p)
        }) && !self.blocks_canvas()
            && session.mode() == crate::interaction::Mode::Live;
        let reference_under_pointer = self
            .mouse_position
            .and_then(|position| self.reference_index_under_client(position));
        match event {
            E::MouseWheel { delta, .. } if reference_under_pointer.is_some() => {
                let index = reference_under_pointer.unwrap();
                let scale = f64::from(self.context.pixels_per_point());
                let pointer_px = self.mouse_position.unwrap();
                let pointer = egui::pos2(
                    (pointer_px[0] / scale) as f32,
                    (pointer_px[1] / scale) as f32,
                );
                let reference = &mut self.references[index];
                let viewport = reference.viewport.unwrap();
                let changed = match delta {
                    winit::event::MouseScrollDelta::PixelDelta(delta) if !self.control_held => {
                        reference_zoom_about(
                            &mut reference.layout,
                            viewport,
                            pointer,
                            egui::pos2(
                                pointer.x + delta.x as f32 / scale as f32,
                                pointer.y + delta.y as f32 / scale as f32,
                            ),
                            1.0,
                        )
                    }
                    winit::event::MouseScrollDelta::PixelDelta(delta) => reference_zoom_about(
                        &mut reference.layout,
                        viewport,
                        pointer,
                        pointer,
                        (delta.y / 120.0).clamp(-1.0, 1.0).exp() as f32,
                    ),
                    winit::event::MouseScrollDelta::LineDelta(_, y) => reference_zoom_about(
                        &mut reference.layout,
                        viewport,
                        pointer,
                        pointer,
                        (f64::from(*y).clamp(-8.0, 8.0) * 0.12).exp() as f32,
                    ),
                };
                self.workspace_needs_save |= changed;
                return (true, true);
            }
            E::PinchGesture { delta, .. }
                if reference_under_pointer.is_some() && delta.is_finite() =>
            {
                let index = reference_under_pointer.unwrap();
                let scale = f64::from(self.context.pixels_per_point());
                let pointer_px = self.mouse_position.unwrap();
                let pointer = egui::pos2(
                    (pointer_px[0] / scale) as f32,
                    (pointer_px[1] / scale) as f32,
                );
                let reference = &mut self.references[index];
                if let Some(viewport) = reference.viewport {
                    self.workspace_needs_save |= reference_zoom_about(
                        &mut reference.layout,
                        viewport,
                        pointer,
                        pointer,
                        delta.clamp(-2.0, 2.0).exp() as f32,
                    );
                }
                return (true, true);
            }
            E::PanGesture { delta, .. } if reference_under_pointer.is_some() => {
                let index = reference_under_pointer.unwrap();
                let scale = f64::from(self.context.pixels_per_point());
                let pointer_px = self.mouse_position.unwrap();
                let pointer = egui::pos2(
                    (pointer_px[0] / scale) as f32,
                    (pointer_px[1] / scale) as f32,
                );
                let reference = &mut self.references[index];
                if let Some(viewport) = reference.viewport {
                    self.workspace_needs_save |= reference_zoom_about(
                        &mut reference.layout,
                        viewport,
                        pointer,
                        egui::pos2(
                            pointer.x + delta.x / scale as f32,
                            pointer.y + delta.y / scale as f32,
                        ),
                        1.0,
                    );
                }
                return (true, true);
            }
            E::MouseWheel { delta, .. } if canvas => {
                let p = self.mouse_position.unwrap();
                let changed = match delta {
                    winit::event::MouseScrollDelta::PixelDelta(delta) if !self.control_held => {
                        self.navigation.pinch(
                            self.workspace_center,
                            p,
                            [p[0] + delta.x, p[1] + delta.y],
                            1.0,
                        )
                    }
                    winit::event::MouseScrollDelta::PixelDelta(delta) => self.navigation.pinch(
                        self.workspace_center,
                        p,
                        p,
                        (delta.y / 120.0).clamp(-1.0, 1.0).exp(),
                    ),
                    winit::event::MouseScrollDelta::LineDelta(_, y) => {
                        let lines = f64::from(*y);
                        self.navigation.pinch(
                            self.workspace_center,
                            p,
                            p,
                            (lines.clamp(-8.0, 8.0) * 0.12).exp(),
                        )
                    }
                };
                self.workspace_needs_save |= changed;
                return (true, true);
            }
            E::PinchGesture { delta, .. } if canvas && delta.is_finite() => {
                let p = self.mouse_position.unwrap();
                self.workspace_needs_save |= self.navigation.pinch(
                    self.workspace_center,
                    p,
                    p,
                    delta.clamp(-2.0, 2.0).exp(),
                );
                return (true, true);
            }
            E::PanGesture { delta, .. } if canvas => {
                let p = self.mouse_position.unwrap();
                self.workspace_needs_save |= self.navigation.pinch(
                    self.workspace_center,
                    p,
                    [p[0] + f64::from(delta.x), p[1] + f64::from(delta.y)],
                    1.0,
                );
                return (true, true);
            }
            E::MouseInput {
                state,
                button: winit::event::MouseButton::Middle,
                ..
            } if canvas || self.pan_cursor.is_some() => {
                if *state == ElementState::Released && self.pan_cursor.is_some() {
                    self.workspace_needs_save = true;
                }
                self.pan_cursor = if *state == ElementState::Pressed {
                    self.mouse_position
                } else {
                    None
                };
                return (true, true);
            }
            E::CursorMoved { .. } if self.pan_cursor.is_some() => {
                let from = self.pan_cursor.unwrap();
                let to = self.mouse_position.unwrap();
                self.workspace_needs_save |=
                    self.navigation.pinch(self.workspace_center, from, to, 1.0);
                self.pan_cursor = Some(to);
                return (true, true);
            }
            _ => {}
        }
        use paint_input::Phase;
        let phase = match event {
            E::MouseInput {
                state,
                button: winit::event::MouseButton::Left,
                ..
            } => Some(if *state == ElementState::Pressed {
                Phase::Down
            } else {
                Phase::Up
            }),
            E::CursorMoved { .. } => Some(Phase::Move),
            _ => None,
        };
        if matches!(
            event,
            E::Focused(false) | E::CursorLeft { .. } | E::Resized(_) | E::ScaleFactorChanged { .. }
        ) {
            self.edit_pointer = None;
            self.edit_points.clear();
        }
        if let Some(phase) = phase
            && let Some(position) = self.mouse_position
            && self.editing_pointer(session, 0, phase, position)
        {
            return (true, true);
        }
        let on_canvas = !self.blocks_canvas()
            && self.mouse_position.is_some_and(|p| {
                self.mapping.is_some_and(|m| m.contains_client(p))
                    && !self.reference_under_client(p)
            });
        match event {
            E::MouseInput {
                state: ElementState::Pressed,
                ..
            } if on_canvas && !self.ui_drag => return (false, false),
            E::MouseInput {
                state: ElementState::Pressed,
                ..
            } => {
                self.ui_drag = true;
                let r = self.input.on_window_event(window, event);
                return (true, r.repaint);
            }
            E::MouseInput {
                state: ElementState::Released,
                ..
            } => {
                let captured = self.ui_drag;
                self.ui_drag = false;
                if !captured {
                    return (false, false);
                }
                let r = self.input.on_window_event(window, event);
                return (true, r.repaint);
            }
            E::Focused(false) | E::CursorLeft { .. } => {
                self.ui_drag = false;
                self.mouse_position = None;
            }
            _ => {}
        }
        let r = self.input.on_window_event(window, event);
        (
            matches!(event, E::CursorMoved { .. }) && (self.ui_drag || self.blocks_canvas()),
            r.repaint,
        )
    }
    pub fn native_pen_ui(
        &mut self,
        s: &paint_input::PointerSample,
        _scale: f64,
        session: &mut CanvasSession,
    ) -> bool {
        let scale = f64::from(self.context.pixels_per_point());
        // Once a pen contact begins on the canvas, keep the native stream with
        // the painting processor while it crosses the paper boundary. Sending
        // these samples to egui would synthesize a Cancel and orphan the stroke.
        if session.live().active() {
            return false;
        }
        if self.editing_pointer(session, s.id, s.phase, s.position_px) {
            return true;
        }
        use paint_input::Phase;
        let outside = self.blocks_canvas()
            || self.reference_under_client(s.position_px)
            || !self
                .mapping
                .is_some_and(|m| m.contains_client(s.position_px));
        if s.phase == Phase::Down && outside && self.pen_ui.len() < 64 {
            self.pen_ui.insert(s.id);
        }
        let captured = self.pen_ui.contains(&s.id);
        if captured || outside {
            let pos = egui::pos2(
                (s.position_px[0] / scale) as f32,
                (s.position_px[1] / scale) as f32,
            );
            let events = &mut self.input.egui_input_mut().events;
            events.push(egui::Event::PointerMoved(pos));
            if matches!(s.phase, Phase::Down | Phase::Up | Phase::Cancel) {
                events.push(egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: s.phase == Phase::Down,
                    modifiers: egui::Modifiers::NONE,
                });
            }
        }
        if matches!(s.phase, Phase::Up | Phase::Cancel) {
            self.pen_ui.remove(&s.id);
        }
        captured || outside
    }
    fn editing_pointer(
        &mut self,
        session: &mut CanvasSession,
        id: u64,
        phase: paint_input::Phase,
        client: [f64; 2],
    ) -> bool {
        use paint_input::Phase;
        if self.tool == EditTool::Brush
            || session.mode() != crate::interaction::Mode::Live
            || self.blocks_canvas()
        {
            return false;
        }
        let on_canvas = self.mapping.is_some_and(|m| m.contains_client(client))
            && !self.reference_under_client(client);
        let captured = self.edit_pointer == Some(id);
        if !on_canvas && !captured {
            return false;
        }
        if phase == Phase::Cancel && !captured {
            return on_canvas;
        }
        if phase == Phase::Cancel || !on_canvas {
            self.edit_pointer = None;
            self.edit_points.clear();
            return true;
        }
        let Some(position) = self
            .mapping
            .and_then(|m| m.routing_viewport().to_document(client).ok())
            .map(|p| p.0)
        else {
            return true;
        };
        match phase {
            Phase::Down if self.edit_pointer.is_none() => {
                session.mouse(MouseEvent::Interrupted, 0);
                if self.tool == EditTool::Eyedropper {
                    match session.live().project().eyedropper(position) {
                        Ok(c) => session.live_mut().project_mut().brush.color = c,
                        Err(e) => self.status = e.to_string(),
                    }
                } else {
                    self.edit_pointer = Some(id);
                    self.edit_points = vec![position, position];
                }
            }
            Phase::Move | Phase::Up if captured => {
                if self.tool == EditTool::Lasso {
                    if append_lasso_point(&mut self.edit_points, position)
                        == LassoAppend::LimitReached
                    {
                        self.edit_pointer = None;
                        self.edit_points.clear();
                        self.status =
                            "Lasso cancelled: simplify the outline to 1024 points or fewer".into();
                        return true;
                    }
                } else {
                    self.edit_points[1] = position;
                }
                if phase == Phase::Up {
                    let kind = match self.tool {
                        EditTool::Rectangle => SelectionKind::Rectangle,
                        EditTool::Ellipse => SelectionKind::Ellipse,
                        _ => SelectionKind::Lasso,
                    };
                    let result = Selection::new(kind, std::mem::take(&mut self.edit_points))
                        .and_then(|s| session.live_mut().project_mut().set_selection(Some(s)));
                    self.edit_pointer = None;
                    self.status = match result {
                        Ok(()) => "Selection ready: brush, fill, delete or transform".into(),
                        Err(e) => e.to_string(),
                    };
                }
            }
            _ => {}
        }
        true
    }
    pub fn tool_shortcut(&mut self, key: char, session: &mut CanvasSession) {
        session.mouse(MouseEvent::Interrupted, 0);
        self.edit_pointer = None;
        self.edit_points.clear();
        if key == '0' {
            self.navigation = Default::default();
            self.workspace_needs_save = true;
            return;
        }
        self.tool = match key {
            'm' => EditTool::Rectangle,
            'e' => EditTool::Ellipse,
            'k' => EditTool::Lasso,
            'i' => EditTool::Eyedropper,
            _ => EditTool::Brush,
        };
    }
    fn remember(&mut self, path: &std::path::Path) {
        let result = self.recent.remember(path).and_then(|_| {
            self.recent_path
                .as_ref()
                .ok_or(paint_document::PaintError::Invalid)
                .and_then(|p| self.recent.save(p))
        });
        if let Err(e) = result {
            self.status = format!("Artwork file succeeded; recent-list update failed: {e}");
        }
        self.recent_cards = recent_project_cards(&self.recent);
    }
    pub fn project_label(&self) -> String {
        self.store
            .as_ref()
            .and_then(|s| s.path.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }
    pub fn test_prompt_label(&self) -> Option<&'static str> {
        if !self.test_path_dialog {
            None
        } else if self.manual_file_dialog {
            Some("TEST FILE PICKER")
        } else if self.new_dialog {
            Some("TEST NEW DOCUMENT")
        } else {
            None
        }
    }
    pub fn blocks_canvas(&self) -> bool {
        self.welcome
            || self.new_dialog
            || self.picker.is_some()
            || self.manual_file_dialog
            || self.confirm.is_some()
            || self.close_requested
            || self.recovery_manager
            || self.cleanup.is_some()
    }
    pub fn shortcut(&mut self, key: char, session: &mut CanvasSession) {
        session.mouse(MouseEvent::Interrupted, 0);
        if key == 't' || key == 'u' {
            self.path = PathBuf::from(&self.path)
                .with_extension("rplbrush")
                .to_string_lossy()
                .into();
            self.picker = Some(if key == 't' {
                FileRequest::SaveBrush
            } else {
                FileRequest::LoadBrush
            });
            return;
        }
        if key == 'v' {
            if let Some(path) = self.recovery_files.first().cloned() {
                if session.live().project().dirty() {
                    self.confirm = Some(Action::Recover(path));
                } else if let Err(e) = self.execute(Action::Recover(path), session) {
                    self.status = e.to_string();
                }
            } else {
                self.status = "No inactive recovery checkpoints found".into();
            }
            return;
        }
        if key == 'o' || key == 'a' {
            self.picker = Some(if key == 'a' {
                FileRequest::ImportOra
            } else {
                FileRequest::OpenProject
            });
            return;
        }
        if key == 'n' {
            self.new_from_welcome = self.welcome;
            self.welcome = false;
            self.new_dialog = true;
            return;
        }
        if key == 'q' {
            self.path = PathBuf::from(&self.path)
                .with_extension("ora")
                .to_string_lossy()
                .into();
        }
        if key == 'e' {
            self.path = PathBuf::from(&self.path)
                .with_extension("png")
                .to_string_lossy()
                .into();
        }
        let action = match key {
            'z' => Action::Undo,
            'y' => Action::Redo,
            's' => Action::Save,
            'e' => Action::Export,
            'q' => Action::ExportOra,
            'f' => Action::Fill(false),
            'd' => Action::Deselect,
            'x' => Action::Fill(true),
            _ => return,
        };
        if let Err(e) = self.execute(action, session) {
            self.status = e.to_string();
        }
    }
    pub fn preserve(&self, session: &CanvasSession) -> Result<(), Failure> {
        if session.live().project().dirty() {
            if let Some(r) = &self.recovery {
                r.flush(session.live().project().state().clone())?;
            } else {
                return Err(
                    "Cannot preserve unsaved artwork: recovery unavailable; save the project first"
                        .into(),
                );
            }
        }
        Ok(())
    }
    pub fn requires_close_save(&self, session: &CanvasSession) -> bool {
        !self.welcome && (session.live().project().dirty() || self.store.is_none())
    }
    /// Persist artwork before normal close. A conflict/failure leaves the window
    /// open so the user can retry, choose Save As, or cancel.
    pub fn save_for_close(&mut self, session: &mut CanvasSession) -> Result<(), Failure> {
        if self.store.is_some() {
            self.execute(Action::Save, session)?;
        } else {
            let (store, path) = {
                let project = session.live_mut().project_mut();
                project.sync_brush_settings();
                let state = project.state().clone();
                let (store, path) = crate::drafts::create(&state, &self.new_name)?;
                project.saved_revision = project.revision;
                (store, path)
            };
            self.path = path.to_string_lossy().into_owned();
            self.status = format!(
                "Draft saved locally: {}",
                crate::drafts::display_name(&path)
            );
            self.store = Some(store);
            self.drafts = crate::drafts::list();
        }
        self.close_ready = true;
        self.close_requested = false;
        Ok(())
    }
    pub fn mark_close_save_failed(&mut self, error: impl std::fmt::Display) {
        self.status = format!("Automatic save failed: {error}");
        self.close_requested = true;
    }
    pub fn needs_autosave(&self, session: &CanvasSession) -> bool {
        self.autosave_deadline(session)
            .is_some_and(|deadline| Instant::now() >= deadline)
    }
    pub fn autosave_deadline(&self, session: &CanvasSession) -> Option<Instant> {
        let project = session.live().project();
        (self.recovery.is_some()
            && !session.live().active()
            && project.dirty()
            && self.last_submitted != project.revision)
            .then(|| self.autosave_schedule.deadline())
            .flatten()
    }
    pub fn recovery_save_pending(&self) -> bool {
        self.recovery_saving
    }
    fn persist_workspace(&self) -> Result<(), String> {
        let Some(path) = &self.preferences_path else {
            return Err("Workspace preference directory is unavailable".into());
        };
        let mut preferences = crate::workspace::WorkspacePreferences {
            dark_theme: self.appearance_mode.is_dark(self.system_dark),
            appearance_mode: Some(self.appearance_mode),
            accent_scheme: self.accent_scheme,
            custom_accent_rgb: self.custom_accent_rgb,
            brush_panel_width: self.brush_panel_width,
            show_controls: self.show_controls,
            references: self
                .references
                .iter()
                .map(|reference| reference.layout.clone())
                .collect(),
            new_document: crate::workspace::NewDocumentPreferences {
                width: self.new_width,
                height: self.new_height,
                dpi: self.new_dpi,
                unit: self.new_unit,
                background: self.new_background,
                transparent: self.new_transparent,
            },
            recent_document_sizes: self.recent_document_sizes.clone(),
            recent_brush_colors: self.recent_brush_colors.clone(),
            canvas_zoom: self.navigation.zoom,
            canvas_pan: self.navigation.pan,
            inspector_tab: self.inspector_tab,
            high_contrast: self.high_contrast,
            ui_scale: self.ui_scale,
            palette_dock: self.palette_dock,
            palette_collapsed: self.palette_collapsed,
            handedness: self.handedness,
            palette_position: self.palette_position,
            ..Default::default()
        };
        if !preferences.validate() {
            return Err("Workspace preferences failed validation".into());
        }
        let bytes = serde_json::to_vec_pretty(&preferences).map_err(|e| e.to_string())?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
        std::fs::write(&temporary, bytes).map_err(|e| e.to_string())?;
        #[cfg(windows)]
        if path.exists() {
            std::fs::remove_file(path).map_err(|e| e.to_string())?;
        }
        std::fs::rename(&temporary, path).map_err(|e| e.to_string())
    }
    fn widgets(
        &mut self,
        ctx: &egui::Context,
        session: &mut CanvasSession,
        actions: &mut Vec<Action>,
        central: &mut egui::Rect,
        approved: &mut bool,
    ) {
        let mut workspace_changed = self.workspace_needs_save
            && self.touch_routes.active_target().is_none()
            && self.pan_cursor.is_none();
        if workspace_changed {
            self.workspace_needs_save = false;
        }
        if !self.welcome {
            egui::TopBottomPanel::top("document tools").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Rust Paint Lab");
                    egui::MenuBar::new().ui(ui, |ui| {
                        ui.menu_button("File", |ui| {
                            if ui.button("New document…     Ctrl+N").clicked() {
                                self.new_from_welcome = false;
                                self.new_dialog = true;
                                ui.close();
                            }
                            if crate::studio_icons::menu_button(
                                ui,
                                crate::studio_icons::Icon::Open,
                                "Open project…     Ctrl+O",
                            )
                            .clicked()
                            {
                                self.picker_from_welcome = false;
                                self.picker = Some(FileRequest::OpenProject);
                                ui.close();
                            }
                            ui.separator();
                            if crate::studio_icons::menu_button(
                                ui,
                                crate::studio_icons::Icon::Save,
                                "Save     Ctrl+S",
                            )
                            .clicked()
                            {
                                actions.push(Action::Save);
                                ui.close();
                            }
                            if ui.button("Save As…").clicked() {
                                self.picker = Some(FileRequest::SaveProjectAs);
                                ui.close();
                            }
                            ui.menu_button("Import", |ui| {
                                if ui.button("Raster image (PNG)…").clicked() {
                                    self.picker = Some(FileRequest::ImportPng);
                                    ui.close();
                                }
                                if ui.button("OpenRaster project…").clicked() {
                                    self.picker = Some(FileRequest::ImportOra);
                                    ui.close();
                                }
                                if ui.button("Reference images…").clicked() {
                                    self.picker = Some(FileRequest::AddReferences);
                                    ui.close();
                                }
                            });
                            ui.menu_button("Export", |ui| {
                                if ui.button("PNG image…     Ctrl+E").clicked() {
                                    self.path = PathBuf::from(&self.path)
                                        .with_extension("png")
                                        .to_string_lossy()
                                        .into();
                                    self.picker = Some(FileRequest::ExportPng);
                                    ui.close();
                                }
                                if ui.button("OpenRaster project…     Ctrl+Shift+E").clicked() {
                                    self.path = PathBuf::from(&self.path)
                                        .with_extension("ora")
                                        .to_string_lossy()
                                        .into();
                                    self.picker = Some(FileRequest::ExportOra);
                                    ui.close();
                                }
                            });
                            ui.separator();
                            if ui.button("Load brush preset…     Ctrl+Alt+B").clicked() {
                                self.path = PathBuf::from(&self.path)
                                    .with_extension("rplbrush")
                                    .to_string_lossy()
                                    .into();
                                self.picker = Some(FileRequest::LoadBrush);
                                ui.close();
                            }
                            if ui.button("Save brush preset…     Ctrl+Shift+B").clicked() {
                                self.path = PathBuf::from(&self.path)
                                    .with_extension("rplbrush")
                                    .to_string_lossy()
                                    .into();
                                self.picker = Some(FileRequest::SaveBrush);
                                ui.close();
                            }
                            if !self.recent.paths.is_empty() {
                                ui.menu_button("Recent projects", |ui| {
                                    for path in self.recent.paths.clone().into_iter().take(8) {
                                        let label =
                                            path.file_name().unwrap_or_default().to_string_lossy();
                                        if ui
                                            .button(label)
                                            .on_hover_text(path.display().to_string())
                                            .clicked()
                                        {
                                            actions.push(Action::Recent(path));
                                            ui.close();
                                        }
                                    }
                                });
                            }
                            if ui.button("Recovery manager…").clicked() {
                                self.recovery_files = paint_document::recovery::directory()
                                    .map(|directory| {
                                        paint_document::recovery::available(&directory)
                                    })
                                    .unwrap_or_default();
                                self.recovery_manager = true;
                                self.recovery_manager_from_home = false;
                                ui.close();
                            }
                        });
                        ui.menu_button("Edit", |ui| {
                            if crate::studio_icons::menu_button_enabled(
                                ui,
                                crate::studio_icons::Icon::Undo,
                                "Undo     Ctrl+Z",
                                session.live().project().undo_count() > 0,
                            )
                            .clicked()
                            {
                                actions.push(Action::Undo);
                                ui.close();
                            }
                            if crate::studio_icons::menu_button_enabled(
                                ui,
                                crate::studio_icons::Icon::Redo,
                                "Redo     Ctrl+Y",
                                session.live().project().redo_count() > 0,
                            )
                            .clicked()
                            {
                                actions.push(Action::Redo);
                                ui.close();
                            }
                            ui.separator();
                            if crate::studio_icons::menu_button(
                                ui,
                                crate::studio_icons::Icon::Deselect,
                                "Deselect     Ctrl+D",
                            )
                            .clicked()
                            {
                                actions.push(Action::Deselect);
                                ui.close();
                            }
                            if crate::studio_icons::menu_button(
                                ui,
                                crate::studio_icons::Icon::Fill,
                                "Fill selection     Ctrl+F",
                            )
                            .clicked()
                            {
                                actions.push(Action::Fill(false));
                                ui.close();
                            }
                            if crate::studio_icons::menu_button(
                                ui,
                                crate::studio_icons::Icon::Eraser,
                                "Erase selection     Ctrl+X",
                            )
                            .clicked()
                            {
                                actions.push(Action::Fill(true));
                                ui.close();
                            }
                        });
                        ui.menu_button("Image", |ui| {
                            if ui.button("Crop to selection").clicked() {
                                actions.push(Action::Crop);
                                ui.close();
                            }
                        });
                        ui.menu_button("Layer", |ui| {
                            if ui.button("New layer").clicked() {
                                actions.push(Action::Add);
                                ui.close();
                            }
                            if ui.button("Duplicate selected layer").clicked() {
                                actions.push(Action::Duplicate);
                                ui.close();
                            }
                            if ui.button("Delete selected layer").clicked() {
                                actions.push(Action::Delete);
                                ui.close();
                            }
                            if ui.button("Clear selected layer").clicked() {
                                actions.push(Action::Clear);
                                ui.close();
                            }
                        });
                        ui.menu_button("View", |ui| {
                            ui.label("Appearance");
                            for (mode, label) in [
                                (crate::workspace::AppearanceMode::Dark, "Dark"),
                                (crate::workspace::AppearanceMode::Light, "Light"),
                                (
                                    crate::workspace::AppearanceMode::FollowSystem,
                                    "Follow System",
                                ),
                            ] {
                                workspace_changed |= ui
                                    .selectable_value(&mut self.appearance_mode, mode, label)
                                    .changed();
                            }
                            ui.separator();
                            ui.label("Accent color");
                            egui::ComboBox::from_id_salt("studio accent scheme")
                                .selected_text(match self.accent_scheme {
                                    crate::workspace::AccentScheme::Violet => "Violet",
                                    crate::workspace::AccentScheme::Blue => "Blue",
                                    crate::workspace::AccentScheme::Teal => "Teal",
                                    crate::workspace::AccentScheme::Coral => "Coral",
                                    crate::workspace::AccentScheme::Green => "Green",
                                    crate::workspace::AccentScheme::Neutral => "Neutral",
                                    crate::workspace::AccentScheme::Custom => "Custom",
                                })
                                .show_ui(ui, |ui| {
                                    for (scheme, label) in [
                                        (crate::workspace::AccentScheme::Violet, "Violet"),
                                        (crate::workspace::AccentScheme::Blue, "Blue"),
                                        (crate::workspace::AccentScheme::Teal, "Teal"),
                                        (crate::workspace::AccentScheme::Coral, "Coral"),
                                        (crate::workspace::AccentScheme::Green, "Green"),
                                        (crate::workspace::AccentScheme::Neutral, "Neutral"),
                                        (crate::workspace::AccentScheme::Custom, "Custom HEX…"),
                                    ] {
                                        workspace_changed |= ui
                                            .selectable_value(
                                                &mut self.accent_scheme,
                                                scheme,
                                                label,
                                            )
                                            .changed();
                                    }
                                });
                            if self.accent_scheme == crate::workspace::AccentScheme::Custom
                                && self.custom_accent_rgb.is_none()
                            {
                                self.custom_accent_rgb = Some([0xA7, 0x8B, 0xFA]);
                                workspace_changed = true;
                            }
                            if self.accent_scheme == crate::workspace::AccentScheme::Custom {
                                ui.horizontal(|ui| {
                                    ui.label("HEX");
                                    ui.add(
                                        egui::TextEdit::singleline(&mut self.custom_accent_hex)
                                            .char_limit(7)
                                            .desired_width(88.0),
                                    );
                                    let preview = crate::color_controls::parse_hex_rgb(
                                        &self.custom_accent_hex,
                                    )
                                    .or(self.custom_accent_rgb)
                                    .unwrap_or([0xA7, 0x8B, 0xFA]);
                                    let (preview_rect, _) = ui.allocate_exact_size(
                                        egui::vec2(24.0, 24.0),
                                        egui::Sense::hover(),
                                    );
                                    ui.painter().rect_filled(
                                        preview_rect.shrink(2.0),
                                        4.0,
                                        egui::Color32::from_rgb(preview[0], preview[1], preview[2]),
                                    );
                                    ui.painter().rect_stroke(
                                        preview_rect.shrink(2.0),
                                        4.0,
                                        ui.visuals().widgets.noninteractive.bg_stroke,
                                        egui::StrokeKind::Inside,
                                    );
                                    if ui.button("Apply").clicked() {
                                        if let Some(color) = crate::color_controls::parse_hex_rgb(
                                            &self.custom_accent_hex,
                                        ) {
                                            self.custom_accent_rgb = Some(color);
                                            workspace_changed = true;
                                            self.status = "Studio accent updated".into();
                                        } else {
                                            self.status =
                                                "Enter a six-digit accent such as #A78BFA".into();
                                        }
                                    }
                                });
                            }
                            ui.separator();
                            workspace_changed |= ui
                                .checkbox(&mut self.high_contrast, "High contrast controls")
                                .changed();
                            ui.horizontal(|ui| {
                                ui.label("Interface size");
                                workspace_changed |= ui
                                    .add(
                                        egui::Slider::new(&mut self.ui_scale, 0.8..=1.6)
                                            .show_value(true),
                                    )
                                    .changed();
                            });
                            if ui.button("Reset appearance").clicked() {
                                self.appearance_mode = crate::workspace::AppearanceMode::Dark;
                                self.accent_scheme = crate::workspace::AccentScheme::Violet;
                                self.custom_accent_rgb = None;
                                self.custom_accent_hex = "#A78BFA".into();
                                self.high_contrast = false;
                                self.ui_scale = 1.0;
                                workspace_changed = true;
                                ui.close();
                            }
                            ui.separator();
                            ui.label("0  Fit canvas");
                            ui.label("F11  Full screen");
                        });
                        ui.menu_button("Window", |ui| {
                            workspace_changed |= ui
                                .checkbox(&mut self.show_controls, "Show workspace panel")
                                .changed();
                            if ui.button("Reset workspace layout").clicked() {
                                self.show_controls = true;
                                self.brush_panel_width = 260.0;
                                self.inspector_tab = crate::workspace::InspectorTab::Layers;
                                self.palette_dock = match self.handedness {
                                    crate::workspace::Handedness::Right => {
                                        crate::workspace::PaletteDock::Left
                                    }
                                    crate::workspace::Handedness::Left => {
                                        crate::workspace::PaletteDock::Right
                                    }
                                };
                                self.palette_collapsed = false;
                                self.palette_position = [72.0, 64.0];
                                self.navigation = Default::default();
                                for (index, reference) in self.references.iter_mut().enumerate() {
                                    reference.layout.position =
                                        [40.0 + index as f32 * 28.0, 80.0 + index as f32 * 24.0];
                                    reference.layout.size = [360.0, 300.0];
                                    reference.layout.view_zoom = 1.0;
                                    reference.layout.view_pan = [0.0, 0.0];
                                    reference.layout.position_locked = false;
                                }
                                workspace_changed = true;
                                ui.close();
                            }
                            ui.separator();
                            ui.label("Tool palette");
                            for (dock, label) in [
                                (crate::workspace::PaletteDock::Left, "Dock left"),
                                (crate::workspace::PaletteDock::Right, "Dock right"),
                                (crate::workspace::PaletteDock::Floating, "Float palette"),
                            ] {
                                if ui
                                    .selectable_value(&mut self.palette_dock, dock, label)
                                    .changed()
                                {
                                    workspace_changed = true;
                                }
                            }
                            ui.label("Handedness");
                            for (hand, label) in [
                                (crate::workspace::Handedness::Right, "Right handed"),
                                (crate::workspace::Handedness::Left, "Left handed"),
                            ] {
                                if ui
                                    .selectable_value(&mut self.handedness, hand, label)
                                    .changed()
                                {
                                    if self.palette_dock != crate::workspace::PaletteDock::Floating
                                    {
                                        self.palette_dock =
                                            if hand == crate::workspace::Handedness::Right {
                                                crate::workspace::PaletteDock::Left
                                            } else {
                                                crate::workspace::PaletteDock::Right
                                            };
                                    }
                                    workspace_changed = true;
                                }
                            }
                            ui.separator();
                            let can_add = self.references.len() < crate::workspace::MAX_REFERENCES;
                            if ui
                                .add_enabled(can_add, egui::Button::new("Add reference images…"))
                                .clicked()
                            {
                                self.picker = Some(FileRequest::AddReferences);
                                ui.close();
                            }
                            ui.label(format!(
                                "Reference board: {} / {}",
                                self.references.len(),
                                crate::workspace::MAX_REFERENCES
                            ));
                            ui.label("Layers and brush settings are in the right panel");
                        });
                        ui.menu_button("Help", |ui| {
                            ui.label("Rust Paint Lab — original alpha studio");
                            ui.separator();
                            ui.label("B Brush · M Rectangle · E Ellipse");
                            ui.label("K Lasso · I Eyedropper");
                            ui.label("Ctrl+N New · Ctrl+O Open · Ctrl+S Save");
                            ui.label("Ctrl+Z / Ctrl+Y Undo / Redo · Esc Close");
                        });
                    });
                });
            });

            let old_tool = self.tool;
            let screen = ctx.content_rect();
            let palette_pos = match self.palette_dock {
                crate::workspace::PaletteDock::Left => [screen.left() + 8.0, screen.top() + 64.0],
                crate::workspace::PaletteDock::Right => {
                    [screen.right() - 64.0, screen.top() + 64.0]
                }
                crate::workspace::PaletteDock::Floating => self.palette_position,
            };
            let palette_response = egui::Area::new(egui::Id::new("tool palette"))
                .default_pos(egui::pos2(palette_pos[0], palette_pos[1]))
                .movable(true)
                .constrain_to(screen.shrink(4.0))
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(ui.visuals().window_fill)
                        .stroke(ui.visuals().window_stroke)
                        .corner_radius(12.0)
                        .inner_margin(6.0)
                        .show(ui, |ui| {
                            ui.vertical_centered(|ui| {
                                ui.add_space(8.0);
                                if ui
                                    .small_button(if self.palette_collapsed {
                                        "Expand tools"
                                    } else {
                                        "Collapse tools"
                                    })
                                    .clicked()
                                {
                                    self.palette_collapsed = !self.palette_collapsed;
                                    workspace_changed = true;
                                }
                                for (tool, icon, name, shortcut) in [
                                    (
                                        EditTool::Brush,
                                        crate::studio_icons::Icon::Brush,
                                        "Brush",
                                        "B",
                                    ),
                                    (
                                        EditTool::Rectangle,
                                        crate::studio_icons::Icon::Rectangle,
                                        "Rectangle selection",
                                        "M",
                                    ),
                                    (
                                        EditTool::Ellipse,
                                        crate::studio_icons::Icon::Ellipse,
                                        "Ellipse selection",
                                        "E",
                                    ),
                                    (
                                        EditTool::Lasso,
                                        crate::studio_icons::Icon::Lasso,
                                        "Freehand lasso",
                                        "K",
                                    ),
                                    (
                                        EditTool::Eyedropper,
                                        crate::studio_icons::Icon::Eyedropper,
                                        "Eyedropper",
                                        "I",
                                    ),
                                ] {
                                    let response = if self.palette_collapsed {
                                        crate::studio_icons::button(
                                            ui,
                                            icon,
                                            name,
                                            Some(shortcut),
                                            self.tool == tool,
                                            [46.0, 46.0],
                                        )
                                    } else {
                                        ui.horizontal(|ui| {
                                            let response = crate::studio_icons::button(
                                                ui,
                                                icon,
                                                name,
                                                Some(shortcut),
                                                self.tool == tool,
                                                [46.0, 46.0],
                                            );
                                            ui.label(name);
                                            response
                                        })
                                        .inner
                                    };
                                    if response.clicked() {
                                        self.tool = tool;
                                        if tool == EditTool::Brush {
                                            self.show_controls = true;
                                            self.inspector_tab =
                                                crate::workspace::InspectorTab::Brush;
                                            workspace_changed = true;
                                        }
                                    }
                                }
                                if !self.palette_collapsed {
                                    let layers =
                                        ui.button("Layers").on_hover_text("Open layer manager");
                                    if layers.clicked() {
                                        self.show_controls = true;
                                        self.inspector_tab = crate::workspace::InspectorTab::Layers;
                                        workspace_changed = true;
                                    }
                                } else if crate::studio_icons::button(
                                    ui,
                                    crate::studio_icons::Icon::Layers,
                                    "Layers",
                                    None,
                                    false,
                                    [46.0, 46.0],
                                )
                                .clicked()
                                {
                                    self.show_controls = true;
                                    self.inspector_tab = crate::workspace::InspectorTab::Layers;
                                    workspace_changed = true;
                                }
                                ui.separator();
                                let can_add_reference =
                                    self.references.len() < crate::workspace::MAX_REFERENCES;
                                if crate::studio_icons::button(
                                    ui,
                                    crate::studio_icons::Icon::Reference,
                                    "Reference Images",
                                    None,
                                    false,
                                    [46.0, 46.0],
                                )
                                .clicked()
                                {
                                    if can_add_reference {
                                        self.picker = Some(FileRequest::AddReferences);
                                    } else {
                                        self.status = format!(
                                            "Reference board is full ({} images maximum)",
                                            crate::workspace::MAX_REFERENCES
                                        );
                                    }
                                }
                                ui.separator();
                                if crate::studio_icons::button(
                                    ui,
                                    crate::studio_icons::Icon::Deselect,
                                    "Deselect",
                                    Some("Ctrl+D"),
                                    false,
                                    [46.0, 42.0],
                                )
                                .clicked()
                                {
                                    actions.push(Action::Deselect);
                                }
                                if crate::studio_icons::button(
                                    ui,
                                    crate::studio_icons::Icon::Fill,
                                    "Fill selection",
                                    Some("Ctrl+F"),
                                    false,
                                    [46.0, 42.0],
                                )
                                .clicked()
                                {
                                    actions.push(Action::Fill(false));
                                }
                                if crate::studio_icons::button(
                                    ui,
                                    crate::studio_icons::Icon::Eraser,
                                    "Erase selection",
                                    Some("Ctrl+X"),
                                    false,
                                    [46.0, 42.0],
                                )
                                .clicked()
                                {
                                    actions.push(Action::Fill(true));
                                }
                            });
                        });
                });
            let rect = palette_response.response.rect;
            let clamped = clamp_reference_position(
                [rect.min.x, rect.min.y],
                [rect.width(), rect.height()],
                screen,
            );
            self.palette_position = [clamped.x, clamped.y];
            if palette_response.response.drag_stopped() {
                if rect.left() <= screen.left() + 24.0 {
                    self.palette_dock = crate::workspace::PaletteDock::Left;
                } else if rect.right() >= screen.right() - 24.0 {
                    self.palette_dock = crate::workspace::PaletteDock::Right;
                } else {
                    self.palette_dock = crate::workspace::PaletteDock::Floating;
                }
                workspace_changed = true;
            }
            if old_tool != self.tool {
                session.mouse(MouseEvent::Interrupted, 0);
                self.edit_pointer = None;
                self.edit_points.clear();
            }

            egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
                let project = session.live().project();
                let save_status = if self.recovery_saving {
                    "Saving recovery"
                } else if project.dirty() && self.recovery.is_some() {
                    "Unsaved changes · recovery available"
                } else if project.dirty() {
                    "Unsaved changes"
                } else if self.store.is_some() {
                    "Saved"
                } else {
                    "Untitled document"
                };
                ui.horizontal(|ui| {
                    ui.strong(save_status);
                    ui.separator();
                    if let Some(error) = session.live().document_error() {
                        ui.colored_label(egui::Color32::LIGHT_RED, error);
                    } else {
                        ui.label(&self.status);
                    }
                });
                ui.small(format!(
                    "{} × {} px · {} dpi",
                    session.live().project().state().width,
                    session.live().project().state().height,
                    session.live().project().state().dpi
                ));
            });

            if self.show_controls {
                let side_panel = egui::SidePanel::right("painting controls")
                    .default_width(self.brush_panel_width)
                    .resizable(true)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for (tab, label) in [
                                    (crate::workspace::InspectorTab::Layers, "Layers"),
                                    (crate::workspace::InspectorTab::Brush, "Brush"),
                                    (crate::workspace::InspectorTab::Edit, "Edit"),
                                ] {
                                    if ui
                                        .selectable_label(self.inspector_tab == tab, label)
                                        .clicked()
                                    {
                                        self.inspector_tab = tab;
                                        workspace_changed = true;
                                    }
                                }
                            });
                            ui.separator();
                            let project = session.live_mut().project_mut();

                            if self.inspector_tab == crate::workspace::InspectorTab::Brush {
                                ui.heading("Brush presets and color");
                                egui::ComboBox::from_label("Preset")
                                    .selected_text(brush_display_name(project.brush.kind))
                                    .show_ui(ui, |ui| {
                                        for k in [
                                            BrushKind::Round,
                                            BrushKind::Ink,
                                            BrushKind::Pencil,
                                            BrushKind::Marker,
                                            BrushKind::Airbrush,
                                            BrushKind::Soft,
                                            BrushKind::Textured,
                                            BrushKind::Calligraphy,
                                            BrushKind::Eraser,
                                            BrushKind::Wash,
                                        ] {
                                            if ui
                                                .selectable_label(
                                                    project.brush.kind == k,
                                                    brush_display_name(k),
                                                )
                                                .clicked()
                                            {
                                                let color = project.brush.color;
                                                project.brush = Brush::preset(k);
                                                project.brush.color = color;
                                            }
                                        }
                                    });

                                ui.horizontal(|ui| {
                                    crate::studio_icons::label(
                                        ui,
                                        crate::studio_icons::Icon::Color,
                                        "Color",
                                    );
                                    ui.color_edit_button_srgb(&mut project.brush.color);
                                    let mut eraser = project.brush.kind == BrushKind::Eraser;
                                    if ui.checkbox(&mut eraser, "Eraser mode").changed() {
                                        project.brush.kind = if eraser {
                                            BrushKind::Eraser
                                        } else {
                                            BrushKind::Round
                                        };
                                    }
                                });
                                if self.hex_color_for != Some(project.brush.color) {
                                    self.hex_color =
                                        crate::color_controls::format_hex_rgb(project.brush.color);
                                    self.hex_color_for = Some(project.brush.color);
                                }
                                ui.collapsing("Color values (sRGB)", |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label("HEX");
                                        let response = ui.add(
                                            egui::TextEdit::singleline(&mut self.hex_color)
                                                .char_limit(7)
                                                .desired_width(88.0),
                                        );
                                        let apply = ui.button("Apply").clicked()
                                            || (response.lost_focus()
                                                && ui.input(|input| {
                                                    input.key_pressed(egui::Key::Enter)
                                                }));
                                        if apply {
                                            if let Some(color) =
                                                crate::color_controls::parse_hex_rgb(
                                                    &self.hex_color,
                                                )
                                            {
                                                project.brush.color = color;
                                                self.hex_color =
                                                    crate::color_controls::format_hex_rgb(color);
                                                self.hex_color_for = Some(color);
                                                self.status = "Brush color updated".into();
                                            } else {
                                                self.status =
                                                    "Enter a six-digit color such as #4B86D1"
                                                        .into();
                                            }
                                        }
                                    });
                                    let mut rgb = project.brush.color;
                                    ui.horizontal(|ui| {
                                        ui.label("RGB");
                                        for component in &mut rgb {
                                            ui.add(
                                                egui::DragValue::new(component)
                                                    .range(0..=255)
                                                    .speed(1),
                                            );
                                        }
                                    });
                                    if rgb != project.brush.color {
                                        project.brush.color = rgb;
                                        self.hex_color = crate::color_controls::format_hex_rgb(rgb);
                                        self.hex_color_for = Some(rgb);
                                    }
                                });
                                ui.collapsing("Recent brush colors", |ui| {
                                    if self.recent_brush_colors.is_empty() {
                                        ui.small("Colors you choose will appear here.");
                                    } else {
                                        ui.horizontal_wrapped(|ui| {
                                            for color in self.recent_brush_colors.clone() {
                                                let hex =
                                                    crate::color_controls::format_hex_rgb(color);
                                                if crate::studio_icons::color_swatch(
                                                    ui,
                                                    egui::Color32::from_rgb(
                                                        color[0], color[1], color[2],
                                                    ),
                                                    &format!("Brush color {hex}"),
                                                )
                                                .clicked()
                                                {
                                                    project.brush.color = color;
                                                    self.hex_color = hex;
                                                    self.hex_color_for = Some(color);
                                                }
                                            }
                                        });
                                    }
                                });

                                ui.horizontal(|ui| {
                                    ui.label("Size");
                                    ui.add(
                                        egui::DragValue::new(&mut project.brush.size)
                                            .range(1.0..=256.0)
                                            .speed(0.25)
                                            .suffix(" px"),
                                    );
                                });
                                ui.add(
                                    egui::Slider::new(&mut project.brush.size, 1.0..=256.0)
                                        .show_value(false)
                                        .text("Size"),
                                );
                                ui.horizontal(|ui| {
                                    ui.label("Opacity");
                                    ui.add(
                                        egui::DragValue::new(&mut project.brush.opacity)
                                            .range(0.0..=1.0)
                                            .speed(0.01),
                                    );
                                });
                                ui.add(
                                    egui::Slider::new(&mut project.brush.opacity, 0.0..=1.0)
                                        .show_value(false)
                                        .text("Opacity"),
                                );
                                ui.collapsing("More brush settings", |ui| {
                                    ui.add(
                                        egui::Slider::new(&mut project.brush.flow, 0.0..=1.0)
                                            .text("Flow"),
                                    );
                                    ui.add(
                                        egui::Slider::new(&mut project.brush.hardness, 0.0..=1.0)
                                            .text("Hardness"),
                                    );
                                    ui.add(
                                        egui::Slider::new(&mut project.brush.spacing, 0.05..=1.0)
                                            .text("Spacing"),
                                    );
                                    ui.add(
                                        egui::Slider::new(&mut project.brush.smoothing, 0.0..=1.0)
                                            .text("Smoothing"),
                                    );
                                    ui.add(
                                        egui::Slider::new(
                                            &mut project.brush.angle,
                                            -std::f64::consts::PI..=std::f64::consts::PI,
                                        )
                                        .text("Angle"),
                                    );
                                    ui.checkbox(&mut project.brush.pressure_size, "Pressure size");
                                    ui.checkbox(
                                        &mut project.brush.pressure_opacity,
                                        "Pressure opacity",
                                    );
                                    ui.checkbox(&mut project.brush.tilt, "Tilt angle");
                                });
                            }

                            if self.inspector_tab == crate::workspace::InspectorTab::Edit {
                                ui.heading("Canvas and selection");
                                let mut transparent = project.state().background.is_none();

                                if ui
                                    .checkbox(&mut transparent, "Transparent background")
                                    .changed()
                                    && let Err(e) = project.edit(|s| {
                                        s.background =
                                            if transparent { None } else { Some([255; 3]) };
                                        Ok(())
                                    })
                                {
                                    self.status = e.to_string();
                                }

                                ui.collapsing("Edit selected pixels", |ui| {
                                    ui.label("Selected layer only; outside canvas is clipped.");

                                    ui.horizontal(|ui| {
                                        ui.label("Move X/Y");
                                        ui.add(
                                            egui::DragValue::new(&mut self.transform_offset[0])
                                                .range(-4096.0..=4096.0),
                                        );
                                        ui.add(
                                            egui::DragValue::new(&mut self.transform_offset[1])
                                                .range(-4096.0..=4096.0),
                                        );
                                    });

                                    ui.add(
                                        egui::Slider::new(&mut self.transform_scale, 0.1..=4.0)
                                            .text("Scale"),
                                    );
                                    ui.add(
                                        egui::Slider::new(
                                            &mut self.transform_angle,
                                            -180.0..=180.0,
                                        )
                                        .text("Rotation"),
                                    );

                                    let bounds = project
                                        .state()
                                        .selection
                                        .as_ref()
                                        .map(Selection::bounds)
                                        .unwrap_or([
                                            0.0,
                                            0.0,
                                            project.state().width as f64,
                                            project.state().height as f64,
                                        ]);

                                    let cx = (bounds[0] + bounds[2]) / 2.0;
                                    let cy = (bounds[1] + bounds[3]) / 2.0;

                                    if ui.button("Apply transform").clicked() {
                                        let a = self.transform_angle.to_radians();
                                        let (sin, cos) = a.sin_cos();
                                        let c = cos * self.transform_scale;
                                        let b = sin * self.transform_scale;
                                        actions.push(Action::Transform([
                                            c,
                                            b,
                                            -b,
                                            c,
                                            cx - c * cx + b * cy + self.transform_offset[0],
                                            cy - b * cx - c * cy + self.transform_offset[1],
                                        ]));
                                    }

                                    ui.horizontal(|ui| {
                                        if ui.button("Flip H").clicked() {
                                            actions.push(Action::Transform([
                                                -1.0,
                                                0.0,
                                                0.0,
                                                1.0,
                                                2.0 * cx,
                                                0.0,
                                            ]));
                                        }

                                        if ui.button("Flip V").clicked() {
                                            actions.push(Action::Transform([
                                                1.0,
                                                0.0,
                                                0.0,
                                                -1.0,
                                                0.0,
                                                2.0 * cy,
                                            ]));
                                        }
                                    });

                                    if ui
                                        .add_enabled(
                                            project.state().selection.is_some(),
                                            egui::Button::new("Crop document to selection bounds"),
                                        )
                                        .clicked()
                                    {
                                        actions.push(Action::Crop);
                                    }
                                });
                            }

                            if self.inspector_tab == crate::workspace::InspectorTab::Layers {
                                let selected = project.state().selected;
                                let layer_count = project.state().layers.len();
                                let selected_index = project
                                    .state()
                                    .layers
                                    .iter()
                                    .position(|layer| layer.id == selected)
                                    .unwrap_or(0);

                                ui.separator();
                                ui.heading("Layers");
                                ui.small(format!(
                                    "{layer_count} layers · top of list is frontmost"
                                ));
                                ui.horizontal(|ui| {
                                    if ui.button("New layer").clicked() {
                                        actions.push(Action::Add);
                                    }
                                    if ui.button("Duplicate").clicked() {
                                        actions.push(Action::Duplicate);
                                    }
                                    if ui
                                        .add_enabled(layer_count > 1, egui::Button::new("Delete"))
                                        .clicked()
                                    {
                                        actions.push(Action::Delete);
                                    }
                                });

                                ui.horizontal(|ui| {
                                    if ui
                                        .add_enabled(
                                            selected_index + 1 < layer_count,
                                            egui::Button::new("Raise"),
                                        )
                                        .clicked()
                                    {
                                        actions.push(Action::Move(1));
                                    }
                                    if ui
                                        .add_enabled(selected_index > 0, egui::Button::new("Lower"))
                                        .clicked()
                                    {
                                        actions.push(Action::Move(-1));
                                    }
                                    if ui.button("Clear layer").clicked() {
                                        actions.push(Action::Clear);
                                    }
                                });

                                let layers: Vec<_> = project
                                    .state()
                                    .layers
                                    .iter()
                                    .rev()
                                    .map(|l| {
                                        (
                                            l.id,
                                            l.name.clone(),
                                            l.visible,
                                            l.locked,
                                            l.opacity,
                                            l.blend,
                                        )
                                    })
                                    .collect();

                                for (
                                    id,
                                    mut name,
                                    mut visible,
                                    mut locked,
                                    mut opacity,
                                    mut blend,
                                ) in layers
                                {
                                    ui.group(|ui| {
                                        let mut changed = false;
                                        ui.horizontal(|ui| {
                                            if ui.selectable_label(id == selected, &name).clicked()
                                            {
                                                actions.push(Action::Select(id));
                                            }
                                            changed |=
                                                ui.checkbox(&mut visible, "Visible").changed();
                                            changed |= ui.checkbox(&mut locked, "Locked").changed();
                                        });
                                        egui::CollapsingHeader::new("Layer settings")
                                            .id_salt(("layer-settings", id))
                                            .show(ui, |ui| {
                                                changed |= ui
                                                    .add(
                                                        egui::TextEdit::singleline(&mut name)
                                                            .char_limit(64)
                                                            .hint_text("Layer name"),
                                                    )
                                                    .changed();
                                                let mut opacity_percent =
                                                    f32::from(opacity) * 100.0 / 255.0;
                                                if ui
                                                    .add(
                                                        egui::Slider::new(
                                                            &mut opacity_percent,
                                                            0.0..=100.0,
                                                        )
                                                        .suffix("%")
                                                        .text("Opacity"),
                                                    )
                                                    .changed()
                                                {
                                                    opacity =
                                                        (opacity_percent * 2.55).round() as u8;
                                                    changed = true;
                                                }
                                                let old = blend;
                                                egui::ComboBox::from_id_salt(("blend", id))
                                                    .selected_text(format!("{blend:?}"))
                                                    .show_ui(ui, |ui| {
                                                        for b in [
                                                            Blend::Normal,
                                                            Blend::Multiply,
                                                            Blend::Screen,
                                                            Blend::Overlay,
                                                        ] {
                                                            ui.selectable_value(
                                                                &mut blend,
                                                                b,
                                                                format!("{b:?}"),
                                                            );
                                                        }
                                                    });
                                                changed |= old != blend;
                                            });
                                        if changed {
                                            actions.push(Action::Layer(
                                                id, name, visible, locked, opacity, blend,
                                            ));
                                        }
                                    });
                                }

                                ui.separator();
                                ui.label("Ctrl-Z / Ctrl-Y: history");
                            }
                        });
                    });
                let panel_width = side_panel.response.rect.width();
                if (panel_width - self.brush_panel_width).abs() > 0.5 {
                    self.brush_panel_width = panel_width.clamp(180.0, 640.0);
                    workspace_changed = true;
                }
            }

            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    *central = ui.available_rect_before_wrap();
                });

            let screen_viewport = ctx.content_rect();
            for (index, reference) in self.references.iter_mut().enumerate() {
                let title = reference
                    .layout
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Reference");
                let reference_id = reference.layout.path.clone();
                let id = egui::Id::new(("reference-board", reference_id));
                let mut open = true;
                let mut remove_requested = false;
                let mut center_requested = false;
                let mut reset_position_requested = false;
                let before = reference.layout.clone();
                let window_size = [
                    before.size[0].min(screen_viewport.width().max(1.0)),
                    before.size[1].min(screen_viewport.height().max(1.0)),
                ];
                let desired_pos =
                    clamp_reference_position(before.position, window_size, screen_viewport);
                let area = egui::AreaState::load(ctx, id);
                let current_pos = area.map(|state| state.left_top_pos());
                let correction = current_pos.and_then(|current| {
                    let safe_current = clamp_reference_position(
                        [current.x, current.y],
                        window_size,
                        screen_viewport,
                    );
                    let persisted_differs = current.distance(desired_pos) > 1.0
                        && before.position != [current.x, current.y];
                    let target = if persisted_differs {
                        desired_pos
                    } else {
                        safe_current
                    };
                    (current.distance(target) > 1.0).then_some(target)
                });
                let mut window = egui::Window::new(title)
                    .id(id)
                    .open(&mut open)
                    .movable(!before.position_locked)
                    .resizable(true)
                    .max_size(screen_viewport.size())
                    .default_size(egui::vec2(window_size[0], window_size[1]));
                if let Some(pos) = correction {
                    window = window.current_pos(pos);
                } else {
                    window = window.default_pos(desired_pos);
                }
                let response = window.show(ctx, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        if ui.small_button("−").on_hover_text("Zoom out").clicked() {
                            if let Some(viewport) = reference.viewport {
                                reference_zoom_about(
                                    &mut reference.layout,
                                    viewport,
                                    viewport.center(),
                                    viewport.center(),
                                    0.8,
                                );
                            } else {
                                reference.layout.view_zoom =
                                    (reference.layout.view_zoom * 0.8).max(0.25);
                            }
                        }
                        if ui.small_button("+").on_hover_text("Zoom in").clicked() {
                            if let Some(viewport) = reference.viewport {
                                reference_zoom_about(
                                    &mut reference.layout,
                                    viewport,
                                    viewport.center(),
                                    viewport.center(),
                                    1.25,
                                );
                            } else {
                                reference.layout.view_zoom =
                                    (reference.layout.view_zoom * 1.25).min(8.0);
                            }
                        }
                        if ui.small_button("Fit").on_hover_text("Fit image").clicked() {
                            reference.layout.view_zoom = 1.0;
                            reference.layout.view_pan = [0.0, 0.0];
                        }
                        if ui.small_button("Reset view").clicked() {
                            reference.layout.view_zoom = 1.0;
                            reference.layout.view_pan = [0.0, 0.0];
                        }
                        if ui
                            .small_button("Center")
                            .on_hover_text("Center reference panel")
                            .clicked()
                        {
                            center_requested = true;
                        }
                        if ui
                            .small_button("Reset pos")
                            .on_hover_text("Reset reference panel position")
                            .clicked()
                        {
                            reset_position_requested = true;
                        }
                        ui.checkbox(&mut reference.layout.position_locked, "Lock")
                            .on_hover_text("Lock floating panel position");
                        ui.weak(format!("{:.0}%", reference.layout.view_zoom * 100.0));
                        if ui.button("Remove").clicked() {
                            remove_requested = true;
                        }
                    });
                    ui.add(
                        egui::Slider::new(&mut reference.layout.opacity, 0.05..=1.0)
                            .text("Opacity"),
                    );
                    let available = ui.available_size().max(egui::vec2(48.0, 48.0));
                    let source_size = reference.texture.size_vec2();
                    let (viewport, image_response) =
                        ui.allocate_exact_size(available, egui::Sense::drag());
                    image_response.clone().on_hover_text(format!(
                        "Original image: {} × {} pixels",
                        reference.source_size[0], reference.source_size[1]
                    ));
                    reference.viewport = Some(viewport);
                    if image_response.hovered() {
                        let scroll_y = ui.input(|input| input.smooth_scroll_delta.y);
                        if scroll_y != 0.0
                            && let Some(pointer) = image_response.hover_pos()
                        {
                            reference_zoom_about(
                                &mut reference.layout,
                                viewport,
                                pointer,
                                pointer,
                                (scroll_y * 0.002).exp(),
                            );
                        }
                    }
                    if image_response.dragged() {
                        let delta = image_response.drag_delta();
                        reference.layout.view_pan[0] =
                            (reference.layout.view_pan[0] + delta.x).clamp(-10000.0, 10000.0);
                        reference.layout.view_pan[1] =
                            (reference.layout.view_pan[1] + delta.y).clamp(-10000.0, 10000.0);
                    }
                    if let Some(image_rect) = reference_image_rect(
                        viewport,
                        source_size,
                        reference.layout.view_zoom,
                        reference.layout.view_pan,
                    ) {
                        ui.painter().with_clip_rect(viewport).image(
                            reference.texture.id(),
                            image_rect,
                            egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0)),
                            egui::Color32::from_white_alpha(
                                (reference.layout.opacity * 255.0).round() as u8,
                            ),
                        );
                    }
                    if image_response.dragged() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
                    } else if image_response.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
                    }
                });
                if let Some(response) = response {
                    let rect = response.response.rect;
                    let next_position = if center_requested {
                        centered_reference_position([rect.width(), rect.height()], screen_viewport)
                    } else if reset_position_requested {
                        clamp_reference_position(
                            [40.0 + index as f32 * 28.0, 80.0 + index as f32 * 24.0],
                            [rect.width(), rect.height()],
                            screen_viewport,
                        )
                    } else {
                        clamp_reference_position(
                            [rect.min.x, rect.min.y],
                            [rect.width(), rect.height()],
                            screen_viewport,
                        )
                    };
                    reference.layout.position = [next_position.x, next_position.y];
                    reference.layout.size = [rect.width(), rect.height()];
                    if center_requested || reset_position_requested {
                        workspace_changed = true;
                    }
                    workspace_changed |= reference.layout != before;
                }
                if !open || remove_requested {
                    workspace_changed = true;
                    reference.layout.opacity = 0.0;
                }
            }
            if self
                .references
                .iter()
                .any(|reference| reference.layout.size[0] == 0.0)
            {
                // Invalid persisted geometry is sanitized by WorkspacePreferences.
                workspace_changed = true;
            }
            self.references
                .retain(|reference| reference.layout.opacity >= 0.05);
            if workspace_changed
                && !ctx.input(|input| input.pointer.any_down())
                && let Err(error) = self.persist_workspace()
            {
                self.status = format!("Could not save workspace layout: {error}");
            }
        }

        if !self.welcome && !self.new_dialog && !self.manual_file_dialog {
            let screen = ctx.content_rect();
            egui::Area::new(egui::Id::new("canvas zoom controls"))
                .order(egui::Order::Foreground)
                .fixed_pos(egui::pos2(screen.right() - 250.0, screen.bottom() - 48.0))
                .show(ctx, |ui| {
                    egui::Frame::new()
                        .fill(ui.visuals().panel_fill)
                        .corner_radius(10.0)
                        .inner_margin(egui::Margin::same(6))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                if ui.small_button("−").on_hover_text("Zoom out").clicked() {
                                    self.workspace_needs_save |= self.navigation.pinch(
                                        self.workspace_center,
                                        self.workspace_center,
                                        self.workspace_center,
                                        0.8,
                                    );
                                }
                                ui.weak(format!("{:.0}%", self.navigation.zoom * 100.0));
                                if ui.small_button("+").on_hover_text("Zoom in").clicked() {
                                    self.workspace_needs_save |= self.navigation.pinch(
                                        self.workspace_center,
                                        self.workspace_center,
                                        self.workspace_center,
                                        1.25,
                                    );
                                }
                                if ui.button("Fit canvas").clicked() {
                                    self.navigation = Default::default();
                                    self.workspace_needs_save = true;
                                }
                            });
                        });
                });
        }

        if self.welcome {
            let screen = ctx.content_rect();
            let mut open_recent = None;
            let mut remove_draft = None;
            egui::Area::new(egui::Id::new("studio home screen"))
                .order(egui::Order::Foreground)
                .fixed_pos(screen.min)
                .interactable(true)
                .show(ctx, |ui| {
                    ui.set_min_size(screen.size());
                    ui.set_max_size(screen.size());
                    egui::Frame::new()
                        .fill(ctx.style().visuals.panel_fill)
                        .show(ui, |ui| {
                            ui.set_min_size(screen.size());
                            ui.set_max_size(screen.size());
                            egui::ScrollArea::vertical().show(ui, |ui| {
                                ui.add_space(28.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(32.0);
                                    ui.vertical(|ui| {
                                        ui.label(egui::RichText::new("RUST PAINT LAB").small().strong().color(ui.visuals().hyperlink_color));
                                        ui.add_space(8.0);
                                        ui.heading(egui::RichText::new("Make room for your next idea.").size(30.0));
                                        ui.label("Your studio, references, and artwork stay on this device.");
                                    });
                                });
                                ui.add_space(22.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.add_space(32.0);
                                    if ui.add_sized([190.0, 52.0], egui::Button::new(egui::RichText::new("＋  New document").strong())).clicked() {
                                        self.picker_from_welcome = false;
                                        if !self.test_path_dialog { self.new_name = "Untitled".into(); }
                                        self.new_from_welcome = true;
                                        self.new_dialog = true;
                                        self.welcome = false;
                                    }
                                    if ui.add_sized([180.0, 52.0], egui::Button::new("Open a project…")).clicked() {
                                        self.picker_from_welcome = true;
                                        self.picker = Some(FileRequest::OpenProject);
                                        self.welcome = false;
                                    }
                                    if ui.button("Appearance").clicked() {
                                        self.appearance_mode = if self.appearance_mode.is_dark(self.system_dark) { crate::workspace::AppearanceMode::Light } else { crate::workspace::AppearanceMode::Dark };
                                        if let Err(error) = self.persist_workspace() { self.status = format!("Could not save appearance preference: {error}"); }
                                    }
                                });
                                ui.add_space(28.0);
                                ui.horizontal(|ui| {
                                    ui.add_space(32.0);
                                    ui.vertical(|ui| {
                                        ui.heading("Recent artwork");
                                        ui.label("Projects are ordered by most recently opened.");
                                        ui.add_space(8.0);
                                        if self.recent.paths.is_empty() {
                                            egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(12.0).inner_margin(24.0).show(ui, |ui| {
                                                ui.set_min_width((screen.width() - 100.0).max(300.0));
                                                ui.label(egui::RichText::new("Your next canvas starts here").size(20.0).strong());
                                                ui.label("Create a document or open an existing .rpl project to see it in this space.");
                                            });
                                        } else {
                                            egui::Grid::new("home recent projects").num_columns(2).spacing([14.0, 14.0]).show(ui, |ui| {
                                                for (index, card) in self.recent_cards.iter().take(8).enumerate() {
                                                    let path = &card.path;
                                                    let name = path.file_stem().unwrap_or_default().to_string_lossy();
                                                    let details = if let Some(bytes) = card.bytes {
                                                        let size_mb = bytes as f64 / (1024.0 * 1024.0);
                                                        let dimensions = card.dimensions.map(|(w, h, dpi)| format!("{w} × {h} px · {dpi} DPI")).unwrap_or_else(|| "Dimensions unavailable".into());
                                                        format!("{} · {size_mb:.1} MiB\n{}", dimensions, recent_opened_label(card.opened_unix_seconds))
                                                    } else {
                                                        "File moved or unavailable\nUse Open to locate the project again".into()
                                                    };
                                                    let available = card.bytes.is_some();
                                                    let card = egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(10.0).inner_margin(14.0).show(ui, |ui| {
                                                        ui.set_min_width((screen.width() * 0.34).clamp(220.0, 420.0));
                                                        ui.set_min_height(88.0);
                                                        ui.vertical(|ui| {
                                                            ui.label(egui::RichText::new(name).strong().size(16.0));
                                                            ui.label(egui::RichText::new(details).small());
                                                            if !available { ui.colored_label(ui.visuals().error_fg_color, "Missing project"); }
                                                        });
                                                    });
                                                    if available && ui.interact(card.response.rect, egui::Id::new(("recent project", index)), egui::Sense::click()).clicked() {
                                                        open_recent = Some(path.clone());
                                                    }
                                                    if index % 2 == 1 { ui.end_row(); }
                                                }
                                            });
                                        }
                                    });
                                    ui.add_space(24.0);
                                    ui.vertical(|ui| {
                                        ui.heading("Continue safely");
                                        ui.label("Recovery and unnamed work stay separate from exported artwork.");
                                        if !self.recovery_files.is_empty() {
                                            ui.add_space(8.0);
                                            egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(10.0).inner_margin(12.0).show(ui, |ui| {
                                                ui.label(egui::RichText::new("Recoverable work").strong());
                                                if ui.button(format!("Manage {} recovery checkpoints…", self.recovery_files.len())).clicked() {
                                                    self.recovery_files = paint_document::recovery::directory()
                                                        .map(|directory| paint_document::recovery::available(&directory))
                                                        .unwrap_or_default();
                                                    self.recovery_manager = true;
                                                    self.recovery_manager_from_home = true;
                                                }
                                                for path in self.recovery_files.iter().take(4) {
                                                    let label = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unsaved document").replace(['_', '-'], " ");
                                                    if ui.button(format!("Recover · {label}")).clicked() { actions.push(Action::Recover(path.clone())); self.welcome = false; }
                                                }
                                            });
                                        } else if ui.button("Recovery manager…").clicked() {
                                            self.recovery_files = paint_document::recovery::directory()
                                                .map(|directory| paint_document::recovery::available(&directory))
                                                .unwrap_or_default();
                                            self.recovery_manager = true;
                                            self.recovery_manager_from_home = true;
                                        }
                                        if !self.drafts.is_empty() {
                                            ui.add_space(10.0);
                                            egui::Frame::new().fill(ui.visuals().faint_bg_color).corner_radius(10.0).inner_margin(12.0).show(ui, |ui| {
                                                ui.label(egui::RichText::new("Saved drafts").strong());
                                                for path in self.drafts.iter().take(crate::drafts::MAX_DRAFTS) {
                                                    ui.horizontal(|ui| {
                                                        if ui.button(crate::drafts::display_name(path)).clicked() { self.picker_from_welcome = true; actions.push(Action::Recent(path.clone())); self.welcome = false; }
                                                        if ui.small_button("Remove…").clicked() { remove_draft = Some(path.clone()); }
                                                    });
                                                }
                                            });
                                        }
                                    });
                                });
                                ui.add_space(30.0);
                                ui.horizontal(|ui| { ui.add_space(32.0); ui.small("Ctrl+N New document · Ctrl+O Open · your projects and references remain local"); });
                                ui.add_space(22.0);
                            });
                        });
                });
            if let Some(path) = open_recent {
                self.picker_from_welcome = true;
                actions.push(Action::Recent(path));
                self.welcome = false;
            }
            if let Some(path) = remove_draft {
                self.draft_cleanup = Some(path);
            }
        } else if self.recovery_manager && self.cleanup.is_none() {
            egui::Modal::new(egui::Id::new("recovery manager")).show(ctx, |ui| {
                ui.set_min_width(520.0);
                ui.heading("Artwork recovery manager");
                ui.label("These local checkpoints are separate from saved projects. Recover opens a copy as a new unsaved document.");
                if self.recovery_files.is_empty() {
                    ui.label("No inactive recovery checkpoints are available.");
                } else {
                    egui::ScrollArea::vertical().max_height(360.0).show(ui, |ui| {
                        for path in self.recovery_files.clone() {
                            let name = path.file_stem().and_then(|name| name.to_str())
                                .unwrap_or("Unsaved artwork")
                                .strip_prefix("recovery-")
                                .unwrap_or_else(|| path.file_stem().and_then(|name| name.to_str()).unwrap_or("Unsaved artwork"))
                                .replace(['_', '-'], " ");
                            let bytes = std::fs::metadata(&path).ok().map(|metadata| metadata.len());
                            let dimensions = storage::peek_dimensions(&path)
                                .map(|(width, height, dpi)| format!("{width} × {height} px · {dpi} DPI"))
                                .unwrap_or_else(|| "Project dimensions unavailable".into());
                            egui::Frame::new().fill(ui.visuals().faint_bg_color)
                                .corner_radius(8.0).inner_margin(10.0).show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.vertical(|ui| {
                                            ui.strong(name);
                                            ui.label(dimensions);
                                            ui.label(bytes.map(|size| format!("{} KiB", size.div_ceil(1024)))
                                                .unwrap_or_else(|| "Checkpoint file unavailable".into()));
                                        });
                                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                            if ui.button("Remove…").clicked() {
                                                self.cleanup = Some(path.clone());
                                            }
                                            if ui.button("Recover").clicked() {
                                                actions.push(Action::Recover(path.clone()));
                                                self.recovery_manager = false;
                                                self.welcome = false;
                                            }
                                        });
                                    });
                                });
                            ui.add_space(6.0);
                        }
                    });
                }
                ui.separator();
                if ui.button("Done").clicked() {
                    self.recovery_manager = false;
                    if self.recovery_manager_from_home {
                        self.welcome = true;
                    }
                    self.recovery_manager_from_home = false;
                }
            });
        } else if let Some(path) = self.cleanup.clone() {
            egui::Modal::new(egui::Id::new("delete recovery checkpoint")).show(ctx, |ui| {
                ui.heading("Delete recovery checkpoint?");
                ui.label(path.to_string_lossy());
                ui.label("Deletes this checkpoint and its backup. First verify an independent saved project. Active sessions and ordinary artwork files are protected.");
                if ui.button("Delete this recovery and backup").clicked() {
                    actions.push(Action::RemoveRecovery(path));
                    self.cleanup = None;
                }
                if ui.button("Cancel").clicked() {
                    self.cleanup = None;
                }
            });
        } else if let Some(path) = self.draft_cleanup.clone() {
            egui::Modal::new(egui::Id::new("delete persistent draft")).show(ctx, |ui| {
                ui.heading("Remove this saved draft?");
                ui.label(crate::drafts::display_name(&path));
                ui.label("This permanently removes the selected draft and its backup. It does not affect other projects or the active document.");
                if ui.button("Remove draft and backup").clicked() {
                    actions.push(Action::RemoveDraft(path));
                    self.draft_cleanup = None;
                }
                if ui.button("Cancel").clicked() {
                    self.draft_cleanup = None;
                }
            });
        } else if self.new_dialog {
            egui::Modal::new(egui::Id::new("new document")).show(ctx, |ui| {
                ui.set_min_width(560.0);
                ui.heading("Create a new canvas");
                ui.label("Choose a starting format, then adjust exact dimensions if needed.");
                ui.horizontal(|ui| {
                    for (label, width, height) in [
                        ("Portrait", 1800.0, 2400.0),
                        ("Landscape", 2400.0, 1800.0),
                        ("Square", 2048.0, 2048.0),
                    ] {
                        let selected = (self.new_width - width).abs() < 0.5
                            && (self.new_height - height).abs() < 0.5;
                        if ui
                            .add_sized(
                                [132.0, 62.0],
                                egui::Button::selectable(selected, label),
                            )
                            .clicked()
                        {
                            self.new_unit = crate::document_setup::DocumentUnit::Pixels;
                            self.new_width = width;
                            self.new_height = height;
                        }
                    }
                });
                let preview_width: f32 = 128.0;
                let aspect = (self.new_height / self.new_width).clamp(0.25, 4.0) as f32;
                let preview_size = [preview_width.min(128.0 / aspect), (preview_width * aspect).min(128.0)];
                ui.horizontal(|ui| {
                    let (preview, _) = ui.allocate_exact_size(
                        egui::vec2(preview_size[0], preview_size[1]),
                        egui::Sense::hover(),
                    );
                    let painter = ui.painter();
                    if self.new_transparent {
                        let cell = 8.0;
                        let colors = [ui.visuals().faint_bg_color, ui.visuals().extreme_bg_color];
                        for y in 0..((preview.height() / cell).ceil() as i32) {
                            for x in 0..((preview.width() / cell).ceil() as i32) {
                                let rect = egui::Rect::from_min_size(
                                    preview.min + egui::vec2(x as f32 * cell, y as f32 * cell),
                                    egui::vec2(cell, cell),
                                ).intersect(preview);
                                painter.rect_filled(rect, 0.0, colors[((x + y) & 1) as usize]);
                            }
                        }
                    } else {
                        painter.rect_filled(
                            preview,
                            4.0,
                            egui::Color32::from_rgb(
                                self.new_background[0],
                                self.new_background[1],
                                self.new_background[2],
                            ),
                        );
                    }
                    painter.rect_stroke(preview, 4.0, ui.visuals().window_stroke, egui::StrokeKind::Inside);
                    ui.vertical(|ui| {
                        ui.strong("Canvas preview");
                        if let (Some(width), Some(height)) = (
                            self.new_unit.to_pixels(self.new_width, self.new_dpi),
                            self.new_unit.to_pixels(self.new_height, self.new_dpi),
                        ) {
                            ui.label(format!("{width} × {height} px"));
                        }
                        ui.label(if self.new_transparent { "Transparent background" } else { "Solid background" });
                        ui.label(format!("{} DPI", self.new_dpi));
                    });
                });
                ui.add_sized([320.0, 36.0], egui::TextEdit::singleline(&mut self.new_name).hint_text("Document name"));
                egui::CollapsingHeader::new("Advanced dimensions, presets and resolution")
                    .default_open(true)
                    .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Preset");
                    egui::ComboBox::from_id_salt("new-document-preset")
                        .selected_text("Choose preset…")
                        .show_ui(ui, |ui| {
                            for preset in crate::document_setup::PRESETS {
                                if ui.selectable_label(false, preset.name).clicked() {
                                    self.new_unit = crate::document_setup::DocumentUnit::Pixels;
                                    self.new_width = f64::from(preset.width);
                                    self.new_height = f64::from(preset.height);
                                    self.new_dpi = preset.dpi;
                                }
                            }
                        });
                });
                if !self.recent_document_sizes.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label("Recent size");
                        egui::ComboBox::from_id_salt("recent-document-size")
                            .selected_text("Choose recent dimensions…")
                            .show_ui(ui, |ui| {
                                for size in self.recent_document_sizes.iter().copied() {
                                    let label = format!(
                                        "{} × {} px · {} DPI",
                                        size.width_px, size.height_px, size.dpi
                                    );
                                    if ui.selectable_label(false, label).clicked() {
                                        self.new_unit = crate::document_setup::DocumentUnit::Pixels;
                                        self.new_width = f64::from(size.width_px);
                                        self.new_height = f64::from(size.height_px);
                                        self.new_dpi = size.dpi;
                                    }
                                }
                            });
                    });
                }
                ui.horizontal(|ui| {
                    ui.label("Units");
                    egui::ComboBox::from_id_salt("new-document-units")
                        .selected_text(self.new_unit.label())
                        .show_ui(ui, |ui| {
                            for unit in crate::document_setup::DocumentUnit::ALL {
                                if ui.selectable_label(self.new_unit == unit, unit.label()).clicked() {
                                    let width_px = self.new_unit.to_pixels(self.new_width, self.new_dpi);
                                    let height_px = self.new_unit.to_pixels(self.new_height, self.new_dpi);
                                    self.new_unit = unit;
                                    if let Some(width_px) = width_px {
                                        self.new_width = unit.from_pixels(width_px, self.new_dpi).unwrap_or(self.new_width);
                                    }
                                    if let Some(height_px) = height_px {
                                        self.new_height = unit.from_pixels(height_px, self.new_dpi).unwrap_or(self.new_height);
                                    }
                                }
                            }
                        });
                });
                ui.horizontal(|ui| {
                    ui.label("Width");
                    ui.add(egui::DragValue::new(&mut self.new_width).range(1.0..=100000.0).speed(1.0));
                    ui.label("Height");
                    ui.add(egui::DragValue::new(&mut self.new_height).range(1.0..=100000.0).speed(1.0));
                    if ui.button("Swap").clicked() { std::mem::swap(&mut self.new_width, &mut self.new_height); }
                });
                ui.horizontal(|ui| {
                    ui.label("Resolution");
                    ui.add(egui::DragValue::new(&mut self.new_dpi).range(1..=9600).suffix(" DPI"));
                    ui.color_edit_button_srgb(&mut self.new_background);
                    ui.checkbox(&mut self.new_transparent, "Transparent");
                });
                });
                let dimensions = (
                    self.new_unit.to_pixels(self.new_width, self.new_dpi),
                    self.new_unit.to_pixels(self.new_height, self.new_dpi),
                );
                if let (Some(width), Some(height)) = dimensions {
                    ui.label(format!("{} × {} pixels · sparse tiles (up to 128 MiB artwork tile data)", width, height));
                } else {
                    ui.colored_label(egui::Color32::LIGHT_RED, "These settings exceed the current 4096 × 4096 project limit.");
                }
                ui.small("DPI affects print-size conversion; changing it does not add pixel detail. Raster color is sRGB RGBA8.");
                ui.horizontal(|ui| {
                let can_create = dimensions.0.is_some() && dimensions.1.is_some();
                if ui.add_enabled(can_create, egui::Button::new("Create document")).clicked()
                    || (can_create && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    actions.push(Action::New);
                    self.new_dialog = false;
                    self.welcome = false;
                }
                if ui.button("Cancel").clicked() {
                    self.new_dialog = false;
                    if self.new_from_welcome {
                        self.welcome = true;
                    }
                    self.new_from_welcome = false;
                }
                });
            });
        } else if self.manual_file_dialog {
            egui::Modal::new(egui::Id::new("test project file path")).show(ctx, |ui| {
                ui.heading("Test workflow file path");
                // Plain Enter works in the new-document form but can be consumed
                // by this focused text field. Read it before the TextEdit handles
                // the event so the headless workflow can reliably submit the path.
                let submit = ui.input(|i| i.key_pressed(egui::Key::Enter));
                ui.add(egui::TextEdit::singleline(&mut self.path).char_limit(4096));
                ui.horizontal(|ui| {
                    if ui.button("Continue").clicked() || submit {
                        if let Some(action) = self.manual_file_action.take() {
                            actions.push(action);
                        }
                        self.manual_file_dialog = false;
                        self.manual_file_submit = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.manual_file_action = None;
                        self.manual_file_dialog = false;
                        self.manual_file_submit = false;
                    }
                });
            });
        } else if self.confirm.is_some() {
            egui::Modal::new(egui::Id::new("preserve unsaved artwork")).show(ctx, |ui| {
                ui.heading("Preserve unsaved artwork");
                ui.label(
                    "Save first, or retain a recovery checkpoint before replacing this document.",
                );
                if ui.button("Keep recovery and continue").clicked() {
                    if self.preserve(session).is_ok() {
                        *approved = true;
                        if let Some(action) = self.confirm.take() {
                            actions.push(action);
                        }
                    } else {
                        self.status =
                            "Recovery unavailable: save current artwork before continuing".into();
                    }
                }
                if ui.button("Cancel").clicked() {
                    self.confirm = None;
                }
            });
        } else if self.close_requested {
            egui::Modal::new(egui::Id::new("close artwork")).show(ctx, |ui| {
                ui.heading("Automatic save did not complete");
                ui.label("Your artwork is still open. Retry the save, choose another project location, or cancel closing.");
                if ui.button("Retry save").clicked() {
                    actions.push(Action::RetryClose);
                }
                if ui.button("Save As…").clicked() {
                    self.picker = Some(FileRequest::SaveProjectAs);
                }
                if ui.button("Cancel").clicked() {
                    self.close_requested = false;
                }
            });
        }
    }
    pub fn frame(
        &mut self,
        window: &Window,
        session: &mut CanvasSession,
        size: [u32; 2],
    ) -> egui::FullOutput {
        if session.mode() != crate::interaction::Mode::Live {
            self.touches.clear();
            self.pan_cursor = None;
            self.edit_pointer = None;
            self.edit_points.clear();
        }
        if let Some(theme) = window.theme() {
            self.system_dark = theme == winit::window::Theme::Dark;
        }
        let appearance_key = (
            self.appearance_mode,
            self.accent_scheme,
            self.custom_accent_rgb,
            self.system_dark,
            self.high_contrast,
            (self.ui_scale * 1000.0).round() as u32,
        );
        if self.appearance_cache != Some(appearance_key) {
            let mut visuals = crate::appearance::visuals(
                self.appearance_mode,
                self.system_dark,
                self.accent_scheme,
                self.custom_accent_rgb,
            );
            if self.high_contrast {
                visuals = crate::appearance::high_contrast(
                    visuals,
                    self.appearance_mode.is_dark(self.system_dark),
                );
            }
            self.context.set_visuals(visuals);
            self.context.set_zoom_factor(self.ui_scale);
            self.appearance_cache = Some(appearance_key);
        }
        let raw = self.input.take_egui_input(window);
        let ctx = self.context.clone();
        let mut actions = Vec::new();
        if self.test_path_dialog && self.manual_file_dialog && self.manual_file_submit {
            self.manual_file_submit = false;
            self.manual_file_dialog = false;
            if let Some(action) = self.manual_file_action.take() {
                eprintln!("TEST_PATH_DIALOG: queued action for deterministic submission");
                actions.push(action);
            }
        }
        let mut central = egui::Rect::NOTHING;
        let mut approved = false;
        let mut output = ctx.run(raw, |ctx| {
            self.widgets(ctx, session, &mut actions, &mut central, &mut approved)
        });

        if let Some(request) = self.picker.take() {
            if self.test_path_dialog && !matches!(request, FileRequest::AddReferences) {
                self.manual_file_action = request.action();
                self.manual_file_dialog = true;
                self.context.request_repaint();
            } else {
                let mut dialog = rfd::FileDialog::new();
                let (title, filter, extensions, saving) = match request {
                    FileRequest::OpenProject => {
                        ("Open project", "Rust Paint Lab project", vec!["rpl"], false)
                    }
                    FileRequest::SaveProjectAs => (
                        "Save project as",
                        "Rust Paint Lab project",
                        vec!["rpl"],
                        true,
                    ),
                    FileRequest::SaveBrush => (
                        "Save brush preset",
                        "Rust Paint Lab brush",
                        vec!["rplbrush"],
                        true,
                    ),
                    FileRequest::LoadBrush => (
                        "Load brush preset",
                        "Rust Paint Lab brush",
                        vec!["rplbrush"],
                        false,
                    ),
                    FileRequest::ExportPng => ("Export PNG", "PNG image", vec!["png"], true),
                    FileRequest::ImportPng => ("Import image", "Raster image", vec!["png"], false),
                    FileRequest::ImportOra => {
                        ("Open OpenRaster", "OpenRaster image", vec!["ora"], false)
                    }
                    FileRequest::ExportOra => {
                        ("Export OpenRaster", "OpenRaster image", vec!["ora"], true)
                    }
                    FileRequest::AddReferences => (
                        "Add reference images",
                        "Reference images",
                        vec!["png", "jpg", "jpeg"],
                        false,
                    ),
                };
                dialog = dialog.set_title(title).add_filter(filter, &extensions);
                if let Some(parent) = PathBuf::from(&self.path)
                    .parent()
                    .filter(|p| !p.as_os_str().is_empty())
                {
                    dialog = dialog.set_directory(parent);
                }
                if saving && let Some(name) = PathBuf::from(&self.path).file_name() {
                    dialog = dialog.set_file_name(name.to_string_lossy());
                }
                let selected = if matches!(request, FileRequest::AddReferences) {
                    dialog.pick_files().unwrap_or_default()
                } else if saving {
                    dialog.save_file().into_iter().collect()
                } else {
                    dialog.pick_file().into_iter().collect()
                };
                if selected.is_empty() {
                    self.status = "File selection canceled".into();
                    if self.picker_from_welcome {
                        self.welcome = true;
                    }
                    self.picker_from_welcome = false;
                } else if matches!(request, FileRequest::AddReferences) {
                    actions.extend(selected.into_iter().map(Action::AddReference));
                    self.picker_from_welcome = false;
                } else if let Some(path) = selected.into_iter().next() {
                    self.path = path.to_string_lossy().into_owned();
                    if let Some(action) = request.action() {
                        actions.push(action);
                    }
                }
            }
        }

        let mut reference_imported = 0;
        let mut reference_errors = Vec::new();
        for a in actions {
            if matches!(a, Action::RetryClose) {
                if let Err(error) = self.save_for_close(session) {
                    self.status = format!("Automatic save failed: {error}");
                    self.close_requested = true;
                }
                window.request_redraw();
                continue;
            }
            if let Action::AddReference(path) = a {
                match add_reference_path(&ctx, &mut self.references, path) {
                    Ok(()) => reference_imported += 1,
                    Err(error) => reference_errors.push(error),
                }
                continue;
            }
            if matches!(
                a,
                Action::New
                    | Action::Open
                    | Action::Recover(_)
                    | Action::Recent(_)
                    | Action::ImportOra
            ) && session.live().project().dirty()
                && self.confirm.is_none()
                && !approved
            {
                self.confirm = Some(a);
                continue;
            }
            let opening_from_welcome =
                self.picker_from_welcome && matches!(&a, Action::Open | Action::Recent(_));
            session.mouse(MouseEvent::Interrupted, 0);
            let result = self.execute(a, session);
            if let Err(error) = result {
                self.status = error.to_string();
                if opening_from_welcome {
                    self.welcome = true;
                }
            } else if opening_from_welcome {
                self.picker_from_welcome = false;
            }
            window.request_redraw();
        }
        let brush_color = session.live().project().brush.color;
        if let Some(previous) = self.observed_brush_color
            && previous != brush_color
        {
            crate::workspace::remember_brush_color(&mut self.recent_brush_colors, brush_color);
            if let Err(error) = self.persist_workspace() {
                self.status = format!("Could not save recent brush colors: {error}");
            }
        }
        self.observed_brush_color = Some(brush_color);
        if reference_imported > 0 || !reference_errors.is_empty() {
            let mut status = if reference_imported == 0 {
                "No reference images were added".to_string()
            } else {
                if let Err(error) = self.persist_workspace() {
                    reference_errors.push(format!("layout persistence failed: {error}"));
                }
                format!(
                    "Added {reference_imported} reference image(s); large previews are scaled for the GPU budget"
                )
            };
            if !reference_errors.is_empty() {
                status.push_str(" · ");
                status.push_str(&reference_errors.join(" · "));
            }
            self.status = status;
            // The picker returns after egui has produced this frame's texture
            // delta. Request another frame so newly loaded reference textures
            // are uploaded and the floating image windows become visible.
            window.request_redraw();
        }
        let sf = f64::from(output.pixels_per_point);
        let rect = [
            f64::from(central.min.x) * sf,
            f64::from(central.min.y) * sf,
            (f64::from(central.max.x) * sf).min(size[0] as f64),
            (f64::from(central.max.y) * sf).min(size[1] as f64),
        ];
        let state = session.live().project().state();
        self.mapping =
            CanvasMapping::for_document(size, [state.width as f64, state.height as f64], rect)
                .and_then(|m| m.navigated(self.navigation.zoom, self.navigation.pan))
                .ok();
        self.workspace_center = [(rect[0] + rect[2]) / 2.0, (rect[1] + rect[3]) / 2.0];
        if let Some(m) = self.mapping {
            session.live_mut().set_mapping(m);
        }
        if session.mode() == crate::interaction::Mode::Live
            && let Some(mapping) = self.mapping
        {
            let selection = if self.edit_pointer.is_some() {
                let kind = match self.tool {
                    EditTool::Ellipse => SelectionKind::Ellipse,
                    EditTool::Lasso => SelectionKind::Lasso,
                    _ => SelectionKind::Rectangle,
                };
                Selection::new(kind, self.edit_points.clone()).ok()
            } else {
                session.live().project().state().selection.clone()
            };
            if let Some(selection) = selection {
                let to_screen = |p: [f64; 2]| {
                    mapping.to_clip(p).ok().map(|p| {
                        egui::pos2(
                            (p[0] + 1.0) * size[0] as f32 / 2.0 / output.pixels_per_point,
                            (1.0 - p[1]) * size[1] as f32 / 2.0 / output.pixels_per_point,
                        )
                    })
                };
                let [x, y, r, b] = selection.bounds();
                let points: Vec<_> = match selection.kind() {
                    SelectionKind::Rectangle => vec![[x, y], [r, y], [r, b], [x, b]],
                    SelectionKind::Ellipse => (0..64)
                        .map(|i| {
                            let a = i as f64 * std::f64::consts::TAU / 64.0;
                            [
                                (x + r) / 2.0 + a.cos() * (r - x) / 2.0,
                                (y + b) / 2.0 + a.sin() * (b - y) / 2.0,
                            ]
                        })
                        .collect(),
                    SelectionKind::Lasso => selection.points().to_vec(),
                };
                let points: Vec<_> = points.into_iter().filter_map(to_screen).collect();
                for (width, color) in [(3.0, egui::Color32::BLACK), (1.0, egui::Color32::WHITE)] {
                    output.shapes.push(egui::epaint::ClippedShape {
                        clip_rect: central,
                        shape: egui::Shape::closed_line(
                            points.clone(),
                            egui::Stroke::new(width, color),
                        ),
                    });
                }
            }
        }
        if self.tool == EditTool::Brush
            && session.mode() == crate::interaction::Mode::Live
            && let Some(pen) = session.live().last_pen()
            && pen.phase != paint_input::Phase::Cancel
            && pen.in_range
            && !self.reference_under_client(pen.position_px)
            && let Some(mapping) = self.mapping.filter(|m| m.contains_client(pen.position_px))
        {
            let point = mapping
                .routing_viewport()
                .to_document(pen.position_px)
                .expect("valid mapped hover")
                .0;
            if let (Ok(center), Ok(edge)) = (
                mapping.to_clip(point),
                mapping.to_clip([
                    point[0] + session.live().project().brush.size / 2.0,
                    point[1],
                ]),
            ) {
                let center = egui::pos2(
                    (center[0] + 1.0) * size[0] as f32 / 2.0 / output.pixels_per_point,
                    (1.0 - center[1]) * size[1] as f32 / 2.0 / output.pixels_per_point,
                );
                let radius = ((edge[0] - mapping.to_clip(point).expect("validated point")[0])
                    * size[0] as f32
                    / 2.0
                    / output.pixels_per_point)
                    .abs();
                for (width, color) in [(3.0, egui::Color32::BLACK), (1.0, egui::Color32::WHITE)] {
                    output.shapes.push(egui::epaint::ClippedShape {
                        clip_rect: central,
                        shape: egui::Shape::circle_stroke(
                            center,
                            radius,
                            egui::Stroke::new(width, color),
                        ),
                    });
                }
            }
        }
        session.live_mut().project_mut().sync_brush_settings();
        let now = Instant::now();
        let project = session.live().project();
        self.autosave_schedule
            .observe(project.revision, project.dirty(), now);
        if let Some(recovery) = &self.recovery {
            for result in recovery.poll() {
                self.recovery_saving = false;
                match result {
                    Ok(()) => self.status = "Recovery checkpoint complete".into(),
                    Err(e) => {
                        self.last_submitted = u64::MAX;
                        self.autosave_schedule.retry_later(now);
                        self.status = format!("RECOVERY FAILED: {e}; will retry shortly")
                    }
                }
            }
            let project = session.live().project();
            if project.dirty()
                && self.last_submitted != project.revision
                && !session.live().active()
                && self.autosave_schedule.ready(now)
            {
                if recovery.submit(project.state().clone()) {
                    self.last_submitted = project.revision;
                    self.autosave_schedule.submitted();
                    self.recovery_saving = true;
                } else {
                    self.autosave_schedule.retry_later(now);
                }
            }
        }
        self.input
            .handle_platform_output(window, output.platform_output.clone());
        output
    }
    fn execute(&mut self, a: Action, session: &mut CanvasSession) -> Result<(), Failure> {
        let project = session.live_mut().project_mut();
        match a {
            Action::ImportOra => {
                let state = paint_document::openraster::load(std::path::Path::new(&self.path))?;
                let next = Recovery::start(&paint_document::recovery::directory()?)?;
                *project = Engine::from_state(state)?;
                project.revision = 1;
                self.recovery = Some(next);
                self.store = None;
                self.path = "imported-artwork.rpl".into();
                self.last_submitted = u64::MAX;
                self.status="OpenRaster imported. Save As .rpl to retain native editing. Layer locks/presets/history are not in ORA; resolution rounds to integer DPI".into();
            }
            Action::ExportOra => {
                paint_document::openraster::export(std::path::Path::new(&self.path), project)?;
                self.status="OpenRaster exported: raster layers retained; background becomes a layer. Locks, presets and history are omitted".into();
            }
            Action::RemoveRecovery(path) => {
                paint_document::recovery::remove(&paint_document::recovery::directory()?, &path)?;
                self.recovery_files.retain(|p| p != &path);
                self.status = "Selected recovery checkpoint and backup removed".into();
            }
            Action::RemoveDraft(path) => {
                crate::drafts::remove(&path)?;
                self.drafts = crate::drafts::list();
                self.status = "Selected draft and backup removed".into();
            }
            Action::AddReference(_) => {
                return Err("Reference import is handled by the workspace".into());
            }
            Action::RetryClose => {
                return Err("Close retry must be handled by the workspace".into());
            }
            Action::Fill(erase) => project.fill_selection(erase)?,
            Action::Deselect => project.set_selection(None)?,
            Action::Transform(m) => project.transform_selection(m)?,
            Action::Crop => project.crop_selection()?,
            Action::Undo => {
                project.undo();
            }
            Action::Redo => {
                project.redo();
            }
            Action::Add => project.add_layer()?,
            Action::Duplicate => project.duplicate_layer()?,
            Action::Delete => project.delete_layer()?,
            Action::Clear => project.clear_layer()?,
            Action::Select(id) => project.select(id),
            Action::Move(delta) => project.edit(|s| {
                let i = s
                    .layers
                    .iter()
                    .position(|l| l.id == s.selected)
                    .ok_or(paint_document::PaintError::Invalid)?;
                let j = (i as i32 + delta).clamp(0, s.layers.len() as i32 - 1) as usize;
                s.layers.swap(i, j);
                Ok(())
            })?,
            Action::Layer(id, name, visible, locked, opacity, blend) => project.edit(|s| {
                let l = s
                    .layers
                    .iter_mut()
                    .find(|l| l.id == id)
                    .ok_or(paint_document::PaintError::Invalid)?;
                l.name = name;
                l.visible = visible;
                l.locked = locked;
                l.opacity = opacity;
                l.blend = blend;
                Ok(())
            })?,
            Action::New => {
                // Each document keeps its own recovery lineage. Never replace the
                // previous document checkpoint with the next document snapshot.
                let state = crate::document_setup::build_state(
                    self.new_width,
                    self.new_height,
                    self.new_unit,
                    self.new_dpi,
                    self.new_background,
                    self.new_transparent,
                )?;
                let width = state.width;
                let height = state.height;
                let recent_size = crate::workspace::RecentDocumentSize {
                    width_px: width,
                    height_px: height,
                    dpi: self.new_dpi,
                };
                self.recent_document_sizes
                    .retain(|size| *size != recent_size);
                self.recent_document_sizes.insert(0, recent_size);
                self.recent_document_sizes
                    .truncate(crate::workspace::MAX_RECENT_DOCUMENT_SIZES);
                let next = Recovery::start(&paint_document::recovery::directory()?)?;
                self.recovery = Some(next);
                *project = Engine::from_state(state)?;
                self.store = None;
                self.path = format!(
                    "{}.rpl",
                    crate::document_setup::sanitized_document_stem(&self.new_name)
                );
                self.last_submitted = u64::MAX;
                self.welcome = false;
                self.status = format!("Created {width} × {height} px at {} DPI", self.new_dpi);
                if let Err(error) = self.persist_workspace() {
                    self.status =
                        format!("Document created; setup preference save failed: {error}");
                }
            }
            Action::Recover(path) => {
                let (state, fallback) = paint_document::recovery::restore(&path)?;
                let next = Recovery::start(&paint_document::recovery::directory()?)?;
                *project = Engine::from_state(state)?;
                project.revision = 1;
                self.recovery = Some(next);
                self.path = "recovered-artwork.rpl".into();
                self.store = None;
                self.last_submitted = u64::MAX;
                self.welcome = false;
                self.status = if fallback {
                    "RECOVERED PREVIOUS CHECKPOINT: newest recovery data was invalid; edits after the previous complete checkpoint may be missing. Save As a new native project"
                } else {
                    "Recovery restored into a new unsaved document; Save As a native project"
                }
                .into();
            }
            Action::Open | Action::Recent(_) => {
                let path = if let Action::Recent(p) = &a {
                    p.clone()
                } else {
                    PathBuf::from(&self.path)
                };
                let (store, state) = Store::open(path)?;
                let next = Recovery::start(&paint_document::recovery::directory()?)?;
                *project = Engine::from_state(state)?;
                self.recovery = Some(next);
                self.path = store.path.to_string_lossy().into();
                let path = store.path.clone();
                self.store = Some(store);
                self.remember(&path);
                self.last_submitted = u64::MAX;
                self.welcome = false;
            }
            Action::Save | Action::SaveAs => {
                project.sync_brush_settings();
                if matches!(a, Action::SaveAs) || self.store.is_none() {
                    let path = native_project_path(std::path::Path::new(&self.path));
                    let mut replacement = Store::new(path.clone());
                    replacement.save(project.state())?;
                    self.store = Some(replacement);
                    self.path = path.to_string_lossy().into_owned();
                } else {
                    self.store.as_mut().unwrap().save(project.state())?;
                }
                project.saved_revision = project.revision;
                if self.close_requested {
                    self.close_ready = true;
                }
                self.status = "Editable project saved; previous valid file retained as .bak".into();
                let path = self.store.as_ref().unwrap().path.clone();
                self.remember(&path);
            }
            Action::SaveBrush => {
                let path = native_project_path(std::path::Path::new(&self.path))
                    .with_extension("rplbrush");
                storage::save_brush_preset(&path, &project.brush)?;
                self.path = path.to_string_lossy().into_owned();
                self.status =
                    "Versioned brush preset saved without replacing an existing file".into();
            }
            Action::LoadBrush => {
                let path = native_project_path(std::path::Path::new(&self.path))
                    .with_extension("rplbrush");
                project.brush = storage::load_brush_preset(&path)?;
                self.path = path.to_string_lossy().into_owned();
                self.status = "Validated brush preset loaded".into();
            }
            Action::Export => {
                storage::export_png(std::path::Path::new(&self.path), project)?;
                self.status = "PNG exported (flattened 8-bit sRGB RGBA)".into();
            }
            Action::Import => {
                storage::import_png(std::path::Path::new(&self.path), project)?;
                self.status="PNG imported as 8-bit encoded-sRGB. ICC profiles, animation and 16-bit precision are not preserved".into();
            }
        }
        Ok(())
    }
    pub fn paint(
        &mut self,
        g: &Renderer,
        view: &wgpu::TextureView,
        output: egui::FullOutput,
        size: [u32; 2],
    ) {
        let jobs = self
            .context
            .tessellate(output.shapes, output.pixels_per_point);
        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: size,
            pixels_per_point: output.pixels_per_point,
        };
        for (id, delta) in &output.textures_delta.set {
            self.painter.update_texture(&g.device, &g.queue, *id, delta);
        }
        let mut encoder = g.device.create_command_encoder(&Default::default());
        let extra = self
            .painter
            .update_buffers(&g.device, &g.queue, &mut encoder, &jobs, &screen);
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("painting controls"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            self.painter
                .render(&mut pass.forget_lifetime(), &jobs, &screen);
        }
        g.queue.submit(extra.into_iter().chain([encoder.finish()]));
        for id in &output.textures_delta.free {
            self.painter.free_texture(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LassoAppend, append_lasso_point, native_project_path};
    use std::path::{Path, PathBuf};

    #[test]
    fn lasso_keeps_spacing_and_never_silently_truncates() {
        let mut points = vec![[0.0, 0.0]; 2];
        assert_eq!(
            append_lasso_point(&mut points, [1.0, 0.0]),
            LassoAppend::Unchanged
        );
        assert_eq!(
            append_lasso_point(&mut points, [2.0, 0.0]),
            LassoAppend::Added
        );
        assert_eq!(points, vec![[0.0, 0.0], [0.0, 0.0], [2.0, 0.0]]);

        points.resize(paint_document::editing::MAX_SELECTION_POINTS, [2.0, 0.0]);
        let count = points.len();
        assert_eq!(
            append_lasso_point(&mut points, [4.0, 0.0]),
            LassoAppend::LimitReached
        );
        assert_eq!(points.len(), count);
    }

    #[test]
    fn lasso_capacity_does_not_reject_subthreshold_motion() {
        let mut points = vec![[0.0, 0.0]; paint_document::editing::MAX_SELECTION_POINTS];
        assert_eq!(
            append_lasso_point(&mut points, [1.0, 0.0]),
            LassoAppend::Unchanged
        );
        assert_eq!(points.len(), paint_document::editing::MAX_SELECTION_POINTS);
    }

    #[test]
    fn native_save_path_is_not_confused_by_image_export_path() {
        assert_eq!(
            native_project_path(Path::new("artwork.png")),
            PathBuf::from("artwork.rpl")
        );
        assert_eq!(
            native_project_path(Path::new("artwork")),
            PathBuf::from("artwork.rpl")
        );
    }
}
