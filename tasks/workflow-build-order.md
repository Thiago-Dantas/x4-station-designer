# Workflow: build order viewer and editor (user request)

Status: **done** (2026-10-13); the in-game check is still open

User request: "can we edit the module build order? ... change the build order and set priorities for specific modules; a module build order viewer would be nice", later: "drag and drop on the build order to reorder things".

## Background
See docs/x4-construction-plans.md, "Build order and stages":
- Order = entry order, with every module after its predecessor.
- Stages = `bookmark="1"`.
- No priority field exists; a priority is a position in the order.

## Steps
- [x] Core (`design.rs`): `move_in_build_order` (checked: never before its predecessor), `build_first` (+ chain it hangs on), `build_last` (+ everything on it), `set_stage_end`. All undoable, renumbering, and returning the moved module's new index.
- [x] Ops/MCP: `move_in_build_order`, `build_first`, `build_last`, `set_stage_end`; `describe_design` has `stage_end`, and reorders return `moved_to`
- [x] Builder: Build order tab (rows, stage dividers, drag a row onto another, click to select), *Show up to step* slider (later modules hidden in 3D, rows dimmed); selection panel with *Build first* / *Build last* / *A build stage ends here*
- [x] Selection follows the moved module (scene `focus`)
- [x] Drag and drop: `dragDropEnabled: false` (Tauri's file-drop handler blocks HTML5 drag and drop on Windows)
- [x] Tests:
  - unit test `build_order_moves_respect_attachments`
  - ui_smoke: 84 rows; step 10 dims 74; build first moves "Terran L Solid Storage" from #50 to #4, still selected; stage divider; undo; synthetic drag of row 14 onto row 10
- [ ] In game: export a reordered plan and watch the build order (needs the user)

## Notes
- 2026-10-13: the native drag path can't be automated, because CDP and synthetic events bypass the Windows OLE drop target that Tauri's handler occupied. The user needs to confirm it by hand.
