#![deny(unsafe_code)]
// The shared library is still forbid(unsafe_code). Only this small HWND observer
// needs FFI; it owns no input loop and does not manipulate capture.
#[cfg(windows)]
#[allow(unsafe_code)]
mod windows_capture;
use canvas_demo::{
    gpu::{Failure, Renderer},
    interaction::{CanvasSession, Mode, MouseEvent},
    playback::Scenario,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

struct Graphics {
    #[cfg(windows)]
    capture: windows_capture::Observer,
    // Surface owns an Arc to Window, and is dropped before the instance/window.
    surface: wgpu::Surface<'static>,
    renderer: Renderer,
    instance: wgpu::Instance,
    config: wgpu::SurfaceConfiguration,
    window: Arc<Window>,
    size: [u32; 2],
    recoveries: u8,
    studio: Option<canvas_demo::studio::Studio>,
    raster: Option<canvas_demo::raster_gpu::RasterGpu>,
}
impl Graphics {
    async fn new(
        event_loop: &ActiveEventLoop,
        _proxy: winit::event_loop::EventLoopProxy<()>,
        _epoch: Instant,
        alpha: bool,
    ) -> Result<Self, Failure> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Rust Paint Lab — starting GPU canvas")
                    .with_inner_size(winit::dpi::LogicalSize::new(1000, 700))
                    .with_max_inner_size(winit::dpi::PhysicalSize::new(4096, 4096)),
            )?,
        );
        #[cfg(windows)]
        let capture = windows_capture::Observer::install(window.clone(), _proxy, _epoch)?;
        let mut descriptor = wgpu::InstanceDescriptor::from_env_or_default();
        // Basic SM5 shader only: use the Windows-supplied compiler, no DXC DLL bundle.
        descriptor.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;
        let instance = wgpu::Instance::new(&descriptor);
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface), power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default()
        }).await.map_err(|e| format!("No compatible graphics adapter: {e}. Update the graphics driver or try WGPU_BACKEND=vulkan."))?;
        eprintln!("Canvas adapter: {:?}", adapter.get_info());
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("No compatible surface format")?;
        if let Some(format) = surface
            .get_capabilities(&adapter)
            .formats
            .into_iter()
            .find(|f| f.is_srgb())
        {
            config.format = format;
        }
        let renderer = Renderer::new(&adapter, config.format).await?;
        let max = renderer.device.limits().max_texture_dimension_2d.min(4096);
        window.set_max_inner_size(Some(winit::dpi::PhysicalSize::new(max, max)));
        let studio =
            alpha.then(|| canvas_demo::studio::Studio::new(&renderer, &window, config.format));
        let mut this = Self {
            #[cfg(windows)]
            capture,
            surface,
            renderer,
            instance,
            config,
            window,
            size: [0; 2],
            recoveries: 0,
            studio,
            raster: None,
        };
        this.resize([size.width, size.height])?;
        Ok(this)
    }
    fn resize(&mut self, size: [u32; 2]) -> Result<(), Failure> {
        self.size = size;
        if size.contains(&0) {
            return Ok(());
        }
        let limit = self
            .renderer
            .device
            .limits()
            .max_texture_dimension_2d
            .min(4096);
        if size.iter().any(|v| *v > limit) {
            return Err(format!("Window size exceeds bounded render limit {limit}px").into());
        }
        self.config.width = size[0];
        self.config.height = size[1];
        self.surface.configure(&self.renderer.device, &self.config);
        self.renderer.check_error()
    }
    fn draw(&mut self, session: &mut CanvasSession) -> Result<bool, Failure> {
        self.renderer.check_error()?;
        if self.size.contains(&0) {
            return Ok(false);
        }
        let (frame, suboptimal) = match self.surface.get_current_texture() {
            Ok(frame) => {
                let suboptimal = frame.suboptimal;
                (frame, suboptimal)
            }
            Err(wgpu::SurfaceError::Timeout) => return Ok(false),
            Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                self.recoveries += 1;
                if self.recoveries > 8 {
                    return Err("Surface repeatedly lost/outdated".into());
                }
                self.surface = self.instance.create_surface(self.window.clone())?;
                self.resize(self.size)?;
                return Ok(false);
            }
            Err(error) => return Err(format!("Surface error: {error}").into()),
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        if let Some(studio) = &mut self.studio {
            let start = Instant::now();
            let output = studio.frame(&self.window, session, self.size);
            if session.mode() == Mode::Live {
                let dimensions = [
                    session.live().project().state().width,
                    session.live().project().state().height,
                ];
                if self.raster.as_ref().is_none_or(|r| r.size() != dimensions) {
                    self.raster = Some(canvas_demo::raster_gpu::RasterGpu::new(
                        &self.renderer,
                        self.config.format,
                        dimensions,
                    ));
                    session.live_mut().project_mut().damage_all();
                }
                if let Some(mapping) = studio.mapping {
                    let upload = Instant::now();
                    self.raster.as_ref().unwrap().draw(
                        &self.renderer,
                        &view,
                        session.live_mut().project_mut(),
                        mapping,
                    )?;
                    studio.last_upload_us = upload.elapsed().as_micros();
                } else {
                    self.renderer
                        .draw(&view, &canvas_demo::scene::Scene::default(), self.size)?;
                }
            } else {
                self.renderer.draw(&view, session.scene(), self.size)?;
            }
            studio.paint(&self.renderer, &view, output, self.size);
            studio.last_frame_us = start.elapsed().as_micros();
        } else {
            self.renderer.draw(&view, session.scene(), self.size)?;
        }
        self.window.pre_present_notify();
        frame.present();
        self.recoveries = 0;
        if suboptimal {
            self.resize(self.size)?;
        }
        Ok(true)
    }
}

struct App {
    graphics: Option<Graphics>,
    session: CanvasSession,
    proxy: winit::event_loop::EventLoopProxy<()>,
    clock: Instant,
    tick: Instant,
    elapsed: u64,
    paused: bool,
    occluded: bool,
    failure: Option<String>,
    exit_after_demo: bool,
    frames: u64,
    needs_redraw: bool,
    alpha: bool,
    modifiers: winit::keyboard::ModifiersState,
    recovery_poll_deadline: Option<Instant>,
}
impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl std::fmt::Display) {
        self.failure = Some(error.to_string());
        eprintln!("Canvas failed: {error}");
        event_loop.exit();
    }
    fn time_ms(&self) -> u64 {
        self.clock.elapsed().as_millis().min(u64::MAX as u128) as u64
    }
    fn mouse(&mut self, event: MouseEvent) {
        let live = self.session.live();
        let active = live.active();
        let samples = live.scene().sample_count();
        let notice = live.notice();
        self.session.mouse(event, self.time_ms());
        let live = self.session.live();
        if active
            || live.active()
            || samples != live.scene().sample_count()
            || notice != live.notice()
        {
            self.needs_redraw = true;
            if let Some(g) = &self.graphics {
                g.window.request_redraw();
            }
        }
    }
    fn check_capture(&mut self) {
        #[cfg(windows)]
        if let Some(g) = &self.graphics {
            let lost = g.capture.take_lost();
            let events = g.capture.drain_pen();
            if lost {
                self.mouse(MouseEvent::CaptureLost);
            } else if !events.is_empty() {
                for mut event in events {
                    let _source_counter = event.performance_count;
                    if let Some(g) = &mut self.graphics
                        && let Some(studio) = &mut g.studio
                        && studio.native_pen_ui(
                            &event.sample,
                            g.window.scale_factor(),
                            &mut self.session,
                        )
                    {
                        event.sample.phase = paint_input::Phase::Cancel;
                        event.sample.in_contact = false;
                    }
                    self.session.pen(event.sample);
                }
                self.needs_redraw = true;
                if let Some(g) = &self.graphics {
                    g.window.request_redraw();
                }
            }
        }
    }
    fn reset_replay_clock(&mut self) {
        self.elapsed = 0;
        self.tick = Instant::now();
        self.paused = false;
        self.needs_redraw = true;
    }
    fn resize(&mut self, size: [u32; 2]) -> Result<(), Failure> {
        self.mouse(MouseEvent::Resized(if self.alpha { [0, 0] } else { size }));
        if let Some(g) = &mut self.graphics {
            g.resize(size)?;
            g.window.request_redraw();
        }
        self.needs_redraw = true;
        Ok(())
    }
    fn request_close(&mut self) -> bool {
        self.mouse(MouseEvent::Interrupted);
        if let Some(graphics) = &mut self.graphics
            && let Some(studio) = &mut graphics.studio
            && studio.requires_close_save(&self.session)
        {
            if let Err(error) = studio.save_for_close(&mut self.session) {
                studio.mark_close_save_failed(error);
            }
            graphics.window.request_redraw();
            return true;
        }
        false
    }
}
impl ApplicationHandler for App {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. })
            && let Some(g) = &self.graphics
        {
            g.window.request_redraw();
        }
    }
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, (): ()) {
        self.check_capture();
    }
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.graphics.is_none() {
            match pollster::block_on(Graphics::new(
                event_loop,
                self.proxy.clone(),
                self.clock,
                self.alpha,
            )) {
                Ok(graphics) => {
                    self.session.mouse(
                        MouseEvent::Resized(if self.alpha { [0, 0] } else { graphics.size }),
                        self.time_ms(),
                    );
                    graphics.window.request_redraw();
                    self.graphics = Some(graphics);
                    self.needs_redraw = true;
                    self.tick = Instant::now();
                }
                Err(e) => self.fail(event_loop, e),
            }
        }
    }
    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.mouse(MouseEvent::Interrupted);
        self.graphics = None;
        self.tick = Instant::now();
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        self.check_capture();
        if let Some(g) = &mut self.graphics
            && let Some(studio) = &mut g.studio
        {
            let (blocked, repaint) = studio.window_event(&g.window, &event, &mut self.session);
            if repaint {
                g.window.request_redraw();
            }
            if blocked {
                self.mouse(MouseEvent::Interrupted);
                return;
            }
        }
        let result: Result<(), Failure> = (|| {
            match event {
                WindowEvent::CloseRequested => {
                    if !self.request_close() {
                        event_loop.exit();
                    }
                }
                WindowEvent::Destroyed => {
                    self.mouse(MouseEvent::Interrupted);
                    event_loop.exit();
                }
                WindowEvent::ModifiersChanged(m) => self.modifiers = m.state(),
                WindowEvent::Focused(focused) => self.mouse(MouseEvent::Focus(focused)),
                WindowEvent::CursorEntered { .. } => self.mouse(MouseEvent::Entered),
                WindowEvent::CursorLeft { .. } => self.mouse(MouseEvent::Left),
                WindowEvent::CursorMoved { position, .. } => {
                    self.mouse(MouseEvent::Moved([position.x, position.y]))
                }
                WindowEvent::MouseInput { state, button, .. } => self.mouse(MouseEvent::Button {
                    primary: button == MouseButton::Left,
                    pressed: state == ElementState::Pressed,
                }),
                WindowEvent::Resized(s) => self.resize([s.width, s.height])?,
                WindowEvent::ScaleFactorChanged { .. } => {
                    self.mouse(MouseEvent::Interrupted);
                    if let Some(g) = &self.graphics {
                        let size = g.window.inner_size();
                        self.resize([size.width, size.height])?;
                    }
                }
                WindowEvent::Occluded(value) => {
                    if value {
                        self.mouse(MouseEvent::Interrupted);
                    }
                    self.occluded = value;
                    self.tick = Instant::now();
                    if !value && let Some(g) = &self.graphics {
                        self.needs_redraw = true;
                        g.window.request_redraw();
                    }
                }
                WindowEvent::KeyboardInput { event, .. }
                    if event.state == ElementState::Pressed && !event.repeat =>
                {
                    if let Some(g) = &mut self.graphics
                        && let Some(studio) = &mut g.studio
                    {
                        if studio.context.wants_keyboard_input() {
                            return Ok(());
                        }
                        if !self.modifiers.control_key() {
                            let key = match event.physical_key {
                                PhysicalKey::Code(KeyCode::Digit0) => Some('0'),
                                PhysicalKey::Code(KeyCode::KeyB) => Some('b'),
                                PhysicalKey::Code(KeyCode::KeyM) => Some('m'),
                                PhysicalKey::Code(KeyCode::KeyE) => {
                                    Some(if self.modifiers.shift_key() { 'q' } else { 'e' })
                                }
                                PhysicalKey::Code(KeyCode::KeyK) => Some('k'),
                                PhysicalKey::Code(KeyCode::KeyI) => Some('i'),
                                _ => None,
                            };
                            if let Some(key) = key {
                                studio.tool_shortcut(key, &mut self.session);
                                g.window.request_redraw();
                                return Ok(());
                            }
                        }
                        if self.modifiers.control_key() {
                            let key = match event.physical_key {
                                PhysicalKey::Code(KeyCode::KeyN) => Some('n'),
                                PhysicalKey::Code(KeyCode::KeyB) if self.modifiers.shift_key() => {
                                    Some('t')
                                }
                                PhysicalKey::Code(KeyCode::KeyB) if self.modifiers.alt_key() => {
                                    Some('u')
                                }
                                PhysicalKey::Code(KeyCode::KeyZ) => Some('z'),
                                PhysicalKey::Code(KeyCode::KeyY) => Some('y'),
                                PhysicalKey::Code(KeyCode::KeyS) => Some('s'),
                                PhysicalKey::Code(KeyCode::KeyO) => {
                                    Some(if self.modifiers.shift_key() { 'a' } else { 'o' })
                                }
                                PhysicalKey::Code(KeyCode::KeyE) => {
                                    Some(if self.modifiers.shift_key() { 'q' } else { 'e' })
                                }
                                PhysicalKey::Code(KeyCode::KeyR) if self.modifiers.shift_key() => {
                                    Some('v')
                                }
                                PhysicalKey::Code(KeyCode::KeyF) => Some('f'),
                                PhysicalKey::Code(KeyCode::KeyD) => Some('d'),
                                PhysicalKey::Code(KeyCode::KeyX) => Some('x'),
                                _ => None,
                            };
                            if let Some(key) = key {
                                studio.shortcut(key, &mut self.session);
                                g.window.request_redraw();
                                return Ok(());
                            }
                        }
                    }
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Digit1 | KeyCode::Digit2 | KeyCode::Digit3) => {
                            let scenario = match event.physical_key {
                                PhysicalKey::Code(KeyCode::Digit1) => Scenario::Completed,
                                PhysicalKey::Code(KeyCode::Digit2) => Scenario::Cancelled,
                                _ => Scenario::Concurrent,
                            };
                            self.session.select_replay(scenario, self.time_ms());
                            self.reset_replay_clock();
                        }
                        PhysicalKey::Code(KeyCode::KeyL) => {
                            self.session.set_mode(Mode::Live, self.time_ms());
                            self.needs_redraw = true;
                        }
                        PhysicalKey::Code(KeyCode::KeyP) => {
                            self.session
                                .select_replay(self.session.scenario(), self.time_ms());
                            self.reset_replay_clock();
                        }
                        PhysicalKey::Code(KeyCode::Tab) => {
                            let mode = if self.session.mode() == Mode::Live {
                                Mode::Replay
                            } else {
                                Mode::Live
                            };
                            self.session.set_mode(mode, self.time_ms());
                            self.reset_replay_clock();
                        }
                        PhysicalKey::Code(KeyCode::KeyC) => {
                            self.session.clear_live(self.time_ms());
                            self.needs_redraw = true;
                        }
                        PhysicalKey::Code(KeyCode::KeyR) => {
                            self.session.restart_replay();
                            self.reset_replay_clock();
                        }
                        PhysicalKey::Code(KeyCode::Space)
                            if self.session.mode() == Mode::Replay =>
                        {
                            self.paused = !self.paused;
                            self.tick = Instant::now();
                        }
                        PhysicalKey::Code(KeyCode::Escape) => {
                            if !self.request_close() {
                                event_loop.exit();
                            }
                        }
                        PhysicalKey::Code(KeyCode::F11) => {
                            if let Some(g) = &self.graphics {
                                g.window.set_fullscreen(if g.window.fullscreen().is_some() {
                                    None
                                } else {
                                    Some(winit::window::Fullscreen::Borderless(None))
                                });
                            }
                        }
                        _ => {}
                    }
                    if let Some(g) = &self.graphics {
                        g.window.request_redraw();
                    }
                }
                WindowEvent::RedrawRequested => {
                    let now = Instant::now();
                    if self.session.mode() == Mode::Replay && !self.paused && !self.occluded {
                        self.elapsed = self.elapsed.saturating_add(
                            now.duration_since(self.tick)
                                .as_millis()
                                .min(u64::MAX as u128) as u64,
                        );
                    }
                    self.tick = now;
                    self.session.advance_replay(self.elapsed)?;
                    if let Some(g) = &mut self.graphics {
                        let title = if g.studio.is_some() && self.session.mode() == Mode::Live {
                            format!(
                                "Rust Paint Lab Alpha — {} — {}×{}{}",
                                g.studio.as_ref().unwrap().project_label(),
                                self.session.live().project().state().width,
                                self.session.live().project().state().height,
                                if self.session.live().project().dirty() {
                                    " *"
                                } else {
                                    ""
                                }
                            )
                        } else if self.session.mode() == Mode::Live {
                            let state = if self.session.live().notice().is_some() {
                                "Canvas limit: C to clear"
                            } else {
                                "Left draw | C clear | P replay | Esc close"
                            };
                            format!(
                                "Rust Paint Lab | LIVE mouse | {} completed | {} | orange preview / blue committed | {state}",
                                self.session.scene().committed().len(),
                                if self.session.live().active() {
                                    "DRAWING"
                                } else {
                                    "READY"
                                }
                            )
                        } else {
                            let status = if self.session.replay_finished() {
                                "FINISHED"
                            } else if self.paused {
                                "PAUSED"
                            } else {
                                "PLAYING"
                            };
                            format!(
                                "Rust Paint Lab | REPLAY {} | {status} | 1/2/3 select, R replay, Space pause, L live",
                                self.session.scenario().name()
                            )
                        };
                        let title = if let Some(prompt) = g
                            .studio
                            .as_ref()
                            .and_then(|studio| studio.test_prompt_label())
                        {
                            format!("{title} | {prompt}")
                        } else {
                            title
                        };
                        let presented = g.draw(&mut self.session)?;
                        // Publish the ready title only after the first layout and frame.
                        if presented {
                            g.window.set_title(&title);
                        }
                        if g.studio.as_ref().is_some_and(|s| s.close_ready) {
                            event_loop.exit();
                        }
                        self.needs_redraw = !presented;
                        if presented {
                            self.frames += 1;
                        }
                        if self.exit_after_demo && self.alpha && self.frames > 10 && presented {
                            eprintln!("ALPHA_WINDOW_SMOKE: presented controls and raster document");
                            event_loop.exit();
                        }
                        if self.exit_after_demo
                            && self.session.mode() == Mode::Replay
                            && self.session.replay_finished()
                            && presented
                        {
                            eprintln!(
                                "WINDOW_SMOKE: rendered {} frames and completed replay",
                                self.frames
                            );
                            event_loop.exit();
                        }
                    }
                }
                _ => {} // No native pen, touch painting, or navigation gestures.
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.fail(event_loop, e);
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let can_render =
            !self.occluded && self.graphics.as_ref().is_some_and(|g| !g.size.contains(&0));
        if !can_render {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        let now = Instant::now();
        let autosave_deadline = self
            .graphics
            .as_ref()
            .and_then(|g| g.studio.as_ref())
            .and_then(|s| s.autosave_deadline(&self.session));
        let autosave_due = autosave_deadline.is_some_and(|deadline| now >= deadline);
        let recovery_pending = self
            .graphics
            .as_ref()
            .and_then(|g| g.studio.as_ref())
            .is_some_and(|s| s.recovery_save_pending());
        if recovery_pending {
            let deadline = *self
                .recovery_poll_deadline
                .get_or_insert(now + Duration::from_millis(100));
            let frame_tick_needed = self.needs_redraw
                || (self.exit_after_demo && self.alpha)
                || (self.session.mode() == Mode::Replay
                    && !self.paused
                    && !self.session.replay_finished());
            let poll_due = now >= deadline;
            if poll_due || autosave_due {
                if let Some(g) = &self.graphics {
                    g.window.request_redraw();
                }
                self.recovery_poll_deadline = Some(now + Duration::from_millis(100));
            }
            if frame_tick_needed || autosave_due {
                event_loop
                    .set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(16)));
            } else {
                event_loop.set_control_flow(ControlFlow::WaitUntil(if poll_due {
                    now + Duration::from_millis(16)
                } else {
                    deadline
                }));
            }
            return;
        }
        self.recovery_poll_deadline = None;
        if autosave_due {
            if let Some(g) = &self.graphics {
                g.window.request_redraw();
            }
            event_loop.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(16)));
        } else if self.needs_redraw
            || (self.exit_after_demo && self.alpha)
            || (self.session.mode() == Mode::Replay
                && !self.paused
                && !self.session.replay_finished())
        {
            event_loop.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(16)));
        } else if let Some(deadline) = autosave_deadline {
            event_loop.set_control_flow(ControlFlow::WaitUntil(deadline));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}
fn main() -> Result<(), Failure> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args
        .iter()
        .any(|a| !["--exit-after-demo", "--compat-mouse", "--alpha-smoke"].contains(&a.as_str()))
    {
        return Err("usage: canvas-demo [--exit-after-demo]".into());
    }
    let event_loop = EventLoop::<()>::with_user_event().build()?;
    let mut session = CanvasSession::new([1000, 700]);
    let alpha = !args
        .iter()
        .any(|a| ["--compat-mouse", "--exit-after-demo"].contains(&a.as_str()));
    if alpha {
        session.live_mut().enable_document();
    }
    if args.iter().any(|a| a == "--exit-after-demo") {
        session.select_replay(Scenario::Completed, 0);
    }
    let mut app = App {
        graphics: None,
        session,
        proxy: event_loop.create_proxy(),
        clock: Instant::now(),
        tick: Instant::now(),
        elapsed: 0,
        paused: false,
        occluded: false,
        failure: None,
        exit_after_demo: args
            .iter()
            .any(|a| a == "--exit-after-demo" || a == "--alpha-smoke"),
        frames: 0,
        needs_redraw: true,
        alpha,
        modifiers: winit::keyboard::ModifiersState::empty(),
        recovery_poll_deadline: None,
    };
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.failure {
        return Err(error.into());
    }
    Ok(())
}
