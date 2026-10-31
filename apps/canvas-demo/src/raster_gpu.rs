//! Dirty-tile upload and color-explicit display; authoritative pixels stay on CPU.
use crate::{
    gpu::{Failure, Renderer},
    scene::{BACKGROUND, CanvasMapping},
};
use paint_document::{Engine, TILE};
use wgpu::util::DeviceExt;
pub struct RasterGpu {
    texture: wgpu::Texture,
    bind: wgpu::BindGroup,
    pipeline: wgpu::RenderPipeline,
    size: [u32; 2],
}
impl RasterGpu {
    pub fn new(g: &Renderer, format: wgpu::TextureFormat, size: [u32; 2]) -> Self {
        let texture = g.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("document composite"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let layout = g
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let sampler = g.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let view = texture.create_view(&Default::default());
        let bind = g.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = g.device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sRGB raster display"),
            source: wgpu::ShaderSource::Wgsl(include_str!("raster.wgsl").into()),
        });
        let pl = g
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[&layout],
                push_constant_ranges: &[],
            });
        let attrs = wgpu::vertex_attr_array![0=>Float32x2,1=>Float32x2];
        let pipeline = g
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: None,
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: 16,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attrs,
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
        Self {
            texture,
            bind,
            pipeline,
            size,
        }
    }
    pub fn size(&self) -> [u32; 2] {
        self.size
    }
    pub fn draw(
        &self,
        g: &Renderer,
        view: &wgpu::TextureView,
        engine: &mut Engine,
        mapping: CanvasMapping,
    ) -> Result<(), Failure> {
        for key in engine.take_damage() {
            let bytes = engine.composite_tile(key);
            let w = TILE.min(self.size[0] - key.0 * TILE);
            let h = TILE.min(self.size[1] - key.1 * TILE);
            g.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d {
                        x: key.0 * TILE,
                        y: key.1 * TILE,
                        z: 0,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &bytes,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(TILE * 4),
                    rows_per_image: Some(TILE),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }
        let [w, h] = mapping.document_size();
        let points = [[0.0, 0.0], [w, 0.0], [w, h], [0.0, h]];
        let uv = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
        let mut bytes = Vec::with_capacity(96);
        for i in [0, 1, 2, 0, 2, 3] {
            for v in mapping.to_clip(points[i])?.into_iter().chain(uv[i]) {
                bytes.extend_from_slice(&v.to_ne_bytes());
            }
        }
        let buffer = g
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: &bytes,
                usage: wgpu::BufferUsages::VERTEX,
            });
        let mut encoder = g.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("raster canvas"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: BACKGROUND[0],
                            g: BACKGROUND[1],
                            b: BACKGROUND[2],
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));
            let [x, y, w, h] = mapping.scissor();
            if w > 0 && h > 0 {
                pass.set_scissor_rect(x, y, w, h);
                pass.draw(0..6, 0..1);
            }
        }
        g.queue.submit([encoder.finish()]);
        g.check_error()
    }
}
