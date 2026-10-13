# Workflow: move placed modules; warn about unsaved changes (user requests)

Status: **done** (2026-10-13)

User: "we need to alert of closing without saving, also i would like to move modules around, looks like theres no way to move modules that have been placed, only remove them".

## Plan
- Unsaved changes:
  - [x] Core `Design`: snapshot at the last save/export/open, `is_dirty()`, `mark_saved()`
  - [x] Ops/app: save and export mark the design saved; the scene carries `dirty`
  - [x] UI: unsaved marker (toolbar and window title); ask before closing, New and Open
- Moving:
  - [x] Core `reattach(index, snap, to, to_snap, twist)`:
    - moves the module and its subtree rigidly onto another free snap;
    - refuses targets inside its own subtree, and snaps in use;
    - re-sequences the subtree after the new parent if needed;
    - is undoable and returns the new index
  - [x] Core `detach(index)`: the module becomes free-standing in place
  - [x] MCP: `reattach_module`, `detach_module` (roots already have `move_module`)
  - [x] UI: *Move…* mode (cones outside the subtree, the moved module's snap and twist, Esc cancels), *Detach*, a translate gizmo for free-standing modules
- [x] Tests (unit + ui_smoke) and docs

## Notes
- 2026-10-13: dirty = plan differs from the snapshot taken at the last save, export or open, so undoing back to it counts as unchanged. Agent saves and exports over the live link refresh the builder's marker: `changes_design` now includes them.
- 2026-10-13: closing uses `getCurrentWindow().onCloseRequested`; after it, Tauri destroys the window, which needs `core:window:allow-destroy`. The window title needs `core:window:allow-set-title`.
- 2026-10-13: tests:
  - unit tests `reattach_moves_subtree_and_keeps_order_valid` and `dirty_tracks_changes_since_saved`;
  - ui_smoke: the unsaved marker; Move… re-attaches the beam onto another cross snap (no problems); the `move_root` command moves the station; save clears the unsaved state.
  - Not automated: the close prompt (a native dialog) and dragging the gizmo by mouse.
