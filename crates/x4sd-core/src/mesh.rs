//! X4 `.xmf` meshes (format reverse-engineered; see `docs/x4-game-data.md`).
//!
//! Layout, little endian: a 64-byte header (`XUMF`, version, counts), one
//! 188-byte description per data buffer (vertex or index; Direct3D 9 style
//! vertex declaration), one 136-byte record per material (first index, index
//! count, name), then the buffers, zlib-compressed. Coordinates are the game's
//! (left-handed, Y up) and triangles are wound clockwise.

use crate::{Error, Result};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mesh {
    pub positions: Vec<[f32; 3]>,
    /// Unit normals, one per vertex (zero when the file has none).
    pub normals: Vec<[f32; 3]>,
    /// First texture coordinates, one per vertex (zero when the file has none).
    /// Direct3D convention: v grows downwards in the texture.
    pub uvs: Vec<[f32; 2]>,
    /// Triangle list.
    pub indices: Vec<u32>,
    /// Index ranges per material: (material name, first index, index count).
    pub groups: Vec<(String, u32, u32)>,
}

const INDEX_BUFFER: u32 = 30;
const USAGE_POSITION: u32 = 0;
const USAGE_NORMAL: u32 = 3;
const USAGE_TEXCOORD: u32 = 5;
const DECL_FLOAT2: u32 = 1;
const DECL_FLOAT3: u32 = 2;
const DECL_FLOAT16_2: u32 = 15;
const DECL_D3DCOLOR: u32 = 4;

/// Byte size of a Direct3D 9 vertex element type (`D3DDECLTYPE`).
fn decl_size(t: u32) -> Option<usize> {
    Some(match t {
        0 | 4 | 5 | 6 | 8 | 9 | 11 | 13 | 14 | 15 => 4,
        1 | 7 | 10 | 12 | 16 => 8,
        2 => 12,
        3 => 16,
        _ => return None,
    })
}

struct Buffer {
    kind: u32,
    data: Vec<u8>,
    count: usize,
    item_size: usize,
    /// (D3DDECLTYPE, usage): the usage's low byte is the D3DDECLUSAGE, the next
    /// byte the usage index; the upper bytes can hold leftover data.
    decl: Vec<(u32, u32)>,
}

pub fn parse(data: &[u8]) -> Result<Mesh> {
    let bad = |what: &str| Error(format!("xmf: {what}"));
    if data.len() < 64 || &data[..4] != b"XUMF" {
        return Err(bad("not an XUMF file"));
    }
    if data[5] != 0 {
        return Err(bad("big-endian files are not supported"));
    }
    let u32_at = |o: usize| -> Result<u32> {
        data.get(o..o + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or_else(|| bad("truncated"))
    };
    let header_size = u16::from_le_bytes([data[6], data[7]]) as usize;
    let (n_buf, buf_size, n_mat, mat_size) = (data[8] as usize, data[9] as usize, data[10] as usize, data[11] as usize);
    if u32_at(22)? != 4 {
        return Err(bad("only triangle lists are supported"));
    }

    let materials_at = header_size + n_buf * buf_size;
    let data_at = materials_at + n_mat * mat_size;
    let mut buffers = Vec::new();
    for i in 0..n_buf {
        let o = header_size + i * buf_size;
        let (offset, compressed, csize) = (u32_at(o + 8)? as usize, u32_at(o + 12)? != 0, u32_at(o + 24)? as usize);
        let (count, item_size) = (u32_at(o + 28)? as usize, u32_at(o + 32)? as usize);
        let decl = (0..u32_at(o + 56)? as usize)
            .map(|k| Ok((u32_at(o + 60 + 8 * k)?, u32_at(o + 64 + 8 * k)?)))
            .collect::<Result<Vec<_>>>()?;
        let raw = data.get(data_at + offset..data_at + offset + csize).ok_or_else(|| bad("buffer outside the file"))?;
        let data = if compressed {
            miniz_oxide::inflate::decompress_to_vec_zlib(raw).map_err(|e| bad(&format!("zlib: {e:?}")))?
        } else {
            raw.to_vec()
        };
        if data.len() != count * item_size {
            return Err(bad("buffer size does not match its item count"));
        }
        buffers.push(Buffer { kind: u32_at(o)?, data, count, item_size, decl });
    }

    let mut mesh = Mesh::default();
    for b in buffers.iter().filter(|b| b.kind != INDEX_BUFFER) {
        let mut offset = 0;
        for &(t, usage) in &b.decl {
            let read = |i: usize, k: usize| b.data[i * b.item_size + offset + k];
            match (usage & 0xFF, t) {
                (USAGE_POSITION, DECL_FLOAT3) => {
                    mesh.positions = (0..b.count)
                        .map(|i| [0, 4, 8].map(|k| f32::from_le_bytes([read(i, k), read(i, k + 1), read(i, k + 2), read(i, k + 3)])))
                        .collect();
                }
                // D3DCOLOR stores B, G, R, A: the normal is (R, G, B) mapped from 0..255 to -1..1.
                (USAGE_NORMAL, DECL_D3DCOLOR) => {
                    mesh.normals = (0..b.count).map(|i| [2, 1, 0].map(|k| read(i, k) as f32 / 127.5 - 1.0)).collect();
                }
                // Only the first texture coordinate set (usage index 0).
                (USAGE_TEXCOORD, DECL_FLOAT2) if usage >> 8 & 0xFF == 0 => {
                    mesh.uvs = (0..b.count)
                        .map(|i| [0, 4].map(|k| f32::from_le_bytes([read(i, k), read(i, k + 1), read(i, k + 2), read(i, k + 3)])))
                        .collect();
                }
                (USAGE_TEXCOORD, DECL_FLOAT16_2) if usage >> 8 & 0xFF == 0 => {
                    mesh.uvs = (0..b.count).map(|i| [0, 2].map(|k| f16_to_f32(u16::from_le_bytes([read(i, k), read(i, k + 1)])))).collect();
                }
                _ => {}
            }
            offset += decl_size(t).ok_or_else(|| bad(&format!("unknown vertex element type {t}")))?;
        }
    }
    if mesh.positions.is_empty() {
        return Err(bad("no vertex positions"));
    }
    if mesh.normals.len() != mesh.positions.len() {
        mesh.normals = vec![[0.0; 3]; mesh.positions.len()];
    }
    if mesh.uvs.len() != mesh.positions.len() {
        mesh.uvs = vec![[0.0; 2]; mesh.positions.len()];
    }

    let ib = buffers.iter().find(|b| b.kind == INDEX_BUFFER).ok_or_else(|| bad("no index buffer"))?;
    mesh.indices = match ib.item_size {
        2 => ib.data.as_chunks::<2>().0.iter().map(|c| u16::from_le_bytes(*c) as u32).collect(),
        4 => ib.data.as_chunks::<4>().0.iter().map(|c| u32::from_le_bytes(*c)).collect(),
        n => return Err(bad(&format!("unsupported index size {n}"))),
    };
    if mesh.indices.iter().any(|&i| i as usize >= mesh.positions.len()) {
        return Err(bad("index out of range"));
    }

    for i in 0..n_mat {
        let o = materials_at + i * mat_size;
        let name = data.get(o + 8..o + mat_size).ok_or_else(|| bad("truncated material"))?;
        let name = String::from_utf8_lossy(name.split(|&b| b == 0).next().unwrap_or_default()).into_owned();
        mesh.groups.push((name, u32_at(o)?, u32_at(o + 4)?));
    }
    if mesh.groups.is_empty() {
        mesh.groups.push((String::new(), 0, mesh.indices.len() as u32));
    }
    Ok(mesh)
}

fn f16_to_f32(h: u16) -> f32 {
    let sign = if h & 0x8000 != 0 { -1.0 } else { 1.0 };
    let exp = (h >> 10 & 0x1F) as i32;
    let frac = (h & 0x3FF) as f32;
    sign * match exp {
        0 => frac * 2f32.powi(-24),
        31 => f32::INFINITY,
        _ => (1.0 + frac / 1024.0) * 2f32.powi(exp - 15),
    }
}
