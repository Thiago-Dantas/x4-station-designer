//! Game textures: gzip-packed DDS files with block-compressed (BC) images and
//! a full mip chain, e.g. `assets/textures/generic/generic_p1_hulltexture_01_diff.gz`.
//! The blocks are handed on as they are; GPUs (and WebGL) decode them.

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Bc1,
    Bc2,
    Bc3,
    Bc4,
    Bc5,
    Bc7,
}

impl Format {
    /// Bytes per 4×4 block.
    pub fn block_bytes(self) -> usize {
        match self {
            Format::Bc1 | Format::Bc4 => 8,
            _ => 16,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Texture {
    pub format: Format,
    /// Mip levels, largest first: (width, height, blocks).
    pub mips: Vec<(u32, u32, Vec<u8>)>,
}

/// Unpacks a gzip member (RFC 1952); other data is returned unchanged.
pub fn gunzip(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 18 || data[..2] != [0x1f, 0x8b] {
        return Ok(data.to_vec());
    }
    let bad = || Error("gzip: malformed header".to_owned());
    let flags = data[3];
    let mut pos = 10;
    if flags & 4 != 0 {
        let len = u16::from_le_bytes([*data.get(pos).ok_or_else(bad)?, *data.get(pos + 1).ok_or_else(bad)?]) as usize;
        pos += 2 + len;
    }
    for flag in [8, 16] {
        if flags & flag != 0 {
            pos += data.get(pos..).ok_or_else(bad)?.iter().position(|&b| b == 0).ok_or_else(bad)? + 1;
        }
    }
    if flags & 2 != 0 {
        pos += 2;
    }
    let body = data.get(pos..data.len() - 8).ok_or_else(bad)?;
    miniz_oxide::inflate::decompress_to_vec(body).map_err(|e| Error(format!("gzip: {e:?}")))
}

/// Reads a DDS file, keeping only mip levels no larger than `max_size` pixels
/// on their longer side (the smallest level is always kept).
pub fn parse_dds(data: &[u8], max_size: u32) -> Result<Texture> {
    let bad = |what: &str| Error(format!("dds: {what}"));
    if data.len() < 128 || &data[..4] != b"DDS " {
        return Err(bad("not a DDS file"));
    }
    let u32_at = |o: usize| u32::from_le_bytes(data[o..o + 4].try_into().unwrap());
    let (height, width, mip_count) = (u32_at(12), u32_at(16), u32_at(28).max(1));
    let (format, mut pos) = match &data[84..88] {
        b"DXT1" => (Format::Bc1, 128),
        b"DXT3" => (Format::Bc2, 128),
        b"DXT5" => (Format::Bc3, 128),
        b"ATI1" | b"BC4U" => (Format::Bc4, 128),
        b"ATI2" | b"BC5U" => (Format::Bc5, 128),
        b"DX10" if data.len() >= 148 => {
            let format = match u32_at(128) {
                71 | 72 => Format::Bc1,
                74 | 75 => Format::Bc2,
                77 | 78 => Format::Bc3,
                80 | 81 => Format::Bc4,
                83 | 84 => Format::Bc5,
                98 | 99 => Format::Bc7,
                other => return Err(bad(&format!("unsupported DXGI format {other}"))),
            };
            (format, 148)
        }
        other => return Err(bad(&format!("unsupported format {:?}", String::from_utf8_lossy(other)))),
    };
    let mut mips = Vec::new();
    let (mut w, mut h) = (width.max(1), height.max(1));
    for level in 0..mip_count {
        let size = w.div_ceil(4) as usize * h.div_ceil(4) as usize * format.block_bytes();
        let blocks = data.get(pos..pos + size).ok_or_else(|| bad("truncated mip chain"))?;
        if w.max(h) <= max_size || level + 1 == mip_count {
            mips.push((w, h, blocks.to_vec()));
        }
        pos += size;
        (w, h) = ((w / 2).max(1), (h / 2).max(1));
    }
    Ok(Texture { format, mips })
}
