# Workflow: shift-select in the build order (user request)

Status: **done** (2026-10-16)

User request (2026-10-16): "on the load order screen, i want to be able to shift select multiple modules for dragging". That screen is the Build order tab.

The row you grab lands on the row you drop on. The rest of the range stays in the same order around it, as one undo step.

Follow-up the same day: a drop that would build a module before the one it attaches to was refused with "module X would be built before module Y". That was wrong for this screen. The block goes to the closest step that still keeps every module after the one it attaches to. If that is where it already sits, nothing moves and there is no undo step. Dragging a row uses the pointer, so the mouse wheel keeps scrolling the list during the drag and afterwards.

- [x] Core: `Design::move_block_in_build_order` snaps to the closest legal step (`build_order_block_moves_together`)
- [x] Builder: pointer drag, wheel scrolls during the drag, closest step instead of the error
- [x] Docs: `docs/usage.md` and the tab hint
- [x] Check: DevTools on TerranScrapyard (a real mouse drag, wheel 0→350 during the drag and 350→700 after; a 3-module drop that could not take the exact slot landed as `Moved 3 modules to steps 6-8`, then undo). Local release build: `target\release\x4sd-next.exe` (the open `x4sd.exe` was locked, so the new exe has this name)
