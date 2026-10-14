"""Builder test for choosing the connecting snap (placing and moving):

    python -I research/snap_pick_smoke.py <x4sd.exe> <scratch dir>

1. places a Terran beam, arms the Terran disc: the snap list has direction labels;
   C cycles to its top snap, T twists; hovering a cone shows the see-through preview
2. clicking attaches the disc by that snap with that twist; the data for the
   preview math (`attachMatrix`) and the core's result go to <scratch dir>/snap-pick.json
3. detach, Move…: the disc's own snaps show as orange cones; clicking one picks
   it; the move uses it
4. the placed disc's *Connected by* (and C) changes its snap in place
Screenshots go to <scratch dir>/snap-*.png.
"""
import json
import os
import sys
import time

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import decode_png, find_color, start_builder  # noqa: E402

exe, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)
SNAP = (0x37, 0xD0, 0xC0)
OWN = (0x9A, 0x5A, 0x2A)
OWN_CHOSEN = (0xFF, 0x9A, 0x3D)
STATUS = "document.getElementById('status').textContent"


def arm(name):
    cdp.js("{ const s = document.getElementById('search'); s.value = %r; s.dispatchEvent(new Event('input')) }" % name)
    cdp.js("[...document.querySelectorAll('.module')].find(m => m.textContent.includes(%r)).click()" % name)
    time.sleep(0.8)


def key(k, shift=False):
    cdp.js("window.dispatchEvent(new KeyboardEvent('keydown', {key: %r, shiftKey: %s}))" % (k, "true" if shift else "false"))


def hover(x, y):
    cdp.call("Input.dispatchMouseEvent", type="mouseMoved", x=x, y=y)
    time.sleep(0.5)


def ghost_pixels(png, box):
    """Pixels tinted by the preview (light cyan, see-through): blue well above red."""
    w, h, ch, rows = decode_png(png)
    n = 0
    for y in range(box[1], min(box[3], h), 3):
        for x in range(box[0], min(box[2], w), 3):
            r, g, b = rows[y][x * ch:x * ch + 3]
            n += b > 120 and b - r > 50 and g > 110 and abs(g - b) < 60
    return n


app, cdp = start_builder(exe)
try:
    cdp.wait_for(STATUS + ".includes('modules loaded')")
    cdp.js("document.querySelectorAll('.plot-grid input').forEach(i => { i.value = 10; i.dispatchEvent(new Event('change')); })")
    arm("Terran Base Connection Structure 02")
    cdp.js("document.getElementById('btn-place-root').click()")
    cdp.wait_for(STATUS + ".startsWith('Placed')")
    vp = cdp.js("(() => { const r = document.getElementById('viewport').getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom].map(Math.round) })()")

    # 1. Arm the disc: labelled snaps; C to the top one, T twists once.
    arm("Terran Disc Defence Platform")
    labels = cdp.js("[...document.getElementById('armed-snap').options].map(o => o.text)")
    print("1. disc snaps:", labels)
    assert any(t.startswith("top · ") for t in labels), labels
    for _ in range(len(labels)):
        if cdp.js("document.getElementById('armed-snap').selectedOptions[0].text").startswith("top"):
            break
        key("c")
    top = cdp.js("document.getElementById('armed-snap').value")
    key("t")
    twist = cdp.js("Number(document.getElementById('armed-twist').value)")
    print("   chosen:", cdp.js("document.getElementById('armed-snap').selectedOptions[0].text"), "| twist", twist, "|", cdp.js(STATUS))
    assert twist != 0
    before = cdp.screenshot(os.path.join(out, "snap-1-armed.png"))
    cone = find_color(before, SNAP, vp)
    hover(*cone)
    during = cdp.screenshot(os.path.join(out, "snap-2-preview.png"))
    g0, g1 = ghost_pixels(before, vp), ghost_pixels(during, vp)
    print("   preview pixels before/while hovering:", g0, g1)
    assert g1 > g0 + 200, (g0, g1)

    # 2. Attach: by the chosen snap and twist.
    cdp.click(*cone)
    cdp.wait_for(STATUS + ".startsWith('Attached')")
    s = cdp.js("window.__TAURI_INTERNALS__.invoke('scene')")
    disc = s["entries"][1]
    print("2. attached:", disc["attached"])
    assert disc["attached"]["snap"] == top and round(disc["attached"]["twist"]) == twist
    shapes = {e["macro"]: cdp.js("window.__TAURI_INTERNALS__.invoke('module_shape', { macroName: %r })" % e["macro"]) for e in s["entries"]}
    beam = s["entries"][0]
    json.dump({
        "pred": {"position": beam["position"], "rotation": beam["rotation"]},
        "to": next(x for x in shapes[beam["macro"]]["snaps"] if x["name"] == disc["attached"]["to_snap"]),
        "own": next(x for x in shapes[disc["macro"]]["snaps"] if x["name"] == top),
        "twist": twist,
        "result": {"position": disc["position"], "rotation": disc["rotation"]},
    }, open(os.path.join(out, "snap-pick.json"), "w"))
    time.sleep(0.5)
    cdp.screenshot(os.path.join(out, "snap-3-attached.png"))

    # 3. Detach, Move…: own snaps as orange cones; click one that isn't chosen.
    cdp.js("document.querySelectorAll('#order-list .order-row')[1].click()")
    cdp.js("document.getElementById('btn-detach').click()")
    cdp.wait_for(STATUS + ".startsWith('Detached')")
    cdp.js("document.getElementById('btn-move').click()")
    cdp.wait_for("document.getElementById('armed-title').textContent === 'Moving'")
    time.sleep(1.0)
    png = cdp.screenshot(os.path.join(out, "snap-4-moving.png"))
    # A dim cone away from the bright (chosen) one: its blurred edge has the dim colour too.
    bright = find_color(png, OWN_CHOSEN, vp)
    w, h, ch, rows = decode_png(png)
    dim = [(x, y) for y in range(vp[1], vp[3], 2) for x in range(vp[0], vp[2], 2)
           if all(abs(rows[y][x * ch + k] - OWN[k]) < 12 for k in range(3)) and abs(x - bright[0]) + abs(y - bright[1]) > 150]
    # The biggest blob: textures can hold a few pixels of that colour.
    blob = max(([(x, y) for x, y in dim if abs(x - a) < 40 and abs(y - b) < 40] for a, b in dim), key=len, default=[])
    own = (sum(x for x, _ in blob) // len(blob), sum(y for _, y in blob) // len(blob)) if blob else None
    was = cdp.js("document.getElementById('armed-snap').value")
    print("3. own cone at", own, "| chosen before:", was)
    assert own
    cdp.click(*own)
    time.sleep(0.5)
    picked = cdp.js("document.getElementById('armed-snap').value")
    print("   after clicking it:", picked, "|", cdp.js(STATUS))
    assert picked != was and cdp.js(STATUS).startswith("Connects by")
    png = cdp.screenshot(os.path.join(out, "snap-5-picked.png"))
    target = find_color(png, SNAP, vp)
    hover(*target)
    cdp.screenshot(os.path.join(out, "snap-6-move-preview.png"))
    cdp.click(*target)
    cdp.wait_for(STATUS + ".startsWith('Moved onto')")
    s = cdp.js("window.__TAURI_INTERNALS__.invoke('scene')")
    moved = next(e for e in s["entries"] if e["macro"] == disc["macro"])
    print("   moved:", moved["attached"])
    assert moved["attached"]["snap"] == picked
    time.sleep(0.5)
    cdp.screenshot(os.path.join(out, "snap-7-moved.png"))

    # 4. Placed and selected: change its connecting snap in place (C, then the dropdown).
    assert cdp.js("!document.getElementById('sel-snap-row').hidden && document.getElementById('sel-snap').value") == picked
    key("c")
    cdp.wait_for(STATUS + ".includes('now connects by')")
    s = cdp.js("window.__TAURI_INTERNALS__.invoke('scene')")
    after = next(e for e in s["entries"] if e["macro"] == disc["macro"])["attached"]
    print("4. C:", after, "|", cdp.js(STATUS))
    assert after["snap"] != picked and (after["to"], after["to_snap"]) == (moved["attached"]["to"], moved["attached"]["to_snap"])
    cdp.js("{ const l = document.getElementById('sel-snap'); l.value = %r; l.dispatchEvent(new Event('change')) }" % top)
    cdp.wait_for(STATUS + ".includes('now connects by top')")
    s = cdp.js("window.__TAURI_INTERNALS__.invoke('scene')")
    after = next(e for e in s["entries"] if e["macro"] == disc["macro"])["attached"]
    print("   dropdown:", after)
    assert after["snap"] == top and after["to_snap"] == moved["attached"]["to_snap"]
    assert cdp.js("document.querySelector('#issues li').textContent") == "No problems found"
    time.sleep(0.5)
    cdp.screenshot(os.path.join(out, "snap-8-changed.png"))
    print("OK")
finally:
    app.terminate()
