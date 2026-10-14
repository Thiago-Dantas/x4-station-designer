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
sys.stdout.reconfigure(encoding="utf-8", errors="replace")  # UI text (≈, ×) on a cp1252 console
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
    # Build cost (right pane): the whole station while nothing is selected.
    cdp.wait_for("document.getElementById('cost-title').textContent.includes('whole station')")
    cdp.wait_for("document.getElementById('cost-total').textContent.startsWith('≈')")
    station_cost = cdp.js("document.getElementById('cost-total').textContent")
    print("   cost:", cdp.js("document.getElementById('cost-title').textContent"), "|", station_cost, "|",
          cdp.js("[...document.querySelectorAll('#cost-lines tr')].map(r => r.children[0].textContent + ' ' + r.children[1].textContent).join(', ')"))
    # Production (right pane): flows, workforce, storage; solar output follows the sunlight
    # (remembered, so start from 100%).
    sun = "{ const s = document.getElementById('sunlight'); s.value = %d; s.dispatchEvent(new Event('change')); }"
    cdp.js(sun % 100)
    cdp.wait_for("document.getElementById('economy-summary').textContent.startsWith('Workforce')")
    flows = "[...document.querySelectorAll('#economy-flows tr')].map(r => r.children[0].textContent + ' ' + r.children[1].textContent)"
    cells = "(%s.find(t => t.startsWith('Energy Cells')) ?? '')" % flows
    print("   production:", cdp.js("document.getElementById('economy-summary').textContent").replace("\n", " | "))
    print("   flows:", cdp.js(flows + ".length"), "rows; first", cdp.js(flows + ".slice(0, 3).join(', ')"), "; last", cdp.js(flows + ".slice(-3).join(', ')"))
    print("   storage:", cdp.js("document.getElementById('economy-storage').textContent"), "| notes:",
          cdp.js("[...document.querySelectorAll('#economy-notes li')].map(l => l.textContent).join('; ')"))
    full_sun = cdp.js(cells)
    made = cdp.js("[...document.querySelectorAll('#economy-flows tr')].find(r => r.children[0].textContent === 'Energy Cells')?.title ?? ''")
    if made.startswith("Made 0,") or not made:
        print("   no solar modules in this plan (%s): sunlight can't change it" % made)
    else:
        cdp.js(sun % 50)
        cdp.wait_for("%s !== %r" % (cells, full_sun))
        print("   energy cells at 100%% / 50%% sunlight: %s / %s" % (full_sun, cdp.js(cells)))
        cdp.js(sun % 100)
        cdp.wait_for("%s === %r" % (cells, full_sun))
    # Clicking a module selects it (modules are instanced per type; the pick maps instance -> entry).
    centre = cdp.js("(() => { const r = document.getElementById('viewport').getBoundingClientRect(); return [Math.round(r.left + r.width / 2), Math.round(r.top + r.height / 2)] })()")
    cdp.click(*centre)
    cdp.wait_for("!document.getElementById('selection-panel').hidden")
    cdp.wait_for("document.getElementById('cost-title').textContent.startsWith('Build cost: ') && !document.getElementById('cost-title').textContent.includes('whole station')")
    print("   module cost:", cdp.js("document.getElementById('cost-title').textContent"), "|", cdp.js("document.getElementById('cost-total').textContent"))
    print("   selected:", cdp.js("document.getElementById('sel-name').textContent"), "|",
          cdp.js("document.getElementById('sel-details').textContent.split(String.fromCharCode(10))[0]"))

    # Build order tab: rows, timeline preview, build first, stage end, undo.
    cdp.js("document.getElementById('tab-order').click()")
    rows = cdp.js("document.querySelectorAll('#order-list .order-row').length")
    cdp.js("{ const s = document.getElementById('build-step'); s.value = 10; s.dispatchEvent(new Event('input')); }")
    later = cdp.js("document.querySelectorAll('#order-list .order-row.later').length")
    time.sleep(0.8)
    cdp.screenshot(os.path.join(out, "ui-order-step10.png"))
    cdp.js("{ const s = document.getElementById('build-step'); s.value = s.max; s.dispatchEvent(new Event('input')); }")
    name50 = cdp.js("document.querySelectorAll('#order-list .order-row')[49].querySelector('.n').textContent")
    cdp.js("document.querySelectorAll('#order-list .order-row')[49].click()")
    cdp.js("document.getElementById('btn-build-first').click()")
    cdp.wait_for("document.getElementById('status').textContent.startsWith('Moved to the front')")
    where = cdp.js("[...document.querySelectorAll('#order-list .order-row')].findIndex(r => r.classList.contains('selected'))")
    moved = cdp.js("document.getElementById('sel-name').textContent")
    print(f"   build order: {rows} rows, step 10 dims {later}; '{name50}' (#50) build first -> now #{where + 1}, selected: '{moved}'")
    assert moved == name50 and where + 1 < 50, (moved, where)
    cdp.js("document.getElementById('stage-end').click()")
    cdp.wait_for("document.querySelectorAll('#order-list .stage-divider').length === 1")
    print("   stage divider:", cdp.js("document.querySelector('#order-list .stage-divider').textContent"))
    cdp.js("window.dispatchEvent(new KeyboardEvent('keydown', {key: 'z', ctrlKey: true}))")
    time.sleep(0.4)
    cdp.js("window.dispatchEvent(new KeyboardEvent('keydown', {key: 'z', ctrlKey: true}))")
    cdp.wait_for(f"document.querySelectorAll('#order-list .order-row')[49].querySelector('.n').textContent === '{name50}'")
    print("   after 2x undo, row 50 is again:", cdp.js("document.querySelectorAll('#order-list .order-row')[49].querySelector('.n').textContent"))
    assert rows == 84 and later == 74, (rows, later)
    # Drag and drop in the build order: a module that attaches to one of the first 9 rows,
    # dragged from further down onto row 10.
    drag = """(() => {
      const rows = [...document.querySelectorAll('#order-list .order-row')];
      const from = rows.findIndex((r, i) => i > 10 && /#(\d+)/.test(r.title) && Number(r.title.match(/#(\d+)/)[1]) < 10);
      const name = rows[from].querySelector('.n').textContent;
      const data = new DataTransfer();
      rows[from].dispatchEvent(new DragEvent('dragstart', { dataTransfer: data, bubbles: true }));
      rows[9].dispatchEvent(new DragEvent('dragover', { dataTransfer: data, bubbles: true, cancelable: true }));
      rows[9].dispatchEvent(new DragEvent('drop', { dataTransfer: data, bubbles: true, cancelable: true }));
      return [from + 1, name];
    })()"""
    dragged_from, dragged = cdp.js(drag)
    cdp.wait_for("document.getElementById('status').textContent.startsWith('Moved #')")
    now10 = cdp.js("document.querySelectorAll('#order-list .order-row')[9].querySelector('.n').textContent")
    print(f"   drag row {dragged_from} ('{dragged}') onto row 10 -> row 10 is now '{now10}'")
    assert now10 == dragged, (now10, dragged)
    cdp.js("window.dispatchEvent(new KeyboardEvent('keydown', {key: 'z', ctrlKey: true}))")
    cdp.wait_for(f"document.querySelectorAll('#order-list .order-row')[{dragged_from - 1}].querySelector('.n').textContent === '{dragged}'")
    cdp.js("document.getElementById('tab-modules').click()")

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
    print("   unsaved marker shown:", not cdp.js("document.getElementById('dirty').hidden"))

    # Move the beam (#2) onto another free snap of the cross: select it in the build order, Move..., click a cone.
    cdp.js("document.getElementById('tab-order').click()")
    cdp.js("document.querySelectorAll('#order-list .order-row')[1].click()")
    cdp.js("document.getElementById('tab-modules').click()")
    cdp.js("document.getElementById('btn-move').click()")
    cdp.wait_for("document.getElementById('armed-title').textContent === 'Moving'")
    time.sleep(1.0)
    target = find_color(cdp.screenshot(os.path.join(out, "ui-4b-moving.png")), (0x37, 0xD0, 0xC0), vp)
    cdp.click(*target)
    cdp.wait_for("document.getElementById('status').textContent.startsWith('Moved onto')")
    print("   move:", cdp.js("document.getElementById('status').textContent"), "| checks:",
          cdp.js("[...document.querySelectorAll('#issues li')].map(l => l.textContent).join('; ')"))
    assert cdp.js("document.querySelector('#issues li').textContent") == "No problems found"
    # The builder's move_root command (the gizmo calls it) moves the whole station.
    moved = cdp.js("window.__TAURI_INTERNALS__.invoke('move_root', { index: 1, position: [500, 0, 0] }).then(s => s.entries.map(e => Math.round(e.position[0])))")
    saved = cdp.js("window.__TAURI_INTERNALS__.invoke('save_design', { file: %r }).then(() => window.__TAURI_INTERNALS__.invoke('scene')).then(s => s.dirty)" % os.path.join(out, "ui-saved.xml").replace("\\", "/"))
    print("   move_root x positions:", moved, "| unsaved after save:", saved)
    assert moved[0] == 500 and saved is False
    cdp.js("window.__TAURI_INTERNALS__.invoke('undo')")  # move_root
    cdp.js("window.dispatchEvent(new KeyboardEvent('keydown', {key: 'z', ctrlKey: true}))")  # the re-attach
    time.sleep(0.5)
    # Arm the beam again so snap cones show for the next steps.
    cdp.js("document.querySelector('.module').click()")
    time.sleep(1.0)
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
    # Q/E move straight down/up (E: the scene shifts down on screen); arrows orbit the camera.
    cdp.hold_key("KeyE", "e", 0.4)
    time.sleep(0.3)
    after_e = find_color(cdp.screenshot(None), (0x37, 0xD0, 0xC0), vp)
    cdp.hold_key("ArrowRight", "ArrowRight", 0.5)
    time.sleep(0.3)
    after_arrow = find_color(cdp.screenshot(os.path.join(out, "ui-6-rotated.png")), (0x37, 0xD0, 0xC0), vp)
    print("6. after E", after_e, "-> after ArrowRight", after_arrow)
    assert after_e[1] > after_w[1] + 20, "E should shift the scene down"
    assert after_arrow != after_e, "an arrow key should rotate the view"

    # Planner tab: Hull Parts x1 (any race), generate, place from a row, buy a chain ware.
    cdp.js("document.getElementById('tab-planner').click()")
    cdp.js("document.querySelectorAll('#planner-targets button').forEach(b => b.click())")
    cdp.js("{ const r = document.getElementById('planner-race'); r.value = ''; r.dispatchEvent(new Event('change')); }")
    cdp.js("document.getElementById('planner-buy').querySelectorAll('button').forEach(b => b.click())")
    # Ware search: typing filters the list; Enter adds the highlighted ware.
    cdp.js("{ const i = document.getElementById('planner-ware'); i.value = 'hull par'; i.dispatchEvent(new Event('input')); }")
    cdp.wait_for("!document.getElementById('planner-ware-list').hidden")
    print("7. ware search 'hull par':", cdp.js("[...document.querySelectorAll('#planner-ware-list div')].map(d => d.textContent).join(', ')"))
    cdp.js("document.getElementById('planner-ware').dispatchEvent(new KeyboardEvent('keydown', {key: 'Enter'}))")
    assert cdp.js("[...document.querySelectorAll('#planner-targets li span')].map(s => s.textContent).join()") == "Hull Parts"
    assert cdp.js("document.getElementById('planner-ware-list').hidden")
    cdp.js("document.getElementById('planner-generate').click()")
    cdp.wait_for("/^\\d+ modules to add/.test(document.getElementById('planner-summary').textContent)")
    rows = "[...document.querySelectorAll('.planned-row')]"
    print("   planner:", cdp.js("document.getElementById('planner-summary').textContent").replace("\n", " | "))
    print("   rows:", cdp.js(rows + ".map(r => r.querySelector('.name').textContent + ' ' + r.querySelector('.n').textContent).join(', ')"))
    cdp.screenshot(os.path.join(out, "ui-7-planner.png"))
    first = cdp.js(rows + "[0].querySelector('.name').textContent")
    cdp.js(rows + "[0].click()")
    cdp.wait_for("!document.getElementById('armed-panel').hidden && document.getElementById('armed-name').textContent === %r" % first)
    print("   clicked row arms:", cdp.js("document.getElementById('armed-name').textContent"))
    # Place it on a free snap: its row counts it.
    cdp.js("document.getElementById('btn-frame').click()")
    time.sleep(0.8)
    cone = find_color(cdp.screenshot(None), (0x37, 0xD0, 0xC0), vp)
    cdp.click(*cone)
    cdp.wait_for("document.getElementById('status').textContent.startsWith('Attached Hull Part Production')")
    hull_row = rows + ".find(r => r.querySelector('.name').textContent === 'Hull Part Production')"
    cdp.wait_for(hull_row + ".querySelector('.n').textContent === '1/1'")
    print("   placed on", cone, "->", cdp.js(hull_row + ".querySelector('.n').textContent"), "done:", cdp.js(hull_row + ".classList.contains('done')"), "|",
          cdp.js("document.getElementById('planner-summary').textContent.split(String.fromCharCode(10))[0]"))
    cdp.js("window.dispatchEvent(new KeyboardEvent('keydown', {key: 'Escape'}))")
    cdp.js(rows + ".find(r => r.querySelector('.name').textContent === 'Energy Cell Production').querySelector('button').click()")
    cdp.wait_for("document.querySelectorAll('#planner-buy button').length === 1 && !%s.some(r => r.querySelector('.name').textContent === 'Energy Cell Production')" % rows)
    print("   buying energy cells:", cdp.js("document.getElementById('planner-buy').textContent"), "|",
          cdp.js("document.getElementById('planner-summary').textContent").replace("\n", " | "))
    # Buying re-plans for the same station: the placed hull part still counts, nothing is added twice.
    assert cdp.js(hull_row + ".querySelector('.n').textContent") == "1/1"
    cdp.js("document.querySelector('#planner-buy button').click()")
    cdp.wait_for("%s.some(r => r.querySelector('.name').textContent === 'Energy Cell Production')" % rows)
    summary = cdp.js("document.getElementById('planner-summary').textContent")
    assert summary.startswith("18 modules to add, 17 still to place"), summary
    # Place all: everything still to place, on the largest plot so no enlarge prompt
    # (a native dialog) can block the test; then undo it in one step.
    cdp.js("document.querySelectorAll('.plot-grid input').forEach(i => { i.value = 10; i.dispatchEvent(new Event('change')); })")
    cdp.wait_for("document.getElementById('plot-summary').textContent.startsWith('Plot 20×20×20')")
    count = "window.__TAURI_INTERNALS__.invoke('scene').then(s => s.entries.length)"
    before, todo = cdp.js(count), cdp.js("document.getElementById('planner-place').textContent")
    cdp.js("document.getElementById('planner-place').click()")
    cdp.wait_for("document.getElementById('status').textContent.startsWith('Placed ')", timeout=60)
    time.sleep(1.0)
    cdp.screenshot(os.path.join(out, "ui-8-placed.png"))
    issues = cdp.js("[...document.querySelectorAll('#issues li')].map(l => l.textContent).join('; ')")
    print("8.", todo, "->", cdp.js("document.getElementById('planner-placed').textContent"), "|", before, "->", cdp.js(count), "modules | checks:", issues)
    assert cdp.js(rows + ".every(r => r.classList.contains('done'))") and cdp.js("document.getElementById('planner-place').hidden")
    assert issues == "No problems found", issues
    # Buying a chain ware after placing doesn't bring the plan back to place again.
    cdp.js(rows + ".find(r => r.querySelector('.name').textContent === 'Graphene Production').querySelector('button').click()")
    cdp.wait_for("!%s.some(r => r.querySelector('.name').textContent === 'Graphene Production')" % rows)
    left = cdp.js("document.getElementById('planner-place').hidden ? '' : document.getElementById('planner-place').textContent")
    print("   buy graphene after placing ->", cdp.js("document.getElementById('planner-summary').textContent.split(String.fromCharCode(10))[0]"), "|", left or "nothing to place")
    assert cdp.js(hull_row + ".querySelector('.n').textContent") == "1/1" and not left, left
    cdp.js("document.querySelector('#planner-buy button').click()")
    cdp.wait_for("%s.some(r => r.querySelector('.name').textContent === 'Graphene Production')" % rows)
    assert cdp.js(rows + ".every(r => r.classList.contains('done'))") and cdp.js("document.getElementById('planner-place').hidden")

    # Equipment: select a module with groups, equip one group, copy it to its type,
    # then the whole station by level and by picks; undo those four steps.
    summary = "document.getElementById('equip-summary').textContent"
    print("9. equipment:", cdp.js(summary))
    target = cdp.js("window.__TAURI_INTERNALS__.invoke('scene').then(s => s.entries.find(e => e.equipment[1] > 0 && s.entries.filter(x => x.macro === e.macro).length > 1).index)")
    cdp.js("document.getElementById('tab-order').click()")
    cdp.js("document.querySelectorAll('#order-list .order-row')[%d].click()" % (target - 1))
    cdp.js("document.getElementById('tab-planner').click()")
    cdp.wait_for("!document.getElementById('sel-equipment').hidden && document.querySelectorAll('#sel-equipment-groups select').length > 0")
    groups = cdp.js("[...document.querySelectorAll('#sel-equipment-groups .equip-row span')].map(s => s.textContent).join(', ')")
    first = cdp.js("{ const s = document.querySelector('#sel-equipment-groups select'); s.value = s.options[1].value; s.dispatchEvent(new Event('change')); s.options[1].textContent }")
    cdp.wait_for("%s.startsWith('1 of')" % summary)
    print("   #%d %s: %s; set the first to %s -> %s" % (target, cdp.js("document.getElementById('sel-name').textContent"), groups, first, cdp.js(summary)))
    cdp.js("document.getElementById('btn-equip-same').click()")
    cdp.wait_for("document.getElementById('status').textContent.startsWith('Copied to')")
    print("   ", cdp.js("document.getElementById('status').textContent"), "->", cdp.js(summary))
    cdp.js("{ const l = document.getElementById('equip-level'); l.value = 'high'; } document.getElementById('btn-equip-level').click()")
    cdp.wait_for("document.getElementById('status').textContent.startsWith('Equipped')")
    total = cdp.js("%s.split(' of ')[1].split(' ')[0]" % summary)
    print("   level high:", cdp.js("document.getElementById('status').textContent"), "->", cdp.js(summary))
    assert cdp.js(summary).startswith("%s of %s" % (total, total)), cdp.js(summary)
    old_status = cdp.js("document.getElementById('status').textContent")
    cdp.js("{ const s = document.getElementById('pick-turrets-medium'); s.value = s.options[1].value; } document.getElementById('btn-equip-picks').click()")
    cdp.wait_for("document.getElementById('status').textContent.startsWith('Equipped') && document.getElementById('status').textContent !== %r" % old_status, timeout=30)
    print("   picks (%s):" % cdp.js("document.getElementById('pick-turrets-medium').selectedOptions[0].textContent"), cdp.js("document.getElementById('status').textContent"))
    assert cdp.js("[...document.querySelectorAll('#issues li')].map(l => l.textContent).join('; ')") == "No problems found"
    time.sleep(0.5)
    cdp.screenshot(os.path.join(out, "ui-9-equipment.png"))
    for _ in range(4):
        cdp.js("window.dispatchEvent(new KeyboardEvent('keydown', {key: 'z', ctrlKey: true}))")
        time.sleep(0.4)
    cdp.wait_for("%s.startsWith('0 of')" % summary)
    print("   after 4x Ctrl+Z:", cdp.js(summary))
    cdp.js("window.dispatchEvent(new KeyboardEvent('keydown', {key: 'z', ctrlKey: true}))")
    cdp.wait_for("%s.then(n => n === %d)" % (count, before))
    print("   after Ctrl+Z:", cdp.js(count), "modules,", cdp.js("document.getElementById('planner-place').textContent"))
    cdp.js("document.querySelectorAll('#planner-targets button').forEach(b => b.click())")
finally:
    app.terminate()
