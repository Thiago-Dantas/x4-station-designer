"""Research parser for X4 `.xmf` meshes (reverse-engineered, see docs/x4-game-data.md).

    python -I research/xmf.py <file.xmf> ...      # print structure and bounds

Layout (little endian):
  header, 64 bytes: "XUMF", u8 version (3), u8 big-endian flag, u16 header size (64),
    u8 buffer count, u8 buffer-description size (188), u8 material count,
    u8 material size (136), u16 0, u32 vertex count, u32 index count,
    u32 primitive type (4 = triangle list), zero padding
  buffer descriptions, 188 bytes each:
    u32 type (0 = vertex, 30 = index), u32 usage index, u32 data offset,
    u32 compressed, u32 0, u32 format, u32 compressed size, u32 item count,
    u32 item size, u32 sections, 16 bytes reserved,
    u32 declaration element count, 16 x (u32 D3DDECLTYPE, u32 D3DDECLUSAGE)
  materials, 136 bytes each: u32 first index, u32 index count, char[128] name
  data: each buffer at <data start> + offset, zlib-compressed if `compressed`
"""
import struct
import sys
import zlib

DECL_SIZE = {0: 4, 1: 8, 2: 12, 3: 16, 4: 4, 5: 4, 6: 4, 7: 8, 8: 4, 9: 4, 10: 8, 11: 4, 12: 8, 13: 4, 14: 4, 15: 4, 16: 8}
USAGE = {0: "position", 1: "blendweight", 2: "blendindices", 3: "normal", 5: "texcoord", 6: "tangent", 7: "binormal", 10: "color"}


def parse(data):
    assert data[:4] == b"XUMF", data[:4]
    version, big_endian, header_size = data[4], data[5], struct.unpack_from("<H", data, 6)[0]
    n_buf, buf_size, n_mat, mat_size = data[8], data[9], data[10], data[11]
    n_vert, n_idx, prim = struct.unpack_from("<III", data, 14)
    buffers = []
    for i in range(n_buf):
        o = header_size + i * buf_size
        f = struct.unpack_from("<10I", data, o)
        n_decl = struct.unpack_from("<I", data, o + 56)[0]
        decl = [struct.unpack_from("<II", data, o + 60 + 8 * k) for k in range(n_decl)]
        buffers.append(dict(type=f[0], usage_index=f[1], offset=f[2], compressed=f[3], format=f[5],
                            csize=f[6], count=f[7], item_size=f[8], sections=f[9], decl=decl))
    mo = header_size + n_buf * buf_size
    materials = []
    for i in range(n_mat):
        first, count = struct.unpack_from("<II", data, mo + i * mat_size)
        name = data[mo + i * mat_size + 8: mo + (i + 1) * mat_size].split(b"\0")[0].decode()
        materials.append((first, count, name))
    start = mo + n_mat * mat_size
    for b in buffers:
        raw = data[start + b["offset"]: start + b["offset"] + b["csize"]]
        b["data"] = zlib.decompress(raw) if b["compressed"] else raw
        assert len(b["data"]) == b["count"] * b["item_size"], (len(b["data"]), b["count"], b["item_size"])
    return dict(version=version, big_endian=big_endian, vertices=n_vert, indices=n_idx, primitive=prim,
                buffers=buffers, materials=materials)


def positions(mesh):
    for b in mesh["buffers"]:
        offset = 0
        for t, usage in b["decl"]:
            if usage & 0xFF == 0:  # low byte = usage; upper bytes may hold leftover data
                assert t == 2, f"position type {t}"
                return [struct.unpack_from("<3f", b["data"], i * b["item_size"] + offset) for i in range(b["count"])]
            offset += DECL_SIZE[t]
    return []


def indices(mesh):
    b = next(b for b in mesh["buffers"] if b["type"] == 30)
    fmt = {2: "H", 4: "I"}[b["item_size"]]
    return list(struct.unpack(f"<{b['count']}{fmt}", b["data"]))


def bounds(points):
    lo = [min(p[i] for p in points) for i in range(3)]
    hi = [max(p[i] for p in points) for i in range(3)]
    return lo, hi


if __name__ == "__main__":
    for path in sys.argv[1:]:
        m = parse(open(path, "rb").read())
        print(f"== {path.split('/')[-1]}: v{m['version']} vertices {m['vertices']} indices {m['indices']} primitive {m['primitive']}")
        for b in m["buffers"]:
            decl = ", ".join(f"{USAGE.get(u, u)}:{t}" for t, u in b["decl"])
            print(f"   buffer type {b['type']} format {b['format']} items {b['count']} x {b['item_size']} B compressed {b['compressed']} [{decl}]")
        print("   materials:", m["materials"])
        pts, idx = positions(m), indices(m)
        lo, hi = bounds(pts)
        print(f"   bounds min {[round(v, 2) for v in lo]} max {[round(v, 2) for v in hi]}; max index {max(idx)} < {len(pts)}")
