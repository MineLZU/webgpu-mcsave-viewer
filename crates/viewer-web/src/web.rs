//! The wasm-bindgen surface. The host owns the WebSocket, downloads and input; this side owns state and the GPU.

use crate::assets::AssetPack;
use crate::dictionary::Dictionary;
use crate::render::{Camera, TerrainRenderer};
use std::sync::Arc;
use viewer_protocol::{ChunkRange, Message, ResyncRequest, Subscribe, decode, encode};
use viewer_world_state::{Applied, WorldState};
use wasm_bindgen::prelude::*;

/// Sections baked per frame. Each bake runs on the page's only thread, so this bounds a frame's stall.
const BAKE_BUDGET: usize = 6;
/// Subscription radius in chunks; the engine keeps one chunk more than that.
const VIEW_RADIUS: i32 = 8;

#[wasm_bindgen]
pub struct Viewer {
    surface: wgpu::Surface<'static>,
    gpu: Arc<wgpu_mc::Gpu>,
    config: wgpu::SurfaceConfiguration,
    world: WorldState,
    terrain: Option<TerrainRenderer>,
    content: Option<(String, usize)>,
    camera: Camera,
    generation: u32,
    outgoing: Vec<Vec<u8>>,
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    JsValue::from_str(&error.to_string())
}

/// The engine's graph names formats by these strings; an sRGB canvas format is drawn as its linear twin.
fn output_format(format: wgpu::TextureFormat) -> (wgpu::TextureFormat, &'static str) {
    match format {
        wgpu::TextureFormat::Rgba8Unorm | wgpu::TextureFormat::Rgba8UnormSrgb => {
            (wgpu::TextureFormat::Rgba8Unorm, "rgba8unorm")
        }
        _ => (wgpu::TextureFormat::Bgra8Unorm, "bgra8unorm"),
    }
}

#[wasm_bindgen]
impl Viewer {
    /// Opens WebGPU on `canvas`. Fails, rather than falling back, when the browser has no WebGPU adapter.
    pub async fn create(canvas: web_sys::HtmlCanvasElement) -> Result<Viewer, JsValue> {
        let (width, height) = (canvas.width().max(1), canvas.height().max(1));
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(js_error)?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(js_error)?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("viewer"),
                ..Default::default()
            })
            .await
            .map_err(js_error)?;
        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or("canvas surface is not supported")?;
        config.format = output_format(config.format).0;
        surface.configure(&device, &config);
        // The engine never reads `Gpu::surface`: this side presents, so it keeps the surface itself.
        let gpu = Arc::new(wgpu_mc::Gpu {
            instance,
            adapter,
            surface: parking_lot::Mutex::new(None),
            device,
            queue,
            pipeline_cache: None,
        });
        Ok(Viewer {
            surface,
            gpu,
            config,
            world: WorldState::default(),
            terrain: None,
            content: None,
            camera: Camera {
                position: glam::DVec3::new(8.0, 90.0, 8.0),
                yaw: 0.6,
                pitch: -0.35,
                fov_y: 70f32.to_radians(),
            },
            generation: 0,
            outgoing: Vec::new(),
        })
    }
}

#[wasm_bindgen]
impl Viewer {
    /// Applies one WebSocket binary frame. Returns `"session"` when a `Hello` started a session (the host then
    /// downloads [`Viewer::dictionary_hash`] and [`Viewer::assets_hash`]), otherwise `"ok"`; errors close the socket.
    pub fn handle_frame(&mut self, frame: &[u8]) -> Result<String, JsValue> {
        let message = decode(frame).map_err(js_error)?;
        let applied = self.world.apply(&message).map_err(js_error)?;
        if applied == Applied::SessionStarted {
            self.terrain = None;
            self.content = None;
        }
        let dirty = self.world.take_dirty();
        if let Some(terrain) = &mut self.terrain {
            terrain.mark_dirty(dirty);
        }
        for pos in self.world.take_resync() {
            self.outgoing.push(
                encode(&Message::ResyncRequest(ResyncRequest::Section(pos))).map_err(js_error)?,
            );
        }
        Ok(if applied == Applied::SessionStarted {
            "session"
        } else {
            "ok"
        }
        .to_owned())
    }

    pub fn dictionary_hash(&self) -> Option<String> {
        self.world
            .hello()
            .map(|hello| crate::hex(&hello.dictionary_hash))
    }

    pub fn assets_hash(&self) -> Option<String> {
        self.world
            .hello()
            .map(|hello| crate::hex(&hello.assets_hash))
    }

    /// Takes the verified dictionary and asset pack and builds the terrain renderer from them: bakes every
    /// block model into the atlas, then queues every Section already held. Fails when the dictionary
    /// disagrees with the session.
    pub fn load_content(&mut self, dictionary: &[u8], assets: &[u8]) -> Result<(), JsValue> {
        let hello = self.world.hello().ok_or("no session")?;
        let dictionary = Dictionary::parse(dictionary).map_err(js_error)?;
        if dictionary.block_states.len() as u64 != hello.block_state_count as u64
            || dictionary.biomes.len() as u64 != hello.biome_count as u64
        {
            return Err(js_error("dictionary sizes differ from the session's Hello"));
        }
        let assets = Arc::new(AssetPack::unpack(assets).map_err(js_error)?);
        let (_, format_name) = output_format(self.config.format);
        let mut terrain = TerrainRenderer::new(
            self.gpu.clone(),
            assets.clone(),
            &dictionary,
            (self.config.width, self.config.height),
            format_name,
            VIEW_RADIUS,
        )
        .map_err(js_error)?;
        terrain.mark_dirty(self.world.held_sections());
        self.content = Some((
            format!(
                "{} · {} 状态 · {} 资源 · {} 无模型",
                dictionary.game_version,
                dictionary.block_states.len(),
                assets.len(),
                terrain.table.unmodelled
            ),
            assets.len(),
        ));
        self.terrain = Some(terrain);
        Ok(())
    }

    /// Encodes a `Subscribe` for the columns within the view radius of the camera, in dimension 0.
    pub fn subscribe(&mut self) -> Result<Vec<u8>, JsValue> {
        self.generation = self.generation.wrapping_add(1);
        let centre = crate::render::camera_section(&self.camera);
        let range = ChunkRange {
            min_x: centre.x - VIEW_RADIUS,
            min_z: centre.z - VIEW_RADIUS,
            max_x: centre.x + VIEW_RADIUS,
            max_z: centre.z + VIEW_RADIUS,
        };
        encode(&Message::Subscribe(Subscribe {
            generation: self.generation,
            dimension: 0,
            ranges: vec![range],
        }))
        .map_err(js_error)
    }

    /// Messages this side wants sent, such as resync requests; empty when there are none.
    pub fn take_outgoing(&mut self) -> Vec<js_sys::Uint8Array> {
        self.outgoing
            .drain(..)
            .map(|bytes| js_sys::Uint8Array::from(bytes.as_slice()))
            .collect()
    }

    /// Moves the camera by `forward`/`right`/`up` blocks along its own axes and turns it by `yaw`/`pitch`
    /// radians. Returns whether it entered another chunk column, in which case the host re-subscribes.
    pub fn move_camera(&mut self, forward: f64, right: f64, up: f64, yaw: f32, pitch: f32) -> bool {
        let before = crate::render::camera_section(&self.camera);
        self.camera.yaw += yaw;
        self.camera.pitch = (self.camera.pitch + pitch).clamp(-1.55, 1.55);
        let (sin, cos) = (self.camera.yaw as f64).sin_cos();
        self.camera.position += glam::DVec3::new(
            -sin * forward + cos * right,
            up,
            -cos * forward - sin * right,
        );
        let after = crate::render::camera_section(&self.camera);
        before.x != after.x || before.z != after.z
    }

    pub fn status(&self) -> String {
        let content = self
            .content
            .as_ref()
            .map_or("资源未就绪".to_owned(), |(line, _)| line.clone());
        let pending = self.terrain.as_ref().map_or(0, TerrainRenderer::pending);
        let p = self.camera.position;
        format!(
            "{content}\n{} 个 Section，{pending} 待烘焙\n位置 {:.1} {:.1} {:.1}",
            self.world.section_count(),
            p.x,
            p.y,
            p.z
        )
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.gpu.device, &self.config);
        if let Some(terrain) = &self.terrain {
            terrain.resize(self.config.width, self.config.height);
        }
    }

    /// Bakes up to [`BAKE_BUDGET`] Sections and draws one frame. A lost or outdated surface is reconfigured
    /// and the frame skipped.
    pub fn frame(&mut self) {
        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(texture)
            | wgpu::CurrentSurfaceTexture::Suboptimal(texture) => texture,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.surface.configure(&self.gpu.device, &self.config);
                return;
            }
            _ => return,
        };
        let view = texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame"),
            });
        match &mut self.terrain {
            Some(terrain) => {
                terrain.bake(&self.world, &self.camera, BAKE_BUDGET);
                terrain.render(
                    &mut encoder,
                    &view,
                    (self.config.width, self.config.height),
                    &self.camera,
                );
            }
            None => drop(encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sky"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.47,
                            g: 0.65,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            })),
        }
        self.gpu.queue.submit([encoder.finish()]);
        self.gpu.queue.present(texture);
    }
}
