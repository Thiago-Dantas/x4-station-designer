"""Records the README GIFs (docs/media/*.gif): plays scripted scenes in the real
builder over DevTools and turns the frames into GIFs with gifski.

    python -I research/record_gifs.py <x4sd.exe> <out dir> [scene ...] [--gifski PATH]

Scenes: build, planner, agent, analysis, optimize, spire (default: all). Each starts a fresh
builder (no unsaved-changes dialog can block it) with the page fixed at
1440x810, and writes <out dir>/<scene>.gif. A second DevTools connection grabs
JPEG frames in a thread while the scene runs. Input is synthetic, so a drawn
cursor follows the mouse events and rings on clicks.
"""
import base64
import glob
import io
import json
import os
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import urllib.request

sys.stdout.reconfigure(encoding="utf-8", errors="replace")
here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from cdp import PORT, Cdp, link_port_in_use, start_builder  # noqa: E402
import numpy as np  # noqa: E402
from PIL import Image  # noqa: E402

W, H = 1440, 810
FPS = 12
GIF_WIDTH = 1080
CONE = (0x37, 0xD0, 0xC0)  # free-snap cone colour (ui_smoke.py)

CURSOR_JS = r"""(() => {
  if (document.getElementById('rec-cursor')) return;
  const style = document.createElement('style');
  style.textContent = `
    #rec-cursor { position: fixed; left: 0; top: 0; z-index: 2147483647; pointer-events: none; width: 22px; height: 30px; }
    .rec-ring { position: fixed; z-index: 2147483646; pointer-events: none; width: 36px; height: 36px; margin: -18px 0 0 -18px;
                border: 3px solid #ffd54a; border-radius: 50%; animation: rec-ring .45s ease-out forwards; }
    @keyframes rec-ring { from { transform: scale(.3); opacity: 1 } to { transform: scale(1.3); opacity: 0 } }
    #rec-caption { position: fixed; left: 50%; bottom: 42px; transform: translateX(-50%); z-index: 2147483645; pointer-events: none;
                   background: rgba(10, 14, 20, .82); color: #fff; font: 600 18px system-ui, sans-serif; padding: 8px 16px; border-radius: 6px; }
    #rec-log { position: fixed; z-index: 2147483645; pointer-events: none; background: rgba(8, 10, 14, .9); color: #9fe7a4;
               font: 15px Consolas, monospace; padding: 8px 10px; border-radius: 6px; border: 1px solid #2c3a2e; white-space: pre; }
    #rec-log .t { color: #8aa; } #rec-log .r { color: #ddd; }`;
  document.head.appendChild(style);
  const c = document.createElement('div');
  c.id = 'rec-cursor';
  c.innerHTML = '<svg width="22" height="30" viewBox="0 0 22 30"><path d="M1 1 L1 23 L7 18 L11 28 L15 26 L11 17 L19 17 Z" fill="#fff" stroke="#000" stroke-width="1.6" stroke-linejoin="round"/></svg>';
  document.body.appendChild(c);
  const move = e => c.style.transform = `translate(${e.clientX - 1}px, ${e.clientY - 1}px)`;
  window.addEventListener('pointermove', move, true);
  window.addEventListener('pointerdown', e => {
    move(e);
    const r = document.createElement('div');
    r.className = 'rec-ring';
    r.style.left = e.clientX + 'px';
    r.style.top = e.clientY + 'px';
    document.body.appendChild(r);
    setTimeout(() => r.remove(), 500);
  }, true);
})()"""


class Recorder:
    """Grabs JPEG frames of the page on its own DevTools connection."""

    def __init__(self):
        targets = json.load(urllib.request.urlopen(f"http://127.0.0.1:{PORT}/json"))
        page = next(t for t in targets if t.get("type") == "page")
        self.cdp = Cdp(page["webSocketDebuggerUrl"])
        self.frames, self.running, self.paused, self.skipped = [], False, False, 0.0

    def _run(self):
        while self.running:
            data = self.cdp.call("Page.captureScreenshot", format="jpeg", quality=90)["data"]
            if self.paused:  # cut out of the GIF (a long wait), with no jump in the timing
                now = time.time()
                self.skipped += now - self.paused_at
                self.paused_at = now
                continue
            self.frames.append((time.time() - self.skipped, base64.b64decode(data)))

    def pause(self, on):
        self.paused_at = time.time()
        self.paused = on

    def start(self):
        self.running = True
        self.thread = threading.Thread(target=self._run, daemon=True)
        self.thread.start()

    def stop(self):
        self.running = False
        self.thread.join()
        return self.frames


def encode(frames, gif, gifski, fps=FPS, width=GIF_WIDTH, quality=80, speed=1.0):
    """Resamples the timed frames to a steady fps (played `speed` times as fast) and writes the GIF."""
    start, end = frames[0][0], frames[-1][0]
    tmp = tempfile.mkdtemp(prefix="x4sd-gif-")
    try:
        i, n, t = 0, 0, start
        while t <= end:
            while i + 1 < len(frames) and frames[i + 1][0] <= t:
                i += 1
            Image.open(io.BytesIO(frames[i][1])).convert("RGB").save(os.path.join(tmp, f"f{n:05d}.png"))
            n, t = n + 1, t + speed / fps
        subprocess.run([gifski, "--fps", str(fps), "--width", str(width), "--quality", str(quality),
                        "-o", os.path.abspath(gif), *sorted(glob.glob("f*.png", root_dir=tmp))],
                       check=True, capture_output=True, cwd=tmp)  # relative names: Windows caps the command line
    finally:
        shutil.rmtree(tmp)
    print(f"   {os.path.basename(gif)}: {n} frames, {n / fps:.1f} s, {os.path.getsize(gif) / 1e6:.1f} MB "
          f"(captured {len(frames)} at {len(frames) / max(end - start, 1e-6):.1f} fps)")


class Scene:
    """Helpers that act like a person: a moving cursor, real clicks and typing."""

    def __init__(self, cdp):
        self.cdp = cdp
        self.x, self.y = W / 2, H / 2

    def js(self, expression):
        return self.cdp.js(expression)

    def wait(self, expression, timeout=30):
        self.cdp.wait_for(expression, timeout)

    def mouse(self, kind, x, y, buttons=0, button="none"):
        self.cdp.call("Input.dispatchMouseEvent", type=kind, x=x, y=y, button=button, buttons=buttons, clickCount=1)

    def move(self, x, y, seconds=0.5, buttons=0):
        steps = max(2, int(seconds * 40))
        x0, y0 = self.x, self.y
        for k in range(1, steps + 1):
            s = k / steps
            s = s * s * (3 - 2 * s)  # ease in and out
            self.x, self.y = x0 + (x - x0) * s, y0 + (y - y0) * s
            self.mouse("mouseMoved", self.x, self.y, buttons, "left" if buttons else "none")
            time.sleep(seconds / steps)

    def click(self, x, y, seconds=0.5, pause=0.15):
        self.move(x, y, seconds)
        time.sleep(pause)
        self.mouse("mousePressed", x, y, 1, "left")
        self.mouse("mouseReleased", x, y, 0, "left")

    def centre(self, selector):
        return self.js("(() => { const r = document.querySelector(%r).getBoundingClientRect(); return [r.left + r.width / 2, r.top + r.height / 2] })()" % selector)

    def click_el(self, selector, seconds=0.5):
        self.js("document.querySelector(%r).scrollIntoView({block: 'nearest'})" % selector)
        self.click(*self.centre(selector), seconds)

    def type(self, text, delay=0.04):
        for ch in text:
            self.cdp.call("Input.insertText", text=ch)
            time.sleep(delay)

    def key(self, key, code=None, vk=None):
        code = code or "Key" + key.upper()
        vk = vk or ord(key.upper())
        self.cdp.call("Input.dispatchKeyEvent", type="keyDown", code=code, key=key, windowsVirtualKeyCode=vk)
        self.cdp.call("Input.dispatchKeyEvent", type="keyUp", code=code, key=key, windowsVirtualKeyCode=vk)

    def drag(self, x0, y0, x1, y1, seconds=1.5, button="left"):
        """Holds a mouse button from (x0, y0) to (x1, y1): left orbits, right pans."""
        mask = {"left": 1, "right": 2}[button]
        self.move(x0, y0, 0.3)
        self.cdp.call("Input.dispatchMouseEvent", type="mousePressed", x=x0, y=y0, button=button, buttons=mask, clickCount=1)
        steps = int(seconds * 40)
        for k in range(1, steps + 1):
            self.x, self.y = x0 + (x1 - x0) * k / steps, y0 + (y1 - y0) * k / steps
            self.cdp.call("Input.dispatchMouseEvent", type="mouseMoved", x=self.x, y=self.y, button=button, buttons=mask)
            time.sleep(seconds / steps)
        self.cdp.call("Input.dispatchMouseEvent", type="mouseReleased", x=x1, y=y1, button=button, buttons=0, clickCount=1)

    def viewport(self):
        return self.js("(() => { const r = document.getElementById('viewport').getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom].map(Math.round) })()")

    def cones(self):
        """Screen centres of the free snap cones: each cone is one connected blob of its
        colour (zoomed out, several cones sit close together, so a radius would merge them)."""
        vp = self.viewport()
        im = Image.open(io.BytesIO(self.cdp.screenshot(None))).convert("RGB").crop(vp)
        w, h = im.size
        px = im.load()
        mask = {(x, y) for y in range(h) for x in range(w) if all(abs(px[x, y][k] - CONE[k]) < 30 for k in range(3))}
        blobs = []
        while mask:
            todo, blob = [mask.pop()], []
            while todo:
                x, y = todo.pop()
                blob.append((x, y))
                for n in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                    if n in mask:
                        mask.remove(n)
                        todo.append(n)
            if len(blob) >= 6:
                blobs.append((vp[0] + sum(x for x, _ in blob) / len(blob), vp[1] + sum(y for _, y in blob) / len(blob)))
        return blobs

    def cone(self):
        """The free snap cone nearest the view's centre."""
        vp = self.viewport()
        cx, cy = (vp[0] + vp[2]) / 2, (vp[1] + vp[3]) / 2
        return min(self.cones(), key=lambda b: (b[0] - cx) ** 2 + (b[1] - cy) ** 2)

    def zoom(self, notches):
        """Mouse wheel at the view's middle: positive zooms out."""
        vp = self.viewport()
        x, y = (vp[0] + vp[2]) / 2, (vp[1] + vp[3]) / 2
        for _ in range(abs(notches)):
            self.cdp.call("Input.dispatchMouseEvent", type="mouseWheel", x=x, y=y, deltaX=0, deltaY=100 if notches > 0 else -100)
            time.sleep(0.12)

    def caption(self, text):
        self.js("""(() => { let c = document.getElementById('rec-caption');
          if (!%(t)r) { c?.remove(); return; }
          if (!c) { c = document.createElement('div'); c.id = 'rec-caption'; document.body.appendChild(c); }
          c.textContent = %(t)r; })()""" % {"t": text})


    def click_module(self, search, name):
        """Types into the palette search and arms the module called `name`."""
        self.click_el("#search")
        self.js("document.getElementById('search').select()")
        self.type(search)
        time.sleep(0.4)
        row = "[...document.querySelectorAll('#module-list .module')].find(m => m.textContent.startsWith(%r))" % name
        self.js(row + ".scrollIntoView({block: 'nearest'})")
        self.click(*self.js("(() => { const r = %s.getBoundingClientRect(); return [r.left + 90, r.top + r.height / 2] })()" % row))
        self.wait("document.getElementById('armed-name').textContent === %r" % name)

    def attach_at_cone(self, hover=0.8):
        """Moves onto a free snap cone, lets the preview show, clicks it."""
        time.sleep(0.3)
        x, y = self.cone()
        self.move(x, y, 0.6)
        time.sleep(hover)
        count = "window.__TAURI_INTERNALS__.invoke('scene').then(s => s.entries.length)"
        before = self.js(count)
        self.click(x, y, 0.1)
        self.wait("%s.then(n => n > %d)" % (count, before))

    def open_section(self, panel):
        """Opens a collapsed right-pane section (e.g. 'economy-panel')."""
        self.click_el("#%s > h3" % panel)
        self.wait("!document.getElementById(%r).classList.contains('collapsed')" % panel)

    def set_select(self, selector, value):
        """Native drop-downs don't show in screenshots: point at it, then set the value."""
        x, y = self.centre(selector)
        self.move(x, y, 0.5)
        # A JS pointerdown only rings the drawn cursor; a real click would open the native list.
        self.js("document.querySelector(%r).dispatchEvent(new PointerEvent('pointerdown', {bubbles: true, clientX: %d, clientY: %d}))" % (selector, x, y))
        self.js("{ const s = document.querySelector(%r); s.value = %r; s.dispatchEvent(new Event('change')); }" % (selector, value))

    def orbit(self, dx=260, seconds=2.5):
        vp = self.viewport()
        cx, cy = (vp[0] + vp[2]) / 2, (vp[1] + vp[3]) / 2
        self.drag(cx - dx / 2, cy + 40, cx + dx / 2, cy + 10, seconds)

    def log(self, line):
        """Agent-scene log box in the view's top-left corner (newest line last)."""
        self.js("""(() => { let l = document.getElementById('rec-log');
          if (!l) { l = document.createElement('div'); l.id = 'rec-log'; document.body.appendChild(l);
            const r = document.getElementById('viewport').getBoundingClientRect();
            l.style.left = (r.left + 10) + 'px'; l.style.top = (r.top + 10) + 'px'; }
          const d = document.createElement('div'); d.innerHTML = %r; l.appendChild(d);
          while (l.children.length > 9) l.firstChild.remove(); })()""" % line)


def status_starts(prefix):
    return "document.getElementById('status').textContent.startsWith(%r)" % prefix


def new_design(cdp, plot_km=None):
    """Setup step (not recorded): an empty design, optionally with a big plot so no
    enlarge prompt (a native dialog CDP can't click) blocks Place all."""
    cdp.js("document.getElementById('btn-new').click()")
    cdp.wait_for(status_starts("New design"))
    if plot_km:
        cdp.js("document.querySelectorAll('.plot-grid input').forEach(i => { i.value = %d; i.dispatchEvent(new Event('change')); })" % plot_km)
        cdp.wait_for("document.getElementById('plot-summary').textContent.startsWith('Plot %d×')" % (2 * plot_km))


def clear_planner(cdp):
    """Setup step: the planner remembers its targets and extras (shown once its tab opens); empty them."""
    cdp.js("document.getElementById('tab-planner').click()")
    time.sleep(1.0)
    left = "document.querySelectorAll('#planner-targets button, #planner-extras button')"
    for _ in range(20):
        if not cdp.js(left + ".length"):
            break
        cdp.js(left + "[0].click()")
        time.sleep(0.3)
    cdp.js("document.getElementById('tab-modules').click()")


def scene_build(s):
    """Snap-building: a hub, then modules attached at free snaps (hover previews, click attaches)."""
    s.click_module("cross", "Terran Cross Connection Structure 01")
    s.click_el("#btn-place-root")
    s.wait(status_starts("Placed"))
    time.sleep(0.8)
    s.click_module("energy cell", "Terran Energy Cell Production")
    s.attach_at_cone(1.0)
    time.sleep(0.4)
    s.attach_at_cone(0.5)
    time.sleep(0.4)
    s.click_module("living quarters", "Terran M Living Quarters")
    s.attach_at_cone(0.6)
    time.sleep(0.4)
    s.click_module("luxury dock", "Terran 4M10S Luxury Dock Area")
    s.attach_at_cone(0.6)
    time.sleep(0.4)
    s.key("Escape", "Escape", 27)
    s.key("f")
    time.sleep(1.0)
    s.orbit(300, 3)
    time.sleep(0.8)


def scene_planner(s):
    """Production planner: a target ware to a module list, Place all lays the station out."""
    s.click_el("#tab-planner")
    time.sleep(0.4)
    s.click_el("#planner-ware")
    s.type("hull par")
    s.wait("!document.getElementById('planner-ware-list').hidden")
    time.sleep(0.6)
    s.key("Enter", "Enter", 13)
    s.wait("document.querySelectorAll('#planner-targets li').length === 1")
    time.sleep(0.4)
    s.set_select("#planner-race", "terran")
    time.sleep(0.6)
    s.click_el("#planner-generate")
    s.wait("/^\\d+ modules to add/.test(document.getElementById('planner-summary').textContent)")
    time.sleep(2.5)
    s.click_el("#planner-place")
    s.wait(status_starts("Placed "), 90)
    s.key("f")
    time.sleep(1.5)
    s.open_section("economy-panel")
    time.sleep(1.0)
    s.orbit(300, 3.5)
    time.sleep(1.5)


AGENT_CALLS = [
    ("new_design", {"name": "Agent outpost"}),
    ("add_module", {"macro": "struct_ter_cross_01_macro"}),
    ("attach_module", {"macro": "struct_ter_base_02_macro", "to": 1, "to_snap": "{snap0}"}),
    ("attach_module", {"macro": "struct_ter_base_02_macro", "to": 1, "to_snap": "{snap1}", "twist": 90}),
    ("attach_module", {"macro": "hab_ter_l_01_macro", "to": 2, "to_snap": "{end2}"}),
    ("auto_place", {"from_plan": {"targets": [{"ware": "computronicsubstrate", "modules": 1}], "race": "terran"}}),
    ("fit_plot", {"grow_only": True}),
    ("validate_design", {}),
]


def scene_agent(s, exe, profile):
    """An agent drives `x4sd mcp`; the open builder shows each edit (live link, D-7)."""
    proc = subprocess.Popen([exe, "--profile-dir", profile, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE,
                            text=True, encoding="utf-8")
    ids = iter(range(1, 1000))

    def send(method, params=None, notify=False):
        msg = {"jsonrpc": "2.0", "method": method, **({"params": params} if params is not None else {})}
        if not notify:
            msg["id"] = next(ids)
        proc.stdin.write(json.dumps(msg) + "\n")
        proc.stdin.flush()
        while not notify:
            reply = json.loads(proc.stdout.readline())
            if reply.get("id") == msg["id"]:
                return reply

    def call(tool, args):
        result = send("tools/call", {"name": tool, "arguments": args})["result"]
        assert not result.get("isError"), result
        return json.loads(result["content"][0]["text"])

    try:
        send("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "gif", "version": "0"}})
        send("notifications/initialized", notify=True)
        s.caption("An AI agent builds through MCP, live in the open builder")
        s.log('<span class="t">agent ⇄ x4sd mcp</span>')
        time.sleep(1.5)
        snaps = {}
        for tool, args in AGENT_CALLS:
            args = json.loads(json.dumps(args).replace('"{snap0}"', '"%s"' % snaps.get("snap0", ""))
                              .replace('"{snap1}"', '"%s"' % snaps.get("snap1", "")).replace('"{end2}"', '"%s"' % snaps.get("end2", "")))
            shown = ", ".join(f"{k}={json.dumps(v) if not isinstance(v, dict) else '…'}" for k, v in args.items())
            s.log(f"→ {tool}({shown})".replace("<", "&lt;"))
            r = call(tool, args)
            if tool == "add_module":
                arms = call("free_snaps", {"index": r["index"]})
                snaps["snap0"], snaps["snap1"] = arms[0]["name"], arms[1]["name"]
            elif tool == "attach_module" and "end2" not in snaps:
                snaps["end2"] = call("free_snaps", {"index": 2})[0]["name"]
            if tool == "auto_place":
                s.log(f'<span class="r">  ✓ {len(r["placed"])} modules placed</span>')
            elif tool == "validate_design":
                s.log('<span class="r">  ✓ %s</span>' % ("no problems" if not r["issues"] else f'{len(r["issues"])} issues'))
            time.sleep(0.4)
            s.key("f")
            time.sleep(1.4)
        s.caption("")
        time.sleep(0.5)
        s.orbit(300, 3)
        time.sleep(1.0)
    finally:
        proc.stdin.close()
        proc.wait(10)


def scene_analysis(s):
    """A saved game plan: textured models, production, cost and load readouts, build order timeline."""
    s.click_el("#btn-open-game")
    s.wait("[...document.querySelectorAll('.plan-row')].length > 0")
    time.sleep(0.6)
    row = "[...document.querySelectorAll('.plan-row')].find(r => r.textContent.startsWith('TerranScrapyard'))"
    s.click(*s.js("(() => { const r = %s.getBoundingClientRect(); return [r.left + 80, r.top + r.height / 2] })()" % row))
    s.wait(status_starts("Opened"))
    time.sleep(2.5)  # textures stream in
    s.orbit(260, 3)
    for panel in ("economy-panel", "cost-panel", "load-panel"):
        s.open_section(panel)
        time.sleep(1.4)
        s.click_el("#%s > h3" % panel)
    s.click_el("#tab-order")
    time.sleep(0.4)
    # Synthetic drags don't move a range input: set its value step by step, with the cursor on the thumb.
    left, right, y, top = s.js("(() => { const e = document.getElementById('build-step'), r = e.getBoundingClientRect(); return [r.left + 8, r.right - 8, r.top + r.height / 2, Number(e.max)] })()")
    step = "{ const e = document.getElementById('build-step'); e.value = %d; e.dispatchEvent(new Event('input')); }"
    s.move(left, y, 0.5)
    s.mouse("mousePressed", left, y, 1, "left")
    s.js(step % 0)
    time.sleep(0.6)
    for k in range(1, top + 1):
        s.x = left + (right - left) * k / top
        s.mouse("mouseMoved", s.x, y, 1, "left")
        s.js(step % k)
        time.sleep(5.0 / top)
    s.mouse("mouseReleased", s.x, y, 0, "left")
    time.sleep(1.5)


def setup_optimize(cdp):
    """Planner without targets (Optimize plans the shortfall fixes), both Optimize boxes unticked."""
    clear_planner(cdp)
    cdp.js("['optimize-rebuild', 'planner-fewer'].forEach(id => { const c = document.getElementById(id); if (c.checked) c.click(); })")


def scene_optimize(s):
    """Optimize on the player's Terran megashipyard: rebuild connection structures, fewer of them."""
    s.click_el("#btn-open-game")
    s.wait("[...document.querySelectorAll('.plan-row')].length > 0")
    time.sleep(0.6)
    row = "[...document.querySelectorAll('.plan-row')].find(r => r.textContent.startsWith('megashipyard terran') && r.textContent.includes('saved'))"
    s.click(*s.js("(() => { const r = %s.getBoundingClientRect(); return [r.left + 80, r.top + r.height / 2] })()" % row))
    s.wait(status_starts("Opened"), 60)
    time.sleep(2.5)  # textures stream in
    s.open_section("load-panel")
    time.sleep(0.6)
    s.orbit(220, 2.5)
    s.click_el("#tab-planner")
    time.sleep(0.4)
    s.click_el("#optimize-rebuild")
    time.sleep(0.3)
    s.click_el("#planner-fewer")
    time.sleep(0.4)
    s.click_el("#planner-optimize")
    s.wait("!document.getElementById('planner-apply').hidden", 120)
    time.sleep(2.5)
    s.click_el("#planner-apply")
    time.sleep(1.0)
    started = time.time()
    s.rec.pause(True)
    s.wait(status_starts("Optimized"), 300)
    s.rec.pause(False)
    print(f"   optimize took {time.time() - started:.1f} s (cut from the GIF):", s.js("document.getElementById('status').textContent"))
    time.sleep(2.5)
    s.orbit(300, 3.5)
    time.sleep(1.5)


ENTRIES = "window.__TAURI_INTERNALS__.invoke('scene').then(s => s.entries)"


def probe_cones(s, accept, order, first=False):
    """Which free snap each cone on screen is: the view doesn't say, so each cone (in
    `order`) is tried with a click and undone, with the recorder paused. Returns
    [((x, y), (to, to_snap))] for the snaps `accept(to, to_snap)` likes (only the
    first one when `first`); the cursor is back where it was."""
    time.sleep(0.3)
    vp = s.viewport()
    count = len(s.js(ENTRIES))
    s.rec.pause(True)
    found = []
    for x, y in sorted(s.cones(), key=lambda c: order(c, vp)):
        s.mouse("mouseMoved", x, y)
        s.mouse("mousePressed", x, y, 1, "left")
        s.mouse("mouseReleased", x, y, 0, "left")
        try:
            s.wait("%s.then(e => e.length > %d)" % (ENTRIES, count), 3)
        except Exception:
            continue  # missed the cone
        a = s.js(ENTRIES)[-1]["attached"]
        s.cdp.call("Input.dispatchKeyEvent", type="keyDown", key="z", code="KeyZ", windowsVirtualKeyCode=90, modifiers=2)
        s.cdp.call("Input.dispatchKeyEvent", type="keyUp", key="z", code="KeyZ", windowsVirtualKeyCode=90, modifiers=2)
        s.wait("%s.then(e => e.length === %d)" % (ENTRIES, count))
        if accept(a["to"], a["to_snap"]):
            found.append(((x, y), (a["to"], a["to_snap"])))
            if first:
                break
    s.mouse("mouseMoved", s.x, s.y)
    time.sleep(0.6)  # the click rings fade
    s.rec.pause(False)
    return found


def cone_point(s, index, snap):
    """Where a free snap's cone shows, in the game's frame: 40 m out from the snap (the
    cone's centroid, view3d's snapGeometry)."""
    e = next(e for e in s.js(ENTRIES) if e["index"] == index)
    sn = next(x for x in s.js("window.__TAURI_INTERNALS__.invoke('module_shape', { macroName: %r })" % e["macro"])["snaps"] if x["name"] == snap)
    w, x, y, z = e["rotation"]
    r = np.array([[1 - 2 * (y * y + z * z), 2 * (x * y - w * z), 2 * (x * z + w * y)],
                  [2 * (x * y + w * z), 1 - 2 * (x * x + z * z), 2 * (y * z - w * x)],
                  [2 * (x * z - w * y), 2 * (y * z + w * x), 1 - 2 * (x * x + y * y)]])
    facing = np.array(sn["facing"], float)
    return np.array(e["position"]) + r @ (np.array(sn["position"]) + 40 * facing / np.linalg.norm(facing))


def fit_camera(world, screen):
    """Projective camera fitted (DLT) to points seen on screen; returns world point -> (x, y)."""
    rows = []
    for k, (X, Y, Z) in world.items():
        u, v = screen[k]
        rows.append([X, Y, Z, 1, 0, 0, 0, 0, -u * X, -u * Y, -u * Z, -u])
        rows.append([0, 0, 0, 0, X, Y, Z, 1, -v * X, -v * Y, -v * Z, -v])
    P = np.linalg.svd(np.array(rows))[2][-1].reshape(3, 4)

    def project(p):
        h = P @ np.append(p, 1)
        return h[0] / h[2], h[1] / h[2]
    worst = max(np.hypot(*np.subtract(project(world[k]), screen[k])) for k in world)
    print(f"   camera fitted to {len(world)} cones, worst miss {worst:.1f} px")
    return project


def scene_spire(s):
    """A spire on a 20 km plot: six Terran cross connection structures stacked, each by its
    bottom snap and twisted 15° from the one below, then Terran L Living Quarters on every
    side snap of the crosses (four stairs)."""
    s.caption("Six cross connection structures, each twisted 15°")
    s.click_module("cross", "Terran Cross Connection Structure 01")
    s.click_el("#btn-place-root")
    s.wait(status_starts("Placed"))
    s.key("f")
    time.sleep(0.4)
    s.click_module("cross", "Terran Cross Connection Structure 01")  # placing the first one disarmed it
    s.set_select("#armed-panel .rot-step", "15")
    time.sleep(0.3)
    for _ in range(5):
        top = s.js(ENTRIES)[-1]["index"]
        found = probe_cones(s, lambda to, snap: to == top and snap == "connectionsnap006", lambda c, vp: c[1], first=True)
        assert found, "the top snap isn't on screen"
        (x, y), _ = found[0]
        s.move(x, y, 0.35)
        time.sleep(0.3)
        for _ in range(6):  # connect by its bottom snap
            if s.js("document.getElementById('armed-snap').value") == "connectionsnap005":
                break
            s.key("c")
            time.sleep(0.3)
        if s.js("document.getElementById('armed-twist').value") != "15":
            s.key("t")  # the preview turns 15° (on the one below, so each level adds 15°)
            time.sleep(0.5)
        s.click(x, y, 0.05)
        s.wait("%s.then(e => e.length > 1 && e.at(-1).attached?.to === %d)" % (ENTRIES, top))
        a = s.js(ENTRIES)[-1]["attached"]
        assert a["snap"] == "connectionsnap005" and round(a["twist"]) == 15, a
        s.key("f")
        time.sleep(0.4)
    levels = [e["index"] for e in s.js(ENTRIES)]
    s.caption("L Living Quarters on every side snap")
    s.click_module("living quarters", "Terran L Living Quarters")
    s.key("f")
    s.js("{ const t = document.getElementById('armed-twist'); t.value = '0'; t.dispatchEvent(new Event('change')); }")  # flat rings
    s.zoom(5)
    # The camera stays put from here: find the crosses' side cones once, before any habitat.
    # The ones behind the spire are hidden: fit the camera to the visible ones and work out
    # where those are (a click there still picks the cone, as picking ignores the station).
    sides = ["connectionsnap00%d" % k for k in range(1, 5)]
    found = dict((snap, at) for at, snap in probe_cones(s, lambda to, snap: to in levels and snap in sides, lambda c, vp: 0))
    project = fit_camera({snap: cone_point(s, *snap) for snap in found}, found)
    for (to, snap) in sorted({(to, snap) for to in levels for snap in sides}, key=lambda f: (levels.index(f[0]), f[1])):  # bottom level first
        x, y = found.get((to, snap)) or project(cone_point(s, to, snap))
        s.move(x, y, 0.25)
        time.sleep(0.15)
        s.click(x, y, 0.02)
        s.wait("%s.then(e => e.at(-1).attached?.to === %d && e.at(-1).attached.to_snap === %r)" % (ENTRIES, to, snap))
    s.key("Escape", "Escape", 27)
    s.caption("")
    s.key("f")
    time.sleep(0.8)
    s.orbit(300, 2.5)
    time.sleep(1.0)


# name: (play, setup)
SCENES = {
    "build": (scene_build, lambda cdp: new_design(cdp, 5)),
    "planner": (scene_planner, lambda cdp: (new_design(cdp, 10), clear_planner(cdp))),
    "agent": (scene_agent, None),
    "analysis": (scene_analysis, None),
    "optimize": (scene_optimize, setup_optimize),
    "spire": (scene_spire, lambda cdp: new_design(cdp, 10)),
}

# gifski quality per scene (default 80): big textured stations in motion make large GIFs.
QUALITY = {"optimize": 60}
# Playback speed per scene (default 1): long builds play faster.
SPEED = {"spire": 1.5}

SETUP_JS = """(() => {
  const st = document.createElement('style'); st.textContent = '#viewport .fps { display: none }'; document.head.appendChild(st);
  const q = document.getElementById('quality'); if (q.value !== 'normal') { q.value = 'normal'; q.dispatchEvent(new Event('change')); }
  const t = document.getElementById('textures'); if (!t.checked) t.click();
  document.getElementById('status').textContent = '';  // the start-up status names the profile folder (user name)
})()"""


def run(exe, out, name, gifski):
    play, setup = SCENES[name]
    args, extra = (), ()
    if name == "agent":
        if link_port_in_use():
            raise SystemExit("a builder is already open: the agent would edit that one. Close it first.")
        profile = os.path.join(out, "agent-profile")  # exports never touch the real profile
        os.makedirs(profile, exist_ok=True)
        args, extra = ("--profile-dir", profile), (exe, profile)
    app, cdp = start_builder(exe, *args)
    try:
        cdp.wait_for("document.getElementById('status').textContent.includes('modules loaded')", 60)
        cdp.call("Emulation.setDeviceMetricsOverride", width=W, height=H, deviceScaleFactor=1, mobile=False)
        # The blueprint limit (D-16) is remembered in the test profile: record on the full palette.
        if cdp.js("document.getElementById('bp-on').checked"):
            cdp.js("document.getElementById('bp-on').click()")
            cdp.wait_for("!document.getElementById('module-count').textContent.includes('blueprints')")
        cdp.js(SETUP_JS)
        if setup:
            setup(cdp)
        cdp.js(CURSOR_JS)
        time.sleep(1.0)
        s = Scene(cdp)
        s.mouse("mouseMoved", s.x, s.y)
        rec = s.rec = Recorder()
        rec.start()
        try:
            play(s, *extra)
        except Exception:
            cdp.screenshot(os.path.join(out, name + "-error.png"))
            print("   status:", cdp.js("document.getElementById('status').textContent"))
            raise
        finally:
            frames = rec.stop()
        encode(frames, os.path.join(out, name + ".gif"), gifski, quality=QUALITY.get(name, 80), speed=SPEED.get(name, 1.0))
    finally:
        app.terminate()
        app.wait(10)
        time.sleep(1.0)  # let WebView2 let go of the profile before the next scene


if __name__ == "__main__":
    argv = sys.argv[1:]
    gifski = r"H:\dev\gifski\gifski.exe"
    if "--gifski" in argv:
        i = argv.index("--gifski")
        gifski = argv[i + 1]
        del argv[i:i + 2]
    exe, out, names = argv[0], argv[1], argv[2:] or list(SCENES)
    os.makedirs(out, exist_ok=True)
    for name in names:
        print(name)
        run(exe, out, name, gifski)
