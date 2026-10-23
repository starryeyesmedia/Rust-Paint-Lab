#![forbid(unsafe_code)]
use canvas_demo::{
    gpu::{Failure, Renderer},
    playback::{Playback, Scenario},
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    event::{ElementState, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

struct Graphics {
    // Surface owns an Arc to Window, and is dropped before the instance/window.
    surface: wgpu::Surface<'static>,
    renderer: Renderer,
    instance: wgpu::Instance,
    config: wgpu::SurfaceConfiguration,
    window: Arc<Window>,
    size: [u32; 2],
    recoveries: u8,
}
impl Graphics {
    async fn new(event_loop: &ActiveEventLoop) -> Result<Self, Failure> {
        let window = Arc::new(
            event_loop.create_window(
                Window::default_attributes()
                    .with_title("Rust Paint Lab — starting GPU canvas")
                    .with_inner_size(winit::dpi::LogicalSize::new(1000, 700))
                    .with_max_inner_size(winit::dpi::PhysicalSize::new(4096, 4096)),
            )?,
        );
        let mut descriptor = wgpu::InstanceDescriptor::new_with_display_handle_from_env(Box::new(
            event_loop.owned_display_handle(),
        ));
        // Basic SM5 shader only: use the Windows-supplied compiler, no DXC DLL bundle.
        descriptor.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;
        let instance = wgpu::Instance::new(descriptor);
        let surface = instance.create_surface(window.clone())?;
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface), power_preference: wgpu::PowerPreference::HighPerformance, ..Default::default()
        }).await.map_err(|e| format!("No compatible graphics adapter: {e}. Update the graphics driver or try WGPU_BACKEND=vulkan."))?;
        eprintln!("Canvas adapter: {:?}", adapter.get_info());
        let size = window.inner_size();
        let config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .ok_or("No compatible surface format")?;
        let renderer = Renderer::new(&adapter, config.format).await?;
        let max = renderer.device.limits().max_texture_dimension_2d.min(4096);
        window.set_max_inner_size(Some(winit::dpi::PhysicalSize::new(max, max)));
        let mut this = Self {
            surface,
            renderer,
            instance,
            config,
            window,
            size: [0; 2],
            recoveries: 0,
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
    fn draw(&mut self, scene: &canvas_demo::scene::Scene) -> Result<bool, Failure> {
        self.renderer.check_error()?;
        if self.size.contains(&0) {
            return Ok(false);
        }
        let (frame, suboptimal) = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame) => (frame, false),
            wgpu::CurrentSurfaceTexture::Suboptimal(frame) => (frame, true),
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.recoveries += 1;
                if self.recoveries > 8 {
                    return Err("Surface repeatedly lost/outdated; restart the demo and check graphics drivers".into());
                }
                // Recreate surface for Lost; also safe for Outdated. Never recreate the event loop.
                self.surface = self.instance.create_surface(self.window.clone())?;
                self.resize(self.size)?;
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("Surface validation failed; see GPU diagnostics".into());
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.renderer.draw(&view, scene, self.size)?;
        self.window.pre_present_notify();
        self.renderer.queue.present(frame);
        self.recoveries = 0;
        if suboptimal {
            self.resize(self.size)?;
        }
        Ok(true)
    }
}

struct App {
    graphics: Option<Graphics>,
    playback: Playback,
    scenario: Scenario,
    tick: Instant,
    elapsed: u64,
    paused: bool,
    occluded: bool,
    failure: Option<String>,
    exit_after_demo: bool,
    frames: u64,
    needs_redraw: bool,
}
impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: impl std::fmt::Display) {
        self.failure = Some(error.to_string());
        eprintln!("Canvas failed: {error}");
        event_loop.exit();
    }
    fn restart(&mut self, scenario: Scenario) -> Result<(), Failure> {
        self.playback = Playback::new(scenario)?;
        self.scenario = scenario;
        self.needs_redraw = true;
        self.elapsed = 0;
        self.tick = Instant::now();
        self.paused = false;
        Ok(())
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
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.graphics.is_none() {
            match pollster::block_on(Graphics::new(event_loop)) {
                Ok(graphics) => {
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
        self.graphics = None;
        self.tick = Instant::now();
    }
    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let result: Result<(), Failure> = (|| {
            match event {
                WindowEvent::CloseRequested => event_loop.exit(),
                WindowEvent::Resized(s) => {
                    if let Some(g) = &mut self.graphics {
                        g.resize([s.width, s.height])?;
                        self.needs_redraw = true;
                        g.window.request_redraw();
                    }
                }
                WindowEvent::Occluded(value) => {
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
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Digit1) => self.restart(Scenario::Completed)?,
                        PhysicalKey::Code(KeyCode::Digit2) => self.restart(Scenario::Cancelled)?,
                        PhysicalKey::Code(KeyCode::Digit3) => self.restart(Scenario::Concurrent)?,
                        PhysicalKey::Code(KeyCode::KeyR) => self.restart(self.scenario)?,
                        PhysicalKey::Code(KeyCode::Space) => {
                            self.paused = !self.paused;
                            self.tick = Instant::now();
                        }
                        PhysicalKey::Code(KeyCode::Escape) => event_loop.exit(),
                        _ => {}
                    }
                    if let Some(g) = &self.graphics {
                        g.window.request_redraw();
                    }
                }
                WindowEvent::RedrawRequested => {
                    let now = Instant::now();
                    if !self.paused && !self.occluded {
                        self.elapsed = self.elapsed.saturating_add(
                            now.duration_since(self.tick)
                                .as_millis()
                                .min(u64::MAX as u128) as u64,
                        );
                    }
                    self.tick = now;
                    self.playback.advance_to(self.elapsed)?;
                    if let Some(g) = &mut self.graphics {
                        let status = if self.playback.finished() {
                            "FINISHED"
                        } else if self.paused {
                            "PAUSED"
                        } else {
                            "PLAYING"
                        };
                        g.window.set_title(&format!("Rust Paint Lab | {} | {} | orange preview / blue committed | 1/2/3 select, R replay, Space pause, Esc close", self.scenario.name(), status));
                        let presented = g.draw(&self.playback.scene)?;
                        self.needs_redraw = !presented;
                        if presented {
                            self.frames += 1;
                        }
                        if self.exit_after_demo && self.playback.finished() && presented {
                            eprintln!(
                                "WINDOW_SMOKE: rendered {} frames and completed replay",
                                self.frames
                            );
                            event_loop.exit();
                        }
                    }
                }
                _ => {} // Deliberately no live pointer/mouse painting.
            }
            Ok(())
        })();
        if let Err(e) = result {
            self.fail(event_loop, e);
        }
    }
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if !self.occluded
            && self.graphics.as_ref().is_some_and(|g| !g.size.contains(&0))
            && (self.needs_redraw || (!self.paused && !self.playback.finished()))
        {
            event_loop.set_control_flow(ControlFlow::WaitUntil(
                Instant::now() + Duration::from_millis(16),
            ));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
    }
}
fn main() -> Result<(), Failure> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a != "--exit-after-demo") {
        return Err("usage: canvas-demo [--exit-after-demo]".into());
    }
    let event_loop = EventLoop::new()?;
    let mut app = App {
        graphics: None,
        playback: Playback::new(Scenario::Completed)?,
        scenario: Scenario::Completed,
        tick: Instant::now(),
        elapsed: 0,
        paused: false,
        occluded: false,
        failure: None,
        exit_after_demo: !args.is_empty(),
        frames: 0,
        needs_redraw: true,
    };
    event_loop.run_app(&mut app)?;
    if let Some(error) = app.failure {
        return Err(error.into());
    }
    Ok(())
}
