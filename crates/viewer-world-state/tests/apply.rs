use viewer_protocol::*;
use viewer_world_state::{Applied, BlockLookup, WorldState};

fn hello(session_id: u64) -> Message {
    Message::Hello(Hello {
        protocol_version: PROTOCOL_VERSION,
        session_id,
        game_version: "1.21.11".into(),
        dictionary_hash: [0; 32],
        assets_hash: [0; 32],
        block_state_count: 100,
        biome_count: 4,
        dimensions: vec![Dimension {
            id: "minecraft:overworld".into(),
            min_y: -64,
            height: 384,
        }],
    })
}

fn pos(x: i32, y: i32, z: i32) -> SectionPos {
    SectionPos {
        dimension: 0,
        x,
        y,
        z,
    }
}

fn snapshot(pos: SectionPos, revision: u64, state: u32) -> Message {
    Message::SectionSnapshot(SectionSnapshot {
        pos,
        revision,
        light_revision: revision,
        blocks: Paletted::Single(state),
        biomes: Paletted::Single(0),
        sky_light: LightLayer::Unknown,
        block_light: LightLayer::Unknown,
    })
}

fn delta(pos: SectionPos, base_revision: u64, index: u16, state: u32) -> Message {
    Message::BlockDelta(BlockDelta {
        pos,
        base_revision,
        revision: base_revision + 1,
        changes: vec![BlockChange { index, state }],
    })
}

fn started() -> WorldState {
    let mut world = WorldState::default();
    assert_eq!(world.apply(&hello(1)).unwrap(), Applied::SessionStarted);
    world
}

#[test]
fn messages_before_hello_or_outside_bounds_change_nothing() {
    let mut world = WorldState::default();
    assert!(world.apply(&snapshot(pos(0, 0, 0), 1, 1)).is_err());
    let mut world = started();
    assert!(
        world.apply(&snapshot(pos(0, 24, 0), 1, 1)).is_err(),
        "above the build range"
    );
    assert!(
        world.apply(&snapshot(pos(0, 0, 0), 1, 100)).is_err(),
        "state outside the dictionary"
    );
    assert_eq!(world.section_count(), 0);
}

#[test]
fn stone_becomes_ore_through_a_matching_delta() {
    let mut world = started();
    world.apply(&snapshot(pos(-1, -4, 2), 5, 1)).unwrap();
    assert_eq!(world.block(0, -16, -64, 32), BlockLookup::State(1));
    assert_eq!(
        world
            .apply(&delta(pos(-1, -4, 2), 5, cell_index(0, 0, 0) as u16, 7))
            .unwrap(),
        Applied::Changed
    );
    assert_eq!(world.block(0, -16, -64, 32), BlockLookup::State(7));
    assert_eq!(world.block(0, -15, -64, 32), BlockLookup::State(1));
    assert_eq!(world.section(&pos(-1, -4, 2)).unwrap().revision, 6);
}

#[test]
fn a_delta_on_an_unknown_base_requests_a_resync_and_is_not_applied() {
    let mut world = started();
    world.apply(&snapshot(pos(0, 0, 0), 5, 1)).unwrap();
    assert_eq!(
        world.apply(&delta(pos(0, 0, 0), 7, 0, 9)).unwrap(),
        Applied::NeedsResync
    );
    assert_eq!(
        world.apply(&delta(pos(3, 0, 3), 1, 0, 9)).unwrap(),
        Applied::NeedsResync,
        "no snapshot held"
    );
    assert_eq!(world.block(0, 0, 0, 0), BlockLookup::State(1));
    let mut resync = world.take_resync();
    resync.sort_by_key(|pos| pos.x);
    assert_eq!(resync, vec![pos(0, 0, 0), pos(3, 0, 3)]);
    assert!(world.take_resync().is_empty());
}

#[test]
fn repeated_and_older_messages_do_not_roll_state_back() {
    let mut world = started();
    world.apply(&snapshot(pos(0, 0, 0), 5, 1)).unwrap();
    world.apply(&delta(pos(0, 0, 0), 5, 0, 9)).unwrap();
    assert_eq!(
        world.apply(&delta(pos(0, 0, 0), 5, 0, 3)).unwrap(),
        Applied::Stale
    );
    assert_eq!(
        world.apply(&snapshot(pos(0, 0, 0), 5, 2)).unwrap(),
        Applied::Stale
    );
    assert_eq!(world.block(0, 0, 0, 0), BlockLookup::State(9));
}

#[test]
fn a_change_dirties_its_section_and_neighbours_inside_the_build_range() {
    let mut world = started();
    world.apply(&snapshot(pos(4, 0, 4), 1, 1)).unwrap();
    assert_eq!(world.take_dirty().len(), 27);
    world.apply(&snapshot(pos(4, 19, 4), 1, 1)).unwrap();
    assert_eq!(
        world.take_dirty().len(),
        18,
        "no neighbours above the top Section"
    );
    world.apply(&delta(pos(4, 0, 4), 1, 5, 2)).unwrap();
    assert!(world.take_dirty().contains(&pos(5, 1, 3)));
}

#[test]
fn independent_light_updates_apply_without_block_changes() {
    let mut world = started();
    world.apply(&snapshot(pos(0, 0, 0), 1, 1)).unwrap();
    world.take_dirty();
    let light = |light_revision| {
        Message::LightUpdate(LightUpdate {
            pos: pos(0, 0, 0),
            light_revision,
            sky_light: LightLayer::Data(Box::new([0xff; 2048])),
            block_light: LightLayer::Unknown,
        })
    };
    assert_eq!(world.apply(&light(2)).unwrap(), Applied::Changed);
    assert_eq!(world.apply(&light(2)).unwrap(), Applied::Stale);
    assert_eq!(
        world.section(&pos(0, 0, 0)).unwrap().revision,
        1,
        "block revision untouched"
    );
    assert_eq!(world.take_dirty().len(), 27);
}

#[test]
fn unloading_forgets_terrain_as_unknown_and_hello_starts_over() {
    let mut world = started();
    world.apply(&snapshot(pos(2, 0, 2), 1, 0)).unwrap();
    world.apply(&snapshot(pos(2, 1, 2), 1, 0)).unwrap();
    assert_eq!(
        world.block(0, 32, 0, 32),
        BlockLookup::State(0),
        "air while held"
    );
    let unload = ChunkLifecycle {
        dimension: 0,
        x: 2,
        z: 2,
        state: ChunkState::Unloaded,
    };
    world.apply(&Message::ChunkLifecycle(unload)).unwrap();
    assert_eq!(
        world.block(0, 32, 0, 32),
        BlockLookup::Unknown,
        "unloaded is not air"
    );
    assert_eq!(world.column(0, 2, 2), Some(ChunkState::Unloaded));
    assert_eq!(world.block(0, 0, -65, 0), BlockLookup::OutsideWorld);
    world.apply(&snapshot(pos(0, 0, 0), 1, 1)).unwrap();
    world.apply(&hello(2)).unwrap();
    assert_eq!(world.section_count(), 0);
    assert_eq!(world.hello().unwrap().session_id, 2);
}
