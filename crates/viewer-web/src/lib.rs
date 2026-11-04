//! The browser viewer's wasm module: WebGPU lifecycle, display-protocol intake and content loading.
//!
//! `dictionary` and `assets` are target-independent and tested natively; `web` is the wasm-bindgen surface
//! the TypeScript host in `web-client/src/main.ts` drives. Terrain drawing reuses `wgpu-mc`.

// As in `wgpu-mc/src/lib.rs`: proving `WmRenderer: Send` walks wgpu-core's whole hub, which clippy's
// `arc_with_non_send_sync` lint asks for and the default limit of 128 does not reach.
#![recursion_limit = "512"]

pub mod assets;
pub mod dictionary;
pub mod render;

#[cfg(target_arch = "wasm32")]
mod web;

/// Lowercase hex of a hash, as the gateway names content in `/content/{sha256}`.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
