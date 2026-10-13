"""UI smoke test for the builder: drives the real app through the Chrome
DevTools Protocol that WebView2 exposes, using only the standard library.

    python -I research/ui_smoke.py <x4sd.exe> <scratch dir>

Starts x4sd (builder mode) with remote debugging, then:
  1. waits for the game data, screenshots the empty builder
  2. opens the player's "TerranScrapyard" plan, screenshots it
  3. starts a new design, places a Terran cross as first module, finds a snap
     cone by its colour in a screenshot, attaches a beam by clicking it
  4. checks the scene through the UI's own status and issue list
Screenshots go to <scratch dir>/ui-*.png.
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import find_color, start_builder  # noqa: E402

exe, out = sys.argv[1], sys.argv[2]
os.makedirs(out, exist_ok=True)

app, cdp = start_builder(exe)
try:
    cdp.wait_for("document.getElementById('status').textContent.includes('modules loaded')")
    print("1.", cdp.js("document.getElementById('status').textContent"))
    print("   palette:", cdp.js("document.getElementById('module-count').textContent"))
    # Race filter: Terran + Boron, then All again (the choice is remembered, so end on All).
    chip = "[...document.querySelectorAll('#race-filter button')].find(b => b.textContent === '%s').click()"
    cdp.js(chip % "Terran")
    cdp.js(chip % "Boron")
    two = cdp.js("document.getElementById('module-count').textContent")
    cdp.js(chip % "All")
    print("   races Terran+Boron:", two, "| All:", cdp.js("document.getElementById('module-count').textContent"))
    assert two.startswith("83 of"), two
    cdp.screenshot(os.path.join(out, "ui-1-empty.png"))

    cdp.js("document.getElementById('btn-open-game').click()")
    cdp.wait_for("[...document.querySelectorAll('.plan-row')].length > 0")
    cdp.js("[...document.querySelectorAll('.plan-row')].find(r => r.textContent.startsWith('TerranScrapyard')).click()")
    cdp.wait_for("document.getElementById('status').textContent.includes('Opened')")
    print("2.", cdp.js("document.getElementById('status').textContent"), "|",
          cdp.js("[...document.querySelectorAll('#issues li')].map(l => l.textContent).join('; ')"))
    time.sleep(1.0)
    cdp.screenshot(os.path.join(out, "ui-2-terranscrapyard.png"))
    # Clicking a module selects it (modules are instanced per type; the pick maps instance -> entry).
    centre = cdp.js("(() => { const r = document.getElementById('viewport').getBoundingClientRect(); return [Math.round(r.left + r.width / 2), Math.round(r.top + r.height / 2)] })()")
    cdp.click(*centre)
    cdp.wait_for("!document.getElementById('selection-panel').hidden")
    print("   selected:", cdp.js("document.getElementById('sel-name').textContent"), "|",
          cdp.js("document.getElementById('sel-details').textContent.split(String.fromCharCode(10))[0]"))

    cdp.js("document.getElementById('btn-new').click()")
    cdp.wait_for("document.getElementById('status').textContent.includes('New design')")
    cdp.js("{ const s = document.getElementById('search'); s.value = 'Terran Cross Connection Structure 01'; s.dispatchEvent(new Event('input')) }")
    cdp.js("document.querySelector('.module').click()")
    cdp.wait_for("!document.getElementById('armed-panel').hidden")
    cdp.js("document.getElementById('btn-place-root').click()")
    cdp.wait_for("document.getElementById('status').textContent.startsWith('Placed')")
    print("   plot:", cdp.js("document.getElementById('plot-summary').textContent"))
    # A roomy plot (5 km each side) so no enlarge prompt (a native dialog) can block the test.
    cdp.js("document.querySelectorAll('.plot-grid input').forEach(i => { i.value = 5; i.dispatchEvent(new Event('change')); })")
    cdp.wait_for("document.getElementById('plot-summary').textContent.startsWith('Plot 10×10×10')")
    print("   plot after setting 5 km per side:", cdp.js("document.getElementById('plot-summary').textContent"))
    time.sleep(1.0)
    # Arm a beam and click one of the cross's snap cones.
    cdp.js("{ const s = document.getElementById('search'); s.value = 'Terran Base Connection Structure 02'; s.dispatchEvent(new Event('input')) }")
    cdp.js("document.querySelector('.module').click()")
    time.sleep(1.0)
    png = cdp.screenshot(os.path.join(out, "ui-3-cross.png"))
    vp = cdp.js("(() => { const r = document.getElementById('viewport').getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom].map(Math.round) })()")
    cone = find_color(png, (0x37, 0xD0, 0xC0), vp)
    print("3. snap cone at", cone)
    cdp.click(*cone)
    time.sleep(1.0)
    print("   status after click:", cdp.js("document.getElementById('status').textContent"))
    cdp.wait_for("document.getElementById('status').textContent.startsWith('Attached')")
    print("  ", cdp.js("document.getElementById('status').textContent"), "|",
          cdp.js("[...document.querySelectorAll('#issues li')].map(l => l.textContent).join('; ')"))
    time.sleep(1.0)
    cdp.screenshot(os.path.join(out, "ui-4-attached.png"))
    cdp.js("window.dispatchEvent(new KeyboardEvent('keydown', {key: 'z', ctrlKey: true}))")
    time.sleep(0.5)
    print("4. after Ctrl+Z, undo enabled:", not cdp.js("document.getElementById('btn-undo').disabled"),
          "| redo enabled:", not cdp.js("document.getElementById('btn-redo').disabled"))

    # WASD pans like a right drag: holding D moves the view right (the scene shifts left),
    # holding W moves it up (the scene shifts down).
    time.sleep(0.5)
    before = find_color(cdp.screenshot(None), (0x37, 0xD0, 0xC0), vp)
    cdp.hold_key("KeyD", "d", 0.4)
    time.sleep(0.3)
    after_d = find_color(cdp.screenshot(None), (0x37, 0xD0, 0xC0), vp)
    cdp.hold_key("KeyW", "w", 0.4)
    time.sleep(0.3)
    after_w = find_color(cdp.screenshot(os.path.join(out, "ui-5-panned.png")), (0x37, 0xD0, 0xC0), vp)
    print("5. cone", before, "-> after D", after_d, "-> after W", after_w)
    assert after_d[0] < before[0] - 20, "D should shift the scene left"
    assert after_w[1] > after_d[1] + 20, "W should shift the scene down"
finally:
    app.terminate()
