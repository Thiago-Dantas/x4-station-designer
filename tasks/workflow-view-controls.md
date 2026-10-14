# Workflow: middle-click pick, W/S forward, rotation steps and free rotation (user requests)

Status: **done** (2026-10-14); attached modules tilt about their joint (whether the game builds tilted joints is to be checked)

User, 2026-10-14:
- "when placing modules, i want middle mouse click to copy the currently selected module to be placed, like a pallete selection"
- "something happened that WS started behaving like QE / WS should move forward/backward" (also: "it feels faster now")
- "i want to be able to tilt and rotate placed modules, this will enable creating more interesting stations with cool and different geometry"

## Plan
- [x] **Middle click** in the view: a click (not a drag; middle-drag still zooms) on a placed module arms that module type, like a palette click. On empty space it arms the selected module's type.
- [x] **W/S** move forward and backward along the view direction kept level (A/D left/right, Q/E down/up as before).
- [x] **Rotation step** setting (90°, 45°, 30°, 15°, 5°) for twisting attached modules (⟲/⟳, R / Shift+R) and for the twist when attaching. The game's own plans use 3°, 15°, 30° and 45° twists, and its build menu has an angle step.
- [x] **Free rotation** of free-standing modules (with everything attached to them): a rotate gizmo next to the move gizmo, snapping to the rotation step, so they can be tilted in any axis. Core `Design::move_root` already takes a full transform.
  - An attached module can only turn around its snap; tilting it off the snap would detach it. That is left out; ask the user.
- [x] Targeted checks; docs.

## Notes
- 2026-10-14, targeted CDP check (scratch):
  - **W/S:** W moves a snap marker outward from the view centre (242 to 322 px), and S brings it back.
  - **Middle click:** on a placed module, it arms that type.
  - **Rotation step 45:** the twist buttons read 45°, R twists by 45°, and the attach twists offered are 0, 45, …, 315.
  - **Gizmo toggle:** it shows only for a free-standing module.
  - **Root rotation:** `move_root` with a rotation (90° yaw) carries the attached beam from z 600 to x 600, still on its snap. The only issues were plot ones (it now needs 3 km in X).
  - ui_smoke's W check now expects forward motion.
- The rotate gizmo itself (dragging its rings) wasn't driven by the check: synthetic drags on TransformControls aren't reliable. Its result goes through the same `move_root` call.
- **Hypothesis:** the game builds a free-standing module (or a tilted sub-assembly) where its offset says. Test C's floating module showed "Everything is OK" in the plan status, but the build itself was not watched.

## Follow-up (user, 2026-10-14)
"when i click on the rotation step R and shift+R stop working, i have to select again the module" / "after tilting how do i attach? would be nice to tilt while still attached, make it more painless to do"
- [x] The rotation-step dropdown gives up keyboard focus after a change (the shortcuts ignore keys while a dropdown has focus).
- [x] Tilt attached modules: turn them about their joint (where the snaps meet), staying linked, with everything on them.
  - The rotate gizmo also works for attached modules, placed at the joint.
  - R keeps the tilt; *Straighten* removes it.
  - Core: `Design::rotate_attached`, `straighten`; `set_twist` keeps a tilt.
  - The check list names tilted modules ("tilted N° off its snap"): the game's build menu only twists around snaps, so whether it builds tilts is to be checked in game.
- [x] Small fixes seen in the screenshot: the controls text still said "WASD: pan"; two "Equipment" headings (the station one becomes "Station equipment").
- 2026-10-14, checks:
  - Core unit test `tilt_keeps_the_joint_and_survives_twists`.
  - Scratch CDP check of the builder:
    - after a step change focus is back on the page, and a real R press twists by 30°;
    - `rotate_attached` 30° keeps the snap on the joint (only "tilted 30.0° off its snap");
    - R keeps the tilt (twist 30 to 60);
    - *Straighten* removes it, keeping the twist;
    - the new help text and "Station equipment" heading show.

## Follow-up 2 (user, 2026-10-14)
"we should allow free placement of the first module of a station, currently it always centers on the plot" / "also start plots with a 2x2x2x2x2x2 size which is the game default" / "the rotation gizmo currently centers on the connection point, make it center the module like in game"
- [x] First module where you click (ground plane y = 0, rounded to 10 m; a drop too); *Place as first module* keeps the origin.
- [x] New designs: 2 km on every side (4×4×4 km), the game's default.
- [x] The rotate gizmo of an attached module at the module's origin; turning still keeps its joint (`rotate_attached`).
- [x] User, same day: "lets change QE to follow the game default comands / QE is camera rotation / ZX is Z-axis": Q/E turn the camera (like the left/right arrows), Z/X move down/up (Z down, X up; tell the user, easy to swap). Ctrl+Z stays undo.
- 2026-10-14, checks (scratch CDP):
  - a new design shows "Plot 4×4×4 km";
  - clicking left of the view centre with a solar module armed placed it at (-500, 0, -2010) (ground plane, 10 m steps);
  - the rotate gizmo of an attached solar module shows on the module (screenshot);
  - on an asymmetric scene (average snap-cone position), X raises the view (y 539 to 776), Z lowers it back, and E/Q turn the camera and back;
  - game-data test `plots` passes with the 2 km default;
  - ui_smoke's vertical-key check now uses X.
- 2026-10-14, user: the gizmo didn't show while a module was armed for placing. "clicking on the rotate gizmo should disarm the placement and enable rotation on the selected module, only if a module is selected".
  - The gizmo button now stays visible whenever a module is selected (not while moving, or with the build-order preview). Clicked while placing, it disarms and shows the rotate gizmo.
  - Its label follows whether the gizmo is showing: it said "Hide gizmo" while nothing showed.
- 2026-10-14, user: "on the rotate gizmo, it only updates the view when i release the mouse click, i want the view to update as i drag".
  - While a gizmo is dragged, the view moves the module and everything on it along (an attached one swings about its joint); the design still changes once, on release (one undo step).
  - Checked by a real mouse drag over CDP on TerranScrapyard (rotation step 30°), for attached pier #2 and the root #1: mid-drag, before any edit was sent, the modules had turned; the screenshot after release showed the same pose.
