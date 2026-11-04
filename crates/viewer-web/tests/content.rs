use std::io::Write;
use viewer_web::assets::AssetPack;
use viewer_web::dictionary::Dictionary;

const DICTIONARY: &str = r#"{"format":1,"game_version":"1.21.11",
  "block_states":[["minecraft:air",{}],["minecraft:oak_log",{"axis":"x"}],["minecraft:oak_log",{"axis":"y"}]],
  "blocks":{"minecraft:air":0,"minecraft:oak_log":2},
  "biomes":[{"id":"minecraft:forest","temperature":0.7},{"id":"minecraft:plains","temperature":0.8}]}"#;

#[test]
fn dictionary_keeps_state_and_biome_order() {
    let dictionary = Dictionary::parse(DICTIONARY.as_bytes()).unwrap();
    assert_eq!(dictionary.game_version, "1.21.11");
    assert_eq!(dictionary.block_states.len(), 3);
    assert_eq!(dictionary.block_states[2].block, "minecraft:oak_log");
    assert_eq!(dictionary.block_states[2].properties["axis"], "y");
    assert_eq!(dictionary.biomes[1].id, "minecraft:plains");
    assert_eq!(dictionary.biomes[0].fields["temperature"], 0.7);
}

#[test]
fn dictionary_rejects_other_formats_and_shapes() {
    assert!(Dictionary::parse(br#"{"format":2}"#).is_err());
    assert!(
        Dictionary::parse(
            br#"{"format":1,"game_version":"x","block_states":[[1,{}]],"biomes":[]}"#
        )
        .is_err()
    );
    assert!(Dictionary::parse(b"not json").is_err());
}

fn zip_of(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in entries {
        writer.start_file(*name, options).unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[test]
fn asset_pack_unpacks_deflated_entries_by_path() {
    let pack = AssetPack::unpack(&zip_of(&[
        (
            "assets/minecraft/blockstates/stone.json",
            br#"{"variants":{"":{"model":"minecraft:block/stone"}}}"#,
        ),
        ("assets/minecraft/models/block/stone.json", b"{}"),
    ]))
    .unwrap();
    assert_eq!(pack.len(), 2);
    assert!(
        pack.get("assets/minecraft/blockstates/stone.json")
            .unwrap()
            .starts_with(b"{\"variants\"")
    );
    assert_eq!(
        pack.paths_under("assets/minecraft/models/"),
        vec!["assets/minecraft/models/block/stone.json"]
    );
}

#[test]
fn asset_pack_rejects_escaping_paths_and_non_zip_bytes() {
    assert!(AssetPack::unpack(&zip_of(&[("../evil.json", b"{}")])).is_err());
    assert!(AssetPack::unpack(b"not a zip").is_err());
}
