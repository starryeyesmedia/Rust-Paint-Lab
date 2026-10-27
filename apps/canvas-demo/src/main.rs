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

enum AppEvent {
    #[cfg(windows)]
    Wake,
    AccessKit(egui_winit::accesskit_winit::Event),
}

impl From<egui_winit::accesskit_winit::Event> for AppEvent {
    fn from(event: egui_winit::accesskit_winit::Event) -> Self {
        Self::AccessKit(event)
    }
}

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
    home_clear_logged: bool,
    screenshot_capture_enabled: bool,
    screenshot_capture: Option<egui_wgpu::capture::CaptureState>,
    startup_smoke_enabled: bool,
    startup_smoke_first_frame: bool,
}
impl Graphics {
    async fn new(
        event_loop: &ActiveEventLoop,
        _proxy: winit::event_loop::EventLoopProxy<AppEvent>,
        _epoch: Instant,
        alpha: bool,
    ) -> Result<Self, Failure> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("BitchPixel — Creative Studio")
                    .with_inner_size(winit::dpi::LogicalSize::new(1000, 700))
                    .with_max_inner_size(winit::dpi::PhysicalSize::new(4096, 4096))
                    .with_visible(false),
            )?,
        );
        #[cfg(windows)]
        let capture = windows_capture::Observer::install(window.clone(), _proxy.clone(), _epoch)?;
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
        let capabilities = surface.get_capabilities(&adapter);
        if let Some(format) = capabilities.formats.into_iter().find(|f| f.is_srgb()) {
            config.format = format;
        }
        let screenshot_test = std::env::var_os("RPL_TEST_PATH_DIALOG").is_some()
            && std::env::var_os("RPL_UI_SCREENSHOT_DIR").is_some();
        let renderer = Renderer::new(&adapter, config.format).await?;
        let max = renderer.device.limits().max_texture_dimension_2d.min(4096);
        window.set_max_inner_size(Some(winit::dpi::PhysicalSize::new(max, max)));
        let mut studio =
            alpha.then(|| canvas_demo::studio::Studio::new(&renderer, &window, config.format));
        if let Some(studio) = &mut studio {
            studio.init_accesskit(event_loop, &window, _proxy.clone());
        }
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
            home_clear_logged: false,
            screenshot_capture_enabled: screenshot_test,
            screenshot_capture: None,
            startup_smoke_enabled: std::env::var_os("RPL_STARTUP_SMOKE").is_some(),
            startup_smoke_first_frame: false,
        };
        this.resize([size.width, size.height])?;
        // AccessKit's winit adapter must exist before the native window is
        // first shown. Keep the window hidden while GPU and Studio setup can
        // still fail, then expose it and explicitly schedule the first frame.
        this.window.set_visible(true);
        if this.startup_smoke_enabled {
            eprintln!("WINDOW_STARTUP_SMOKE_VISIBLE");
        }
        this.window.request_redraw();
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
        let screenshot_name = self.take_screenshot_request()?;
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
        // Most backends do not allow readback from their swapchain texture.
        // For explicit UI-test captures, egui-wgpu's capture helper provides
        // an offscreen render target and copies the composed frame both to the
        // swapchain and a bounded readback buffer. Normal application frames
        // continue to render directly to the surface.
        let capture_view = if screenshot_name.is_some() {
            let capture = self.screenshot_capture.get_or_insert_with(|| {
                egui_wgpu::capture::CaptureState::new(&self.renderer.device, &frame.texture)
            });
            capture.update(&self.renderer.device, &frame.texture);
            Some(capture.texture.create_view(&Default::default()))
        } else {
            None
        };
        let surface_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let view = capture_view.as_ref().unwrap_or(&surface_view);
        if let Some(studio) = &mut self.studio {
            let start = Instant::now();
            let output = studio.frame(&self.window, session, self.size);
            let home_surface = studio.home_surface_color();
            if home_surface.is_some()
                && std::env::var_os("RPL_TEST_PATH_DIALOG").is_some()
                && !self.home_clear_logged
            {
                self.home_clear_logged = true;
                eprintln!("UI_TEST_HOME_SURFACE_CLEAR: enabled");
            }
            if let Some(color) = home_surface {
                let [red, green, blue, alpha] = color.to_array();
                self.renderer.clear(
                    view,
                    [red, green, blue, alpha].map(|component| f32::from(component) / 255.0),
                )?;
            } else if session.mode() == Mode::Live {
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
                        view,
                        session.live_mut().project_mut(),
                        mapping,
                    )?;
                    studio.last_upload_us = upload.elapsed().as_micros();
                } else {
                    self.renderer
                        .draw(view, &canvas_demo::scene::Scene::default(), self.size)?;
                }
            } else {
                self.renderer.draw(view, session.scene(), self.size)?;
            }
            studio.paint(&self.renderer, view, output, self.size);
            studio.last_frame_us = start.elapsed().as_micros();
        } else {
            self.renderer.draw(view, session.scene(), self.size)?;
        }
        if let Some(name) = screenshot_name {
            let mut encoder =
                self.renderer
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("UI test screenshot copy"),
                    });
            let buffer = self
                .screenshot_capture
                .as_mut()
                .expect("capture target is initialized for an active request")
                .copy_textures(&self.renderer.device, &frame, &mut encoder);
            self.renderer.queue.submit([encoder.finish()]);
            self.write_test_screenshot(&name, buffer)?;
        }
        self.window.pre_present_notify();
        frame.present();
        if self.startup_smoke_enabled && !self.startup_smoke_first_frame {
            self.startup_smoke_first_frame = true;
            eprintln!("WINDOW_STARTUP_SMOKE_FIRST_FRAME");
        }
        self.recoveries = 0;
        if suboptimal {
            self.resize(self.size)?;
        }
        Ok(true)
    }

    /// Consume one request from the Linux UI screenshot harness. This is only
    /// enabled when both test-only environment variables are present.
    fn take_screenshot_request(&self) -> Result<Option<String>, Failure> {
        if !self.screenshot_capture_enabled {
            return Ok(None);
        }
        let Some(directory) =
            std::env::var_os("RPL_UI_SCREENSHOT_DIR").map(std::path::PathBuf::from)
        else {
            return Ok(None);
        };
        let request = directory.join("capture.request");
        let name = match std::fs::read_to_string(&request) {
            Ok(name) => name.trim().to_owned(),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        std::fs::remove_file(&request)?;
        if name.is_empty()
            || name.len() > 80
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("invalid screenshot request name".into());
        }
        Ok(Some(name))
    }

    /// Read the rendered test target only after an explicit request. The
    /// capture target and transfer buffer are bounded by the existing 4096 px
    /// window limit; no capture allocations occur during normal launches.
    fn write_test_screenshot(&self, name: &str, buffer: wgpu::Buffer) -> Result<(), Failure> {
        let [width, height] = self.size;
        let bgra = matches!(
            self.config.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        );
        let row_bytes = width.checked_mul(4).ok_or("screenshot row overflow")?;
        let alignment = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_row_bytes = row_bytes
            .checked_add(alignment - 1)
            .ok_or("screenshot row overflow")?
            / alignment
            * alignment;
        let buffer_size = u64::from(padded_row_bytes)
            .checked_mul(u64::from(height))
            .ok_or("screenshot buffer overflow")?;
        if buffer_size > 128 * 1024 * 1024 {
            return Err("screenshot exceeds the 128 MiB readback bound".into());
        }
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        self.renderer.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: Some(Duration::from_secs(15)),
        })?;
        receiver.recv_timeout(Duration::from_secs(15))??;
        let mapped = buffer.slice(..).get_mapped_range();
        let output_size = usize::try_from(width)
            .ok()
            .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)))
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or("screenshot output size overflow")?;
        let mut pixels = Vec::with_capacity(output_size);
        for row in mapped
            .chunks_exact(padded_row_bytes as usize)
            .take(height as usize)
        {
            for pixel in row[..row_bytes as usize].chunks_exact(4) {
                if bgra {
                    pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
                } else {
                    pixels.extend_from_slice(pixel);
                }
            }
        }
        drop(mapped);
        buffer.unmap();
        if pixels.len() != output_size {
            return Err("screenshot readback returned an incomplete pixel buffer".into());
        }
        let mut capture = Vec::with_capacity(16 + pixels.len());
        capture.extend_from_slice(b"RPLCAP01");
        capture.extend_from_slice(&width.to_le_bytes());
        capture.extend_from_slice(&height.to_le_bytes());
        capture.extend_from_slice(&pixels);
        let directory = std::env::var_os("RPL_UI_SCREENSHOT_DIR")
            .map(std::path::PathBuf::from)
            .ok_or("screenshot directory disappeared during capture")?;
        let temporary = directory.join(format!(".{name}.rplcap.tmp"));
        let destination = directory.join(format!("{name}.rplcap"));
        std::fs::write(&temporary, capture)?;
        std::fs::rename(temporary, destination)?;
        eprintln!("UI_SCREENSHOT_GPU_CAPTURED: {name} {width}x{height}");
        Ok(())
    }
}

struct App {
    graphics: Option<Graphics>,
    session: CanvasSession,
    proxy: winit::event_loop::EventLoopProxy<AppEvent>,
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
        if std::env::var_os("RPL_TEST_PATH_DIALOG").is_some() && !active && live.active() {
            eprintln!(
                "UI_TEST_MOUSE_STROKE_STARTED: {}",
                live.scene().sample_count()
            );
        }
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
impl ApplicationHandler<AppEvent> for App {
    fn new_events(&mut self, _event_loop: &ActiveEventLoop, cause: StartCause) {
        if matches!(cause, StartCause::ResumeTimeReached { .. })
            && let Some(g) = &self.graphics
        {
            g.window.request_redraw();
        }
    }
    fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            #[cfg(windows)]
            AppEvent::Wake => self.check_capture(),
            AppEvent::AccessKit(event) => {
                if let Some(graphics) = &mut self.graphics
                    && graphics.window.id() == event.window_id
                {
                    if let egui_winit::accesskit_winit::WindowEvent::ActionRequested(request) =
                        event.window_event
                        && let Some(studio) = &mut graphics.studio
                    {
                        studio.accesskit_action_request(request);
                    }
                    graphics.window.request_redraw();
                }
            }
        }
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
                        if event.physical_key == PhysicalKey::Code(KeyCode::Escape)
                            && studio.dismiss_flyout()
                        {
                            g.window.request_redraw();
                            return Ok(());
                        }
                        if !studio.blocks_canvas()
                            && event.physical_key == PhysicalKey::Code(KeyCode::F6)
                        {
                            studio.shortcut('i', &mut self.session);
                            g.window.request_redraw();
                            return Ok(());
                        }
                        let focused_text_edit = studio
                            .context
                            .memory(|memory| memory.focused())
                            .is_some_and(|id| {
                                egui::TextEdit::load_state(&studio.context, id).is_some()
                            });
                        if focused_text_edit {
                            return Ok(());
                        }
                        if !studio.blocks_canvas()
                            && self.session.mode() == Mode::Live
                            && let PhysicalKey::Code(key) = event.physical_key
                            && matches!(key, KeyCode::BracketLeft | KeyCode::BracketRight)
                        {
                            studio.adjust_brush_size(
                                key == KeyCode::BracketRight,
                                self.modifiers.shift_key(),
                                &mut self.session,
                            );
                            g.window.request_redraw();
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
                                PhysicalKey::Code(KeyCode::KeyC) => Some('c'),
                                PhysicalKey::Code(KeyCode::KeyL) => Some('l'),
                                PhysicalKey::Code(KeyCode::KeyR) => Some('r'),
                                PhysicalKey::Code(KeyCode::KeyO) => Some('o'),
                                PhysicalKey::Code(KeyCode::KeyG) => Some('g'),
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
                                PhysicalKey::Code(KeyCode::KeyN) if self.modifiers.shift_key() => {
                                    Some('j')
                                }
                                PhysicalKey::Code(KeyCode::KeyN) => Some('n'),
                                PhysicalKey::Code(KeyCode::KeyM) if self.modifiers.shift_key() => {
                                    Some('g')
                                }
                                PhysicalKey::Code(KeyCode::KeyB) if self.modifiers.alt_key() => {
                                    Some('u')
                                }
                                PhysicalKey::Code(KeyCode::KeyB) if self.modifiers.shift_key() => {
                                    Some('t')
                                }
                                PhysicalKey::Code(KeyCode::KeyB) => Some('b'),
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
                                PhysicalKey::Code(KeyCode::KeyL) => Some('l'),
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
                                "BitchPixel — {} — {}×{}{}",
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
                                "BitchPixel | LIVE mouse | {} completed | {} | orange preview / blue committed | {state}",
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
                                "BitchPixel | REPLAY {} | {status} | 1/2/3 select, R replay, Space pause, L live",
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
                        let title = if std::env::var_os("RPL_TEST_PATH_DIALOG").is_some() {
                            format!("{title} | RPL_UI_TEST_WINDOW")
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
    let event_loop = EventLoop::<AppEvent>::with_user_event().build()?;
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
