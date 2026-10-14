# Workflow: planner extras: defence, ship bays, storage targets (user request)

Status: **done** (2026-10-14); an in-game check is still open

User, 2026-10-14: "i want the planner to be able to add modules like defense discs, ship factories and storage, along with production targets, with the user being able to specify their storage targets (1M container, 500k solid, 200k gas for example). for defense discs we want to be able to command the direction they face, say i want to add 6 discs, they could be split facing forward and back, or 1 in each direction of the cube. if the user ever wants to fine tune they can place manually"

## Game facts (verified 2026-10-14 from the game files, `x4sd list-modules` / `snaps`, macros extracted to the scratchpad)
- Defence modules (class `defencemodule`): per race a Disc Defence Platform (`defence_<race>_disc_01_macro`), a Bridge Defence Platform (`_tube_`) and an Administrative Centre (`_claim_`).
  - The Argon disc has 4 snaps in its plane (±X, ±Z at 200 m). The Terran disc has 6 (±X, ±Z at 400 m, ±Y at 200 m). The bridge has 2 (±Z).
- Ship bays (class `buildmodule`): S/M, L, XL fabrication bays and maintenance bays, generic plus Boron and Terran variants. Workforce max: L fab 700, S/M fab 800, XL fab 700, maintenance 400–500. The planner already counts it (`Module.economy.workforce`).
- Storage (Argon): L 1,000,000 m³ (container, solid, liquid); M container 250,000, M solid and liquid 500,000; S container 50,000, S solid and liquid 100,000. "Gas" in the game's sense is `liquid` storage.
- Today's planner (planner.rs): storage = the largest single-type module of the race, enough for `storage_hours` of the station's flows.

## Proposed design (to agree)
- **Extra modules:** a list in the Planner tab (search any station module, with a count), planned like targets. Covers ship bays, defence modules, welfare, extra docks. Their workforce gets housed and fed as usual.
- **Storage targets:** a minimum capacity per type (container, solid, liquid). Storage per type = enough for max(target, hours rule). Sizes: L modules for the bulk, then the smallest module that covers the remainder.
- **Defence sides:** defence modules take sides (front +Z, back −Z, left −X, right +X, top +Y, bottom −Y). Auto-placement puts each one on the station's outside on its side; placed after everything else so it ends up outermost; docks' corridors and zones stay clear.
- MCP: `PlanArgs` gains `modules` (macro, count, sides) and `storage` (type → m³); `auto_place` module counts take `sides`.

## Decisions (user, 2026-10-14; all as recommended)
- **Direction = side of the station:** a defence module goes on the outside of the station on its side, outermost there. Its own tilt is whatever fits.
- **Input = count + sides:** one count and the chosen sides; the count is spread evenly (6 + front/back = 3 + 3; 6 + all six = 1 each). Extra counts go to the first sides in the order front, back, left, right, top, bottom.
- **Ship bays = bays + workforce:** no ship-building wares; the user adds those as targets.
- **Storage targets = minimums on top of the hours rule:** per type, the larger of the two; hours 0 = targets only.

## Placement rule for sides (our design)
- Modules with a side are placed after everything else (docks too), so the station they face out from is complete.
- A spot is ranked by its distance from the axis through the station's centre along the side (the middle of that face first).
- It needs a clear **lane**: the module's bounds extended straight out on its side, from its front face to 3 km past the station's bounds. No module may be in it. Once placed, the lane stays clear, so a second disc doesn't go in front of the first.
- If no spot has a clear lane, it goes to the best-ranked spot that fits and is counted as blocked (like docks).

## Steps
- [x] Agree the design
- [x] Core `planner.rs`: `Request.modules` (extra modules, role `extra`), `Request.storage` (minimum m³ per type); storage sizing L bulk + smallest covering remainder
- [x] Core `layout.rs`: `Side`, `Options::sides` (macro → sides, spread evenly), side ranking + lanes, `Placement::sides_blocked`
- [x] Ops/MCP: `PlanArgs.extra_modules` (macro, count, sides; not `modules`, which `OptimizeArgs` already has), `PlanArgs.storage`; `auto_place` modules take `sides`; MCP instructions
- [x] Builder: Planner tab extra-module search + rows with side chips; storage targets (container/solid/liquid, "1M", "500k"); Place all passes sides
- [x] Tests (targeted): game-data test `planner_extras`; scratch CDP check of the builder
- [x] Docs: usage.md, decisions (D-15), x4-construction-plans.md, CLAUDE.md, roadmap
- [ ] In game (user): a station with discs on its sides builds; storage and ship bays as planned

## Notes
- 2026-10-14: **free snaps limit sides.** On "100 solar" (85 connection structures) every disc lands in the middle of its face. On "2 hull parts" (29 modules, 1 hub) the station has a handful of free snaps and none on some faces, and connector routes from them are blocked. Hence the three passes (on its half with a clear lane; clear lane; anywhere) and `sides_blocked`, which the builder turns into "move them by hand".
  - First rank tried: distance from the axis only. It put a "front" disc beside the station (x −2,400, z 0). Now it subtracts how far out the spot is.
  - "On its half" first compared centres; a disc centred at z 0.0002 m passed. Now all of the module must be beyond the centre plane.
- Storage sizing changed for the hours rule too (largest + smallest that covers the rest, not only the largest). "100 solar" keeps its 9×8×8 km plot (it was already 9×8×8 before this change; the 8×8×8 in workflow-production-planner.md is older).
- Builder check (scratch CDP, release build): Hull Parts + 6 Argon discs Front/Back + L fabrication bay, storage 1M container / 200k liquid, junk "lots" refused. The plan shows *Other modules*; storage 2 L container (hours rule), 1 L solid, 1 M liquid. Place all: 27 modules + 1 connection structure, "No problems found"; back discs at z −2,800 m in a row; the front three couldn't go to the front and were reported.
- The 3D view has no marker for the station's front (+Z); the side buttons' tooltips name the axis.
