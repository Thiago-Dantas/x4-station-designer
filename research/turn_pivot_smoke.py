"""Q/E turns after flying in with WASD (user report, 2026-10-15): the view turns
around what is in the middle of it, not around a point far beyond.

    python -I research/turn_pivot_smoke.py <x4sd.exe> <scratch dir>

New design, one Terran cross, Fit view. W then Z fly the camera along its view
direction to near the cross (the orbit target moves along, so it ends up far
beyond the cross). Then E is held for a quarter turn. The cross should stay in
the middle of the view: the share of module pixels in the central box is printed
before and after the turn. Screenshots go to <scratch dir>.
"""
import math
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import decode_png, start_builder  # noqa: E402

exe, out = sys.argv[1], sys.argv[2]
STATUS = "document.getElementById('status').textContent"


def centre_cover(png, box):
    """Share of pixels in the middle fifth of the view that look like a module."""
    w, h, ch, rows = decode_png(png)
    cx, cy, rx, ry = (box[0] + box[2]) // 2, (box[1] + box[3]) // 2, (box[2] - box[0]) // 10, (box[3] - box[1]) // 10
    hit = total = 0
    for y in range(cy - ry, cy + ry, 2):
        for x in range(cx - rx, cx + rx, 2):
            r, g, b = rows[y][x * ch:x * ch + 3]
            total += 1
            hit += max(r, g, b) > 90 and not (abs(r - 0x37) < 20 and abs(g - 0xD0) < 20)
    return hit / total


def hold(code, key, seconds):
    vk = ord(key.upper())
    cdp.call("Input.dispatchKeyEvent", type="keyDown", code=code, key=key, windowsVirtualKeyCode=vk)
    time.sleep(seconds)
    cdp.call("Input.dispatchKeyEvent", type="keyUp", code=code, key=key, windowsVirtualKeyCode=vk)


def shot(name):
    time.sleep(1.2)  # damping settles
    png = cdp.screenshot(os.path.join(out, f"turn-{name}.png"))
    c = centre_cover(png, vp)
    print(f"   {name}: module in {c * 100:.0f}% of the central box")
    return c


app, cdp = start_builder(exe)
try:
    cdp.wait_for(STATUS + ".includes('modules loaded')")
    vp = cdp.js("(() => { const r = document.getElementById('viewport').getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom].map(Math.round) })()")
    cdp.js("document.getElementById('btn-new').click()")
    cdp.wait_for(STATUS + ".includes('New design')")
    cdp.js("{ const s = document.getElementById('search'); s.value = 'Terran Cross Connection Structure 01'; s.dispatchEvent(new Event('input')) }")
    cdp.js("document.querySelector('.module').click()")
    cdp.wait_for("!document.getElementById('armed-panel').hidden")
    cdp.js("document.getElementById('btn-place-root').click()")
    cdp.wait_for(STATUS + ".startsWith('Placed')")
    cdp.js("window.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape'}))")
    cdp.js("document.getElementById('btn-frame').click()")
    fit = shot("0-fit")
    # The default view looks down 3000 over 5657 m level: W and Z in that ratio fly along it, 0.75 of the way.
    pitch = 3000 / math.hypot(4000, 4000)
    t = 0.75 / (0.8 * math.hypot(1, pitch))
    hold("KeyW", "w", t)
    hold("KeyZ", "z", t * pitch)
    near = shot("1-flown-in")
    hold("KeyE", "e", 1.0)
    turned = shot("2-turned-E")
    hold("KeyQ", "q", 1.0)
    back = shot("3-turned-back-Q")
    assert near > 0.5, "flying in should keep the cross in the middle"
    assert turned > 0.5 and back > 0.5, (near, turned, back)
    print("OK")
finally:
    app.kill()
