# Workflow: exact overlap checks on collision meshes (user report)

Status: **done** (2026-10-14)

User, 2026-10-14, with a screenshot of TerranScrapyard showing 6 overlap issues: "something wrong with overlap detection, my terranscrapyard works and was extracted ingame".

## Cause
- `validate` reports two modules as overlapping when their **collision boxes** intersect by more than 1 m (D-9).
- These boxes are axis-aligned boxes around groups of collision-mesh triangles: up to 128 per part, 100 m or less. Around curved or diagonal geometry (the Terran L habitat rings) they enclose empty space.
- The calibration already showed that some depths shrank with finer boxes (98 m to 45 m), so they were box looseness. The ones that stayed were taken for real intersections, but they may also be looseness, from rings.

## Plan
- [x] Core: `CollisionMesh`: the module's collision-mesh triangles in its frame, with a bounding-volume tree (leaves of a few triangles), cached per macro. Collision boxes come from the same parse.
- [x] Core: `layout::meshes_intersect(a, at_a, b, at_b)`: tree-against-tree traversal (box tests), then triangle-against-triangle (segment-triangle tests).
- [x] `overlaps` (validate): pairs the boxes flag are confirmed on the meshes. Auto-placement keeps the box test, which is stricter, so what it places still validates clean.
- [x] Measure on the player plans (expect TerranScrapyard clean) and the library plans; timing.
- [x] Tests (calibration test updated), ui_smoke, docs (x4-construction-plans "Overlaps", D-9 note).

## Notes
- 2026-10-14: results.
  - Player plans: 0 overlaps (all 14 box pairs clear on the meshes). TerranScrapyard opens in the builder with "No problems found".
  - Library: 28 of 84 confirmed.
  - Time: 264 ms first (meshes loaded), then 17 ms.
  - The medical-supplies collision mesh has 75,040 triangles and 32,767 tree nodes.
  - Test C's moved casino (entry 32, 1 km up) now overlaps entries 29 and 65: a real overlap, correctly reported.
  - The issue text no longer shows the box depth ("by about N m"), which was misleading.
- The builder was open while verifying, holding `target\release\x4sd.exe`. So the smoke test ran a copy built in `target\verify`. mcp_smoke refuses to run while a builder is open (it would edit that design); it passed before this change, which doesn't touch MCP code.
