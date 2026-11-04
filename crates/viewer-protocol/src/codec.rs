//! Message encoding and decoding. Both directions enforce the same layout rules, so a value that
//! encodes always decodes back to itself and a decoded value always re-encodes to the same bytes.

use crate::message::*;
use crate::palette::Paletted;
use crate::wire::{ProtocolError, Reader, Writer, invalid};
use crate::{BIOME_CELLS, PROTOCOL_VERSION, SECTION_CELLS};

/// The largest `Hello` dimension list and `Subscribe` range list a message may carry.
pub const MAX_DIMENSIONS: usize = 64;
pub const MAX_RANGES: usize = 64;

/// Vanilla's build-range limits: lowest block Y and one past the highest.
const WORLD_MIN_Y: i32 = -2032;
const WORLD_END_Y: i32 = 2032;

/// Encodes one message, rejecting values that break the layout rules.
pub fn encode(message: &Message) -> Result<Vec<u8>, ProtocolError> {
    let mut writer = Writer::default();
    match message {
        Message::Hello(hello) => encode_hello(&mut writer, hello)?,
        Message::Subscribe(subscribe) => encode_subscribe(&mut writer, subscribe)?,
        Message::SectionSnapshot(snapshot) => encode_snapshot(&mut writer, snapshot)?,
        Message::BlockDelta(delta) => encode_delta(&mut writer, delta)?,
        Message::LightUpdate(update) => {
            writer.u8(tag::LIGHT_UPDATE);
            encode_pos(&mut writer, &update.pos);
            writer.u64(update.light_revision);
            encode_light(&mut writer, &update.sky_light);
            encode_light(&mut writer, &update.block_light);
        }
        Message::ChunkLifecycle(lifecycle) => {
            writer.u8(tag::CHUNK_LIFECYCLE);
            writer.u16(lifecycle.dimension);
            writer.i32(lifecycle.x);
            writer.i32(lifecycle.z);
            writer.u8(lifecycle.state as u8);
        }
        Message::ResyncRequest(request) => {
            writer.u8(tag::RESYNC_REQUEST);
            match request {
                ResyncRequest::Subscription { generation } => {
                    writer.u8(0);
                    writer.u32(*generation);
                }
                ResyncRequest::Section(pos) => {
                    writer.u8(1);
                    encode_pos(&mut writer, pos);
                }
            }
        }
    }
    Ok(writer.bytes)
}

fn check_dimension(dimension: &Dimension) -> Result<(), ProtocolError> {
    let end = dimension.min_y as i64 + dimension.height as i64;
    if dimension.id.is_empty()
        || dimension.min_y % 16 != 0
        || dimension.height == 0
        || !dimension.height.is_multiple_of(16)
        || dimension.min_y < WORLD_MIN_Y
        || end > WORLD_END_Y as i64
    {
        return Err(invalid(format!(
            "dimension {:?} has min_y {} and height {}; both must be multiples of 16 inside {WORLD_MIN_Y}..{WORLD_END_Y}",
            dimension.id, dimension.min_y, dimension.height
        )));
    }
    Ok(())
}

fn check_hello(hello: &Hello) -> Result<(), ProtocolError> {
    if hello.protocol_version != PROTOCOL_VERSION {
        return Err(invalid(format!(
            "protocol version {} is not {PROTOCOL_VERSION}",
            hello.protocol_version
        )));
    }
    if hello.block_state_count == 0 || hello.biome_count == 0 {
        return Err(invalid("dictionary has no block states or no biomes"));
    }
    if hello.dimensions.is_empty() || hello.dimensions.len() > MAX_DIMENSIONS {
        return Err(invalid(format!(
            "{} dimensions is outside 1..={MAX_DIMENSIONS}",
            hello.dimensions.len()
        )));
    }
    hello.dimensions.iter().try_for_each(check_dimension)
}

fn encode_hello(writer: &mut Writer, hello: &Hello) -> Result<(), ProtocolError> {
    check_hello(hello)?;
    writer.u8(tag::HELLO);
    writer.u16(hello.protocol_version);
    writer.u64(hello.session_id);
    writer.string(&hello.game_version)?;
    writer.bytes(&hello.dictionary_hash);
    writer.bytes(&hello.assets_hash);
    writer.u32(hello.block_state_count);
    writer.u32(hello.biome_count);
    writer.u8(hello.dimensions.len() as u8);
    for dimension in &hello.dimensions {
        writer.string(&dimension.id)?;
        writer.i32(dimension.min_y);
        writer.u32(dimension.height);
    }
    Ok(())
}

fn check_ranges(ranges: &[ChunkRange]) -> Result<(), ProtocolError> {
    if ranges.len() > MAX_RANGES {
        return Err(invalid(format!(
            "{} ranges is more than {MAX_RANGES}",
            ranges.len()
        )));
    }
    match ranges
        .iter()
        .find(|range| range.min_x > range.max_x || range.min_z > range.max_z)
    {
        Some(range) => Err(invalid(format!(
            "chunk range {range:?} has a minimum above its maximum"
        ))),
        None => Ok(()),
    }
}

fn encode_subscribe(writer: &mut Writer, subscribe: &Subscribe) -> Result<(), ProtocolError> {
    check_ranges(&subscribe.ranges)?;
    writer.u8(tag::SUBSCRIBE);
    writer.u32(subscribe.generation);
    writer.u16(subscribe.dimension);
    writer.u8(subscribe.ranges.len() as u8);
    for range in &subscribe.ranges {
        writer.i32(range.min_x);
        writer.i32(range.min_z);
        writer.i32(range.max_x);
        writer.i32(range.max_z);
    }
    Ok(())
}

fn encode_pos(writer: &mut Writer, pos: &SectionPos) {
    writer.u16(pos.dimension);
    writer.i32(pos.x);
    writer.i32(pos.y);
    writer.i32(pos.z);
}

fn encode_light(writer: &mut Writer, layer: &LightLayer) {
    match layer {
        LightLayer::Unknown => writer.u8(0),
        LightLayer::Data(nibbles) => {
            writer.u8(1);
            writer.bytes(&nibbles[..]);
        }
    }
}

fn encode_snapshot(writer: &mut Writer, snapshot: &SectionSnapshot) -> Result<(), ProtocolError> {
    writer.u8(tag::SECTION_SNAPSHOT);
    encode_pos(writer, &snapshot.pos);
    writer.u64(snapshot.revision);
    writer.u64(snapshot.light_revision);
    snapshot.blocks.encode(writer, SECTION_CELLS, "blocks")?;
    snapshot.biomes.encode(writer, BIOME_CELLS, "biomes")?;
    encode_light(writer, &snapshot.sky_light);
    encode_light(writer, &snapshot.block_light);
    Ok(())
}

fn check_delta(delta: &BlockDelta) -> Result<(), ProtocolError> {
    if delta.revision <= delta.base_revision {
        return Err(invalid(format!(
            "delta revision {} does not follow base revision {}",
            delta.revision, delta.base_revision
        )));
    }
    if delta.changes.is_empty() || delta.changes.len() > SECTION_CELLS {
        return Err(invalid(format!(
            "{} changes is outside 1..={SECTION_CELLS}",
            delta.changes.len()
        )));
    }
    let mut seen = vec![false; SECTION_CELLS];
    for change in &delta.changes {
        let index = change.index as usize;
        if index >= SECTION_CELLS {
            return Err(invalid(format!(
                "change index {index} is outside the Section"
            )));
        }
        if std::mem::replace(&mut seen[index], true) {
            return Err(invalid(format!("change index {index} appears twice")));
        }
    }
    Ok(())
}

fn encode_delta(writer: &mut Writer, delta: &BlockDelta) -> Result<(), ProtocolError> {
    check_delta(delta)?;
    writer.u8(tag::BLOCK_DELTA);
    encode_pos(writer, &delta.pos);
    writer.u64(delta.base_revision);
    writer.u64(delta.revision);
    writer.u16(delta.changes.len() as u16);
    for change in &delta.changes {
        writer.u16(change.index);
        writer.u32(change.state);
    }
    Ok(())
}

/// Decodes one whole message; `bytes` must contain exactly one message and nothing after it.
pub fn decode(bytes: &[u8]) -> Result<Message, ProtocolError> {
    let mut reader = Reader::new(bytes);
    let message = match reader.u8()? {
        tag::HELLO => Message::Hello(decode_hello(&mut reader)?),
        tag::SUBSCRIBE => Message::Subscribe(decode_subscribe(&mut reader)?),
        tag::SECTION_SNAPSHOT => Message::SectionSnapshot(SectionSnapshot {
            pos: decode_pos(&mut reader)?,
            revision: reader.u64()?,
            light_revision: reader.u64()?,
            blocks: Paletted::decode(&mut reader, SECTION_CELLS, "blocks")?,
            biomes: Paletted::decode(&mut reader, BIOME_CELLS, "biomes")?,
            sky_light: decode_light(&mut reader)?,
            block_light: decode_light(&mut reader)?,
        }),
        tag::BLOCK_DELTA => Message::BlockDelta(decode_delta(&mut reader)?),
        tag::LIGHT_UPDATE => Message::LightUpdate(LightUpdate {
            pos: decode_pos(&mut reader)?,
            light_revision: reader.u64()?,
            sky_light: decode_light(&mut reader)?,
            block_light: decode_light(&mut reader)?,
        }),
        tag::CHUNK_LIFECYCLE => Message::ChunkLifecycle(ChunkLifecycle {
            dimension: reader.u16()?,
            x: reader.i32()?,
            z: reader.i32()?,
            state: match reader.u8()? {
                0 => ChunkState::Hot,
                1 => ChunkState::Unloaded,
                2 => ChunkState::Unavailable,
                state => return Err(invalid(format!("chunk state {state} is not 0, 1 or 2"))),
            },
        }),
        tag::RESYNC_REQUEST => Message::ResyncRequest(match reader.u8()? {
            0 => ResyncRequest::Subscription {
                generation: reader.u32()?,
            },
            1 => ResyncRequest::Section(decode_pos(&mut reader)?),
            kind => return Err(invalid(format!("resync kind {kind} is neither 0 nor 1"))),
        }),
        other => return Err(ProtocolError::UnknownMessageType(other)),
    };
    reader.finish()?;
    Ok(message)
}

fn decode_hello(reader: &mut Reader<'_>) -> Result<Hello, ProtocolError> {
    let protocol_version = reader.u16()?;
    if protocol_version != PROTOCOL_VERSION {
        return Err(invalid(format!(
            "protocol version {protocol_version} is not {PROTOCOL_VERSION}"
        )));
    }
    let mut hello = Hello {
        protocol_version,
        session_id: reader.u64()?,
        game_version: reader.string()?,
        dictionary_hash: reader.array()?,
        assets_hash: reader.array()?,
        block_state_count: reader.u32()?,
        biome_count: reader.u32()?,
        dimensions: Vec::new(),
    };
    for _ in 0..reader.u8()? {
        hello.dimensions.push(Dimension {
            id: reader.string()?,
            min_y: reader.i32()?,
            height: reader.u32()?,
        });
    }
    check_hello(&hello)?;
    Ok(hello)
}

fn decode_subscribe(reader: &mut Reader<'_>) -> Result<Subscribe, ProtocolError> {
    let generation = reader.u32()?;
    let dimension = reader.u16()?;
    let ranges = (0..reader.u8()?)
        .map(|_| {
            Ok(ChunkRange {
                min_x: reader.i32()?,
                min_z: reader.i32()?,
                max_x: reader.i32()?,
                max_z: reader.i32()?,
            })
        })
        .collect::<Result<Vec<_>, ProtocolError>>()?;
    check_ranges(&ranges)?;
    Ok(Subscribe {
        generation,
        dimension,
        ranges,
    })
}

fn decode_pos(reader: &mut Reader<'_>) -> Result<SectionPos, ProtocolError> {
    Ok(SectionPos {
        dimension: reader.u16()?,
        x: reader.i32()?,
        y: reader.i32()?,
        z: reader.i32()?,
    })
}

fn decode_light(reader: &mut Reader<'_>) -> Result<LightLayer, ProtocolError> {
    match reader.u8()? {
        0 => Ok(LightLayer::Unknown),
        1 => Ok(LightLayer::Data(Box::new(reader.array()?))),
        tag => Err(invalid(format!("light layer tag {tag} is neither 0 nor 1"))),
    }
}

fn decode_delta(reader: &mut Reader<'_>) -> Result<BlockDelta, ProtocolError> {
    let pos = decode_pos(reader)?;
    let base_revision = reader.u64()?;
    let revision = reader.u64()?;
    let changes = (0..reader.u16()?)
        .map(|_| {
            Ok(BlockChange {
                index: reader.u16()?,
                state: reader.u32()?,
            })
        })
        .collect::<Result<Vec<_>, ProtocolError>>()?;
    let delta = BlockDelta {
        pos,
        base_revision,
        revision,
        changes,
    };
    check_delta(&delta)?;
    Ok(delta)
}
