//! Custom runner so absent adapters are explicitly SKIPPED, not counted as passed
//! Rust unit tests. CI's Linux software-adapter leg requires execution, not a skip.
use canvas_demo::{
    gpu::{Failure, Renderer},
    playback::Playback,
};
use std::{io::Write, time::Duration};

fn report(message: &str) {
    println!("{message}");
    if std::env::var("GITHUB_ACTIONS").as_deref() == Ok("true") {
        let annotation = message
            .replace('%', "%25")
            .replace('\r', "%0D")
            .replace('\n', "%0A");
        println!("::notice title=Offscreen GPU smoke::{annotation}");
    }
    if let Ok(path) = std::env::var("GITHUB_STEP_SUMMARY")
        && let Ok(mut file) = std::fs::OpenOptions::new().append(true).open(path)
    {
        let _ = writeln!(file, "{message}");
    }
}
fn main() -> Result<(), Failure> {
    pollster::block_on(run())
}
async fn run() -> Result<(), Failure> {
    let mut descriptor = wgpu::InstanceDescriptor::from_env_or_default();
    descriptor.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;
    let instance = wgpu::Instance::new(&descriptor);
    let adapter = match instance
        .request_adapter(&wgpu::RequestAdapterOptions::default())
        .await
    {
        Ok(adapter) => adapter,
        Err(error) => {
            if std::env::var("RPL_REQUIRE_GPU").as_deref() == Ok("1") {
                return Err(format!("Required GPU smoke adapter unavailable: {error}").into());
            }
            report(&format!(
                "GPU_SMOKE: SKIPPED (no compatible adapter: {error})"
            ));
            return Ok(());
        }
    };
    println!("GPU smoke adapter: {:?}", adapter.get_info());
    let renderer = Renderer::new(&adapter, wgpu::TextureFormat::Rgba8UnormSrgb).await?;
    let mut completed = Playback::from_fixture(
        "paint-input-replay 1\nsample 0 1 pen down 100 250 0 - - 1 1 1\nsample 1 1 pen move 400 250 0.5 - - 1 1 1\nsample 2 1 pen up 700 250 0 - - 0 0 1\n",
    )?;
    completed.advance_to(1)?;
    let pixels = render_pixels(&renderer, &completed.scene)?;
    let preview = pixel(&pixels, 86, 128);
    assert!(
        preview[0] > preview[1] + 30 && preview[0] > preview[2] + 50,
        "orange preview missing: {preview:?}"
    );
    let paper = pixel(&pixels, 128, 80);
    assert!(
        paper.iter().take(3).map(|x| u32::from(*x)).sum::<u32>() > 600,
        "paper missing"
    );
    let background = pixel(&pixels, 2, 2);
    assert!(
        background
            .iter()
            .take(3)
            .map(|x| u32::from(*x))
            .sum::<u32>()
            < 350,
        "contrasting outer background missing"
    );
    completed.advance_to(2)?;
    let pixels = render_pixels(&renderer, &completed.scene)?;
    let end_segment = pixel(&pixels, 180, 128);
    assert!(
        end_segment[2] > end_segment[0] + 40 && end_segment[2] > end_segment[1] + 30,
        "committed final segment missing: {end_segment:?}"
    );
    let mut cancelled = Playback::from_fixture(
        "paint-input-replay 1\nsample 0 1 pen down 100 250 0.2 - - 1 1 1\nsample 1 1 pen move 700 250 1 - - 1 1 1\ncapture_lost 2 1\n",
    )?;
    cancelled.advance_to(1)?;
    let pixels = render_pixels(&renderer, &cancelled.scene)?;
    let orange = pixel(&pixels, 128, 128);
    assert!(
        u16::from(orange[0]) > u16::from(orange[1]) + 30,
        "cancel preview missing: {orange:?}"
    );
    cancelled.advance_to(2)?;
    let pixels = render_pixels(&renderer, &cancelled.scene)?;
    let cleared = pixel(&pixels, 128, 128);
    assert!(
        cleared[0] > 220 && cleared[1] > 220 && cleared[2] > 220,
        "cancelled stroke left pixels: {cleared:?}"
    );
    // Live adapter -> Processor -> Router -> Scene -> the same rendering pipeline.
    use canvas_demo::interaction::{LiveMouse, MouseEvent as E};
    let mut mouse = LiveMouse::new([256, 256]);
    mouse.handle(E::Moved([44.0, 128.0]), 0);
    mouse.handle(
        E::Button {
            primary: true,
            pressed: true,
        },
        0,
    );
    mouse.handle(E::Moved([212.0, 128.0]), 1);
    let pixels = render_pixels(&renderer, mouse.scene())?;
    let preview = pixel(&pixels, 128, 128);
    assert!(
        u16::from(preview[0]) > u16::from(preview[1]) + 30,
        "live preview missing"
    );
    mouse.handle(E::CaptureLost, 2);
    let pixels = render_pixels(&renderer, mouse.scene())?;
    assert!(
        pixel(&pixels, 128, 128)[2] > 220,
        "live cancellation left residue"
    );
    mouse.handle(E::Moved([44.0, 128.0]), 3);
    mouse.handle(
        E::Button {
            primary: true,
            pressed: true,
        },
        3,
    );
    mouse.handle(E::Moved([212.0, 128.0]), 4);
    mouse.handle(
        E::Button {
            primary: true,
            pressed: false,
        },
        5,
    );
    let pixels = render_pixels(&renderer, mouse.scene())?;
    let blue = pixel(&pixels, 180, 128);
    assert!(
        u16::from(blue[2]) > u16::from(blue[0]) + 40,
        "live commit missing"
    );
    mouse.clear(6);
    mouse.handle(E::Moved([16.0, 128.0]), 7);
    mouse.handle(
        E::Button {
            primary: true,
            pressed: true,
        },
        7,
    );
    mouse.handle(E::Moved([128.0, 128.0]), 8);
    mouse.handle(
        E::Button {
            primary: true,
            pressed: false,
        },
        9,
    );
    let pixels = render_pixels(&renderer, mouse.scene())?;
    let outside = pixel(&pixels, 15, 128);
    assert!(
        outside.iter().take(3).map(|v| u32::from(*v)).sum::<u32>() < 350,
        "stroke cap leaked outside paper"
    );
    renderer.check_error()?;
    // Exercise the new sparse-raster shader and persistent dirty-tile texture.
    use paint_document::Engine;
    use paint_stroke::{
        CancelReason, StrokeCommand as C, StrokeId, StrokeSample, coordinates::DocumentPoint,
    };
    let mut engine = Engine::new(64, 64, 300)?;
    engine.brush.pressure_size = false;
    engine.brush.pressure_opacity = false;
    engine.brush.size = 12.0;
    engine.brush.color = [220, 30, 40];
    let raster = canvas_demo::raster_gpu::RasterGpu::new(
        &renderer,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        [64, 64],
    );
    let sample = StrokeSample {
        pointer_id: 1,
        tool: paint_input::Tool::Pen,
        position_client_px: [32.0, 32.0],
        position_document: DocumentPoint([32.0, 32.0]),
        pressure: Some(1.0),
        tilt_degrees: [None, None],
        buttons: paint_input::Buttons::default(),
        source_time_ms: 0,
        time_ms: 0,
    };
    engine.apply(&C::BeginStroke {
        stroke_id: StrokeId(1),
        sample,
    })?;
    let p = render_raster_pixels(&renderer, &raster, &mut engine)?;
    assert!(
        pixel(&p, 128, 128)[0] > 180 && pixel(&p, 128, 128)[1] < 70,
        "raster preview missing"
    );
    engine.apply(&C::CancelStroke {
        stroke_id: StrokeId(1),
        last: sample,
        time_ms: 1,
        reason: CancelReason::RouterInterrupted,
    })?;
    let p = render_raster_pixels(&renderer, &raster, &mut engine)?;
    assert!(
        pixel(&p, 128, 128)[1] > 240,
        "raster cancellation left pixels"
    );
    engine.apply(&C::BeginStroke {
        stroke_id: StrokeId(2),
        sample,
    })?;
    engine.apply(&C::CommitStroke {
        stroke_id: StrokeId(2),
        sample,
    })?;
    let p = render_raster_pixels(&renderer, &raster, &mut engine)?;
    assert!(pixel(&p, 128, 128)[1] < 70, "raster commit missing");
    engine.undo();
    let p = render_raster_pixels(&renderer, &raster, &mut engine)?;
    assert!(pixel(&p, 128, 128)[1] > 240, "raster undo left pixels");
    engine.redo();
    let p = render_raster_pixels(&renderer, &raster, &mut engine)?;
    assert!(pixel(&p, 128, 128)[1] < 70, "raster redo missing");
    report(
        "GPU_SMOKE: PASSED (replay and live preview/commit/cancel, final segment, canvas clipping/background; raster preview/cancel/commit/undo/redo)",
    );
    Ok(())
}
fn pixel(pixels: &[u8], x: usize, y: usize) -> [u8; 4] {
    pixels[(y * 256 + x) * 4..(y * 256 + x) * 4 + 4]
        .try_into()
        .unwrap()
}
fn render_pixels(
    renderer: &Renderer,
    scene: &canvas_demo::scene::Scene,
) -> Result<Vec<u8>, Failure> {
    let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("smoke target"),
        size: wgpu::Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    renderer.draw(&texture.create_view(&Default::default()), scene, [256, 256])?;
    let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("smoke readback"),
        size: 256 * 256 * 4,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = renderer.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1024),
                rows_per_image: Some(256),
            },
        },
        wgpu::Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        },
    );
    renderer.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    renderer.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(Duration::from_secs(30)),
    })?;
    rx.recv_timeout(Duration::from_secs(30))??;
    let pixels = buffer.slice(..).get_mapped_range().to_vec();
    buffer.unmap();
    renderer.check_error()?;
    Ok(pixels)
}

fn render_raster_pixels(
    renderer: &Renderer,
    raster: &canvas_demo::raster_gpu::RasterGpu,
    engine: &mut paint_document::Engine,
) -> Result<Vec<u8>, Failure> {
    let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("smoke target"),
        size: wgpu::Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let mapping = canvas_demo::scene::CanvasMapping::for_document(
        [256, 256],
        [64.0, 64.0],
        [0.0, 0.0, 256.0, 256.0],
    )?;
    raster.draw(
        renderer,
        &texture.create_view(&Default::default()),
        engine,
        mapping,
    )?;
    let buffer = renderer.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("smoke readback"),
        size: 256 * 256 * 4,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = renderer.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1024),
                rows_per_image: Some(256),
            },
        },
        wgpu::Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        },
    );
    renderer.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
    renderer.device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: Some(Duration::from_secs(30)),
    })?;
    rx.recv_timeout(Duration::from_secs(30))??;
    let pixels = buffer.slice(..).get_mapped_range().to_vec();
    buffer.unmap();
    renderer.check_error()?;
    Ok(pixels)
}
