//! Display protocol v1: the binary messages between the capture mod, the gateway and the browser.
//!
//! One WebSocket binary frame carries one message. Validation has two layers, kept apart on purpose
//! and mirrored by the Kotlin codec in `viewer-gateway/protocol`:
//!
//! * [`decode`]/[`encode`] enforce the layout itself — lengths, tags, palette shape, revision order.
//! * [`SessionBounds`] checks a well-formed message against the session's `Hello`: dimension index,
//!   section Y inside that dimension, and state/biome IDs inside the dictionary.
//!
//! The byte layout is specified in `docs/viewer-protocol.md`; `samples/` holds the shared vectors.

mod codec;
mod message;
mod palette;
mod wire;

pub use codec::{MAX_DIMENSIONS, MAX_RANGES, decode, encode};
pub use message::*;
pub use palette::{Paletted, bits_for_palette};
pub use wire::ProtocolError;

pub const PROTOCOL_VERSION: u16 = 1;
/// Blocks in a Section.
pub const SECTION_CELLS: usize = 4096;
/// Biome samples in a Section, one per 4×4×4 cube.
pub const BIOME_CELLS: usize = 64;

/// The facts of one session that a message is checked against; taken from its [`Hello`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionBounds {
    pub block_state_count: u32,
    pub biome_count: u32,
    /// Per dimension index: the section Y range, `min_y / 16 .. (min_y + height) / 16`.
    pub section_ys: Vec<std::ops::Range<i32>>,
}

impl SessionBounds {
    pub fn from_hello(hello: &Hello) -> Self {
        Self {
            block_state_count: hello.block_state_count,
            biome_count: hello.biome_count,
            section_ys: hello
                .dimensions
                .iter()
                .map(|dimension| {
                    dimension.min_y / 16..(dimension.min_y + dimension.height as i32) / 16
                })
                .collect(),
        }
    }

    /// Checks `message` against this session. `Hello` always passes: it starts a new session.
    pub fn check(&self, message: &Message) -> Result<(), ProtocolError> {
        match message {
            Message::Hello(_) => Ok(()),
            Message::Subscribe(subscribe) => self.dimension(subscribe.dimension).map(|_| ()),
            Message::SectionSnapshot(snapshot) => {
                self.pos(&snapshot.pos)?;
                self.ids(
                    snapshot.blocks.values(),
                    self.block_state_count,
                    "block state",
                )?;
                self.ids(snapshot.biomes.values(), self.biome_count, "biome")
            }
            Message::BlockDelta(delta) => {
                self.pos(&delta.pos)?;
                let states: Vec<u32> = delta.changes.iter().map(|change| change.state).collect();
                self.ids(&states, self.block_state_count, "block state")
            }
            Message::LightUpdate(update) => self.pos(&update.pos),
            Message::ChunkLifecycle(lifecycle) => self.dimension(lifecycle.dimension).map(|_| ()),
            Message::ResyncRequest(ResyncRequest::Section(pos)) => self.pos(pos),
            Message::ResyncRequest(ResyncRequest::Subscription { .. }) => Ok(()),
        }
    }

    fn dimension(&self, dimension: u16) -> Result<&std::ops::Range<i32>, ProtocolError> {
        self.section_ys.get(dimension as usize).ok_or_else(|| {
            ProtocolError::OutOfBounds(format!(
                "dimension index {dimension} is not one of {}",
                self.section_ys.len()
            ))
        })
    }

    fn pos(&self, pos: &SectionPos) -> Result<(), ProtocolError> {
        let section_ys = self.dimension(pos.dimension)?;
        if !section_ys.contains(&pos.y) {
            return Err(ProtocolError::OutOfBounds(format!(
                "section Y {} is outside {section_ys:?}",
                pos.y
            )));
        }
        Ok(())
    }

    fn ids(&self, ids: &[u32], count: u32, what: &str) -> Result<(), ProtocolError> {
        match ids.iter().find(|id| **id >= count) {
            Some(id) => Err(ProtocolError::OutOfBounds(format!(
                "{what} ID {id} is not below {count}"
            ))),
            None => Ok(()),
        }
    }
}
