# Workflow: better camera on small designs

Status: **done** (created and finished 2026-10-14)

Roadmap item "Builder usability: better camera on small designs". I proposed it myself in Phase 4; it isn't a reported problem yet. The user wants it done (2026-10-14).

## The problem (suspected, not measured)
The camera is tuned for big stations, so with one or a few modules:
- **Empty design:** `View3D.frame()` ([ui/src/view3d.ts](../ui/src/view3d.ts), around line 475) frames a fixed ±2 km box, so a first module of a few hundred metres looks tiny.
- **No refit after the first placement:** `apply(..., frame)` in [ui/src/main.ts](../ui/src/main.ts) refits only when the caller asks (opening a plan, a new design). Placing the first module with "Place as first module" may not refit. Check which calls pass `frame = true`.
- **Zoom limits:** OrbitControls has no `minDistance`/`maxDistance`. The camera's near plane is 10 m (far 500 km, `PerspectiveCamera(50, 1, 10, 500000)`), so zooming in close to one module clips through it.
- **Key speeds:** WASD and Z/X move at `distance × 0.8` per second (`keyNavigate`). Q/E and the arrows turn at a fixed 1.5 rad/s. Close to a small module, check whether that feels too jumpy or too slow.
- **Wheel steps:** check how the wheel zoom feels close up.

## Steps
- [x] Reproduce. Use `research/ui_smoke.py`-style scripting with CDP (`research/cdp.py`) and screenshots:
  - an empty design;
  - one small module (e.g. a Terran S habitat or a connection structure);
  - a few modules.
  - Note what looks wrong before changing anything.
- [x] `frame()` (no change needed, see the repro): frame the placed modules tightly with a minimum distance (so one module fills a sensible part of the view). Keep the empty-design default sensible, e.g. the plot box or about 1 km. Only the module meshes count, not the snap cones, the plot box or the grid.
- [x] (already so) Refit after the first module is placed (the design went from 0 to 1 modules), not on every placement.
- [x] Zoom limits that follow the design size:
  - `minDistance` so the camera can't pass through a module;
  - the near plane scaled with the distance, so close-ups don't clip and big stations don't z-fight;
  - `maxDistance` around the plot size.
- [x] (unchanged, fine) Check the key and wheel speeds close up. Adjust only if they feel wrong, and keep big stations unchanged.
- [x] Verify:
  - screenshots of the cases above before and after;
  - big stations (TerranScrapyard) still frame and move as before;
  - existing `ui_smoke.py` steps 5–6 (WASD/Q/E/arrow checks) still pass.
- [x] Docs: `docs/usage.md` View bullet if behaviour changes; tick the roadmap item.

## Notes
- **Repro 2026-10-14** (`research/camera_smoke.py`, screenshots `before-*.png` in the scratch dir):
  - Empty design: Fit view frames the ±2 km box, which is the default plot box. Fine.
  - One small module (Terran beam, *Place as first module*): it already refits. All three first-placement paths (button, ground click, drop) pass `frame = true`. The beam fills about half the view's width. Fine.
  - A few modules (cross + 3 beams): Fit view frames them tightly. Fine.
  - **Wheel in:** no stop. The orbit target is the module's centre, so 30 notches reach the hull and 60 go inside the beam (only clipped slivers show). This is the real problem.
  - **Wheel out:** no limit. 60 notches out on an empty design shrink the 40 km grid to a speck.
  - W/S at 25 notches in: the speed scales with distance and feels fine. Wheel step (×0.95^1.2 per notch, three r186) also fine.
  - So `frame()` and the refit need no change; the work is the zoom limits.
- **Done 2026-10-14** in `ui/src/view3d.ts`:
  - `stopAtSurface()` runs on OrbitControls' `start` event (wheel and drag zooms). It casts a ray from the camera to the orbit target. If a module is in the way, `minDistance` becomes target-to-surface plus 10 m. That distance doesn't change while dollying, so one ray per zoom step is enough. The floor is 10 m, and `frame()`/`frameModule()` reset it so an old stop can't clamp them.
  - Near plane = distance / 200 (at least 0.5 m), set every frame. At 2 km that's the old 10 m; far views get more depth precision. Far stays 500 km.
  - `maxDistance` = 2 × the distance that fits the larger of the plot box and the station, set in `render()`.
- **Verified** (`camera_smoke.py`, `before-*`/`after-*` screenshots, before = the same script on a build with view3d.ts stashed):
  - one beam: 100 notches in now stop on the hull (100 % hull, no clipping); before, the camera was inside (0.1 %);
  - empty design: 60 notches out stop with the 4 km plot box still well in view (before: a speck);
  - TerranScrapyard: fit 11.9 %, wheel 20 in 80.7 %, then W 73.4 % (73.5 % before; identical otherwise); a double-click zoom to a module right after a hull stop frames it.
  - `ui_smoke.py` passes all steps (5–6: D/W/X/arrow). It needed two fixes for earlier changes unrelated to this task: the heading is now "Station equipment", and the remembered blueprint limit (D-16) is switched off during the run and restored afterwards.
