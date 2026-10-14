# Workflow: move unattached modules by position fields (user request)

Status: **done** (2026-10-14)

User: "move unattached modules in the UI. This is especially helpful for placing the first module precisely within the plot square; right now it's hard to place, as the click may register anywhere within or outside the plot box."

The drag gizmo for free-standing modules already exists (10 m steps). Asked: the user chose **position fields** (X / Y / Z in metres) in the Selected panel.

## Plan
- [x] UI: X / Y / Z number inputs (metres, game frame) in the Selected panel, shown only for a free-standing module; a change calls `move_root` (subtree moves too, rotation kept, one undo step)
- [x] Fields follow the module (gizmo drags, undo, agent edits) unless one is being typed in
- [x] Test: ui_smoke sets a position through the fields and checks the entry moved
- [x] Docs: usage.md, CLAUDE.md, roadmap

## Follow-up (user: "doesn't seem to let me place where I want, I don't see it on the view moving")
Reproduced: the fields apply only on Enter/Tab (`change`); the camera stayed close on the module, so a 200 m move barely showed and 1500 m left the view (the plot box was off-screen); and after placing, the first module stayed armed and unselected, so the gizmo and fields were hidden. The user chose:
- [x] Placing the first module (click, drop or *Place as first module*) selects it and stops placing (`placeFirst`)
- [x] A position-field move frames the whole plot box from the current viewing direction (`View3D.framePlot`)
- Not chosen: apply while typing; a ground grid on the plot floor.
- [x] Checked in the release build with screenshots (focused script): after placing, the Placing panel is hidden and the fields and gizmo show; X = 1500 then blur moves the cross and frames the 4 km plot box with it inside.

## Notes
- 2026-10-14: `move_root` with no rotation keeps the module's rotation; nothing new in the core.
- 2026-10-14: `.pos-grid { display: grid }` overrode the `hidden` attribute; `.pos-grid[hidden]` fixes that. The *Connected by* label has the same problem (`#inspector label { display: flex }`): it shows, empty, on free-standing modules. That bug was there before this change and is still open.
- 2026-10-14: tests: a new ui_smoke step (types 1200 / −300 / 450, checks the entry, undo brings the fields back to 0); run on its own as a focused script, which passed.
