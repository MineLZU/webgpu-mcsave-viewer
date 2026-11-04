//! The release dictionary the gateway serves: what every state and biome ID of a session means.
//! Its format is written by `viewer-gateway/assets-tool/src/main/kotlin/com/nts/viewer/assets/Dictionary.kt`.

use serde_json::Value;
use std::collections::BTreeMap;

/// One block state: its block identifier and complete property set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockStateEntry {
    pub block: String,
    pub properties: BTreeMap<String, String>,
}

/// One biome: its identifier and the official biome file's appearance fields, kept as JSON.
#[derive(Debug, Clone)]
pub struct BiomeEntry {
    pub id: String,
    pub fields: Value,
}

#[derive(Debug, Clone)]
pub struct Dictionary {
    pub game_version: String,
    /// Indexed by state ID.
    pub block_states: Vec<BlockStateEntry>,
    /// Indexed by biome ID.
    pub biomes: Vec<BiomeEntry>,
}

const FORMAT: u64 = 1;

impl Dictionary {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let root: Value = serde_json::from_slice(bytes)
            .map_err(|error| format!("dictionary is not JSON: {error}"))?;
        if root["format"].as_u64() != Some(FORMAT) {
            return Err(format!(
                "dictionary format {} is not {FORMAT}",
                root["format"]
            ));
        }
        let game_version = root["game_version"]
            .as_str()
            .ok_or("dictionary has no game_version")?
            .to_owned();
        let block_states = root["block_states"]
            .as_array()
            .ok_or("dictionary has no block_states")?
            .iter()
            .enumerate()
            .map(|(id, entry)| {
                let block = entry[0]
                    .as_str()
                    .ok_or(format!("state {id} has no block"))?
                    .to_owned();
                let properties = entry[1]
                    .as_object()
                    .ok_or(format!("state {id} has no property object"))?
                    .iter()
                    .map(|(name, value)| {
                        Ok((
                            name.clone(),
                            value
                                .as_str()
                                .ok_or(format!("state {id}: {name} is not a string"))?
                                .to_owned(),
                        ))
                    })
                    .collect::<Result<_, String>>()?;
                Ok(BlockStateEntry { block, properties })
            })
            .collect::<Result<Vec<_>, String>>()?;
        let biomes = root["biomes"]
            .as_array()
            .ok_or("dictionary has no biomes")?
            .iter()
            .map(|entry| {
                Ok(BiomeEntry {
                    id: entry["id"].as_str().ok_or("biome without id")?.to_owned(),
                    fields: entry.clone(),
                })
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            game_version,
            block_states,
            biomes,
        })
    }
}
