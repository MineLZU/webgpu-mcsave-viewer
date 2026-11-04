//! Draws one Section through the browser path on a native adapter and reads the frame back.
//!
//! The same `TerrainRenderer` the wasm module uses: engine registry and atlas baked from an asset pack,
//! the browser terrain shader and graph, a `WorldState` fed protocol messages. Only the wgpu backend
//! differs. Uses the vanilla models and textures under `resources/assets/minecraft`.

// See `src/lib.rs`: proving the engine's `Gpu` is `Send` walks wgpu-core's whole hub.
#![recursion_limit = "512"]

use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use viewer_protocol::*;
use viewer_web::assets::AssetPack;
use viewer_web::dictionary::Dictionary;
use viewer_web::render::{Camera, TerrainRenderer};
use viewer_world_state::WorldState;
use wgpu_mc::wgpu;

const SIZE: u32 = 256;
const SKY: [u8; 3] = [120, 166, 255];

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};
    struct Thread(std::thread::Thread);
    impl Wake for Thread {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(Thread(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park(),
        }
    }
}

fn gpu() -> Option<Arc<wgpu_mc::Gpu>> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter =
        block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).ok()?;
    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("viewer-web render test"),
        required_limits: wgpu::Limits::downlevel_defaults(),
        ..Default::default()
    }))
    .ok()?;
    Some(Arc::new(wgpu_mc::Gpu {
        instance,
        adapter,
        surface: parking_lot::Mutex::new(None),
        device,
        queue,
        pipeline_cache: None,
    }))
}

/// The files of `resources/assets/minecraft` the browser pack carries, zipped as the gateway serves them.
fn asset_pack() -> AssetPack {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources/assets/minecraft");
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for directory in [
        "blockstates",
        "models/block",
        "textures/block",
        "textures/colormap",
    ] {
        for entry in std::fs::read_dir(root.join(directory)).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() {
                let name = format!(
                    "assets/minecraft/{directory}/{}",
                    path.file_name().unwrap().to_string_lossy()
                );
                writer.start_file(name, options).unwrap();
                writer.write_all(&std::fs::read(&path).unwrap()).unwrap();
            }
        }
    }
    AssetPack::unpack(&writer.finish().unwrap().into_inner()).unwrap()
}

const DICTIONARY: &str = r#"{"format":1,"game_version":"test",
  "block_states":[["minecraft:air",{}],["minecraft:stone",{}],["minecraft:dirt",{}],
    ["minecraft:grass_block",{"snowy":"false"}],["minecraft:grass_block",{"snowy":"true"}]],
  "blocks":{"minecraft:air":0,"minecraft:stone":1,"minecraft:dirt":2,"minecraft:grass_block":3},
  "biomes":[{"id":"minecraft:plains","temperature":0.8,"downfall":0.4}]}"#;

/// Stone below y = 4, dirt to y = 6, grass at y = 7, air above: one Section at the origin.
fn world() -> WorldState {
    let mut world = WorldState::default();
    world
        .apply(&Message::Hello(Hello {
            protocol_version: PROTOCOL_VERSION,
            session_id: 1,
            game_version: "test".into(),
            dictionary_hash: [0; 32],
            assets_hash: [0; 32],
            block_state_count: 5,
            biome_count: 1,
            dimensions: vec![Dimension {
                id: "minecraft:overworld".into(),
                min_y: -64,
                height: 384,
            }],
        }))
        .unwrap();
    let cells: Vec<u32> = (0..SECTION_CELLS)
        .map(|cell| match cell / 256 {
            0..=3 => 1,
            4..=6 => 2,
            7 => 3,
            _ => 0,
        })
        .collect();
    world
        .apply(&Message::SectionSnapshot(SectionSnapshot {
            pos: SectionPos {
                dimension: 0,
                x: 0,
                y: 0,
                z: 0,
            },
            revision: 1,
            light_revision: 1,
            blocks: Paletted::from_cells(&cells),
            biomes: Paletted::Single(0),
            sky_light: LightLayer::Data(Box::new([0xff; 2048])),
            block_light: LightLayer::Unknown,
        }))
        .unwrap();
    world
}

#[test]
fn a_section_from_protocol_messages_draws_through_the_browser_path() {
    let Some(gpu) = gpu() else {
        eprintln!("no wgpu adapter on this machine; skipping the render test");
        return;
    };
    let dictionary = Dictionary::parse(DICTIONARY.as_bytes()).unwrap();
    let mut terrain = TerrainRenderer::new(
        gpu.clone(),
        Arc::new(asset_pack()),
        &dictionary,
        (SIZE, SIZE),
        "rgba8unorm",
        2,
    )
    .unwrap();
    assert_eq!(terrain.table.unmodelled, 1, "only air has no model");

    let mut world = world();
    terrain.mark_dirty(world.take_dirty());
    // Above the Section's corner, looking down across it.
    let camera = Camera {
        position: glam::DVec3::new(-6.0, 16.0, -6.0),
        yaw: -2.36,
        pitch: -0.6,
        fov_y: 70f32.to_radians(),
    };
    while terrain.bake(&world, &camera, 64) > 0 {}

    let target = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("render test target"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("render test readback"),
        size: (SIZE * SIZE * 4) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    // Two frames: the first uploads the baked Sections, the second draws them from the arena.
    for _ in 0..2 {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        terrain.render(&mut encoder, &view, (SIZE, SIZE), &camera);
        gpu.queue.submit([encoder.finish()]);
    }
    let mut encoder = gpu
        .device
        .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIZE * 4),
                rows_per_image: Some(SIZE),
            },
        },
        target.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, |result| result.unwrap());
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    let pixels = readback.slice(..).get_mapped_range().unwrap().to_vec();
    // Kept for inspection: `target/viewer-web-render.png`.
    let frame_path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/viewer-web-render.png");
    image::save_buffer(
        &frame_path,
        &pixels,
        SIZE,
        SIZE,
        image::ExtendedColorType::Rgba8,
    )
    .unwrap();

    let (rgba, _) = pixels.as_chunks::<4>();
    let is_sky = |pixel: &[u8; 4]| {
        pixel[..3]
            .iter()
            .zip(SKY)
            .all(|(channel, sky)| channel.abs_diff(sky) <= 2)
    };
    let terrain_pixels = rgba.iter().filter(|pixel| !is_sky(pixel)).count();
    let green = rgba
        .iter()
        .filter(|pixel| {
            let [r, g, b] = [pixel[0], pixel[1], pixel[2]].map(i16::from);
            g > r + 15 && g > b + 15
        })
        .count();
    eprintln!(
        "{terrain_pixels} of {} pixels are terrain, {green} green",
        SIZE * SIZE
    );
    assert!(
        terrain_pixels > (SIZE * SIZE / 10) as usize,
        "the Section covers part of the frame"
    );
    assert!(
        green > 100,
        "the grass top is tinted by the plains colormap"
    );
}
