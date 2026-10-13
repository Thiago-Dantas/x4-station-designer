"""Research: do plan offsets follow from module snap geometry?

For every plan entry with a predecessor, predict its position from the
predecessor's offset and both snap connection offsets, and compare it with the
stored offset. The yaw/pitch/roll convention is unknown, so every axis order and
sign combination is tried and ranked by error.

    python -I research/snapcheck.py <constructionplans.xml>
"""
import itertools
import math
import os
import re
import sys
import xml.etree.ElementTree as ET

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "tools"))
from x4cat import read_cat  # noqa: E402

GAME = r"F:\SteamLibrary\steamapps\common\X4 Foundations"
DLCS = ["ego_dlc_boron", "ego_dlc_pirate", "ego_dlc_split", "ego_dlc_terran", "ego_dlc_timelines", "ego_dlc_ventures"]


# --- virtual file system over base + DLC catalogs (later catalogs win) ---
def build_vfs():
    vfs = {}
    cats = [(os.path.join(GAME, f"{i:02d}.cat"), "") for i in range(1, 10)]
    for dlc in DLCS:
        d = os.path.join(GAME, "extensions", dlc)
        cats += [(os.path.join(d, f), f"extensions/{dlc}/") for f in sorted(os.listdir(d))
                 if re.fullmatch(r"ext_\d+\.cat", f)]
    for cat, prefix in cats:
        dat = cat[:-4] + ".dat"
        for path, offset, size, _, _ in read_cat(cat):
            vfs[(prefix + path).lower()] = (dat, offset, size)
    return vfs


def read(vfs, path):
    dat, offset, size = vfs[re.sub(r"[\\/]+", "/", path).lower()]  # some index values double the separators
    with open(dat, "rb") as f:
        f.seek(offset)
        return f.read(size)


def load_index(vfs, name):
    index = {}
    for prefix in [""] + [f"extensions/{d}/" for d in DLCS]:
        key = f"{prefix}index/{name}.xml"
        if key in vfs:
            for e in ET.fromstring(read(vfs, key)).iter("entry"):
                index[e.get("name").lower()] = e.get("value")
    return index


# --- quaternions as (w, x, y, z) ---
def qmul(a, b):
    aw, ax, ay, az = a
    bw, bx, by, bz = b
    return (aw * bw - ax * bx - ay * by - az * bz, aw * bx + ax * bw + ay * bz - az * by,
            aw * by - ax * bz + ay * bw + az * bx, aw * bz + ax * by - ay * bx + az * bw)


def qconj(q):
    return (q[0], -q[1], -q[2], -q[3])


def qrot(q, v):
    return qmul(qmul(q, (0.0, *v)), qconj(q))[1:]


def axis_q(axis, deg):
    h = math.radians(deg) / 2
    v = [0.0, 0.0, 0.0]
    v["XYZ".index(axis)] = math.sin(h)
    return (math.cos(h), *v)


def parse_offset(off):
    """Returns (position, rotation-spec). rotation-spec is ('q', quat) or ('e', yaw, pitch, roll)."""
    if off is None:
        return (0.0, 0.0, 0.0), ("e", 0.0, 0.0, 0.0)
    p = off.find("position")
    pos = tuple(float(p.get(k, 0)) for k in "xyz") if p is not None else (0.0, 0.0, 0.0)
    q = off.find("quaternion")
    if q is not None:
        return pos, ("q", tuple(float(q.get(k, 0)) for k in ("qw", "qx", "qy", "qz")))
    r = off.find("rotation")
    if r is not None:
        return pos, ("e", float(r.get("yaw", 0)), float(r.get("pitch", 0)), float(r.get("roll", 0)))
    return pos, ("e", 0.0, 0.0, 0.0)


def to_quat(spec, conv):
    if spec[0] == "q":
        return spec[1]
    order, signs = conv
    angle = {"Y": spec[1], "X": spec[2], "Z": spec[3]}
    q = (1.0, 0.0, 0.0, 0.0)
    for axis in order:
        q = qmul(q, axis_q(axis, signs["XYZ".index(axis)] * angle[axis]))
    return q


# --- module snap geometry ---
def module_snaps(vfs, macros, components, macro):
    mpath = macros[macro.lower()]
    mroot = ET.fromstring(read(vfs, mpath + ".xml"))
    ref = mroot.find(".//macro/component").get("ref")
    croot = ET.fromstring(read(vfs, components[ref.lower()] + ".xml"))
    snaps = {}
    for c in croot.iter("connection"):
        if "snap" in (c.get("tags") or "").split():
            snaps[c.get("name").lower()] = parse_offset(c.find("offset"))
    return snaps


def main():
    plans_path = sys.argv[1]
    vfs = build_vfs()
    macros, components = load_index(vfs, "macros"), load_index(vfs, "components")
    plans = ET.parse(plans_path).getroot().findall("plan")

    cache, missing = {}, set()
    pairs = []  # (plan name, child entry, pred entry, child snap, pred snap)
    for plan in plans:
        entries = {e.get("index"): e for e in plan.findall("entry")}
        for e in entries.values():
            pred = e.find("predecessor")
            if pred is None:
                continue
            p = entries[pred.get("index")]
            snaps = []
            for ent, conn in ((e, e.get("connection")), (p, pred.get("connection"))):
                m = ent.get("macro")
                if m not in cache:
                    try:
                        cache[m] = module_snaps(vfs, macros, components, m)
                    except KeyError as err:
                        cache[m] = None
                        missing.add(f"{m} ({err})")
                s = cache[m] and cache[m].get(conn.lower())
                snaps.append(s)
            if None in snaps:
                missing.add(f"snap {e.get('macro')}:{e.get('connection')} / {p.get('macro')}:{pred.get('connection')}")
                continue
            pairs.append((plan.get("name"), e, p, snaps[0], snaps[1]))

    print(f"plans: {len(plans)}  linked entries checked: {len(pairs)}  unresolved: {len(missing)}")
    for m in sorted(missing)[:15]:
        print("  unresolved:", m)

    results = []
    for order in itertools.permutations("YXZ"):
        for signs in itertools.product((1, -1), repeat=3):
            conv = (order, signs)
            errs = []
            for _, e, p, sc, sp in pairs:
                pc, rc = parse_offset(e.find("offset"))
                pp, rp = parse_offset(p.find("offset"))
                qc, qp = to_quat(rc, conv), to_quat(rp, conv)
                a, b = qrot(qp, sp[0]), qrot(qc, sc[0])
                pred = tuple(pp[i] + a[i] - b[i] for i in range(3))
                errs.append(math.dist(pred, pc))
            results.append((sum(e < 0.01 for e in errs), max(errs), "".join(order), signs, errs))
    results.sort(key=lambda r: (-r[0], r[1]))
    print("\nbest conventions (matches < 1 cm out of %d):" % len(pairs))
    for ok, worst, order, signs, _ in results[:6]:
        print(f"  order {order} signs(x,y,z) {signs}: {ok} match, worst {worst:.3f} m")

    ok, worst, order, signs, errs = results[0]
    bad = [(err, pl, e, p) for err, (pl, e, p, _, _) in zip(errs, pairs) if err >= 0.01]
    bad.sort(key=lambda b: -b[0])
    print(f"\nmismatches with best convention: {len(bad)}")
    for err, pl, e, p in bad[:10]:
        print(f"  {err:9.3f} m  [{pl}] #{e.get('index')} {e.get('macro')}:{e.get('connection')}"
              f" -> #{p.get('index')} {p.get('macro')}:{e.find('predecessor').get('connection')}")

    # Orientation: child = pred * predsnap * flip * twist(theta about snap Z) * inverse(childsnap)?
    # The residual D = inverse(pred * predsnap * flip) * child * childsnap must be a pure Z rotation.
    conv = (tuple(order), signs)
    for flip_axis in "YX":
        flip = axis_q(flip_axis, 180)
        twists, off_axis = {}, 0
        for (_, e, p, sc, sp), err in zip(pairs, errs):
            if err >= 0.01:
                continue
            qc = to_quat(parse_offset(e.find("offset"))[1], conv)
            qp = to_quat(parse_offset(p.find("offset"))[1], conv)
            qsc, qsp = to_quat(sc[1], conv), to_quat(sp[1], conv)
            d = qmul(qconj(qmul(qmul(qp, qsp), flip)), qmul(qc, qsc))
            if d[0] < 0:
                d = tuple(-x for x in d)
            if math.hypot(d[1], d[2]) > 1e-3:
                off_axis += 1
                continue
            theta = round(math.degrees(2 * math.atan2(d[3], d[0]))) % 360
            twists[theta] = twists.get(theta, 0) + 1
        print(f"\nflip 180 about {flip_axis}: {off_axis} not a pure snap-axis twist;"
              f" twist angles (deg: count) {dict(sorted(twists.items()))}")


if __name__ == "__main__":
    main()
