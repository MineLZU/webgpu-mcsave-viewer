//! Terrain drawing through `wgpu-mc`: its block registry, atlas, Section mesher and render graph, fed from
//! the browser's [`WorldState`] and the gateway's asset pack instead of the JVM.
//!
//! What this side supplies in place of the game: a [`ResourceProvider`] over the asset pack, the browser
//! terrain shader and graph (no immediates), the state-ID → `BlockstateKey` table, face flags from model
//! cull bits, a [`BlockStateProvider`] over a 3×3×3 Section window, and the camera matrices.

use crate::assets::AssetPack;
use crate::dictionary::Dictionary;
use std::sync::Arc;
use wgpu_mc::WmRenderer;
use wgpu_mc::mc::block::{BlockstateKey, ChunkBlockState, FaceFlags};
use wgpu_mc::mc::resource::{ResourcePath, ResourceProvider};
use wgpu_mc::minecraft_assets::schemas::blockstates::multipart::StateValue;
use wgpu_mc::render::pipeline::BLOCK_ATLAS;

/// The bit a model's cull mask uses for each face, in `wgpu-mc`'s `Direction` order (west, east, down,
/// up, north, south), moved to the bit `FaceFlags::occlusion` reads it from: Java's `Direction.ordinal()`
/// (down, up, north, south, west, east). The engine's own table is private.
const JAVA_BIT_OF: [u8; 6] = [4, 5, 0, 1, 2, 3];

fn cull_to_java_mask(cull: u8) -> u8 {
    (0..6)
        .filter(|bit| cull >> bit & 1 == 1)
        .fold(0, |mask, bit| mask | 1 << JAVA_BIT_OF[bit as usize])
}

/// Which colour a block's tinted faces take. Vanilla's `BlockColors`, reduced to the cases terrain needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tint {
    None,
    Grass,
    Foliage,
    Water,
    /// A block-specific constant, ARGB with red in the low byte like every engine colour.
    Fixed(u32),
}

/// `0xRRGGBB` as the engine packs a colour: red in the low byte, opaque alpha on top.
pub const fn rgb(hex: u32) -> u32 {
    0xff00_0000 | (hex >> 16 & 0xff) | (hex & 0xff00) | (hex & 0xff) << 16
}

fn tint_of(block: &str) -> Tint {
    match block.strip_prefix("minecraft:").unwrap_or(block) {
        "grass_block" | "short_grass" | "tall_grass" | "fern" | "large_fern" | "potted_fern"
        | "sugar_cane" => Tint::Grass,
        "spruce_leaves" => Tint::Fixed(rgb(0x619961)),
        "birch_leaves" => Tint::Fixed(rgb(0x80a755)),
        "mangrove_leaves" => Tint::Fixed(rgb(0x92c648)),
        "lily_pad" => Tint::Fixed(rgb(0x208030)),
        "water" | "bubble_column" | "water_cauldron" => Tint::Water,
        name if name.ends_with("_leaves")
            && name != "cherry_leaves"
            && name != "azalea_leaves"
            && name != "flowering_azalea_leaves" =>
        {
            Tint::Foliage
        }
        "vine" => Tint::Foliage,
        _ => Tint::None,
    }
}

/// What each dictionary state ID draws as, indexed by state ID.
pub struct StateTable {
    /// Air, or the baked `BlockstateKey`; a state whose model failed to bake is drawn as air.
    pub states: Vec<ChunkBlockState>,
    pub tints: Vec<Tint>,
    /// States drawn with no model at all: every air state, and states whose model failed to bake.
    pub unmodelled: usize,
}

const AIR_BLOCKS: [&str; 3] = ["minecraft:air", "minecraft:cave_air", "minecraft:void_air"];

/// Bakes every block of `dictionary` into the engine's registry and atlas, then resolves every state to
/// its key and records each key's face flags from its model's cull bits.
pub fn build_state_table(wm: &WmRenderer, dictionary: &Dictionary) -> StateTable {
    let mut blocks: Vec<&str> = dictionary
        .block_states
        .iter()
        .map(|state| state.block.as_str())
        .collect();
    blocks.sort_unstable();
    blocks.dedup();
    let paths: Vec<(String, ResourcePath)> = blocks
        .iter()
        .filter(|block| !AIR_BLOCKS.contains(block))
        .map(|block| {
            let (namespace, name) = block.split_once(':').unwrap_or(("minecraft", block));
            (
                block.to_string(),
                ResourcePath(format!("{namespace}:blockstates/{name}.json")),
            )
        })
        .collect();
    wm.mc
        .bake_blocks(wm, paths.iter().map(|(name, path)| (name.as_str(), path)));

    let manager = wm.mc.block_manager.read();
    let atlases = wm.mc.texture_manager.atlases.read();
    let atlas = atlases
        .get(BLOCK_ATLAS)
        .expect("WmRenderer::init creates the block atlas");
    let mut flags: Vec<(u32, FaceFlags)> = Vec::new();
    let mut unmodelled = 0;
    let states = dictionary
        .block_states
        .iter()
        .map(|state| {
            let resolved = (!AIR_BLOCKS.contains(&state.block.as_str()))
                .then(|| manager.blocks.get_full(&state.block))
                .flatten()
                .and_then(|(index, _, block)| {
                    let values: Vec<(String, StateValue)> = state
                        .properties
                        .iter()
                        .map(|(name, value)| (name.clone(), StateValue::String(value.clone())))
                        .collect();
                    let key = values.iter().map(|(name, value)| (name.as_str(), value));
                    block
                        .get_model_by_key(key, &*wm.mc.resource_provider, atlas, 0)
                        .map(|(mesh, augment)| (index, mesh, augment))
                });
            match resolved {
                Some((index, mesh, augment)) => {
                    let key = BlockstateKey {
                        block: index as u16,
                        augment,
                    };
                    let packed = ((index as u32) << 16) | augment as u32;
                    flags.push((
                        packed,
                        FaceFlags {
                            occlusion: cull_to_java_mask(mesh.cull),
                            shades: true,
                            blocks_motion: true,
                            ..FaceFlags::default()
                        },
                    ));
                    ChunkBlockState::State(key)
                }
                None => {
                    unmodelled += 1;
                    ChunkBlockState::Air
                }
            }
        })
        .collect();
    drop((manager, atlases));
    let mut manager = wm.mc.block_manager.write();
    flags.into_iter().for_each(|(key, face_flags)| {
        manager.face_flags.insert(key, face_flags);
    });
    let tints = dictionary
        .block_states
        .iter()
        .map(|state| tint_of(&state.block))
        .collect();
    StateTable {
        states,
        tints,
        unmodelled,
    }
}

const TERRAIN_TEMPLATE: &str = include_str!("../shaders/terrain.wgsl");

/// The three terrain passes, each the browser shader with its own alpha cutoff, and the pass state of
/// each: solid opaque, cutout opaque, then translucent blended without depth writes.
const TERRAIN_PASSES: [(&str, &str, &str, bool, bool); 3] = [
    ("terrain_solid", "-1.0", "replace", true, true),
    ("terrain", "0.5", "replace", true, false),
    (
        "translucent_terrain",
        "0.01",
        "premultiplied_alpha_blending",
        false,
        false,
    ),
];

/// The render graph as `graph.yaml` text: every pass draws `@geo_terrain` into the canvas with the scene's
/// depth texture. YAML rather than JSON because bind-group slots are integer map keys inside an untagged
/// enum, which serde_json cannot read from its string keys; the engine's own config test reads YAML too.
pub fn graph_config(output_format: &str) -> String {
    let mut yaml = String::from("version: \"0.0.1\"\nsupport: wgsl\nresources: {}\npipelines:\n");
    for (name, _, blending, depth_write, clear) in TERRAIN_PASSES {
        yaml.push_str(&format!(
            "  {name}:\n    geometry: \"@geo_terrain\"\n    shader: \"browser_{name}\"\n    depth: \"@texture_depth\"\n    depth_write: {depth_write}\n    clear: {clear}\n    output: [\"@framebuffer_texture\"]\n    output_format: {output_format}\n    blending: {blending}\n    bind_groups:\n      0:\n        0: \"@viewer_view_projection\"\n        1: \"@texture_block_atlas\"\n        2: \"@sampler\"\n      1: \"@bg_ssbo_chunks\"\n      2: \"@bg_section_draws\"\n"
        ));
    }
    yaml
}

/// The asset pack for `minecraft:` paths, and the browser shaders for `wgpu_mc:shaders/browser_*`.
pub struct ContentResources {
    pub assets: Arc<AssetPack>,
}

impl ResourceProvider for ContentResources {
    fn get_bytes(&self, id: &ResourcePath) -> Option<Vec<u8>> {
        let (namespace, path) = id.0.split_once(':')?;
        if namespace == "wgpu_mc" {
            let pass = path
                .strip_prefix("shaders/browser_")?
                .strip_suffix(".wgsl")?;
            let (_, cutoff, ..) = TERRAIN_PASSES.iter().find(|(name, ..)| *name == pass)?;
            return Some(
                TERRAIN_TEMPLATE
                    .replace("{{ALPHA_CUTOFF}}", cutoff)
                    .into_bytes(),
            );
        }
        self.assets
            .get(&format!("assets/{namespace}/{path}"))
            .map(<[u8]>::to_vec)
    }
}

/// Grass, foliage and water colour of every dictionary biome, indexed by biome ID.
pub struct BiomeColors {
    pub grass: Vec<u32>,
    pub foliage: Vec<u32>,
    pub water: Vec<u32>,
}

/// Vanilla's colormap lookup: temperature and downfall clamped to 0..1, downfall scaled by temperature.
fn colormap_sample(
    colormap: Option<&image::RgbaImage>,
    temperature: f64,
    downfall: f64,
    fallback: u32,
) -> u32 {
    let Some(colormap) = colormap else {
        return fallback;
    };
    let temperature = temperature.clamp(0.0, 1.0);
    let downfall = downfall.clamp(0.0, 1.0) * temperature;
    let x = ((1.0 - temperature) * 255.0) as u32;
    let y = ((1.0 - downfall) * 255.0) as u32;
    match colormap.get_pixel_checked(x, y) {
        Some(pixel) => {
            0xff00_0000 | pixel[0] as u32 | (pixel[1] as u32) << 8 | (pixel[2] as u32) << 16
        }
        None => fallback,
    }
}

impl BiomeColors {
    /// Explicit `grass_color`/`foliage_color`/`water_color` of a biome win over the colormap, as in vanilla.
    /// The `swamp` and `dark_forest` grass modifiers are not applied.
    pub fn new(dictionary: &Dictionary, assets: &AssetPack) -> Self {
        let colormap = |name: &str| {
            let bytes = assets.get(&format!("assets/minecraft/textures/colormap/{name}.png"))?;
            image::load_from_memory_with_format(bytes, image::ImageFormat::Png)
                .ok()
                .map(|image| image.to_rgba8())
        };
        let (grass_map, foliage_map) = (colormap("grass"), colormap("foliage"));
        let field = |fields: &serde_json::Value, name: &str| {
            fields
                .get(name)
                .or_else(|| fields.get("effects").and_then(|effects| effects.get(name)))
                .and_then(|value| value.as_u64())
                .map(|hex| rgb(hex as u32))
        };
        let mut colors = Self {
            grass: Vec::new(),
            foliage: Vec::new(),
            water: Vec::new(),
        };
        for biome in &dictionary.biomes {
            let temperature = biome.fields["temperature"].as_f64().unwrap_or(0.8);
            let downfall = biome.fields["downfall"].as_f64().unwrap_or(0.4);
            colors
                .grass
                .push(field(&biome.fields, "grass_color").unwrap_or_else(|| {
                    colormap_sample(grass_map.as_ref(), temperature, downfall, rgb(0x91bd59))
                }));
            colors
                .foliage
                .push(field(&biome.fields, "foliage_color").unwrap_or_else(|| {
                    colormap_sample(foliage_map.as_ref(), temperature, downfall, rgb(0x77ab2f))
                }));
            colors
                .water
                .push(field(&biome.fields, "water_color").unwrap_or(rgb(0x3f76e4)));
        }
        colors
    }
}

/// The 3×3×3 Sections around the one being baked, read from [`viewer_world_state::WorldState`].
///
/// A missing neighbour is drawn as air and fully sky-lit, so its faces toward the baked Section are kept;
/// the state store marks this Section dirty again when the neighbour arrives, which rebakes it.
pub struct SectionWindow<'a> {
    pub origin: glam::IVec3,
    pub sections: [Option<&'a viewer_world_state::SectionState>; 27],
    pub table: &'a StateTable,
    pub colors: &'a BiomeColors,
}

impl SectionWindow<'_> {
    /// The Section holding section-local `pos` (each axis `-16..32`) and the cell inside it.
    fn locate(&self, pos: glam::IVec3) -> Option<(&viewer_world_state::SectionState, usize)> {
        let section = pos.div_euclid(glam::IVec3::splat(16));
        let local = pos.rem_euclid(glam::IVec3::splat(16));
        if section.abs().max_element() > 1 {
            return None;
        }
        let slot = (section.x + 1) + (section.y + 1) * 3 + (section.z + 1) * 9;
        let state = self.sections[slot as usize]?;
        Some((
            state,
            viewer_protocol::cell_index(local.x as usize, local.y as usize, local.z as usize),
        ))
    }

    fn state_id(&self, pos: glam::IVec3) -> Option<u32> {
        self.locate(pos).map(|(section, cell)| section.blocks[cell])
    }
}

fn nibble(layer: &viewer_protocol::LightLayer, cell: usize, unknown: u8) -> u8 {
    match layer {
        viewer_protocol::LightLayer::Unknown => unknown,
        viewer_protocol::LightLayer::Data(nibbles) => nibbles[cell / 2] >> (cell % 2 * 4) & 0xf,
    }
}

impl wgpu_mc::mc::chunk::BlockStateProvider for SectionWindow<'_> {
    fn get_state(&self, pos: glam::IVec3) -> ChunkBlockState {
        self.state_id(pos)
            .and_then(|id| self.table.states.get(id as usize).copied())
            .unwrap_or(ChunkBlockState::Air)
    }

    fn get_light_level(&self, pos: glam::IVec3) -> wgpu_mc::mc::chunk::LightLevel {
        let (sky, block) = match self.locate(pos) {
            Some((section, cell)) => (
                nibble(&section.sky_light, cell, 15),
                nibble(&section.block_light, cell, 0),
            ),
            None => (15, 0),
        };
        wgpu_mc::mc::chunk::LightLevel::from_sky_and_block(sky, block)
    }

    fn is_section_empty(&self, rel_pos: glam::IVec3) -> bool {
        match self.locate(rel_pos * 16) {
            None => true,
            Some((section, _)) => section.blocks.iter().all(|id| {
                matches!(
                    self.table.states.get(*id as usize),
                    None | Some(ChunkBlockState::Air)
                )
            }),
        }
    }

    fn get_block_color(&self, pos: glam::IVec3, tint_index: i32) -> u32 {
        let local = pos - self.origin;
        let Some((section, cell)) = self.locate(local) else {
            return 0xffff_ffff;
        };
        let id = section.blocks[cell] as usize;
        let biome_at = |colors: &[u32]| {
            let quarter = local.rem_euclid(glam::IVec3::splat(16)) / 4;
            let biome =
                section.biomes[((quarter.y * 4 + quarter.z) * 4 + quarter.x) as usize] as usize;
            colors.get(biome).copied().unwrap_or(0xffff_ffff)
        };
        match (
            tint_index,
            self.table.tints.get(id).copied().unwrap_or(Tint::None),
        ) {
            (-1, _) | (_, Tint::None) => 0xffff_ffff,
            (_, Tint::Grass) => biome_at(&self.colors.grass),
            (_, Tint::Foliage) => biome_at(&self.colors.foliage),
            (_, Tint::Water) => biome_at(&self.colors.water),
            (_, Tint::Fixed(color)) => color,
        }
    }
}

/// The engine, its graph, and what this side keeps to drive them: the camera matrix buffer and the
/// Sections waiting to be baked.
pub struct TerrainRenderer {
    pub wm: Arc<WmRenderer>,
    graph: wgpu_mc::render::graph::RenderGraph,
    view_projection: Arc<wgpu_mc::wgpu::Buffer>,
    pub table: StateTable,
    colors: BiomeColors,
    pending: std::collections::HashSet<viewer_protocol::SectionPos>,
}

/// The camera: position in blocks, yaw and pitch in radians, vertical field of view in radians.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub position: glam::DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub fov_y: f32,
}

impl Camera {
    fn forward(&self) -> glam::Vec3 {
        glam::Vec3::new(
            -self.yaw.sin() * self.pitch.cos(),
            self.pitch.sin(),
            -self.yaw.cos() * self.pitch.cos(),
        )
    }
}

impl TerrainRenderer {
    /// Builds the engine on `gpu`, bakes `dictionary`'s blocks from `assets`, and builds the graph for a
    /// `width × height` target of `output_format` (`bgra8unorm` or `rgba8unorm`). `radius` is in chunks.
    pub fn new(
        gpu: Arc<wgpu_mc::Gpu>,
        assets: Arc<AssetPack>,
        dictionary: &Dictionary,
        (width, height): (u32, u32),
        output_format: &str,
        radius: i32,
    ) -> Result<Self, String> {
        let resources: Arc<dyn ResourceProvider> = Arc::new(ContentResources {
            assets: assets.clone(),
        });
        let wm = Arc::new(WmRenderer::new(gpu, resources));
        wm.init();
        let table = build_state_table(&wm, dictionary);
        let colors = BiomeColors::new(dictionary, &assets);
        let scene = wm
            .ensure_scene(wgpu_mc::wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            })
            .ok_or("the scene could not be created")?;
        scene.section_storage.write().set_width(radius + 1);
        let view_projection = Arc::new(wm.gpu.device.create_buffer(
            &wgpu_mc::wgpu::BufferDescriptor {
                label: Some("viewer view-projection"),
                size: 64,
                usage: wgpu_mc::wgpu::BufferUsages::UNIFORM | wgpu_mc::wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            },
        ));
        let config: wgpu_mc::render::shaderpack::ShaderPackConfig =
            serde_norway::from_str(&graph_config(output_format))
                .map_err(|error| format!("render graph config: {error}"))?;
        let mut resources = std::collections::HashMap::new();
        resources.insert(
            "@viewer_view_projection".to_owned(),
            wgpu_mc::render::graph::ResourceBacking::Buffer(
                view_projection.clone(),
                wgpu_mc::wgpu::BufferBindingType::Uniform,
            ),
        );
        // With back-face culling on, this right-handed camera loses exactly the faces that point at it:
        // the engine's baked quads come out wound against `FrontFace::Ccw` here. Culling stays off and
        // the depth test picks the visible face; see the known deviations in `docs/viewer-webgpu.md`.
        wgpu_mc::render::graph::set_pipeline_diagnostics(true, false);
        let graph = wgpu_mc::render::graph::RenderGraph::new(&wm, config, resources, None, None);
        Ok(Self {
            wm,
            graph,
            view_projection,
            table,
            colors,
            pending: Default::default(),
        })
    }
}

/// The Section `camera` stands in.
pub fn camera_section(camera: &Camera) -> glam::IVec3 {
    (camera.position / 16.0).floor().as_ivec3()
}

impl TerrainRenderer {
    /// Queues Sections for baking; only dimension 0 is drawn.
    pub fn mark_dirty(&mut self, positions: impl IntoIterator<Item = viewer_protocol::SectionPos>) {
        self.pending
            .extend(positions.into_iter().filter(|pos| pos.dimension == 0));
    }

    pub fn pending(&self) -> usize {
        self.pending.len()
    }

    /// Bakes up to `budget` queued Sections, nearest to `camera` first, from a 3×3×3 window of `world`.
    /// A Section `world` no longer holds bakes as empty, which removes its old mesh.
    pub fn bake(
        &mut self,
        world: &viewer_world_state::WorldState,
        camera: &Camera,
        budget: usize,
    ) -> usize {
        let centre = camera_section(camera);
        let mut order: Vec<viewer_protocol::SectionPos> = self.pending.iter().copied().collect();
        order.sort_unstable_by_key(|pos| {
            (glam::IVec3::new(pos.x, pos.y, pos.z) - centre)
                .abs()
                .max_element()
        });
        order.truncate(budget);
        for pos in &order {
            self.pending.remove(pos);
            let window = SectionWindow {
                origin: glam::IVec3::new(pos.x, pos.y, pos.z) * 16,
                sections: std::array::from_fn(|slot| {
                    let slot = slot as i32;
                    world.section(&viewer_protocol::SectionPos {
                        dimension: 0,
                        x: pos.x + slot % 3 - 1,
                        y: pos.y + slot / 3 % 3 - 1,
                        z: pos.z + slot / 9 - 1,
                    })
                }),
                table: &self.table,
                colors: &self.colors,
            };
            wgpu_mc::mc::chunk::bake_section(
                glam::IVec3::new(pos.x, pos.y, pos.z),
                &self.wm,
                &window,
            );
        }
        order.len()
    }

    pub fn resize(&self, width: u32, height: u32) {
        let extent = wgpu_mc::wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        if let Some(scene) = self.wm.ensure_scene(extent) {
            scene.resize_depth_texture(&self.wm, width, height);
        }
    }

    /// Uploads finished bakes and records the three terrain passes into `encoder`, drawing into `target`.
    pub fn render(
        &self,
        encoder: &mut wgpu_mc::wgpu::CommandEncoder,
        target: &wgpu_mc::wgpu::TextureView,
        size: (u32, u32),
        camera: &Camera,
    ) {
        let extent = wgpu_mc::wgpu::Extent3d {
            width: size.0,
            height: size.1,
            depth_or_array_layers: 1,
        };
        let Some(scene) = self.wm.ensure_scene(extent) else {
            return;
        };
        let section = camera_section(camera);
        *scene.camera_section_pos.write() = section;
        self.wm.tick_scene(scene);
        let eye = (camera.position - section.as_dvec3() * 16.0).as_vec3();
        let view = glam::Mat4::look_to_rh(eye, camera.forward(), glam::Vec3::Y);
        let projection = glam::Mat4::perspective_rh(
            camera.fov_y,
            size.0 as f32 / size.1.max(1) as f32,
            0.1,
            2048.0,
        );
        let view_projection = projection * view;
        let bytes: Vec<u8> = view_projection
            .to_cols_array()
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect();
        self.wm
            .gpu
            .queue
            .write_buffer(&self.view_projection, 0, &bytes);
        wgpu_mc::render::graph::begin_frame();
        self.graph.render_with_mvp(
            &self.wm,
            encoder,
            scene,
            target,
            None,
            [0.47, 0.65, 1.0],
            view_projection.to_cols_array_2d(),
            [0.0; 3],
        );
    }
}
