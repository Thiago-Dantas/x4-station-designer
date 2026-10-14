# Workflow: choose the connecting snap (Move and placing)

Status: **done** (2026-10-14); the user keeps the preview (C)

User request (2026-10-14): after detaching a defence disc, reattach it by its top snap. Pick which of the module's own snaps connects, when moving and when placing. C (preview) is on trial: the user decides after seeing it whether it stays.

- [x] A. Move: the moving module's own snaps (those not holding its own modules) show as orange cones, the connecting one bright and larger; clicking one picks it (`View3D.setOwnSnaps`, pick kind `own`)
- [x] B. Direction labels in the *Connect by snap* dropdown (`top · connectionsnap006`): the strongest axis of the snap's facing in the module's frame (`snapDirection` in main.ts)
- [x] C. See-through preview of the armed module (or the moved part) on the hovered cone, joined by the chosen snap with the chosen twist (`attachMatrix` = the core's `geometry::attach`, `View3D.setGhost`); C / Shift+C cycles the snap, T / Shift+T the twist. `module_shape` and free snaps now carry each snap's `rotation`.
- [x] Verify: `research/snap_pick_smoke.py` passes (Terran disc: labels, C to top, T to 90°, preview pixels appear on hover, attach uses top + 90°; detach, Move, click an orange cone picks `right`, the move uses it). The preview pose matches the core's attach result to 3e-8 (attachMatrix run in Node on the dumped data).
- [x] Docs: usage.md, dev-setup.md, CLAUDE.md, roadmap
- [x] User decides whether C (preview) stays: kept (2026-10-14)
- [x] D. (user request, 2026-10-14: "move and move back sucks") *Connected by* in the Selected panel of an attached module: changes its connecting snap in place (same parent snap and twist, `reattach_module`); C / Shift+C cycles it. Smoke step 4 passes (C, then the dropdown to top; no problems found).

Finding: only the Terran and Boron disc defence platforms have a top (+Y) snap; the Argon, Teladi, Paranid and Split discs have side snaps only (`x4sd snaps <macro>`).
