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
use std::path::PathBuf;
use winit::window::Window;

struct ReferenceImage {
    layout: crate::workspace::ReferenceLayout,
    texture: egui::TextureHandle,
}

const MAX_REFERENCE_FILE_BYTES: u64 = 32 * 1024 * 1024;
const MAX_REFERENCE_PIXELS: usize = 2_048 * 2_048;
const MAX_REFERENCE_TEXTURE_BYTES: usize = 8 * 1024 * 1024;

fn load_reference_texture(
    context: &egui::Context,
    layout: crate::workspace::ReferenceLayout,
) -> Result<ReferenceImage, String> {
    let metadata = std::fs::metadata(&layout.path).map_err(|e| e.to_string())?;
    if metadata.len() > MAX_REFERENCE_FILE_BYTES {
        return Err("Reference image file exceeds the 32 MiB limit".into());
    }
    let mut reader = image::ImageReader::open(&layout.path)
        .and_then(|reader| reader.with_guessed_format())
        .map_err(|e| e.to_string())?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(MAX_REFERENCE_TEXTURE_BYTES as u64);
    reader.limits(limits);
    let decoded = reader.decode().map_err(|e| e.to_string())?;
    let (width, height) = (decoded.width() as usize, decoded.height() as usize);
    if width == 0
        || height == 0
        || width
            .checked_mul(height)
            .is_none_or(|pixels| pixels > MAX_REFERENCE_PIXELS)
        || width
            .checked_mul(height)
            .and_then(|pixels| pixels.checked_mul(4))
            .is_none_or(|bytes| bytes > MAX_REFERENCE_TEXTURE_BYTES)
    {
        return Err("Reference image exceeds the 2048 × 2048 / 8 MiB decoded limit".into());
    }
    let rgba = decoded.into_rgba8();
    let image = egui::ColorImage::from_rgba_unmultiplied([width, height], rgba.as_raw());
    let name = layout
        .path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("reference image");
    let texture = context.load_texture(name, image, egui::TextureOptions::LINEAR);
    Ok(ReferenceImage { layout, texture })
}

#[cfg(test)]
mod reference_tests {
    use super::*;

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
            },
        )
        .unwrap();
        assert_eq!(loaded.texture.size(), [2, 3]);
        assert_eq!(loaded.layout.path, path);
        std::fs::remove_file(path).unwrap();
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
    new_dialog: bool,
    picker: Option<FileRequest>,
    manual_file_dialog: bool,
    manual_file_action: Option<Action>,
    test_path_dialog: bool,
    confirm: Option<Action>,
    pub close_requested: bool,
    pub close_ready: bool,
    recovery: Option<Recovery>,
    recovery_files: Vec<PathBuf>,
    drafts: Vec<PathBuf>,
    recent: paint_document::recent::Recent,
    recent_path: Option<PathBuf>,
    cleanup: Option<PathBuf>,
    draft_cleanup: Option<PathBuf>,
    last_submitted: u64,
    theme_dark: bool,
    navigation: crate::navigation::Navigation,
    touches: crate::navigation::TouchNavigation,
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
    welcome: bool,
    new_from_welcome: bool,
    picker_from_welcome: bool,
}
impl Studio {
    pub fn new(g: &Renderer, window: &Window, format: wgpu::TextureFormat) -> Self {
        let context = egui::Context::default();
        context.style_mut(|s| {
            s.spacing.interact_size = egui::vec2(36.0, 36.0);
            s.spacing.button_padding = egui::vec2(8.0, 6.0);
        });
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
        let references = preferences
            .references
            .iter()
            .cloned()
            .filter_map(|layout| load_reference_texture(&context, layout).ok())
            .collect();
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
            new_width: 1600.0,
            new_height: 1000.0,
            new_dpi: 300,
            new_unit: crate::document_setup::DocumentUnit::Pixels,
            new_background: [255; 3],
            new_transparent: false,
            new_dialog: false,
            picker: None,
            manual_file_dialog: false,
            manual_file_action: None,
            test_path_dialog,
            confirm: None,
            close_requested: false,
            close_ready: false,
            recovery,
            recovery_files: files,
            drafts: crate::drafts::list(),
            recent,
            recent_path,
            cleanup: None,
            draft_cleanup: None,
            last_submitted: 0,
            theme_dark: preferences.dark_theme,
            navigation: Default::default(),
            touches: Default::default(),
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
            references,
            welcome: true,
            new_from_welcome: false,
            picker_from_welcome: false,
        }
    }
    pub fn window_event(
        &mut self,
        window: &Window,
        event: &winit::event::WindowEvent,
        session: &mut CanvasSession,
    ) -> (bool, bool) {
        use winit::event::{ElementState, WindowEvent as E};
        if let E::CursorMoved { position, .. } = event {
            self.mouse_position = Some([position.x, position.y]);
        }
        if matches!(
            event,
            E::Focused(false) | E::CursorLeft { .. } | E::Resized(_) | E::ScaleFactorChanged { .. }
        ) {
            self.touches.clear();
            self.pan_cursor = None;
        }
        if let E::Touch(touch) = event {
            if session.live().active() {
                self.touches.clear();
                return (false, false);
            }
            let p = [touch.location.x, touch.location.y];
            if !self.blocks_canvas()
                && session.mode() == crate::interaction::Mode::Live
                && (self.mapping.is_some_and(|m| m.contains_client(p))
                    || self.touches.active_count() > 0)
            {
                let phase = match touch.phase {
                    winit::event::TouchPhase::Started => crate::navigation::TouchPhase::Start,
                    winit::event::TouchPhase::Moved => crate::navigation::TouchPhase::Move,
                    winit::event::TouchPhase::Ended => crate::navigation::TouchPhase::End,
                    winit::event::TouchPhase::Cancelled => crate::navigation::TouchPhase::Cancel,
                };
                if let Some(pinch) = self.touches.event(touch.id, phase, p) {
                    self.navigation
                        .pinch(self.workspace_center, pinch.from, pinch.to, pinch.ratio);
                }
                return (true, true);
            }
        }
        let canvas = self
            .mouse_position
            .is_some_and(|p| self.mapping.is_some_and(|m| m.contains_client(p)))
            && !self.blocks_canvas()
            && session.mode() == crate::interaction::Mode::Live;
        match event {
            E::MouseWheel { delta, .. } if canvas => {
                let lines = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => f64::from(*y),
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y / 40.0,
                };
                let p = self.mouse_position.unwrap();
                self.navigation.pinch(
                    self.workspace_center,
                    p,
                    p,
                    (lines.clamp(-8.0, 8.0) * 0.12).exp(),
                );
                return (true, true);
            }
            E::MouseInput {
                state,
                button: winit::event::MouseButton::Middle,
                ..
            } if canvas || self.pan_cursor.is_some() => {
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
            && self
                .mouse_position
                .is_some_and(|p| self.mapping.is_some_and(|m| m.contains_client(p)));
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
        let on_canvas = self.mapping.is_some_and(|m| m.contains_client(client));
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
    }
    pub fn project_label(&self) -> String {
        self.store
            .as_ref()
            .and_then(|s| s.path.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }
    pub fn blocks_canvas(&self) -> bool {
        self.welcome
            || self.new_dialog
            || self.picker.is_some()
            || self.manual_file_dialog
            || self.confirm.is_some()
            || self.close_requested
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
        self.recovery.is_some()
            && session.live().project().dirty()
            && self.last_submitted != session.live().project().revision
    }
    fn persist_workspace(&self) -> Result<(), String> {
        let Some(path) = &self.preferences_path else {
            return Err("Workspace preference directory is unavailable".into());
        };
        let mut preferences = crate::workspace::WorkspacePreferences {
            dark_theme: self.theme_dark,
            brush_panel_width: self.brush_panel_width,
            references: self
                .references
                .iter()
                .map(|reference| reference.layout.clone())
                .collect(),
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
        let mut workspace_changed = false;
        if self.theme_dark {
            ctx.set_visuals(egui::Visuals::dark());
        } else {
            ctx.set_visuals(egui::Visuals::light());
        }

        if !self.welcome {
            egui::TopBottomPanel::top("document tools").show(ctx, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.heading("Rust Paint Lab");
                    if ui.button("New").clicked() {
                        self.new_from_welcome = false;
                        self.new_dialog = true;
                    }

                    if ui.button("Open").clicked() {
                        self.picker_from_welcome = false;
                        self.picker = Some(FileRequest::OpenProject);
                    }

                    if ui.button("Save").clicked() {
                        actions.push(Action::Save);
                    }

                    if ui.button("Save as").clicked() {
                        self.picker = Some(FileRequest::SaveProjectAs);
                    }

                    if ui.button("Save brush…").clicked() {
                        self.path = PathBuf::from(&self.path)
                            .with_extension("rplbrush")
                            .to_string_lossy()
                            .into();
                        self.picker = Some(FileRequest::SaveBrush);
                    }

                    if ui.button("Load brush…").clicked() {
                        self.path = PathBuf::from(&self.path)
                            .with_extension("rplbrush")
                            .to_string_lossy()
                            .into();
                        self.picker = Some(FileRequest::LoadBrush);
                    }

                    if ui.button("Open ORA").clicked() {
                        self.picker = Some(FileRequest::ImportOra);
                    }
                    if ui.button("Export ORA").clicked() {
                        self.path = PathBuf::from(&self.path)
                            .with_extension("ora")
                            .to_string_lossy()
                            .into();
                        self.picker = Some(FileRequest::ExportOra);
                    }
                    if ui.button("Export PNG").clicked() {
                        self.path = PathBuf::from(&self.path)
                            .with_extension("png")
                            .to_string_lossy()
                            .into();
                        self.picker = Some(FileRequest::ExportPng);
                    }

                    if ui.button("Import PNG").clicked() {
                        self.picker = Some(FileRequest::ImportPng);
                    }

                    if ui
                        .add_enabled(
                            session.live().project().undo_count() > 0,
                            egui::Button::new("Undo"),
                        )
                        .clicked()
                    {
                        actions.push(Action::Undo);
                    }

                    if ui
                        .add_enabled(
                            session.live().project().redo_count() > 0,
                            egui::Button::new("Redo"),
                        )
                        .clicked()
                    {
                        actions.push(Action::Redo);
                    }

                    workspace_changed |= ui.checkbox(&mut self.theme_dark, "Dark").changed();
                });
                ui.horizontal_wrapped(|ui| {
                    let old = self.tool;

                    for (tool, label) in [
                        (EditTool::Brush, "Brush B"),
                        (EditTool::Rectangle, "Rect M"),
                        (EditTool::Ellipse, "Ellipse E"),
                        (EditTool::Lasso, "Lasso K"),
                        (EditTool::Eyedropper, "Pick I"),
                    ] {
                        ui.selectable_value(&mut self.tool, tool, label);
                    }

                    if old != self.tool {
                        session.mouse(MouseEvent::Interrupted, 0);
                        self.edit_pointer = None;
                        self.edit_points.clear();
                    }

                    if ui.button("Deselect").clicked() {
                        actions.push(Action::Deselect);
                    }

                    if ui.button("Fill").clicked() {
                        actions.push(Action::Fill(false));
                    }

                    if ui.button("Delete pixels").clicked() {
                        actions.push(Action::Fill(true));
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Reference board");
                    let can_add = self.references.len() < crate::workspace::MAX_REFERENCES;
                    if ui
                        .add_enabled(can_add, egui::Button::new("Browse / Add images…"))
                        .clicked()
                    {
                        self.picker = Some(FileRequest::AddReferences);
                    }
                    ui.small(format!(
                        "{} / {}",
                        self.references.len(),
                        crate::workspace::MAX_REFERENCES
                    ));
                });
            });

            egui::TopBottomPanel::bottom("status").show(ctx,|ui|{
if let Some(error)=session.live().document_error(){
ui.colored_label(egui::Color32::LIGHT_RED,error);
}

else{
ui.label(&self.status);
}
ui.small(format!("{}×{} | {} dpi | {} tiles | {} undo | CPU composite/upload-submit {} µs | frame {} µs",session.live().project().state().width,session.live().project().state().height,session.live().project().state().dpi,session.live().project().state().tile_count(),session.live().project().undo_count(),self.last_upload_us,self.last_frame_us));
let pen_status=if let Some(p)=session.live().last_pen(){format!("Pen {:?} | pressure {} | tilt X {} / Y {} | barrel {} | eraser {}",p.phase,p.pressure.map(|v|format!("{v:.2}")).unwrap_or_else(||"unavailable".into()),p.tilt_degrees[0].map(|v|format!("{v:.0}°")).unwrap_or_else(||"unavailable".into()),p.tilt_degrees[1].map(|v|format!("{v:.0}°")).unwrap_or_else(||"unavailable".into()),p.buttons.barrel,p.buttons.eraser||p.buttons.inverted)}else{"Mouse / native Windows pen · hover ring shows selected brush diameter".into()};
ui.add(egui::Label::new(pen_status).truncate());

}
);

            let side_panel = egui::SidePanel::right("painting controls")
                .default_width(self.brush_panel_width)
                .resizable(true)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.heading("Brush");
                        let project = session.live_mut().project_mut();

                        egui::ComboBox::from_label("Preset")
                            .selected_text(format!("{:?}", project.brush.kind))
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
                                        .selectable_label(project.brush.kind == k, format!("{k:?}"))
                                        .clicked()
                                    {
                                        let color = project.brush.color;
                                        project.brush = Brush::preset(k);
                                        project.brush.color = color;
                                    }
                                }
                            });

                        ui.color_edit_button_srgb(&mut project.brush.color);

                        ui.add(
                            egui::Slider::new(&mut project.brush.size, 1.0..=256.0).text("Size"),
                        );
                        ui.add(
                            egui::Slider::new(&mut project.brush.opacity, 0.0..=1.0)
                                .text("Opacity"),
                        );
                        ui.add(egui::Slider::new(&mut project.brush.flow, 0.0..=1.0).text("Flow"));
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
                        ui.checkbox(&mut project.brush.pressure_opacity, "Pressure opacity");
                        ui.checkbox(&mut project.brush.tilt, "Tilt angle");

                        ui.separator();
                        let mut transparent = project.state().background.is_none();

                        if ui
                            .checkbox(&mut transparent, "Transparent background")
                            .changed()
                            && let Err(e) = project.edit(|s| {
                                s.background = if transparent { None } else { Some([255; 3]) };
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
                                egui::Slider::new(&mut self.transform_angle, -180.0..=180.0)
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

                        ui.heading("Layers");
                        ui.horizontal(|ui| {
                            if ui.button("+").clicked() {
                                actions.push(Action::Add);
                            }

                            if ui.button("Duplicate").clicked() {
                                actions.push(Action::Duplicate);
                            }

                            if ui.button("Delete").clicked() {
                                actions.push(Action::Delete);
                            }
                        });

                        ui.horizontal(|ui| {
                            if ui.button("Up").clicked() {
                                actions.push(Action::Move(1));
                            }

                            if ui.button("Down").clicked() {
                                actions.push(Action::Move(-1));
                            }

                            if ui.button("Clear layer").clicked() {
                                actions.push(Action::Clear);
                            }
                        });

                        let selected = project.state().selected;

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

                        for (id, mut name, mut visible, mut locked, mut opacity, mut blend) in
                            layers
                        {
                            ui.group(|ui| {
                                if ui.selectable_label(id == selected, &name).clicked() {
                                    actions.push(Action::Select(id));
                                }

                                let mut changed = ui
                                    .add(egui::TextEdit::singleline(&mut name).char_limit(64))
                                    .changed();

                                changed |= ui.checkbox(&mut visible, "Visible").changed();
                                changed |= ui.checkbox(&mut locked, "Lock").changed();
                                changed |= ui
                                    .add(
                                        egui::Slider::new(&mut opacity, 0..=255)
                                            .text("Layer opacity"),
                                    )
                                    .changed();

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
                                            ui.selectable_value(&mut blend, b, format!("{b:?}"));
                                        }
                                    });
                                changed |= old != blend;

                                if changed {
                                    actions.push(Action::Layer(
                                        id, name, visible, locked, opacity, blend,
                                    ));
                                }
                            });
                        }

                        ui.separator();
                        ui.label("L: paint · P/1/2/3: replay");
                        ui.label("Ctrl-Z / Ctrl-Y: history");

                        if !self.recent.paths.is_empty() {
                            ui.collapsing("Recent projects", |ui| {
                                for path in self.recent.paths.clone() {
                                    if ui
                                        .button(
                                            path.file_name().unwrap_or_default().to_string_lossy(),
                                        )
                                        .on_hover_text(path.to_string_lossy())
                                        .clicked()
                                    {
                                        actions.push(Action::Recent(path));
                                    }
                                }
                            });
                        }
                        if !self.recovery_files.is_empty() {
                            ui.heading("Recover artwork");
                            for p in self.recovery_files.clone() {
                                ui.horizontal(|ui| {
                                    if ui
                                        .button(p.file_name().unwrap_or_default().to_string_lossy())
                                        .clicked()
                                    {
                                        actions.push(Action::Recover(p.clone()));
                                    }
                                    if ui.button("Delete…").clicked() {
                                        self.cleanup = Some(p);
                                    }
                                });
                            }
                        }
                    });
                });
            let panel_width = side_panel.response.rect.width();
            if (panel_width - self.brush_panel_width).abs() > 0.5 {
                self.brush_panel_width = panel_width.clamp(180.0, 640.0);
                workspace_changed = true;
            }

            egui::CentralPanel::default()
                .frame(egui::Frame::NONE)
                .show(ctx, |ui| {
                    *central = ui.available_rect_before_wrap();
                });

            for reference in &mut self.references {
                let title = reference
                    .layout
                    .path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Reference");
                let reference_id = reference.layout.path.clone();
                let mut open = true;
                let mut remove_requested = false;
                let before = reference.layout.clone();
                let response = egui::Window::new(title)
                    .id(egui::Id::new(("reference-board", reference_id)))
                    .open(&mut open)
                    .resizable(true)
                    .default_pos(egui::pos2(before.position[0], before.position[1]))
                    .default_size(egui::vec2(before.size[0], before.size[1]))
                    .show(ctx, |ui| {
                        ui.add(
                            egui::Slider::new(&mut reference.layout.opacity, 0.05..=1.0)
                                .text("Overlay opacity"),
                        );
                        let available = ui.available_size().max(egui::vec2(64.0, 64.0));
                        let source_size = reference.texture.size_vec2();
                        let scale = (available.x / source_size.x).min(available.y / source_size.y);
                        let fitted = source_size * scale.max(0.0);
                        let mut image = egui::Image::new((reference.texture.id(), source_size))
                            .fit_to_exact_size(fitted);
                        image = image.tint(egui::Color32::from_white_alpha(
                            (reference.layout.opacity * 255.0).round() as u8,
                        ));
                        ui.add(image);
                        if ui.button("Remove reference").clicked() {
                            remove_requested = true;
                        }
                    });
                if let Some(response) = response {
                    let rect = response.response.rect;
                    reference.layout.position = [rect.min.x, rect.min.y];
                    reference.layout.size = [rect.width(), rect.height()];
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

        if self.welcome {
            egui::Modal::new(egui::Id::new("studio welcome")).show(ctx, |ui| {
                ui.set_min_width(440.0);
                ui.heading("Rust Paint Lab");
                ui.label("A quiet place to make something.");
                ui.add_space(12.0);
                if ui
                    .button(egui::RichText::new("New document").size(20.0))
                    .clicked()
                {
                    self.picker_from_welcome = false;
                    if !self.test_path_dialog {
                        self.new_name = "Untitled".into();
                    }
                    self.new_from_welcome = true;
                    self.new_dialog = true;
                    self.welcome = false;
                }
                if ui.button("Open an existing project…").clicked() {
                    self.picker_from_welcome = true;
                    self.picker = Some(FileRequest::OpenProject);
                    self.welcome = false;
                }
                if ui.button("Preferences and appearance").clicked() {
                    self.theme_dark = !self.theme_dark;
                    if let Err(error) = self.persist_workspace() {
                        self.status = format!("Could not save appearance preference: {error}");
                    }
                }
                if !self.recent.paths.is_empty() {
                    ui.separator();
                    ui.heading("Recent projects");
                    for path in self.recent.paths.iter().take(8) {
                        let label = path.file_name().unwrap_or_default().to_string_lossy();
                        if ui
                            .button(label)
                            .on_hover_text(path.display().to_string())
                            .clicked()
                        {
                            self.picker_from_welcome = true;
                            actions.push(Action::Recent(path.clone()));
                            self.welcome = false;
                        }
                    }
                }
                if !self.recovery_files.is_empty() {
                    ui.separator();
                    ui.heading("Recover unsaved work");
                    for path in self.recovery_files.iter().take(4) {
                        if ui
                            .button("Recover latest checkpoint…")
                            .on_hover_text(path.display().to_string())
                            .clicked()
                        {
                            actions.push(Action::Recover(path.clone()));
                            self.welcome = false;
                        }
                    }
                }
                if !self.drafts.is_empty() {
                    ui.separator();
                    ui.heading("Persistent drafts");
                    let mut remove = None;
                    for path in self.drafts.iter().take(crate::drafts::MAX_DRAFTS) {
                        ui.horizontal(|ui| {
                            if ui.button(crate::drafts::display_name(path)).clicked() {
                                self.picker_from_welcome = true;
                                actions.push(Action::Recent(path.clone()));
                                self.welcome = false;
                            }
                            if ui.small_button("Remove…").clicked() {
                                remove = Some(path.clone());
                            }
                        });
                    }
                    if let Some(path) = remove {
                        self.draft_cleanup = Some(path);
                    }
                }
                ui.separator();
                ui.small("Ctrl+N creates a document · Ctrl+O opens a project · Escape closes");
                if ui.button("Close application").clicked() {
                    self.close_ready = true;
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
                ui.heading("New document");
                ui.add_sized([320.0, 36.0], egui::TextEdit::singleline(&mut self.new_name).hint_text("Document name"));
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
                ui.add(egui::TextEdit::singleline(&mut self.path).char_limit(4096));
                ui.horizontal(|ui| {
                    if ui.button("Continue").clicked()
                        || ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        if let Some(action) = self.manual_file_action.take() {
                            actions.push(action);
                        }
                        self.manual_file_dialog = false;
                    }
                    if ui.button("Cancel").clicked() {
                        self.manual_file_action = None;
                        self.manual_file_dialog = false;
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
        let raw = self.input.take_egui_input(window);
        let ctx = self.context.clone();
        let mut actions = Vec::new();
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
                if self.references.len() >= crate::workspace::MAX_REFERENCES {
                    self.status = "Reference board is full (six images maximum)".into();
                    continue;
                }
                let index = self.references.len();
                if self
                    .references
                    .iter()
                    .any(|reference| reference.layout.path == path)
                {
                    self.status = "That image is already on the reference board".into();
                    continue;
                }
                let layout = crate::workspace::ReferenceLayout {
                    path,
                    position: [40.0 + index as f32 * 28.0, 80.0 + index as f32 * 24.0],
                    size: [360.0, 300.0],
                    opacity: 0.72,
                };
                match load_reference_texture(&ctx, layout) {
                    Ok(reference) => {
                        self.references.push(reference);
                        if let Err(error) = self.persist_workspace() {
                            self.status = format!("Reference added; layout save failed: {error}");
                        } else {
                            self.status = "Reference added to the workspace board".into();
                        }
                    }
                    Err(error) => {
                        self.status = format!("Reference image could not be loaded: {error}")
                    }
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
        if let Some(recovery) = &self.recovery {
            for result in recovery.poll() {
                self.status = match result {
                    Ok(()) => "Recovery checkpoint complete".into(),
                    Err(e) => format!("RECOVERY FAILED: {e}; save project manually"),
                };
            }
            let project = session.live().project();
            if project.dirty()
                && self.last_submitted != project.revision
                && recovery.submit(project.state().clone())
            {
                self.last_submitted = project.revision;
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
