"""Copy / Remove sequence like the game (user request, 2026-10-15).

    python -I research/sequence_smoke.py <x4sd.exe> <scratch dir>

New design: a Terran cross with a second cross on it and a third on that one
(#1 <- #2 <- #3). Selects #2 and checks:
- Ctrl+C arms the cross type (Placing); Esc.
- Ctrl+Shift+C starts copying #2's sequence; a click on a free snap cone adds two
  linked copies, nothing unattached; Esc stops. Shift + middle click on empty space
  starts the same mode for the selected module.
- Del removes #2 alone: #3 stays, unattached, and Checks say so; undo.
- Shift+Del removes #2 with everything attached to it.
Before that, with only the first cross: a right click on it opens the module menu
(it selects the module; Detach is off for the first module), Esc closes it, its
Copy arms the type, and a right click on empty space opens nothing.
Screenshots go to <scratch dir>.
"""
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from cdp import decode_png, start_builder  # noqa: E402

exe, out = sys.argv[1], sys.argv[2]
app, cdp = start_builder(exe)
SCENE = "window.__TAURI_INTERNALS__.invoke('scene')"
STATUS = "document.getElementById('status').textContent"


def invoke(cmd, args):
    return cdp.js(f"window.__TAURI_INTERNALS__.invoke('{cmd}', {args})")


def key(k, code, vk, ctrl=False, shift=False):
    mods = (2 if ctrl else 0) | (8 if shift else 0)
    for kind in ("keyDown", "keyUp"):
        cdp.call("Input.dispatchKeyEvent", type=kind, key=k, code=code, windowsVirtualKeyCode=vk, modifiers=mods)
    time.sleep(0.8)


def entries():
    return cdp.js(f"{SCENE}.then(s => s.entries.map(e => [e.index, e.attached ? e.attached.to : null]))")


def issues():
    return cdp.js("[...document.querySelectorAll('#issues li')].map(l => l.textContent).join(' | ')")


def free_cone(png, box):
    """Centre of the biggest free-snap cone (cones next to each other stay apart)."""
    w, h, ch, rows = decode_png(png)
    pixels = {(x, y) for y in range(box[1], min(box[3], h)) for x in range(box[0], min(box[2], w))
              if all(abs(rows[y][x * ch + k] - c) < 30 for k, c in enumerate((0x37, 0xD0, 0xC0)))}
    best = []
    while pixels:
        blob, todo = [], [pixels.pop()]
        while todo:
            x, y = todo.pop()
            blob.append((x, y))
            for n in ((x + 1, y), (x - 1, y), (x, y + 1), (x, y - 1)):
                if n in pixels:
                    pixels.remove(n)
                    todo.append(n)
        best = max(best, blob, key=len)
    return sum(x for x, _ in best) // len(best), sum(y for _, y in best) // len(best)


def right_click(x, y):
    cdp.call("Input.dispatchMouseEvent", type="mouseMoved", x=x, y=y)
    for kind in ("mousePressed", "mouseReleased"):
        cdp.call("Input.dispatchMouseEvent", type=kind, x=x, y=y, button="right", buttons=2 if kind == "mousePressed" else 0, clickCount=1)
    time.sleep(0.6)


def select_row(n):
    cdp.js(f"document.querySelectorAll('.order-row')[{n - 1}].click()")
    time.sleep(0.5)


try:
    cdp.wait_for(f"{STATUS}.includes('modules loaded')")
    cdp.js("document.getElementById('btn-new').click()")
    cdp.wait_for(f"{STATUS}.includes('New design')")
    cdp.js("{ const s = document.getElementById('search'); s.value = 'Terran Cross Connection Structure 01'; s.dispatchEvent(new Event('input')) }")
    cdp.js("document.querySelector('.module').click()")
    cdp.wait_for("!document.getElementById('armed-panel').hidden")
    cdp.js("document.getElementById('btn-place-root').click()")
    cdp.wait_for(f"{STATUS}.startsWith('Placed')")
    key("Escape", "Escape", 27)
    cdp.js("document.getElementById('btn-frame').click()")
    time.sleep(1.2)
    vp = cdp.js("(() => { const r = document.getElementById('viewport').getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom].map(Math.round) })()")
    MENU = "[document.getElementById('module-menu').hidden, document.getElementById('menu-detach').disabled, document.getElementById('sel-name').textContent]"
    right_click((vp[0] + vp[2]) // 2, (vp[1] + vp[3]) // 2)
    menu = cdp.js(MENU)
    print("right click on the cross:", menu)
    cdp.screenshot(os.path.join(out, "seq-menu.png"))
    assert menu[0] is False and menu[1] is True and menu[2].startswith("Terran Cross"), menu
    key("Escape", "Escape", 27)
    assert cdp.js(MENU)[0] is True, "Esc closes the menu"
    right_click((vp[0] + vp[2]) // 2, (vp[1] + vp[3]) // 2)
    cdp.js("document.getElementById('menu-copy').click()")
    time.sleep(0.5)
    armed = cdp.js("[document.getElementById('armed-title').textContent, document.getElementById('armed-panel').hidden, document.getElementById('module-menu').hidden]")
    print("menu Copy:", armed)
    assert armed == ["Placing", False, True], armed
    key("Escape", "Escape", 27)
    right_click(vp[0] + 10, vp[3] - 10)
    assert cdp.js(MENU)[0] is True, "no menu on empty space"
    macro = cdp.js(f"{SCENE}.then(s => s.entries[0].macro)")
    snaps = cdp.js(f"{SCENE}.then(s => s.entries[0].free_snaps.map(f => f.name))")
    invoke("attach_module", f"{{macroName: '{macro}', snap: '{snaps[0]}', to: 1, toSnap: '{snaps[0]}', twist: 0}}")
    free2 = cdp.js(f"{SCENE}.then(s => s.entries[1].free_snaps.map(f => f.name))")
    invoke("attach_module", f"{{macroName: '{macro}', snap: '{snaps[0]}', to: 2, toSnap: '{free2[0]}', twist: 0}}")
    key("Escape", "Escape", 27)
    key("z", "KeyZ", 90, ctrl=True)  # the UI shows backend edits on its next apply
    key("y", "KeyY", 89, ctrl=True)
    print("station:", entries())
    assert entries() == [[1, None], [2, 1], [3, 2]]
    cdp.js("document.querySelector('#checks-panel > h3').click()")  # open Checks

    select_row(2)
    key("c", "KeyC", 67, ctrl=True)
    armed = cdp.js("[document.getElementById('armed-title').textContent, document.getElementById('armed-panel').hidden]")
    print("Ctrl+C:", armed)
    assert armed == ["Placing", False], armed
    key("Escape", "Escape", 27)

    select_row(2)
    key("C", "KeyC", 67, ctrl=True, shift=True)
    title = cdp.js("[document.getElementById('armed-title').textContent, document.getElementById('armed-name').textContent]")
    print("Ctrl+Shift+C:", title)
    assert title[0] == "Copying sequence" and title[1].endswith("+ 1 attached"), title
    time.sleep(1.0)
    png = cdp.screenshot(os.path.join(out, "seq-0-copying.png"))
    vp = cdp.js("(() => { const r = document.getElementById('viewport').getBoundingClientRect(); return [r.left, r.top, r.right, r.bottom].map(Math.round) })()")
    cone = free_cone(png, vp)
    print("free snap cone at", cone)
    cdp.call("Input.dispatchMouseEvent", type="mouseMoved", x=cone[0], y=cone[1])
    time.sleep(0.5)
    cdp.screenshot(os.path.join(out, "seq-1-preview.png"))
    cdp.click(*cone)
    cdp.wait_for(f"{STATUS}.startsWith('Copied')")
    after = entries()
    print("copied:", cdp.js(STATUS), after, "| still copying:", cdp.js("document.getElementById('armed-title').textContent"))
    assert len(after) == 5 and after[4][1] == 4 and after[3][1] is not None, after
    assert "not attached" not in issues(), issues()
    cdp.screenshot(os.path.join(out, "seq-2-copied.png"))
    key("Escape", "Escape", 27)
    assert cdp.js("document.getElementById('armed-panel').hidden")

    # Shift + middle click on empty space: copy the selected module's sequence.
    select_row(2)
    x, y = vp[0] + 10, vp[3] - 10
    for kind in ("mousePressed", "mouseReleased"):
        cdp.call("Input.dispatchMouseEvent", type=kind, x=x, y=y, button="middle", buttons=4 if kind == "mousePressed" else 0, clickCount=1, modifiers=8)
    time.sleep(0.8)
    title = cdp.js("document.getElementById('armed-title').textContent")
    print("Shift+middle click:", title)
    assert title == "Copying sequence", title
    key("Escape", "Escape", 27)

    before = entries()
    select_row(2)
    key("Delete", "Delete", 46)
    alone = entries()
    print("Del:", cdp.js(STATUS), alone, "| checks:", issues())
    assert len(alone) == len(before) - 1 and [2, None] in alone, alone
    assert "entry 2: not attached to the station" in issues(), issues()
    cdp.screenshot(os.path.join(out, "seq-3-removed-alone.png"))
    key("z", "KeyZ", 90, ctrl=True)
    assert entries() == before

    select_row(2)
    key("Delete", "Delete", 46, shift=True)
    seq = entries()
    print("Shift+Del:", cdp.js(STATUS), seq, "| checks:", issues())
    assert len(seq) <= len(before) - 2 and all(e[1] is not None for e in seq[1:]), seq
    print("OK")
finally:
    app.kill()
