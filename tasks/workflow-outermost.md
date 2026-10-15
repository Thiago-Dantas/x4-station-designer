# Workflow: defence platforms and piers outermost — **done**

User report (2026-10-15, screenshot): on the "test" station, an Optimize re-layout put the Terran Disc Defence Platform deep inside the station. "Large piers and defense modules should always be on the outermost."

## Cause
`layout::layer` put defence modules (class `defencemodule`) in layer 1 with production and storage, so Optimize put them in the shell around the core. Only dock areas had a clear corridor out; piers relied on their 100 km approach zone.

## Steps
- [x] Repro test `optimize_puts_defence_and_piers_outside`: "test" + 5 dock areas, 10 piers, 2 discs, Optimize (rebuild). Before: 4 of 13 piers/defence modules had a module on the line out (a disc behind production)
- [x] Fix (D-23): `layout::outermost` = dock areas, piers, defence modules: outer layer and the 3 km corridor, in Optimize and Place all; a module placed on a side keeps its lane instead (else `planner_extras` got worse). After: 0 of 13
- [x] Piers: now need the corridor too (the 2026-10-14 megashipyard problem is gone with break-up: `optimize_keeps_zones_clear` passes)
- [x] Other placement tests: all pass; ignored `fewer_connectors` and `planner_extras` fail exactly as before. Cost: more connection structures (24 → 38 on the repro, megashipyard 17 → 25)
- [x] Wording: status line, MCP descriptions, ops; docs: x4-construction-plans.md, usage.md, decisions (D-23), README; roadmap
- [x] Commit and push

Not checked in the builder over DevTools: the change is in the core (covered by the test); the UI change is one status-line string.

## Round 2 (user, 2026-10-15, screenshots of "test" with 12 Terran discs)
"Defence discs are still on the inside of the station. We have 2 station arms full of production modules, we should have defense discs at the end of either side. Also the station is too much of a cross format, we have lots of empty plot space; instead of just building along a path we should try to fill plot space following our placement rules."

Sample: `research/extracted/profile-samples/test-20261015-discs.xml` (the user's `constructionplan\test.xml`, 215 entries: 168 modules, 12 discs, 1 claim, 10 piers, 6 dock areas, 47 connection structures). Scratch examples `shape.rs`, `snaps.rs` (deleted).

Findings:
- Rebuild reproduces the screenshot: 20x11x19 km, 47 connection structures. All 13 defence modules within 2.5 km of the centre (median module 4.5 km): the corridor rule is met by a clear line up or down. Too weak.
- Cause of the arms: most production is dead ends (smart chips 1 snap; refined metals 2 snaps on one face). Dead ends take a hub's side snaps, so growth goes straight on. The Terran disc has the cross hub's 6 snaps.
- Hub penalty 0/500/1000/2000: no real change.
- Biggest-first (*Place all* order, no layers) on the same modules: 11x13x13 km, 29 connection structures. Layers: 20x11x19, 47.
- Layers without the shell (plain nearest-first in layer 1): median 3.2 km instead of 4.5, plot 20x11x17 (piers and dock areas still go out on arms).
- Fixed plot 12 km cube: all fit, 35 connection structures, 2 break-ups, 1 dock blocked, 6 s; 14 km cube: 38, 1 dock blocked, 8.5 s.

- [x] Agree the approach with the user: **both** (layer order and filling a plot), D-24
- [x] Layer 1 nearest-first (shell and `core_radius` removed)
- [x] Defence platforms last, spread (`Placer::spread`: distance + distance from the nearest defence platform, capped)
- [x] `Options::fill`; Optimize rebuild tries cubes 1..10 km a side, then free as before (keep shape: unchanged)
- [x] Test `optimize_puts_defence_and_piers_outside` now on the 12-disc sample: plot <= 12 km per axis (8x10x9), clear lines out, every defence module in the body's outer half. Placement tests pass; ignored ones fail as before
- [x] Docs: x4-construction-plans.md (*Filling a plot*, *Defence platforms spread*), usage.md, D-24, MCP descriptions, README
- [x] Commit and push

Costs (measured): TerranScrapyard rebuild 7 -> 27 connection structures (10x11x12 km instead of 13x13x12), megashipyard rebuild 25 -> 54 and 12 -> 17 s; keep shape 5.6 -> 14 s on the megashipyard (piers try more snaps in the compact layer order). Not checked in the builder over DevTools; the builder's rebuild calls the same `optimize`.

## Round 3 (user, 2026-10-15)
"We are counting Administrative Centre as a defense platform, it's in the defense section but it holds no equipment; administrative centers are a special case and they should be in the innermost."
- [x] Data: all 7 `claim` modules have 0 slots; disc and bridge platforms 22-56
- [x] `layout::defence` = defence module with slots; `layout::admin` = without: placed first (before every layer, also in *Place all*)
- [x] Test `optimize_puts_defence_and_piers_outside`: claim in the inner tenth of the body (746 m, 137 of 139 farther), 12 discs outer half; 10x10x12 km, 35 connection structures
- [x] Docs: D-23, D-24, x4-construction-plans.md, usage.md, MCP descriptions
- [x] Commit and push
