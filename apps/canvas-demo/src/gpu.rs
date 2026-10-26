//! Same pipeline for the window and offscreen validation. Fixed bounded vertex buffer.
use crate::scene::{BACKGROUND, MAX_VERTICES, Scene};
use std::sync::{Arc, Mutex};
pub type Failure = Box<dyn std::error::Error + Send + Sync>;

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    vertices: wgpu::Buffer,
    error: Arc<Mutex<Option<String>>>,
}
impl Renderer {
    pub async fn new(
        adapter: &wgpu::Adapter,
        format: wgpu::TextureFormat,
    ) -> Result<Self, Failure> {
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("replay canvas"),
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits()),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
                ..Default::default()
            })
            .await?;
        let error = Arc::new(Mutex::new(None));
        let errors = error.clone();
        device.on_uncaptured_error(Arc::new(move |e| {
            if let Ok(mut slot) = errors.lock()
                && slot.is_none()
            {
                *slot = Some(format!("GPU error: {e}"));
            }
        }));
        let errors = error.clone();
        device.set_device_lost_callback(move |reason, message| {
            if let Ok(mut slot) = errors.lock()
                && slot.is_none()
            {
                *slot = Some(format!("GPU device lost ({reason:?}): {message}"));
            }
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fixed width strokes"),
            source: wgpu::ShaderSource::Wgsl(include_str!("strokes.wgsl").into()),
        });
        let attributes = wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4];
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("canvas triangles"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 24,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &attributes,
                }],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let vertices = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bounded stroke vertices"),
            size: (MAX_VERTICES * 24) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let renderer = Self {
            device,
            queue,
            pipeline,
            vertices,
            error,
        };
        renderer.check_error()?;
        Ok(renderer)
    }
    pub fn check_error(&self) -> Result<(), Failure> {
        if let Some(e) = self
            .error
            .lock()
            .map_err(|_| "GPU error lock poisoned")?
            .as_ref()
        {
            return Err(e.clone().into());
        }
        Ok(())
    }
    /// Clear a surface without drawing the document artboard. The studio uses
    /// this while Home is active so unused viewport space inherits the current
    /// application theme instead of revealing the canvas surround.
    pub fn clear(&self, view: &wgpu::TextureView, color: [f32; 4]) -> Result<(), Failure> {
        self.check_error()?;
        if color
            .iter()
            .any(|component| !component.is_finite() || !(0.0..=1.0).contains(component))
        {
            return Err("invalid GPU clear color".into());
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("clear studio surface"),
            });
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("clear studio surface"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: f64::from(color[0]),
                            g: f64::from(color[1]),
                            b: f64::from(color[2]),
                            a: f64::from(color[3]),
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
        }
        self.queue.submit([encoder.finish()]);
        self.check_error()
    }
    pub fn draw(
        &self,
        view: &wgpu::TextureView,
        scene: &Scene,
        size: [u32; 2],
    ) -> Result<(), Failure> {
        self.check_error()?;
        let mesh = scene.mesh(size)?;
        if mesh.len() > MAX_VERTICES {
            return Err("vertex budget exceeded".into());
        }
        let mut bytes = Vec::with_capacity(mesh.len() * 24);
        for v in &mesh {
            for f in v.position.into_iter().chain(v.color) {
                bytes.extend_from_slice(&f.to_ne_bytes());
            }
        }
        self.queue.write_buffer(&self.vertices, 0, &bytes);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("canvas clear and strokes"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: BACKGROUND[0],
                            g: BACKGROUND[1],
                            b: BACKGROUND[2],
                            a: BACKGROUND[3],
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_vertex_buffer(0, self.vertices.slice(..));
            pass.draw(0..6, 0..1); // Paper is independent of stroke clipping.
            let [x, y, width, height] = crate::scene::CanvasMapping::new(size)?.scissor();
            if width > 0 && height > 0 {
                pass.set_scissor_rect(x, y, width, height);
                pass.draw(6..mesh.len() as u32, 0..1);
            }
        }
        self.queue.submit([encoder.finish()]);
        self.check_error()
    }
}
