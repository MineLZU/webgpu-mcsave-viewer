//! The shared sample vectors in `samples/`: what they contain, and that this codec agrees with them.
//!
//! The checked-in files are the contract the Kotlin codec is tested against too. This test rebuilds
//! every sample and requires the files to match byte for byte, so a layout change cannot slip in.
//! Set `VIEWER_PROTOCOL_WRITE_SAMPLES=1` to rewrite them after an intended change.

use serde_json::{Value, json};
use std::path::PathBuf;
use viewer_protocol::*;

const SESSION_BLOCK_STATES: u32 = 30000;
const SESSION_BIOMES: u32 = 65;

fn samples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("samples")
}

fn hello() -> Hello {
    Hello {
        protocol_version: PROTOCOL_VERSION,
        session_id: 0x0123_4567_89ab_cdef,
        game_version: "1.21.11".to_owned(),
        dictionary_hash: std::array::from_fn(|i| i as u8),
        assets_hash: std::array::from_fn(|i| 0xff - i as u8),
        block_state_count: SESSION_BLOCK_STATES,
        biome_count: SESSION_BIOMES,
        dimensions: vec![
            Dimension {
                id: "minecraft:overworld".to_owned(),
                min_y: -64,
                height: 384,
            },
            Dimension {
                id: "minecraft:the_nether".to_owned(),
                min_y: 0,
                height: 256,
            },
            Dimension {
                id: "minecraft:the_end".to_owned(),
                min_y: 0,
                height: 256,
            },
        ],
    }
}

fn light(value_at: impl Fn(usize) -> u8) -> LightLayer {
    let mut nibbles = Box::new([0u8; 2048]);
    for cell in 0..SECTION_CELLS {
        nibbles[cell / 2] |= (value_at(cell) & 0xf) << (cell % 2 * 4);
    }
    LightLayer::Data(nibbles)
}

fn nibble(layer: &LightLayer, cell: usize) -> Option<u8> {
    match layer {
        LightLayer::Unknown => None,
        LightLayer::Data(nibbles) => Some(nibbles[cell / 2] >> (cell % 2 * 4) & 0xf),
    }
}

/// Stone below y = 8 and air above it, with one ore block at local (3, 2, 5): a three-value
/// palette, two bits per cell, packed with no index crossing a byte.
fn layered_snapshot() -> SectionSnapshot {
    let cells: Vec<u32> = (0..SECTION_CELLS)
        .map(|cell| match cell {
            _ if cell == cell_index(3, 2, 5) => 4321,
            _ if cell / 256 < 8 => 1,
            _ => 0,
        })
        .collect();
    SectionSnapshot {
        pos: SectionPos {
            dimension: 0,
            x: -3,
            y: -4,
            z: 7,
        },
        revision: 5,
        light_revision: 2,
        blocks: Paletted::from_cells(&cells),
        biomes: Paletted::from_cells(
            &(0..BIOME_CELLS as u32)
                .map(|i| 40 + i % 2)
                .collect::<Vec<_>>(),
        ),
        sky_light: light(|cell| if cell / 256 < 8 { 0 } else { 15 }),
        block_light: LightLayer::Unknown,
    }
}

/// Seventeen distinct states: five-bit indices, which straddle byte boundaries.
fn wide_palette_snapshot() -> SectionSnapshot {
    let cells: Vec<u32> = (0..SECTION_CELLS as u32)
        .map(|cell| 1000 + (cell * 7) % 17)
        .collect();
    SectionSnapshot {
        pos: SectionPos {
            dimension: 1,
            x: 2_000_000,
            y: 15,
            z: -2_000_000,
        },
        revision: 9,
        light_revision: 9,
        blocks: Paletted::from_cells(&cells),
        biomes: Paletted::Single(7),
        sky_light: LightLayer::Unknown,
        block_light: light(|cell| (cell % 16) as u8),
    }
}

fn encoded(message: Message) -> Vec<u8> {
    encode(&message).expect("sample messages are valid")
}

fn patched(message: Message, offset: usize, bytes: &[u8]) -> Vec<u8> {
    let mut encoded = encoded(message);
    encoded[offset..offset + bytes.len()].copy_from_slice(bytes);
    encoded
}

/// Offset of the block container inside an encoded snapshot: tag, position, two revisions.
const SNAPSHOT_BLOCKS: usize = 1 + 14 + 8 + 8;

fn delta(changes: Vec<BlockChange>) -> BlockDelta {
    BlockDelta {
        pos: SectionPos {
            dimension: 0,
            x: -1,
            y: -1,
            z: -1,
        },
        base_revision: 5,
        revision: 6,
        changes,
    }
}

fn valid_samples() -> Vec<(&'static str, Message)> {
    let change = |index, state| BlockChange { index, state };
    let range = |min_x, min_z, max_x, max_z| ChunkRange {
        min_x,
        min_z,
        max_x,
        max_z,
    };
    vec![
        ("hello", Message::Hello(hello())),
        (
            "subscribe",
            Message::Subscribe(Subscribe {
                generation: 3,
                dimension: 0,
                ranges: vec![range(-8, -8, 7, 7), range(100, -3, 100, -3)],
            }),
        ),
        (
            "subscribe_none",
            Message::Subscribe(Subscribe {
                generation: 4,
                dimension: 0,
                ranges: vec![],
            }),
        ),
        (
            "section_layered",
            Message::SectionSnapshot(layered_snapshot()),
        ),
        (
            "section_wide_palette",
            Message::SectionSnapshot(wide_palette_snapshot()),
        ),
        ("section_empty", Message::SectionSnapshot(empty_snapshot())),
        (
            "delta_single",
            Message::BlockDelta(delta(vec![change(cell_index(3, 2, 5) as u16, 1)])),
        ),
        (
            "delta_multi",
            Message::BlockDelta(delta(vec![
                change(4095, 9),
                change(0, 0),
                change(2048, 29999),
            ])),
        ),
        (
            "light_update",
            Message::LightUpdate(LightUpdate {
                pos: SectionPos {
                    dimension: 2,
                    x: 5,
                    y: 0,
                    z: -5,
                },
                light_revision: 11,
                sky_light: light(|cell| (cell % 16) as u8),
                block_light: light(|_| 15),
            }),
        ),
        (
            "lifecycle_hot",
            Message::ChunkLifecycle(ChunkLifecycle {
                dimension: 0,
                x: -1,
                z: 1,
                state: ChunkState::Hot,
            }),
        ),
        (
            "lifecycle_unavailable",
            Message::ChunkLifecycle(ChunkLifecycle {
                dimension: 1,
                x: 0,
                z: i32::MIN,
                state: ChunkState::Unavailable,
            }),
        ),
        (
            "resync_subscription",
            Message::ResyncRequest(ResyncRequest::Subscription { generation: 3 }),
        ),
        (
            "resync_section",
            Message::ResyncRequest(ResyncRequest::Section(layered_snapshot().pos)),
        ),
    ]
}

fn invalid_samples() -> Vec<(&'static str, Vec<u8>, &'static str)> {
    let layered = || Message::SectionSnapshot(layered_snapshot());
    let multi = || {
        valid_samples()
            .into_iter()
            .find(|(name, _)| *name == "delta_multi")
            .unwrap()
            .1
    };
    let blocks = SNAPSHOT_BLOCKS;
    // Tag, version, session, "1.21.11", two hashes, two counts, dimension count, "minecraft:overworld".
    let first_min_y = 1 + 2 + 8 + 2 + 7 + 32 + 32 + 4 + 4 + 1 + 2 + 19;
    let mut layered_bytes = encoded(layered());
    let truncated = layered_bytes[..layered_bytes.len() - 100].to_vec();
    layered_bytes.push(0);
    let mut delta_empty = patched(
        Message::BlockDelta(delta(vec![BlockChange { index: 1, state: 1 }])),
        31,
        &[0, 0],
    );
    delta_empty.truncate(33);
    vec![
        (
            "hello_wrong_version",
            patched(Message::Hello(hello()), 1, &2u16.to_le_bytes()),
            "invalid",
        ),
        (
            "hello_unaligned_min_y",
            patched(
                Message::Hello(hello()),
                first_min_y,
                &(-60i32).to_le_bytes(),
            ),
            "invalid",
        ),
        (
            "subscribe_inverted_range",
            patched(valid_samples().swap_remove(1).1, 8, &100i32.to_le_bytes()),
            "invalid",
        ),
        ("section_truncated", truncated, "truncated"),
        ("section_trailing_byte", layered_bytes, "trailing_bytes"),
        (
            "section_container_tag",
            patched(layered(), blocks, &[2]),
            "invalid",
        ),
        (
            "section_palette_of_one",
            patched(layered(), blocks + 1, &1u16.to_le_bytes()),
            "invalid",
        ),
        (
            "section_palette_too_long",
            patched(layered(), blocks + 1, &4097u16.to_le_bytes()),
            "invalid",
        ),
        (
            "section_index_width",
            patched(layered(), blocks + 15, &[3]),
            "invalid",
        ),
        (
            "section_index_outside_palette",
            patched(layered(), blocks + 16, &[0xff]),
            "invalid",
        ),
        (
            "light_layer_tag",
            patched(valid_samples().swap_remove(8).1, 23, &[2]),
            "invalid",
        ),
        (
            "delta_stale_revision",
            patched(multi(), 23, &5u64.to_le_bytes()),
            "invalid",
        ),
        (
            "delta_duplicate_index",
            patched(multi(), 39, &4095u16.to_le_bytes()),
            "invalid",
        ),
        (
            "delta_index_outside_section",
            patched(multi(), 33, &4096u16.to_le_bytes()),
            "invalid",
        ),
        ("delta_no_changes", delta_empty, "invalid"),
        (
            "lifecycle_state",
            patched(valid_samples().swap_remove(9).1, 11, &[3]),
            "invalid",
        ),
        (
            "resync_kind",
            patched(valid_samples().swap_remove(11).1, 1, &[2]),
            "invalid",
        ),
        ("unknown_type", vec![0x7f], "unknown_type"),
        ("empty_message", vec![], "truncated"),
    ]
}

fn out_of_bounds_samples() -> Vec<(&'static str, Message)> {
    let at = |dimension, y| SectionSnapshot {
        pos: SectionPos {
            dimension,
            x: 0,
            y,
            z: 0,
        },
        ..empty_snapshot()
    };
    let mut unknown_state = layered_snapshot();
    unknown_state.blocks = Paletted::Indexed {
        palette: vec![1, SESSION_BLOCK_STATES, 0],
        indices: match unknown_state.blocks {
            Paletted::Indexed { indices, .. } => indices,
            _ => unreachable!(),
        },
    };
    vec![
        (
            "section_state_outside_dictionary",
            Message::SectionSnapshot(unknown_state),
        ),
        (
            "section_biome_outside_dictionary",
            Message::SectionSnapshot(SectionSnapshot {
                biomes: Paletted::Single(SESSION_BIOMES),
                ..empty_snapshot()
            }),
        ),
        (
            "section_above_dimension",
            Message::SectionSnapshot(at(0, 24)),
        ),
        (
            "section_below_dimension",
            Message::SectionSnapshot(at(0, -5)),
        ),
        (
            "section_nether_above_dimension",
            Message::SectionSnapshot(at(1, 16)),
        ),
        (
            "section_unknown_dimension",
            Message::SectionSnapshot(at(3, 0)),
        ),
        (
            "delta_state_outside_dictionary",
            Message::BlockDelta(delta(vec![BlockChange {
                index: 0,
                state: SESSION_BLOCK_STATES,
            }])),
        ),
        (
            "subscribe_unknown_dimension",
            Message::Subscribe(Subscribe {
                generation: 1,
                dimension: 9,
                ranges: vec![],
            }),
        ),
    ]
}

/// The fields a sample's manifest entry lists, so a codec in another language can check its
/// decoding without this crate's types. Block cells are probed, not listed.
fn describe(message: &Message) -> Value {
    let pos =
        |pos: &SectionPos| json!({"dimension": pos.dimension, "x": pos.x, "y": pos.y, "z": pos.z});
    let probes = [0, 1, cell_index(3, 2, 5), 2047, 2048, 4095];
    match message {
        Message::Hello(hello) => {
            json!({"type": "hello", "session_id": hello.session_id.to_string(),
            "game_version": hello.game_version, "block_state_count": hello.block_state_count,
            "biome_count": hello.biome_count,
            "dimensions": hello.dimensions.iter().map(|d| json!([d.id, d.min_y, d.height])).collect::<Vec<_>>()})
        }
        Message::Subscribe(subscribe) => {
            json!({"type": "subscribe", "generation": subscribe.generation,
            "dimension": subscribe.dimension,
            "ranges": subscribe.ranges.iter().map(|r| json!([r.min_x, r.min_z, r.max_x, r.max_z])).collect::<Vec<_>>()})
        }
        Message::SectionSnapshot(s) => json!({"type": "section_snapshot", "pos": pos(&s.pos),
            "revision": s.revision, "light_revision": s.light_revision,
            "block_palette": s.blocks.values(), "biome_palette": s.biomes.values(),
            "cells": probes.iter().map(|cell| json!([cell, s.blocks.get(*cell)])).collect::<Vec<_>>(),
            "biome_cells": [[0, s.biomes.get(0)], [1, s.biomes.get(1)], [63, s.biomes.get(63)]],
            "sky_light": probes.iter().map(|cell| json!([cell, nibble(&s.sky_light, *cell)])).collect::<Vec<_>>(),
            "block_light": probes.iter().map(|cell| json!([cell, nibble(&s.block_light, *cell)])).collect::<Vec<_>>()}),
        Message::BlockDelta(d) => json!({"type": "block_delta", "pos": pos(&d.pos),
            "base_revision": d.base_revision, "revision": d.revision,
            "changes": d.changes.iter().map(|c| json!([c.index, c.state])).collect::<Vec<_>>()}),
        Message::LightUpdate(u) => json!({"type": "light_update", "pos": pos(&u.pos),
            "light_revision": u.light_revision,
            "sky_light": probes.iter().map(|cell| json!([cell, nibble(&u.sky_light, *cell)])).collect::<Vec<_>>(),
            "block_light": probes.iter().map(|cell| json!([cell, nibble(&u.block_light, *cell)])).collect::<Vec<_>>()}),
        Message::ChunkLifecycle(l) => json!({"type": "chunk_lifecycle", "dimension": l.dimension,
            "x": l.x, "z": l.z, "state": l.state as u8}),
        Message::ResyncRequest(ResyncRequest::Subscription { generation }) => {
            json!({"type": "resync_request", "kind": "subscription", "generation": generation})
        }
        Message::ResyncRequest(ResyncRequest::Section(p)) => {
            json!({"type": "resync_request", "kind": "section", "pos": pos(p)})
        }
    }
}

/// An all-air Section with no light: the form that clears whatever was held before.
fn empty_snapshot() -> SectionSnapshot {
    SectionSnapshot {
        pos: SectionPos {
            dimension: 0,
            x: 0,
            y: 19,
            z: 0,
        },
        revision: 1,
        light_revision: 1,
        blocks: Paletted::Single(0),
        biomes: Paletted::Single(0),
        sky_light: LightLayer::Unknown,
        block_light: LightLayer::Unknown,
    }
}

/// Every sample with its bytes and manifest entry.
fn all_samples() -> Vec<(String, Vec<u8>, Value)> {
    let mut samples = Vec::new();
    for (name, message) in valid_samples() {
        samples.push((
            name.to_owned(),
            encoded(message.clone()),
            json!({"expect": "ok", "message": describe(&message)}),
        ));
    }
    for (name, bytes, error) in invalid_samples() {
        samples.push((
            name.to_owned(),
            bytes,
            json!({"expect": "decode_error", "error": error}),
        ));
    }
    for (name, message) in out_of_bounds_samples() {
        let entry = json!({"expect": "out_of_bounds", "message": describe(&message)});
        samples.push((name.to_owned(), encoded(message), entry));
    }
    samples
}

fn manifest() -> Value {
    let samples: Vec<Value> = all_samples()
        .into_iter()
        .map(|(name, _, mut entry)| {
            entry["name"] = json!(name);
            entry["file"] = json!(format!("{name}.bin"));
            entry
        })
        .collect();
    json!({"protocol_version": PROTOCOL_VERSION, "session_hello": "hello.bin", "samples": samples})
}

fn error_kind(error: &ProtocolError) -> &'static str {
    match error {
        ProtocolError::Truncated { .. } => "truncated",
        ProtocolError::TrailingBytes { .. } => "trailing_bytes",
        ProtocolError::UnknownMessageType(_) => "unknown_type",
        ProtocolError::Invalid(_) => "invalid",
        ProtocolError::OutOfBounds(_) => "out_of_bounds",
    }
}

/// The checked-in files equal what this codec produces; rewrites them when asked to.
#[test]
fn checked_in_samples_match_the_codec() {
    let dir = samples_dir();
    let manifest_text = format!("{}\n", serde_json::to_string_pretty(&manifest()).unwrap());
    if std::env::var_os("VIEWER_PROTOCOL_WRITE_SAMPLES").is_some() {
        if dir.exists() {
            std::fs::remove_dir_all(&dir).unwrap();
        }
        std::fs::create_dir_all(&dir).unwrap();
        for (name, bytes, _) in all_samples() {
            std::fs::write(dir.join(format!("{name}.bin")), bytes).unwrap();
        }
        std::fs::write(dir.join("manifest.json"), manifest_text).unwrap();
        return;
    }
    let on_disk: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(
        on_disk,
        manifest(),
        "manifest.json differs; rerun with VIEWER_PROTOCOL_WRITE_SAMPLES=1 if intended"
    );
    let files = std::fs::read_dir(&dir)
        .unwrap()
        .filter(|entry| entry.as_ref().unwrap().path().extension() == Some("bin".as_ref()));
    assert_eq!(
        files.count(),
        all_samples().len(),
        "samples/ holds files no sample produces"
    );
    for (name, bytes, _) in all_samples() {
        assert_eq!(
            std::fs::read(dir.join(format!("{name}.bin"))).unwrap(),
            bytes,
            "{name}.bin differs"
        );
    }
}

/// Each sample decodes, or fails, the way its manifest entry says.
#[test]
fn samples_decode_as_their_manifest_says() {
    let bounds = SessionBounds::from_hello(&hello());
    for (name, bytes, entry) in all_samples() {
        let decoded = decode(&bytes);
        match entry["expect"].as_str().unwrap() {
            "ok" => {
                let message = decoded.unwrap_or_else(|error| panic!("{name}: {error}"));
                bounds
                    .check(&message)
                    .unwrap_or_else(|error| panic!("{name}: {error}"));
                assert_eq!(describe(&message), entry["message"], "{name}");
                assert_eq!(
                    encode(&message).unwrap(),
                    bytes,
                    "{name} re-encodes differently"
                );
            }
            "decode_error" => {
                let error = decoded.expect_err(&name);
                assert_eq!(
                    error_kind(&error),
                    entry["error"].as_str().unwrap(),
                    "{name}: {error}"
                );
            }
            "out_of_bounds" => {
                let message = decoded.unwrap_or_else(|error| panic!("{name}: {error}"));
                let error = bounds.check(&message).expect_err(&name);
                assert_eq!(error_kind(&error), "out_of_bounds", "{name}");
            }
            other => panic!("{name}: unknown expectation {other}"),
        }
    }
}
