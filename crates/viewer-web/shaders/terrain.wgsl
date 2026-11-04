// Browser terrain shader for wgpu-mc's arena geometry. No immediates: the one value a pass changes, the
// alpha cutoff, is substituted into `ALPHA_CUTOFF` per pipeline by `crates/viewer-web/src/render.rs`.
// Vertex layout: `wgpu-mc/src/render/pipeline.rs` `Vertex::compressed`, four u32 words per vertex.

const ALPHA_CUTOFF: f32 = {{ALPHA_CUTOFF}};

struct SectionDraw { x: i32, y: i32, z: i32, word_base: u32 };

@group(0) @binding(0) var<uniform> view_projection: mat4x4<f32>;
@group(0) @binding(1) var block_atlas: texture_2d<f32>;
@group(0) @binding(2) var block_sampler: sampler;
@group(1) @binding(0) var<storage, read> chunk_data: array<u32>;
@group(2) @binding(0) var<storage, read> section_draws: array<SectionDraw>;

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec3<f32>,
    @location(2) light: vec2<f32>,
    @location(3) shade: f32,
};

// Minecraft's fixed face shading: up 1.0, down 0.5, north/south 0.8, east/west 0.6.
fn face_shade(normal_bits: u32) -> f32 {
    switch normal_bits {
        case 1u: { return 1.0; }
        case 5u: { return 0.5; }
        case 2u, 6u: { return 0.8; }
        default: { return 0.6; }
    }
}

@vertex
fn vert(@builtin(vertex_index) vertex: u32, @builtin(instance_index) record: u32) -> VertexOut {
    let draw = section_draws[record];
    let base = draw.word_base + vertex * 4u;
    let w0 = chunk_data[base];
    let w1 = chunk_data[base + 1u];
    let w2 = chunk_data[base + 2u];
    let w3 = chunk_data[base + 3u];
    let local = vec3<f32>(f32(w0 & 0xffffu), f32(w0 >> 16u), f32(w1 & 0xffffu)) / 2048.0;
    let section = vec3<f32>(f32(draw.x), f32(draw.y), f32(draw.z)) * 16.0;
    var out: VertexOut;
    out.position = view_projection * vec4<f32>(section + local, 1.0);
    out.uv = vec2<f32>(f32(w1 >> 16u), f32(w2 & 0xffffu)) / 2048.0;
    let flags_high = (w2 >> 24u) & 0xffu;
    let occluders = f32((flags_high >> 5u) & 7u);
    out.shade = face_shade((flags_high >> 2u) & 7u) * (1.0 - 0.2 * occluders);
    out.color = vec3<f32>(f32(w3 & 0xffu), f32((w3 >> 8u) & 0xffu), f32((w3 >> 16u) & 0xffu)) / 255.0;
    let light_byte = w3 >> 24u;
    out.light = vec2<f32>(f32(light_byte >> 4u), f32(light_byte & 15u)) / 15.0;
    return out;
}

// Daylight brightness from the sky and block nibbles: a simplified lightmap, not the game's curve.
fn brightness(light: vec2<f32>) -> f32 {
    let level = max(light.x, light.y * 0.9);
    return 0.12 + 0.88 * level * level;
}

@fragment
fn frag(in: VertexOut) -> @location(0) vec4<f32> {
    let texel = textureSample(block_atlas, block_sampler, in.uv);
    if (texel.a < ALPHA_CUTOFF) {
        discard;
    }
    let lit = texel.rgb * in.color * in.shade * brightness(in.light);
    return vec4<f32>(lit * texel.a, texel.a);
}
