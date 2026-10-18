# Workflow: scroll the planner tab

Status: **done** (2026-10-18)

User, 2026-10-18: "i need to be able to scroll up and down on the planner. on a large enough plan i cant see anything".

## Cause
The Planner tab is a column that fills the left pane. Only the generated module list (`#planner-result`) scrolled, and it was allowed to shrink to nothing. Targets, other modules and the settings above it do not shrink, so on a long plan they fill the pane. The module list, *Generate plan* and the hint then sit past the bottom of the window, and the page itself does not scroll (`body` is `overflow: hidden`).

## Steps
- [x] The Planner tab is the one scroller. The module list sizes to its rows and moves with that scroll.
- [x] The ware and other-module suggestion lists are positioned on the window, so the scrolling tab does not clip them. They open upward when they would run past the bottom, and scrolling the tab keeps them next to the box.
- [x] Checked in the builder (debug exe, DevTools, maximized 1920×1017): a short plan shows *Generate plan* with nothing to scroll; 30 targets and 40 module rows make the tab scroll (2000 px in an 898 px pane) and the module list is not its own scroller; the wheel over a row moves the tab; the last row and the hint come into view; the ware list opens under its box; the other-module list, parked at the bottom of the pane, opens upward and follows the box when the tab scrolls.
- [x] Docs: `docs/usage.md`, this file, the roadmap line. `CLAUDE.md` has nothing to add.
