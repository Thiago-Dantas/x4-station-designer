"""Camera on small designs: screenshots of a big station (TerranScrapyard), an
empty design, one small module and a few modules, after framing, zooming in and
out with the wheel, W, and a double-click zoom to a module.

    python -I research/camera_smoke.py <x4sd.exe> <scratch dir> [prefix]

For each case it prints how much of the viewport the modules cover (pixels
brighter than the background, grid and plot box). Screenshots go to
<scratch dir>/<prefix>-*.png (prefix defaults to "cam").
"""
import os
import sys
import time

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import decode_png, find_color, start_builder  # noqa: E402

exe, out = sys.argv[1], sys.argv[2]
prefix = sys.argv[3] if len(sys.argv) > 3 else "cam"
os.makedirs(out, exist_ok=True)
SNAP = (0x37, 0xD0, 0xC0)
STATUS = "document.getElementById('status').textContent"


def coverage(png, box):
    """Share of viewport pixels that look like a module (bright, not the green plot box or snap cones)."""
    w, h, ch, rows = decode_png(png)
    hit = total = 0
    for y in range(box[1] + 40, min(box[3], h) - 40, 4):
        for x in range(box[0], min(box[2], w), 4):
            r, g, b = rows[y][x * ch:x * ch + 3]
            total += 1
            hit += max(r, g, b) > 90 and not (g > r + 40 and g > b) and not (abs(r - 0x37) < 20 and abs(g - 0xD0) < 20)
    return hit / max(total, 1)


def shot(name):
    time.sleep(1.2)  # damping settles
    png = cdp.screenshot(os.path.join(out, f"{prefix}-{name}.png"))
    print(f"   {name}: modules cover {coverage(png, vp) * 100:.1f}% of the view")
    return png


def arm(name):
    cdp.js("{ const s = document.getElementById('search'); s.value = %r; s.dispatchEvent(new Event('input')) }" % name)
    cdp.js("[...document.querySelectorAll('.module')].find(m => m.textContent.includes(%r)).click()" % name)
    cdp.wait_for("!document.getElementById('armed-panel').hidden")
    time.sleep(0.5)


def wheel(steps, delta=-120):
    cx, cy = (vp[0] + vp[2]) // 2, (vp[1] + vp[3]) // 2
    for _ in range(steps):
        cdp.call("Input.dispatchMouseEvent", type="mouseWheel", x=cx, y=cy, deltaX=0, deltaY=delta)
        time.sleep(0.05)


app, cdp = start_builder(exe)
try:
    cdp.wait_for(STATUS + ".includes('modules loaded')")
    vp = cdp.js("(() => { const r = document.getElementById('viewport').getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom].map(Math.round) })()")
    print("0. a big station (TerranScrapyard)")
    cdp.js("document.getElementById('btn-open-game').click()")
    cdp.wait_for("[...document.querySelectorAll('.plan-row')].length > 0")
    cdp.js("[...document.querySelectorAll('.plan-row')].find(r => r.textContent.startsWith('TerranScrapyard')).click()")
    cdp.wait_for(STATUS + ".includes('Opened')")
    shot("0-big-fit")
    wheel(20)
    shot("0-big-wheel20")
    cdp.hold_key("KeyW", "w", 0.4)
    shot("0-big-wheel20-W")
    wheel(60, 120)
    shot("0-big-out60")
    wheel(100)
    shot("0-big-in100")
    # Double-clicking a build-order row zooms to that module, even right after a zoom stopped on a hull.
    cdp.js("document.getElementById('tab-order').click()")
    cdp.js("document.querySelectorAll('#order-list .order-row')[0].dispatchEvent(new MouseEvent('click', {bubbles: true, detail: 2}))")
    shot("0-big-row1")
    cdp.js("document.getElementById('tab-modules').click()")

    cdp.js("document.getElementById('btn-new').click()")
    cdp.wait_for(STATUS + ".includes('New design')")
    print("1. empty design (plot:", cdp.js("document.getElementById('plot-summary').textContent"), ")")
    shot("1-empty")
    wheel(60, 120)
    shot("1-empty-out60")
    cdp.js("document.getElementById('btn-frame').click()")

    print("2. one small module (Terran beam, placed as first module)")
    arm("Terran Base Connection Structure 02")
    cdp.js("document.getElementById('btn-place-root').click()")
    cdp.wait_for(STATUS + ".startsWith('Placed')")
    cdp.js("window.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape'}))")
    cdp.js("document.querySelectorAll('.plot-grid input').forEach(i => { i.value = 10; i.dispatchEvent(new Event('change')); })")
    cdp.wait_for("document.getElementById('plot-summary').textContent.startsWith('Plot 20×20×20')")
    shot("2-one-placed")
    cdp.js("document.getElementById('btn-frame').click()")
    shot("2-one-fit")
    wheel(15)
    shot("2-one-wheel15")
    wheel(15)
    shot("2-one-wheel30")
    wheel(30)
    shot("2-one-wheel60")
    wheel(40)
    shot("2-one-wheel100")
    cdp.js("document.getElementById('btn-frame').click()")
    wheel(25)
    time.sleep(1.2)
    cdp.hold_key("KeyW", "w", 0.4)
    shot("2-one-wheel25-W")

    print("3. a few modules (beams on free snaps)")
    cdp.js("document.getElementById('btn-frame').click()")
    arm("Terran Cross Connection Structure 01")
    for i in range(3):
        cdp.js("document.getElementById('btn-frame').click()")
        time.sleep(1.0)
        cone = find_color(cdp.screenshot(None), SNAP, vp)
        cdp.click(*cone)
        cdp.wait_for(STATUS + ".startsWith('Attached')")
        cdp.js("document.getElementById('status').textContent = ''")
        if i == 0:
            arm("Terran Base Connection Structure 02")
    cdp.js("window.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape'}))")
    print("   modules:", cdp.js("window.__TAURI_INTERNALS__.invoke('scene').then(s => s.entries.length)"))
    cdp.js("document.getElementById('btn-frame').click()")
    shot("3-few-fit")
    wheel(30)
    shot("3-few-wheel30")

finally:
    app.terminate()
