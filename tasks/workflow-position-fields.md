# Workflow: move unattached modules by position fields (user request)

Status: **done** (2026-10-14)

User: "move unattached modules in the UI. This is especially helpful for placing the first module precisely within the plot square; right now it's hard to place, as the click may register anywhere within or outside the plot box."

The drag gizmo for free-standing modules already exists (10 m steps). Asked: the user chose **position fields** (X / Y / Z in metres) in the Selected panel.

## Plan
- [x] UI: X / Y / Z number inputs (metres, game frame) in the Selected panel, shown only for a free-standing module; a change calls `move_root` (subtree moves too, rotation kept, one undo step)
- [x] Fields follow the module (gizmo drags, undo, agent edits) unless one is being typed in
- [x] Test: ui_smoke sets a position through the fields and checks the entry moved
- [x] Docs: usage.md, CLAUDE.md, roadmap

## Notes
- 2026-10-14: `move_root` with no rotation keeps the module's rotation; nothing new in the core.
- 2026-10-14: a newly placed first module isn't selected (existing behaviour), so select it (click it or its build-order row) to see the fields.
- 2026-10-14: `.pos-grid { display: grid }` overrode the `hidden` attribute; `.pos-grid[hidden]` fixes that. The *Connected by* label has the same problem (`#inspector label { display: flex }`): it shows, empty, on free-standing modules. That bug was there before this change and is still open.
- 2026-10-14: tests: a new ui_smoke step (types 1200 / −300 / 450, checks the entry, undo brings the fields back to 0); run on its own as a focused script, which passed.
