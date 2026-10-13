"""Live-link smoke test (D-7): an agent edits the design open in the builder.

    python -I research/link_smoke.py <x4sd.exe> <scratch dir>

Starts the builder against a scratch profile (so exports never touch the real
one), runs mcp_smoke.py as the agent while it is open, then checks through the
DevTools protocol that the builder shows the agent's station, and screenshots it.
"""
import os
import subprocess
import sys
import time

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from cdp import start_builder  # noqa: E402

exe, out = sys.argv[1], sys.argv[2]
profile = os.path.join(out, "profile")
os.makedirs(profile, exist_ok=True)

app, cdp = start_builder(exe, "--profile-dir", profile)
try:
    cdp.wait_for("document.getElementById('status').textContent.includes('modules loaded')")
    print("builder up:", cdp.js("document.getElementById('patches').textContent"))

    agent = subprocess.run([sys.executable, "-I", os.path.join(here, "mcp_smoke.py"), exe, profile],
                           capture_output=True, text=True, encoding="utf-8", timeout=120)
    print("agent output:")
    print("\n".join("   " + line for line in agent.stdout.splitlines()))
    if agent.returncode != 0:
        print(agent.stderr)
        raise SystemExit("agent failed")

    time.sleep(1.0)
    shown = cdp.js("document.getElementById('patches').textContent")
    print("builder now shows:", shown, "| name:", cdp.js("document.getElementById('design-name').value"),
          "| status:", cdp.js("document.getElementById('status').textContent"),
          "| checks:", cdp.js("[...document.querySelectorAll('#issues li')].map(l => l.textContent).join('; ')"))
    assert shown.startswith("8 modules"), shown
    cdp.js("document.getElementById('btn-frame').click()")
    time.sleep(1.0)
    cdp.screenshot(os.path.join(out, "link-agent-station.png"))
    print("ok")
finally:
    app.terminate()
