//! The browser's copy of the world: Section state keyed by session, dimension and position, updated from
//! display-protocol messages and versioned independently of any GPU mesh.
//!
//! Every message is applied as it arrives; visibility never blocks state. A change marks its Section and
//! the 26 around it dirty, because a mesh samples neighbours for faces, AO, fluids and light. A delta whose
//! base does not match the held revision is not applied: its Section is queued for a resync instead.

use std::collections::{HashMap, HashSet};
use viewer_protocol::{
    ChunkState, Hello, LightLayer, Message, ProtocolError, SectionPos, SessionBounds, cell_index,
};

/// One held Section: dense cells so deltas and mesh sampling are plain indexing.
#[derive(Debug, Clone)]
pub struct SectionState {
    pub revision: u64,
    pub light_revision: u64,
    pub blocks: Box<[u32; 4096]>,
    pub biomes: [u32; 64],
    pub sky_light: LightLayer,
    pub block_light: LightLayer,
}

/// What is known about one block position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockLookup {
    /// No snapshot held for its Section: not air, just unknown.
    Unknown,
    /// Above or below the dimension's build range.
    OutsideWorld,
    State(u32),
}

/// What applying one message did, for the caller's logging and scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    SessionStarted,
    Changed,
    /// Older than or equal to what is held; nothing changed.
    Stale,
    /// A delta on a revision this state does not hold; its Section now awaits a resync.
    NeedsResync,
    Ignored,
}

#[derive(Default)]
pub struct WorldState {
    session: Option<(Hello, SessionBounds)>,
    sections: HashMap<SectionPos, SectionState>,
    columns: HashMap<(u16, i32, i32), ChunkState>,
    dirty: HashSet<SectionPos>,
    resync: HashSet<SectionPos>,
}

impl WorldState {
    pub fn hello(&self) -> Option<&Hello> {
        self.session.as_ref().map(|(hello, _)| hello)
    }

    /// Applies one decoded message. A `Hello` replaces the session and drops all held state; any other
    /// message before a `Hello`, or outside its bounds, is an error and changes nothing.
    pub fn apply(&mut self, message: &Message) -> Result<Applied, ProtocolError> {
        if let Message::Hello(hello) = message {
            *self = Self {
                session: Some((hello.clone(), SessionBounds::from_hello(hello))),
                ..Self::default()
            };
            return Ok(Applied::SessionStarted);
        }
        let (_, bounds) = self
            .session
            .as_ref()
            .ok_or_else(|| ProtocolError::OutOfBounds("no Hello yet".into()))?;
        bounds.check(message)?;
        Ok(match message {
            Message::SectionSnapshot(snapshot) => {
                if self
                    .sections
                    .get(&snapshot.pos)
                    .is_some_and(|held| held.revision >= snapshot.revision)
                {
                    return Ok(Applied::Stale);
                }
                let state = SectionState {
                    revision: snapshot.revision,
                    light_revision: snapshot.light_revision,
                    blocks: Box::new(std::array::from_fn(|cell| snapshot.blocks.get(cell))),
                    biomes: std::array::from_fn(|cell| snapshot.biomes.get(cell)),
                    sky_light: snapshot.sky_light.clone(),
                    block_light: snapshot.block_light.clone(),
                };
                self.sections.insert(snapshot.pos, state);
                self.resync.remove(&snapshot.pos);
                self.mark_dirty(snapshot.pos);
                Applied::Changed
            }
            Message::BlockDelta(delta) => match self.sections.get_mut(&delta.pos) {
                Some(held) if held.revision == delta.base_revision => {
                    delta
                        .changes
                        .iter()
                        .for_each(|change| held.blocks[change.index as usize] = change.state);
                    held.revision = delta.revision;
                    self.mark_dirty(delta.pos);
                    Applied::Changed
                }
                Some(held) if held.revision >= delta.revision => Applied::Stale,
                _ => {
                    self.resync.insert(delta.pos);
                    Applied::NeedsResync
                }
            },
            _ => self.apply_other(message),
        })
    }

    fn apply_other(&mut self, message: &Message) -> Applied {
        match message {
            Message::LightUpdate(update) => match self.sections.get_mut(&update.pos) {
                Some(held) if held.light_revision < update.light_revision => {
                    held.light_revision = update.light_revision;
                    held.sky_light = update.sky_light.clone();
                    held.block_light = update.block_light.clone();
                    self.mark_dirty(update.pos);
                    Applied::Changed
                }
                Some(_) => Applied::Stale,
                // Light for a Section with no blocks yet: the snapshot that follows carries its own light.
                None => Applied::Ignored,
            },
            Message::ChunkLifecycle(lifecycle) => {
                let column = (lifecycle.dimension, lifecycle.x, lifecycle.z);
                self.columns.insert(column, lifecycle.state);
                if lifecycle.state != ChunkState::Hot {
                    // Unloaded and unavailable columns keep no terrain; they read as unknown, never as air.
                    let removed: Vec<SectionPos> = self
                        .sections
                        .keys()
                        .filter(|pos| (pos.dimension, pos.x, pos.z) == column)
                        .copied()
                        .collect();
                    removed.into_iter().for_each(|pos| {
                        self.sections.remove(&pos);
                        self.resync.remove(&pos);
                        self.mark_dirty(pos);
                    });
                }
                Applied::Changed
            }
            _ => Applied::Ignored,
        }
    }

    pub fn section(&self, pos: &SectionPos) -> Option<&SectionState> {
        self.sections.get(pos)
    }

    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// Every Section held, for a renderer that starts after state has already arrived.
    pub fn held_sections(&self) -> Vec<SectionPos> {
        self.sections.keys().copied().collect()
    }

    pub fn column(&self, dimension: u16, x: i32, z: i32) -> Option<ChunkState> {
        self.columns.get(&(dimension, x, z)).copied()
    }

    /// Sections whose mesh may be out of date, cleared by this call. The scheduler orders them.
    pub fn take_dirty(&mut self) -> Vec<SectionPos> {
        self.dirty.drain().collect()
    }

    /// Sections whose held base no longer matches the producer, cleared by this call; ask for snapshots.
    pub fn take_resync(&mut self) -> Vec<SectionPos> {
        self.resync.drain().collect()
    }

    /// The block at world position `(x, y, z)` of `dimension`.
    pub fn block(&self, dimension: u16, x: i32, y: i32, z: i32) -> BlockLookup {
        let Some((_, bounds)) = &self.session else {
            return BlockLookup::Unknown;
        };
        let Some(section_ys) = bounds.section_ys.get(dimension as usize) else {
            return BlockLookup::Unknown;
        };
        let pos = SectionPos {
            dimension,
            x: x.div_euclid(16),
            y: y.div_euclid(16),
            z: z.div_euclid(16),
        };
        if !section_ys.contains(&pos.y) {
            return BlockLookup::OutsideWorld;
        }
        match self.sections.get(&pos) {
            None => BlockLookup::Unknown,
            Some(held) => BlockLookup::State(
                held.blocks[cell_index(
                    x.rem_euclid(16) as usize,
                    y.rem_euclid(16) as usize,
                    z.rem_euclid(16) as usize,
                )],
            ),
        }
    }

    /// Marks `pos` and its 26 neighbours for remeshing; only positions inside the build range count.
    fn mark_dirty(&mut self, pos: SectionPos) {
        let Some((_, bounds)) = &self.session else {
            return;
        };
        let section_ys = bounds.section_ys[pos.dimension as usize].clone();
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let neighbour = SectionPos {
                        dimension: pos.dimension,
                        x: pos.x.wrapping_add(dx),
                        y: pos.y + dy,
                        z: pos.z.wrapping_add(dz),
                    };
                    if section_ys.contains(&neighbour.y) {
                        self.dirty.insert(neighbour);
                    }
                }
            }
        }
    }
}
