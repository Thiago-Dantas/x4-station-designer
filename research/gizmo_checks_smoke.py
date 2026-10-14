"""Live checks during a gizmo drag (user request, 2026-10-14).

    python -I research/gizmo_checks_smoke.py <x4sd.exe> <scratch dir>

New design, a Terran cross typed to X = 1500 m in the default 4 km plot; grabs
the gizmo's red X arrow, drags the module out of the plot and reads the checks
with the button still held (plot box red, "outside the plot", design unchanged),
drags back inside (clean again), releases. Screenshots go to <scratch dir>.
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import find_color, start_builder  # noqa: E402

exe, out = sys.argv[1], sys.argv[2]
app, cdp = start_builder(exe)
CHECKS = "[document.getElementById('issue-count').textContent, [...document.querySelectorAll('#issues li')].map(l => l.textContent).join(' | '), document.getElementById('plot-summary').className]"
POS = "window.__TAURI_INTERNALS__.invoke('scene').then(s => s.entries[0].position.map(Math.round))"


def mouse(kind, x, y, buttons=0):
    cdp.call("Input.dispatchMouseEvent", type=kind, x=x, y=y, button="left" if kind != "mouseMoved" or buttons else "none", buttons=buttons, clickCount=1)


try:
    cdp.wait_for("document.getElementById('status').textContent.includes('modules loaded')")
    cdp.js("document.getElementById('btn-new').click()")
    cdp.wait_for("document.getElementById('status').textContent.includes('New design')")
    cdp.js("{ const s = document.getElementById('search'); s.value = 'Terran Cross Connection Structure 01'; s.dispatchEvent(new Event('input')) }")
    cdp.js("document.querySelector('.module').click()")
    cdp.wait_for("!document.getElementById('armed-panel').hidden")
    cdp.js("document.getElementById('btn-place-root').click()")
    cdp.wait_for("document.getElementById('status').textContent.startsWith('Placed')")
    cdp.js("document.querySelector('#checks-panel > h3').click()")  # open Checks
    cdp.js("{ const i = document.querySelector('#sel-position input'); i.value = 1500; i.dispatchEvent(new Event('change')); }")
    cdp.wait_for("document.getElementById('status').textContent.startsWith('Moved')")
    time.sleep(1.0)
    print("start:", cdp.js(POS), cdp.js(CHECKS))
    png = cdp.screenshot(os.path.join(out, "live-0.png"))
    vp = cdp.js("(() => { const r = document.getElementById('viewport').getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom].map(Math.round) })()")
    arrow = find_color(png, (0xFF, 0x00, 0x00), vp)
    print("X arrow at", arrow)
    x, y = arrow
    mouse("mouseMoved", x, y)
    time.sleep(0.2)
    mouse("mousePressed", x, y, 1)
    for i in range(1, 21):
        mouse("mouseMoved", x + 15 * i, y + 8 * i, 1)
        time.sleep(0.05)
    time.sleep(1.0)
    mid = cdp.js(CHECKS)
    design_mid = cdp.js(POS)
    print("mid-drag (button held):", mid, "| design still at", design_mid)
    cdp.screenshot(os.path.join(out, "live-1-middrag.png"))
    # Back inside before letting go: the checks turn clean again while still dragging.
    for i in range(19, -1, -1):
        mouse("mouseMoved", x + 15 * i, y + 8 * i, 1)
        time.sleep(0.05)
    time.sleep(1.0)
    back = cdp.js(CHECKS)
    print("dragged back (held):", back)
    mouse("mouseReleased", x, y, 0)
    time.sleep(1.0)
    print("released:", cdp.js(POS), cdp.js(CHECKS), "| status:", cdp.js("document.getElementById('status').textContent"))
    assert "outside the plot" in mid[1] and mid[2].endswith("bad"), mid
    assert design_mid == [1500, 0, 0], design_mid
    assert back[0] == "" and "No problems" in back[1], back
    print("OK")
finally:
    app.kill()
