# Workflow: middle-click pick, W/S forward, rotation steps and free rotation (user requests)

Status: **done** (2026-10-14); tilting attached modules left out (would detach them), see notes

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
