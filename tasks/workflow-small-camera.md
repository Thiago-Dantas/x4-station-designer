# Workflow: better camera on small designs

Status: **not started** (created 2026-10-14)

Roadmap item "Builder usability: better camera on small designs". I proposed it myself in Phase 4; it isn't a reported problem yet. The user wants it done (2026-10-14).

## The problem (suspected, not measured)
The camera is tuned for big stations, so with one or a few modules:
- **Empty design:** `View3D.frame()` ([ui/src/view3d.ts](../ui/src/view3d.ts), around line 475) frames a fixed ±2 km box, so a first module of a few hundred metres looks tiny.
- **No refit after the first placement:** `apply(..., frame)` in [ui/src/main.ts](../ui/src/main.ts) refits only when the caller asks (opening a plan, a new design). Placing the first module with "Place as first module" may not refit. Check which calls pass `frame = true`.
- **Zoom limits:** OrbitControls has no `minDistance`/`maxDistance`. The camera's near plane is 10 m (far 500 km, `PerspectiveCamera(50, 1, 10, 500000)`), so zooming in close to one module clips through it.
- **Key speeds:** WASD and Z/X move at `distance × 0.8` per second (`keyNavigate`). Q/E and the arrows turn at a fixed 1.5 rad/s. Close to a small module, check whether that feels too jumpy or too slow.
- **Wheel steps:** check how the wheel zoom feels close up.

## Steps
- [ ] Reproduce. Use `research/ui_smoke.py`-style scripting with CDP (`research/cdp.py`) and screenshots:
  - an empty design;
  - one small module (e.g. a Terran S habitat or a connection structure);
  - a few modules.
  - Note what looks wrong before changing anything.
- [ ] `frame()`: frame the placed modules tightly with a minimum distance (so one module fills a sensible part of the view). Keep the empty-design default sensible, e.g. the plot box or about 1 km. Only the module meshes count, not the snap cones, the plot box or the grid.
- [ ] Refit after the first module is placed (the design went from 0 to 1 modules), not on every placement.
- [ ] Zoom limits that follow the design size:
  - `minDistance` so the camera can't pass through a module;
  - the near plane scaled with the distance, so close-ups don't clip and big stations don't z-fight;
  - `maxDistance` around the plot size.
- [ ] Check the key and wheel speeds close up. Adjust only if they feel wrong, and keep big stations unchanged.
- [ ] Verify:
  - screenshots of the cases above before and after;
  - big stations (TerranScrapyard) still frame and move as before;
  - existing `ui_smoke.py` steps 5–6 (WASD/Q/E/arrow checks) still pass.
- [ ] Docs: `docs/usage.md` View bullet if behaviour changes; tick the roadmap item.

## Notes
