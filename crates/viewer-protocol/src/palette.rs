//! Paletted cells: the block states of a Section and its biome samples.
//!
//! The wire form is `tag u8`, then for tag 0 (single) one `u32` value, and for tag 1 (indexed)
//! `palette_len u16`, that many `u32` values, `bits u8`, and `ceil(cells * bits / 8)` bytes of
//! indices packed least-significant bit first, cell 0 first. `bits` must equal
//! [`bits_for_palette`] of the palette length, so one container has exactly one encoding.

use crate::wire::{ProtocolError, Reader, Writer, invalid};

/// Cell values for one Section: state IDs for blocks, biome IDs for biome samples.
///
/// [`Paletted::Single`] is the only form for a uniform container; [`Paletted::Indexed`] needs at
/// least two distinct palette values, and every index must point into the palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Paletted {
    Single(u32),
    Indexed {
        palette: Vec<u32>,
        indices: Vec<u16>,
    },
}

/// The narrowest index width that addresses `palette_len` entries; at least 1.
pub fn bits_for_palette(palette_len: usize) -> u8 {
    let mut bits = 1u8;
    while (1usize << bits) < palette_len {
        bits += 1;
    }
    bits
}

impl Paletted {
    /// The value of cell `index`.
    pub fn get(&self, index: usize) -> u32 {
        match self {
            Self::Single(value) => *value,
            Self::Indexed { palette, indices } => palette[indices[index] as usize],
        }
    }

    /// Every distinct value; the single value, or the palette.
    pub fn values(&self) -> &[u32] {
        match self {
            Self::Single(value) => std::slice::from_ref(value),
            Self::Indexed { palette, .. } => palette,
        }
    }

    /// Builds the canonical container for `cells`: single when uniform, else a first-seen palette.
    pub fn from_cells(cells: &[u32]) -> Self {
        let mut palette: Vec<u32> = Vec::new();
        let indices: Vec<u16> = cells
            .iter()
            .map(
                |cell| match palette.iter().position(|value| value == cell) {
                    Some(position) => position as u16,
                    None => {
                        palette.push(*cell);
                        (palette.len() - 1) as u16
                    }
                },
            )
            .collect();
        if palette.len() == 1 {
            Self::Single(palette[0])
        } else {
            Self::Indexed { palette, indices }
        }
    }

    fn check(&self, cells: usize, what: &str) -> Result<(), ProtocolError> {
        let Self::Indexed { palette, indices } = self else {
            return Ok(());
        };
        if palette.len() < 2 || palette.len() > cells {
            return Err(invalid(format!(
                "{what} palette length {} is outside 2..={cells}",
                palette.len()
            )));
        }
        for (position, value) in palette.iter().enumerate() {
            if palette[..position].contains(value) {
                return Err(invalid(format!("{what} palette repeats value {value}")));
            }
        }
        if indices.len() != cells {
            return Err(invalid(format!(
                "{what} has {} cells, expected {cells}",
                indices.len()
            )));
        }
        if let Some(index) = indices
            .iter()
            .find(|index| **index as usize >= palette.len())
        {
            return Err(invalid(format!(
                "{what} index {index} is outside its palette"
            )));
        }
        Ok(())
    }

    pub(crate) fn encode(
        &self,
        writer: &mut Writer,
        cells: usize,
        what: &str,
    ) -> Result<(), ProtocolError> {
        self.check(cells, what)?;
        match self {
            Self::Single(value) => {
                writer.u8(0);
                writer.u32(*value);
            }
            Self::Indexed { palette, indices } => {
                writer.u8(1);
                writer.u16(palette.len() as u16);
                palette.iter().for_each(|value| writer.u32(*value));
                let bits = bits_for_palette(palette.len());
                writer.u8(bits);
                let mut packed = vec![0u8; (cells * bits as usize).div_ceil(8)];
                for (cell, index) in indices.iter().enumerate() {
                    for bit in 0..bits as usize {
                        if index >> bit & 1 == 1 {
                            let position = cell * bits as usize + bit;
                            packed[position / 8] |= 1 << (position % 8);
                        }
                    }
                }
                writer.bytes(&packed);
            }
        }
        Ok(())
    }

    pub(crate) fn decode(
        reader: &mut Reader<'_>,
        cells: usize,
        what: &str,
    ) -> Result<Self, ProtocolError> {
        let paletted = match reader.u8()? {
            0 => Self::Single(reader.u32()?),
            1 => {
                let palette_len = reader.u16()? as usize;
                if !(2..=cells).contains(&palette_len) {
                    return Err(invalid(format!(
                        "{what} palette length {palette_len} is outside 2..={cells}"
                    )));
                }
                let palette = (0..palette_len)
                    .map(|_| reader.u32())
                    .collect::<Result<Vec<_>, _>>()?;
                let bits = reader.u8()?;
                if bits != bits_for_palette(palette_len) {
                    return Err(invalid(format!(
                        "{what} index width {bits} does not match palette length {palette_len}"
                    )));
                }
                let packed = reader.take((cells * bits as usize).div_ceil(8))?;
                let indices = (0..cells)
                    .map(|cell| {
                        (0..bits as usize).fold(0u16, |index, bit| {
                            let position = cell * bits as usize + bit;
                            index | (((packed[position / 8] >> (position % 8)) & 1) as u16) << bit
                        })
                    })
                    .collect();
                Self::Indexed { palette, indices }
            }
            tag => {
                return Err(invalid(format!(
                    "{what} container tag {tag} is neither 0 nor 1"
                )));
            }
        };
        paletted.check(cells, what)?;
        Ok(paletted)
    }
}
