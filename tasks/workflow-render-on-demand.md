# Workflow: GPU load of the 3D view (user report)

Status: **done** (2026-10-15)

User: "detailed rendering is almost 100% of my GPU utilization, can we optimize that?"

## Findings
- The view drew every display frame (`setAnimationLoop`), changed or not: 240 fps on a 240 Hz screen.
- Measured (scratch `gpu_check.py`, Windows counter `GPU Engine(*engtype_3D)`, whole machine; TerranScrapyard, Detailed): desktop alone 4–9 %; builder idle **42 %**, orbiting 42 %.

## Steps
- [x] Draw on demand (`View3D.dirty`): set by every method that changes what is drawn, OrbitControls and gizmo `change` events, held keys, and `controls.update()` (true while damping glides). Frame counter shows "idle" when nothing was drawn.
- [x] Measure: idle **4–9 %** (desktop level), counter "idle"; orbiting unchanged (42 %, it still draws every frame while moving)
- [x] Smoke tests touching the view: ui_smoke, snap_pick_smoke (hover preview pixels), gizmo_checks_smoke: pass
- [x] Found on the way (ui_smoke): since Place all clears the planner lists (v1.1.0), *Buy* on a placed plan re-planned with no wares. A plan now keeps the wares and other modules it was made for (`plan.asked`).
- [x] Docs: usage.md

## Possible next steps (not done; for the user)
- Moving cost: cap the frame rate while moving (e.g. 60 fps), or draw at a lower resolution while the camera moves.
- Detailed only near the camera (per-module LOD by distance).
