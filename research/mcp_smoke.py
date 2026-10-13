"""Smoke test for `x4sd mcp`: drives the server over stdio like an agent would.

    python -I research/mcp_smoke.py <path to x4sd.exe> <scratch profile dir>

Builds a small Terran station (hub + 4 beams + habitat), validates it and
exports it into the scratch profile's constructionplan folder.
"""
import json
import os
import socket
import subprocess
import sys

exe, profile = sys.argv[1], sys.argv[2]
if os.environ.get("X4SD_EXPECT_BUILDER") != "1":
    try:
        socket.create_connection(("127.0.0.1", 47823), timeout=0.3).close()
        raise SystemExit("a builder is open: the MCP server would edit its design. Close it first.")
    except OSError:
        pass
proc = subprocess.Popen([exe, "--profile-dir", profile, "mcp"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, encoding="utf-8")
next_id = 0


def send(method, params=None, notify=False):
    global next_id
    msg = {"jsonrpc": "2.0", "method": method}
    if params is not None:
        msg["params"] = params
    if not notify:
        next_id += 1
        msg["id"] = next_id
    proc.stdin.write(json.dumps(msg) + "\n")
    proc.stdin.flush()
    if notify:
        return None
    while True:
        reply = json.loads(proc.stdout.readline())
        if reply.get("id") == next_id:
            return reply


def call(tool, **args):
    reply = send("tools/call", {"name": tool, "arguments": args})
    if "error" in reply:
        raise SystemExit(f"{tool}: protocol error {reply['error']}")
    result = reply["result"]
    text = result["content"][0]["text"]
    if result.get("isError"):
        raise SystemExit(f"{tool}: tool error: {text}")
    return json.loads(text)


init = send("initialize", {"protocolVersion": "2025-06-18", "capabilities": {}, "clientInfo": {"name": "smoke", "version": "0"}})
print("server:", init["result"]["serverInfo"], "protocol", init["result"]["protocolVersion"])
send("notifications/initialized", notify=True)
tools = send("tools/list")["result"]["tools"]
print(len(tools), "tools:", ", ".join(t["name"] for t in tools))

print("terran quarters:", [m["name"] for m in call("list_modules", race="terran", search="quarters")])
call("new_design", name="MCP smoke station")
hub = call("add_module", **{"macro": "struct_ter_cross_01_macro"})["index"]
arms = call("free_snaps", index=hub)
print("hub free snaps:", [s["name"] for s in arms])
for i, arm in enumerate(arms):
    call("attach_module", **{"macro": "struct_ter_base_02_macro", "to": hub, "to_snap": arm["name"], "twist": 90 * i})
end = call("free_snaps", index=2)[0]["name"]
hab = call("attach_module", **{"macro": "hab_ter_l_01_macro", "to": 2, "to_snap": end})
print("habitat:", hab)
print("plot before:", call("describe_design")["plot"]["required"]["text"], "needed; fits:", call("describe_design")["plot"]["fits"])
print("fit_plot:", call("fit_plot", grow_only=True)["plot"]["text"])
print("issues:", call("validate_design")["issues"])
d = call("describe_design")
print("design:", d["name"], "patches", d["patches"], "modules", [(m["index"], m["name"], m["position"]) for m in d["modules"]])
print("remove beam 3:", call("remove_module", index=3), "-> modules", len(call("describe_design")["modules"]))
print("undo:", call("undo"), "-> modules", len(call("describe_design")["modules"]))
try:
    call("attach_module", **{"macro": "struct_ter_base_02_macro", "to": hub, "to_snap": arms[0]["name"]})
except SystemExit as e:
    print("expected error:", e)
print("export:", call("export_design", new_id=True))
proc.stdin.close()
proc.wait(timeout=10)
print("server exit code", proc.returncode)
