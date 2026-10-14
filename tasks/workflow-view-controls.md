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
