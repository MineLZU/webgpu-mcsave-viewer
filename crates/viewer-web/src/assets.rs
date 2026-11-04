//! The gateway's block-model asset pack, unpacked into memory once its hash has been verified.
//! Entries keep their `assets/minecraft/...` paths; see `viewer-gateway/assets-tool/.../AssetPack.kt`.

use std::collections::HashMap;
use std::io::{Cursor, Read};

/// Every file of one asset pack, by its path inside the pack.
#[derive(Debug, Default)]
pub struct AssetPack {
    files: HashMap<String, Vec<u8>>,
}

impl AssetPack {
    pub fn unpack(bytes: &[u8]) -> Result<Self, String> {
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes))
            .map_err(|error| format!("asset pack is not a zip: {error}"))?;
        let mut files = HashMap::with_capacity(archive.len());
        for index in 0..archive.len() {
            let mut entry = archive
                .by_index(index)
                .map_err(|error| format!("asset pack entry {index}: {error}"))?;
            if entry.is_dir() {
                continue;
            }
            let name = entry
                .name()
                .map_err(|error| format!("asset pack entry {index} name: {error}"))?
                .into_owned();
            if name.contains("..") || name.starts_with('/') {
                return Err(format!("asset pack entry {name:?} escapes the pack"));
            }
            let mut data = Vec::with_capacity(entry.size() as usize);
            entry
                .read_to_end(&mut data)
                .map_err(|error| format!("asset pack entry {name}: {error}"))?;
            files.insert(name, data);
        }
        Ok(Self { files })
    }

    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path).map(Vec::as_slice)
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// Paths under `prefix`, sorted, e.g. every `assets/minecraft/blockstates/` file.
    pub fn paths_under(&self, prefix: &str) -> Vec<&str> {
        let mut paths: Vec<&str> = self
            .files
            .keys()
            .map(String::as_str)
            .filter(|path| path.starts_with(prefix))
            .collect();
        paths.sort_unstable();
        paths
    }
}
