"""Smoke test for the blueprint limit (D-16): MCP tools headless, then the builder.

    python -I research/blueprint_smoke.py <x4sd.exe> <save file> <scratch dir>

Part 1 drives `x4sd mcp` (no builder open): read_blueprints, list_modules,
plan_production, auto_place and blueprint_filter. Part 2 opens the builder
(own WebView2 profile, scratch player profile), lets an agent read the save
through the live link, then checks the palette, the checkbox (off, on again
without reading), Re-read, and that the setting survives a restart. The file
dialog itself can't be driven, so the first read comes from the agent.
"""
import json
import os
import subprocess
import sys
import time
sys.stdout.reconfigure(encoding="utf-8", errors="replace")

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from cdp import link_port_in_use, start_builder  # noqa: E402

exe, save, out = sys.argv[1], sys.argv[2], sys.argv[3]
profile = os.path.join(out, "profile")
os.makedirs(profile, exist_ok=True)
if link_port_in_use():
    raise SystemExit("a builder is open: the MCP server would use it. Close it first.")


class Mcp:
    def __init__(self):
        self.proc = subprocess.Popen([exe, "--profile-dir", profile, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, encoding="utf-8")
        self.id = 0
        self.send("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "smoke", "version": "0"}})
        self.send("notifications/initialized", notify=True)

    def send(self, method, params=None, notify=False):
        msg = {"jsonrpc": "2.0", "method": method, **({"params": params} if params is not None else {})}
        if not notify:
            self.id += 1
            msg["id"] = self.id
        self.proc.stdin.write(json.dumps(msg) + "\n")
        self.proc.stdin.flush()
        while not notify:
            reply = json.loads(self.proc.stdout.readline())
            if reply.get("id") == self.id:
                return reply

    def call(self, tool, ok=True, **args):
        result = self.send("tools/call", {"name": tool, "arguments": args})["result"]
        text = result["content"][0]["text"]
        if bool(result.get("isError")) == ok:
            raise SystemExit(f"{tool}: {'error' if ok else 'expected an error'}: {text}")
        return json.loads(text) if ok else text

    def close(self):
        self.proc.stdin.close()
        self.proc.wait(10)


# ---- Part 1: MCP, headless
mcp = Mcp()
assert mcp.call("blueprint_filter")["on"] is False
print("filter on before any read:", mcp.call("blueprint_filter", ok=False, on=True))
everything = len(mcp.call("list_modules"))
start = time.time()
status = mcp.call("read_blueprints", save=save)
print(f"read_blueprints: {status} in {time.time() - start:.2f} s (catalog load included)")
owned = {m["macro"].lower() for m in mcp.call("list_modules")}
assert status["on"] and len(owned) == status["modules"] < everything, (len(owned), everything)

plan = mcp.call("plan_production", targets=[{"ware": "hullparts", "modules": 4}], race="argon")
planned = [m["macro"] for m in plan["modules"]]
print("plan for hull parts:", planned, "| no_blueprint:", plan["no_blueprint"], "| unknown:", plan["unknown_targets"])
assert all(m.lower() in owned for m in planned)

mcp.call("new_design", name="blueprints")
placed = mcp.call("auto_place", from_plan={"targets": [{"ware": "energycells", "modules": 4}]})
connectors = [c["macro"] for c in placed["connectors"]]
print("auto_place connection structures:", sorted(set(connectors)))
assert connectors and all(c.lower() in owned for c in connectors)

assert mcp.call("blueprint_filter", on=False)["on"] is False
assert len(mcp.call("list_modules")) == everything
start = time.time()
assert mcp.call("blueprint_filter", on=True)["on"] is True
assert len(mcp.call("list_modules")) == len(owned)
print(f"filter off: {everything} modules; on again without reading: {len(owned)} in {time.time() - start:.2f} s")
mcp.close()

# ---- Part 2: builder + live link
count = "document.getElementById('module-count').textContent"
checked = "document.getElementById('bp-on').checked"
app, cdp = start_builder(exe, "--profile-dir", profile)
try:
    cdp.js("localStorage.removeItem('x4sd.blueprints'); localStorage.removeItem('x4sd.races'); location.reload()")
    time.sleep(1.0)
    cdp.wait_for("document.getElementById('status').textContent.includes('modules loaded')")
    print("builder up:", cdp.js(count), "| checkbox", cdp.js(checked))
    assert not cdp.js(checked) and cdp.js("document.getElementById('bp-info').hidden")

    agent = Mcp()
    agent.call("read_blueprints", save=save)
    agent.close()
    cdp.wait_for(f"{checked} && {count}.includes('with blueprints')")
    print("after the agent's read:", cdp.js(count), "|", cdp.js("document.getElementById('bp-save').textContent"))
    assert cdp.js(count).startswith(f"{len(owned)} of {len(owned)} ")

    cdp.js("document.getElementById('bp-on').click()")
    cdp.wait_for(f"!{checked} && {count} === '{everything} of {everything} modules'")
    print("unchecked:", cdp.js(count))
    cdp.js("document.getElementById('bp-on').click()")
    cdp.wait_for(f"{checked} && {count}.includes('with blueprints')")
    print("checked again (cached, no dialog):", cdp.js(count), "| status:", cdp.js("document.getElementById('status').textContent"))

    cdp.js("document.getElementById('bp-reread').click()")
    cdp.wait_for("document.getElementById('status').textContent.includes('have blueprints in')")
    print("re-read:", cdp.js("document.getElementById('status').textContent"))
    cdp.screenshot(os.path.join(out, "blueprints-palette.png"))
finally:
    app.terminate()
    app.wait(10)

# Restart: the setting and the cached list come back without reading the save.
app, cdp = start_builder(exe, "--profile-dir", profile)
try:
    cdp.wait_for("document.getElementById('status').textContent.includes('modules loaded')")
    print("after restart:", cdp.js(count), "| checkbox", cdp.js(checked))
    assert cdp.js(checked) and cdp.js(count).startswith(f"{len(owned)} of {len(owned)} ")
    agent = Mcp()
    assert agent.call("blueprint_filter")["on"] is True, "the builder's core has the remembered limit"
    agent.close()
finally:
    app.terminate()
print("ok")
