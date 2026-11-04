//! The messages of display protocol v1. Layout and validation rules are in `docs/viewer-protocol.md`.

use crate::palette::Paletted;

/// Message type tags, the first byte of every message.
pub mod tag {
    pub const HELLO: u8 = 0x01;
    pub const SUBSCRIBE: u8 = 0x02;
    pub const SECTION_SNAPSHOT: u8 = 0x03;
    pub const BLOCK_DELTA: u8 = 0x04;
    pub const LIGHT_UPDATE: u8 = 0x05;
    pub const CHUNK_LIFECYCLE: u8 = 0x06;
    pub const RESYNC_REQUEST: u8 = 0x07;
}

/// A dimension the session can show: its identifier and the build range it covers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dimension {
    pub id: String,
    /// Lowest block Y; a multiple of 16.
    pub min_y: i32,
    /// Build height in blocks; a positive multiple of 16.
    pub height: u32,
}

/// Opens or replaces a session. Everything received before it no longer applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hello {
    pub protocol_version: u16,
    /// Changes whenever the producer's world state restarts; old revisions do not carry over.
    pub session_id: u64,
    pub game_version: String,
    /// SHA-256 of the dictionary that state and biome IDs index into.
    pub dictionary_hash: [u8; 32],
    /// SHA-256 of the block-model asset pack.
    pub assets_hash: [u8; 32],
    /// Number of block-state IDs in the dictionary; every state ID is below it.
    pub block_state_count: u32,
    /// Number of biome IDs in the dictionary; every biome ID is below it.
    pub biome_count: u32,
    /// Messages name a dimension by its index here.
    pub dimensions: Vec<Dimension>,
}

/// An inclusive rectangle of chunk columns in one dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkRange {
    pub min_x: i32,
    pub min_z: i32,
    pub max_x: i32,
    pub max_z: i32,
}

/// Replaces the sender's whole interest. An empty range list unsubscribes; a response is applied
/// only while its generation is still the newest one sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscribe {
    pub generation: u32,
    pub dimension: u16,
    pub ranges: Vec<ChunkRange>,
}

/// A Section's position: dimension index, chunk column, and section Y (block Y divided by 16).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SectionPos {
    pub dimension: u16,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

/// One light layer of a Section: 4096 nibbles, two per byte, low nibble first, cells ordered as
/// [`cell_index`]. [`LightLayer::Unknown`] means the producer has no valid data for the layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightLayer {
    Unknown,
    Data(Box<[u8; 2048]>),
}

/// Index of block `(x, y, z)` (each `0..16`) inside a Section: `(y * 16 + z) * 16 + x`.
pub fn cell_index(x: usize, y: usize, z: usize) -> usize {
    (y * 16 + z) * 16 + x
}

/// The complete state of one Section at `revision`. An all-air snapshot clears earlier content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectionSnapshot {
    pub pos: SectionPos,
    /// Block and biome revision; a [`BlockDelta`] applies on top of it.
    pub revision: u64,
    /// Light revision, counted separately because light changes without blocks changing.
    pub light_revision: u64,
    /// 4096 block-state IDs.
    pub blocks: Paletted,
    /// 64 biome IDs, one per 4×4×4 cube, ordered like blocks at quarter resolution.
    pub biomes: Paletted,
    pub sky_light: LightLayer,
    pub block_light: LightLayer,
}

/// One changed block: its [`cell_index`] and new state ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlockChange {
    pub index: u16,
    pub state: u32,
}

/// Changes to a Section that apply only on top of `base_revision` and produce `revision`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockDelta {
    pub pos: SectionPos,
    pub base_revision: u64,
    pub revision: u64,
    pub changes: Vec<BlockChange>,
}

/// Both light layers of a Section, replacing the held ones when `light_revision` is newer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightUpdate {
    pub pos: SectionPos,
    pub light_revision: u64,
    pub sky_light: LightLayer,
    pub block_light: LightLayer,
}

/// What a chunk column's state is now. `Unloaded` and `Unavailable` keep no terrain and must not be
/// drawn as air.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ChunkState {
    /// Live state follows; snapshots for its Sections are on the way.
    Hot = 0,
    Unloaded = 1,
    /// No reliable data exists, e.g. the column was never generated.
    Unavailable = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkLifecycle {
    pub dimension: u16,
    pub x: i32,
    pub z: i32,
    pub state: ChunkState,
}

/// Asks for fresh snapshots: of every Section in a subscription, or of one Section.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResyncRequest {
    Subscription { generation: u32 },
    Section(SectionPos),
}

/// Any message of display protocol v1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    Hello(Hello),
    Subscribe(Subscribe),
    SectionSnapshot(SectionSnapshot),
    BlockDelta(BlockDelta),
    LightUpdate(LightUpdate),
    ChunkLifecycle(ChunkLifecycle),
    ResyncRequest(ResyncRequest),
}
