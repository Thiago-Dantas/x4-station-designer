# Workflow: live checks while dragging the gizmo (user request)

Status: **done** (2026-10-14)

User: "checks should happen as I move, not only when I let go of the click, so I know I'm in a green position".

## Plan
- [x] App command `preview_gizmo(index, position, rotation)`: on a copy of the design (`Design::from_plan` plus the plot), `move_root` for a free-standing module or `rotate_attached` for an attached one; returns `{issues, plot}` only. The design and undo don't change.
- [x] View: `onGizmoDrag` while dragging (game frame), `setPlotFits` recolours the plot box, `gizmoDragging()`
- [x] UI: one preview request in flight, the latest gizmo pose wins, repeats skipped; Checks list and plot summary redrawn (`renderChecks`); late answers after the drag ends are dropped. The real checks come with the edit on release, as before.
- [x] Test: `research/gizmo_checks_smoke.py` (real mouse drag on the X arrow)
- [x] Docs: usage.md, CLAUDE.md, roadmap

## Notes
- 2026-10-14: a preview takes about 50 ms on a 434-module station (`megashipyard terran`) and 10-50 ms on 84 modules: about 20 updates a second at worst.
- 2026-10-14: test: mid-drag the plot box turns red and Checks shows "outside the plot" (2 issues) while the design stays at X = 1500; dragging back inside, with the button still held, clears them.
- Not done: the dragged module's red issue tint, and the X / Y / Z fields, still update only on release.
