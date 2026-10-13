"""Textures on vs off in the builder: load time, GPU memory, frame rate, screenshots.

    python -I research/texture_smoke.py <x4sd.exe> <scratch dir> [plan name]

Opens the player's plan (default TerranScrapyard) with textures off, measures
the frame rate, switches textures on, waits for them, measures again.
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import start_builder  # noqa: E402

exe, out = sys.argv[1], sys.argv[2]
plan = sys.argv[3] if len(sys.argv) > 3 else "TerranScrapyard"
os.makedirs(out, exist_ok=True)

FPS = """new Promise(done => { let n = 0; const t0 = performance.now();
  const tick = () => { n++; if (performance.now() - t0 < 2000) requestAnimationFrame(tick);
                       else done(Math.round(n * 1000 / (performance.now() - t0))); };
  requestAnimationFrame(tick); })"""
STATUS = "document.getElementById('status').textContent"

app, cdp = start_builder(exe)
try:
    cdp.wait_for(f"{STATUS}.includes('modules loaded')")
    # The builder remembers the setting for the user too: note it, measure from "off", restore it at the end.
    was_on = cdp.js("document.getElementById('textures').checked")
    cdp.js("{ const t = document.getElementById('textures'); if (t.checked) t.click(); }")
    cdp.js("document.getElementById('btn-open-game').click()")
    cdp.wait_for("[...document.querySelectorAll('.plan-row')].length > 0")
    cdp.js(f"[...document.querySelectorAll('.plan-row')].find(r => r.textContent.startsWith('{plan}')).click()")
    started = time.time()
    cdp.wait_for(f"{STATUS}.includes('Opened')", timeout=60)
    print(f"opened {plan} (models) in {time.time() - started:.1f} s;", cdp.js("document.getElementById('patches').textContent"))
    time.sleep(1)
    print("textures off:", cdp.js(FPS), "fps")
    cdp.screenshot(os.path.join(out, "tex-off.png"))

    cdp.js("document.getElementById('textures').click()")
    cdp.wait_for(f"{STATUS}.startsWith('Textures:')", timeout=120)
    print(cdp.js(STATUS))
    time.sleep(1)
    print("textures on: ", cdp.js(FPS), "fps")
    cdp.screenshot(os.path.join(out, "tex-on.png"))
    if not was_on:
        cdp.js("document.getElementById('textures').click()")
finally:
    app.terminate()
