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
    Export,
    Import,
    Recover(PathBuf),
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
    new_width: u32,
    new_height: u32,
    new_dpi: u32,
    new_dialog: bool,
    file_dialog: bool,
    action: Option<Action>,
    confirm: Option<Action>,
    pub close_requested: bool,
    pub close_ready: bool,
    recovery: Option<Recovery>,
    recovery_files: Vec<PathBuf>,
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
            new_width: 1600,
            new_height: 1000,
            new_dpi: 300,
            new_dialog: false,
            file_dialog: false,
            action: None,
            confirm: None,
            close_requested: false,
            close_ready: false,
            recovery,
            recovery_files: files,
            last_submitted: 0,
            theme_dark: true,
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
                    let last = *self.edit_points.last().unwrap();
                    if (position[0] - last[0]).hypot(position[1] - last[1]) >= 2.0
                        && self.edit_points.len() < 1024
                    {
                        self.edit_points.push(position);
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
    pub fn project_label(&self) -> String {
        self.store
            .as_ref()
            .and_then(|s| s.path.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into())
    }
    pub fn blocks_canvas(&self) -> bool {
        self.new_dialog || self.file_dialog || self.confirm.is_some() || self.close_requested
    }
    pub fn shortcut(&mut self, key: char, session: &mut CanvasSession) {
        session.mouse(MouseEvent::Interrupted, 0);
        if key == 'o' {
            self.action = Some(Action::Open);
            self.file_dialog = true;
            return;
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
    pub fn needs_autosave(&self, session: &CanvasSession) -> bool {
        self.recovery.is_some()
            && session.live().project().dirty()
            && self.last_submitted != session.live().project().revision
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
        let mut output=ctx.run(raw,|ctx|{

            if self.theme_dark{
ctx.set_visuals(egui::Visuals::dark());
}
else{
ctx.set_visuals(egui::Visuals::light());
}

            egui::TopBottomPanel::top("document tools").show(ctx,|ui|{
ui.horizontal_wrapped(|ui|{

                ui.heading("Rust Paint Lab");
if ui.button("New").clicked(){
self.new_dialog=true;
}

                if ui.button("Open").clicked(){
self.action=Some(Action::Open);
self.file_dialog=true;
}

                if ui.button("Save").clicked(){
actions.push(Action::Save);
}

                if ui.button("Save as").clicked(){
self.action=Some(Action::SaveAs);
self.file_dialog=true;
}

                if ui.button("Export PNG").clicked(){
self.path=PathBuf::from(&self.path).with_extension("png").to_string_lossy().into();
self.action=Some(Action::Export);
self.file_dialog=true;
}

                if ui.button("Import PNG").clicked(){
self.action=Some(Action::Import);
self.file_dialog=true;
}

                if ui.add_enabled(session.live().project().undo_count()>0,egui::Button::new("Undo")).clicked(){
actions.push(Action::Undo);
}

                if ui.add_enabled(session.live().project().redo_count()>0,egui::Button::new("Redo")).clicked(){
actions.push(Action::Redo);
}

                ui.checkbox(&mut self.theme_dark,"Dark");

            }
);
ui.horizontal_wrapped(|ui|{

                let old=self.tool;

                for (tool,label) in [(EditTool::Brush,"Brush B"),(EditTool::Rectangle,"Rect M"),(EditTool::Ellipse,"Ellipse E"),(EditTool::Lasso,"Lasso K"),(EditTool::Eyedropper,"Pick I")] {
ui.selectable_value(&mut self.tool,tool,label);
}

                if old!=self.tool {
session.mouse(MouseEvent::Interrupted,0);
self.edit_pointer=None;
self.edit_points.clear();
}

                if ui.button("Deselect").clicked(){
actions.push(Action::Deselect);
}

                if ui.button("Fill").clicked(){
actions.push(Action::Fill(false));
}

                if ui.button("Delete pixels").clicked(){
actions.push(Action::Fill(true));
}


            }
);
}
);

            egui::TopBottomPanel::bottom("status").show(ctx,|ui|{
if let Some(error)=session.live().document_error(){
ui.colored_label(egui::Color32::LIGHT_RED,error);
}
else{
ui.label(&self.status);
}
ui.small(format!("{}×{} | {} dpi | {} tiles | {} undo | CPU composite/upload-submit {} µs | frame {} µs",session.live().project().state().width,session.live().project().state().height,session.live().project().state().dpi,session.live().project().state().tile_count(),session.live().project().undo_count(),self.last_upload_us,self.last_frame_us));
}
);

            egui::SidePanel::right("painting controls").default_width(240.0).show(ctx,|ui|{
egui::ScrollArea::vertical().show(ui,|ui|{

                ui.heading("Brush");
let project=session.live_mut().project_mut();

                egui::ComboBox::from_label("Preset").selected_text(format!("{:?}",project.brush.kind)).show_ui(ui,|ui|{
for k in [BrushKind::Round,BrushKind::Ink,BrushKind::Pencil,BrushKind::Marker,BrushKind::Airbrush,BrushKind::Soft,BrushKind::Textured,BrushKind::Calligraphy,BrushKind::Eraser,BrushKind::Wash]{
if ui.selectable_label(project.brush.kind==k,format!("{k:?}")).clicked(){
let color=project.brush.color;
project.brush=Brush::preset(k);
project.brush.color=color;
}
}
}
);

                ui.color_edit_button_srgb(&mut project.brush.color);

                ui.add(egui::Slider::new(&mut project.brush.size,1.0..=256.0).text("Size"));
ui.add(egui::Slider::new(&mut project.brush.opacity,0.0..=1.0).text("Opacity"));
ui.add(egui::Slider::new(&mut project.brush.flow,0.0..=1.0).text("Flow"));
ui.add(egui::Slider::new(&mut project.brush.hardness,0.0..=1.0).text("Hardness"));
ui.add(egui::Slider::new(&mut project.brush.spacing,0.05..=1.0).text("Spacing"));
ui.add(egui::Slider::new(&mut project.brush.smoothing,0.0..=1.0).text("Smoothing"));
ui.add(egui::Slider::new(&mut project.brush.angle,-std::f64::consts::PI..=std::f64::consts::PI).text("Angle"));

                ui.checkbox(&mut project.brush.pressure_size,"Pressure size");
ui.checkbox(&mut project.brush.pressure_opacity,"Pressure opacity");
ui.checkbox(&mut project.brush.tilt,"Tilt angle");

                ui.separator();
let mut transparent=project.state().background.is_none();

                if ui.checkbox(&mut transparent,"Transparent background").changed()
                    && let Err(e)=project.edit(|s|{
s.background=if transparent{
None}
else{
Some([255;
3])}
;
Ok(())}
){
self.status=e.to_string();
}

                ui.collapsing("Edit selected pixels",|ui|{

                    ui.label("Selected layer only; outside canvas is clipped.");

                    ui.horizontal(|ui|{
ui.label("Move X/Y");
ui.add(egui::DragValue::new(&mut self.transform_offset[0]).range(-4096.0..=4096.0));
ui.add(egui::DragValue::new(&mut self.transform_offset[1]).range(-4096.0..=4096.0));
}
);

                    ui.add(egui::Slider::new(&mut self.transform_scale,0.1..=4.0).text("Scale"));
ui.add(egui::Slider::new(&mut self.transform_angle,-180.0..=180.0).text("Rotation"));

                    let bounds=project.state().selection.as_ref().map(Selection::bounds).unwrap_or([0.0,0.0,project.state().width as f64,project.state().height as f64]);

                    let cx=(bounds[0]+bounds[2])/2.0;
let cy=(bounds[1]+bounds[3])/2.0;

                    if ui.button("Apply transform").clicked(){
let a=self.transform_angle.to_radians();
let(sin,cos)=a.sin_cos();
let c=cos*self.transform_scale;
let b=sin*self.transform_scale;
actions.push(Action::Transform([c,b,-b,c,cx-c*cx+b*cy+self.transform_offset[0],cy-b*cx-c*cy+self.transform_offset[1]]));
}

                    ui.horizontal(|ui|{
if ui.button("Flip H").clicked(){
actions.push(Action::Transform([-1.0,0.0,0.0,1.0,2.0*cx,0.0]));
}

if ui.button("Flip V").clicked(){
actions.push(Action::Transform([1.0,0.0,0.0,-1.0,0.0,2.0*cy]));
}
}
);

                    if ui.add_enabled(project.state().selection.is_some(),egui::Button::new("Crop document to selection bounds")).clicked(){
actions.push(Action::Crop);
}

                }
);

                ui.heading("Layers");
ui.horizontal(|ui|{
if ui.button("+").clicked(){
actions.push(Action::Add);
}

if ui.button("Duplicate").clicked(){
actions.push(Action::Duplicate);
}

if ui.button("Delete").clicked(){
actions.push(Action::Delete);
}
}
);

                ui.horizontal(|ui|{
if ui.button("Up").clicked(){
actions.push(Action::Move(1));
}

if ui.button("Down").clicked(){
actions.push(Action::Move(-1));
}

if ui.button("Clear layer").clicked(){
actions.push(Action::Clear);
}
}
);

                let selected=project.state().selected;

                let layers:Vec<_>=project.state().layers.iter().rev().map(|l|(l.id,l.name.clone(),l.visible,l.locked,l.opacity,l.blend)).collect();

                for(id,mut name,mut visible,mut locked,mut opacity,mut blend)in layers{

                    ui.group(|ui|{
if ui.selectable_label(id==selected,&name).clicked(){
actions.push(Action::Select(id));
}

                        let mut changed=ui.add(egui::TextEdit::singleline(&mut name).char_limit(64)).changed();

                        changed|=ui.checkbox(&mut visible,"Visible").changed();
changed|=ui.checkbox(&mut locked,"Lock").changed();
changed|=ui.add(egui::Slider::new(&mut opacity,0..=255).text("Layer opacity")).changed();

                        let old=blend;
egui::ComboBox::from_id_salt(("blend",id)).selected_text(format!("{blend:?}")).show_ui(ui,|ui|{
for b in [Blend::Normal,Blend::Multiply,Blend::Screen,Blend::Overlay]{
ui.selectable_value(&mut blend,b,format!("{b:?}"));
}
}
);
changed|=old!=blend;

                        if changed{
actions.push(Action::Layer(id,name,visible,locked,opacity,blend));
}

                    }
);

                }

                ui.separator();
ui.label("L: paint · P/1/2/3: replay");
ui.label("Ctrl-Z / Ctrl-Y: history");

                if !self.recovery_files.is_empty(){
ui.heading("Recover artwork");
for p in self.recovery_files.clone(){
if ui.button(p.file_name().unwrap_or_default().to_string_lossy()).clicked(){
actions.push(Action::Recover(p));
}
}
}

            }
);
}
);

            egui::CentralPanel::default().frame(egui::Frame::NONE).show(ctx,|ui|{
central=ui.available_rect_before_wrap();
}
);

            if self.new_dialog{
egui::Window::new("New document").collapsible(false).show(ctx,|ui|{
ui.add(egui::Slider::new(&mut self.new_width,64..=4096).text("Width"));
ui.add(egui::Slider::new(&mut self.new_height,64..=4096).text("Height"));
ui.add(egui::Slider::new(&mut self.new_dpi,72..=1200).text("DPI"));
if ui.button("Create").clicked(){
actions.push(Action::New);
self.new_dialog=false;
}

if ui.button("Cancel").clicked(){
self.new_dialog=false;
}
}
);
}

            if self.file_dialog{
egui::Window::new("Project / image path").collapsible(false).show(ctx,|ui|{
ui.label("Enter a full path or a file in the current folder. RPL for editable projects; PNG for images.");
ui.add(egui::TextEdit::singleline(&mut self.path).char_limit(4096));
if ui.button("Continue").clicked()||ui.input(|i|i.key_pressed(egui::Key::Enter)){
if let Some(a)=self.action.take(){
actions.push(a);
}
self.file_dialog=false;
}

if ui.button("Cancel").clicked(){
self.file_dialog=false;
self.action=None;
}
}
);
}

            if self.confirm.is_some(){
egui::Window::new("Preserve unsaved artwork").collapsible(false).show(ctx,|ui|{
ui.label("Save first, or retain a recovery checkpoint before replacing this document.");
if ui.button("Keep recovery and continue").clicked(){
if self.preserve(session).is_ok(){
approved=true;
actions.push(self.confirm.take().unwrap());
}
else{
self.status="Recovery unavailable: save current artwork before continuing".into();
}
}

if ui.button("Cancel").clicked(){
self.confirm=None;
}
}
);
}

            if self.close_requested{
egui::Window::new("Close artwork?").collapsible(false).show(ctx,|ui|{
ui.label("Unsaved artwork will be retained in local recovery. Save the project for an explicit editable file.");
if ui.button("Save project and close").clicked(){
actions.push(Action::Save);
self.action=Some(Action::Save);
}

if ui.button("Keep recovery and close").clicked(){
match self.preserve(session){
Ok(())=>self.close_ready=true,Err(e)=>self.status=e.to_string()}
}

if ui.button("Cancel").clicked(){
self.close_requested=false;
}
}
);
}

        }
);

        for a in actions {
            if matches!(a, Action::New | Action::Open | Action::Recover(_))
                && session.live().project().dirty()
                && self.confirm.is_none()
                && !approved
            {
                self.confirm = Some(a);
                continue;
            }
            session.mouse(MouseEvent::Interrupted, 0);
            let result = self.execute(a, session);
            if let Err(error) = result {
                self.status = error.to_string();
            }
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
                let next = Recovery::start(&paint_document::recovery::directory()?)?;
                self.recovery = Some(next);
                *project = Engine::new(self.new_width, self.new_height, self.new_dpi)?;
                self.store = None;
                self.path = "new-artwork.rpl".into();
                self.last_submitted = u64::MAX;
            }
            Action::Open | Action::Recover(_) => {
                let path = if let Action::Recover(p) = &a {
                    p.clone()
                } else {
                    PathBuf::from(&self.path)
                };
                let (store, state) = Store::open(path)?;
                let next = Recovery::start(&paint_document::recovery::directory()?)?;
                *project = Engine::from_state(state)?;
                self.recovery = Some(next);
                if matches!(a, Action::Recover(_)) {
                    self.path = "recovered-artwork.rpl".into();
                    self.store = None;
                    project.revision = 1;
                } else {
                    self.path = store.path.to_string_lossy().into();
                    self.store = Some(store);
                }
                self.last_submitted = u64::MAX;
            }
            Action::Save | Action::SaveAs => {
                project.sync_brush_settings();
                if matches!(a, Action::SaveAs) || self.store.is_none() {
                    let mut replacement = Store::new(PathBuf::from(&self.path));
                    replacement.save(project.state())?;
                    self.store = Some(replacement);
                } else {
                    self.store.as_mut().unwrap().save(project.state())?;
                }
                project.saved_revision = project.revision;
                if self.close_requested {
                    self.close_ready = true;
                }
                self.status = "Editable project saved; previous valid file retained as .bak".into();
            }
            Action::Export => {
                storage::export_png(std::path::Path::new(&self.path), project)?;
                self.status = "PNG exported (flattened 8-bit sRGB RGBA)".into();
            }
            Action::Import => storage::import_png(std::path::Path::new(&self.path), project)?,
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
