# Workflow: re-tune auto-placement for honest layouts

Status: **in progress**

Since the facing rule (2026-10-15, workflow-joined-snaps.md), placement can no longer stack modules inside each other. Layouts are honest but larger, and dense stations seal up: every free snap faces into a neighbouring module, so nothing more attaches (the user's "test" station, Teladi 50 claytronics).

## User direction (2026-10-15)
"for re-tuning, we should explore breaking up the station and adding connectors to make more connection points. maybe have some rules for what we want to insert for instance, if we only have internal modules (those that cant have turrets) left to place, we should try adding connectors on the inside of the station"

## Ideas to explore
- **Break up and insert connectors:** when no free snap works, take a module off the surface (or from inside), put a connection hub (or strut + hub) in its place and re-attach it, so one connection point becomes several.
- **Rules by what is left to place:**
  - only internal modules (no turret/shield slots: habitats, welfare…) left: add connectors on the *inside* of the station, keeping the defensible shell outside (layers, workflow-optimize.md);
  - defensible modules: outside;
  - docks, piers, bays: outermost with clear approach zones (D-12).
- **Exits for any race:** `leaves_exit` keeps one free snap that the run's hub fits on. The "test" station kept one for an Argon cross; a later Terran run (bigger cross) found none. Keep several, or one any race's hub fits.
- Revisit `HUB_PENALTY_M` / `FEWER_HUBS_PENALTY_M`, tuned on stacked layouts.

## Acceptance
- The ignored game-data tests pass again: `auto_place_keeps_room_to_grow`, `fewer_connectors`, `planner_extras` (or their expectations are changed with the user's agreement).
- The user's "test" station (`research/extracted/profile-samples/test-20261015.xml`) takes their extras (10 × `pier_ter_harbor_03_macro`, 5 × `dockarea_ter_m_station_01_standard_macro`, 1 × `defence_ter_claim_01_macro`), or as many as approach zones allow.
- No overlaps; docs numbers (usage.md *Fewer connection structures*, x4-construction-plans.md "Auto-placement") measured again.

## Tools
- Game-data tests `break_up_opens_a_sealed_station` and `auto_place_keeps_room_to_grow`; the user's sealed station: `research/extracted/profile-samples/test-20261015.xml` (git-ignored copy). The scratch examples used on 2026-10-15 were deleted; write new ones as needed and delete them when done.

## Decisions (user, 2026-10-15)
- Rebuild connection structures **off**: move the station's modules only when nothing fits otherwise. **On**: move things around and add connectors freely (Place all lays the station out again too). D-22.
- Undoing a Place all gives its plan items back, added to the plan (user, 2026-10-15).

## Steps
- [x] Agree the approach with the user
- [x] `Design::insert_between` (hub into a joint, subtree moves out; returns the renumbering)
- [x] `Placer::break_up` (joints by layer rule, checked before moving; `Options::break_up`); docks/piers/bays try it with clear space before a blocked spot
- [x] Optimize: `OptimizeOptions::partial` (only added modules may stay out); MCP `optimize_station.partial`, `auto_place.break_up` (default true)
- [x] Builder: Place all follows *Rebuild connection structures*; one placement routine for Place all and Apply plan (partial prompt, undo back to before); undo/redo of a Place all gives plan items back / takes them
- [x] Tests: `break_up_opens_a_sealed_station` (new), `auto_place_keeps_room_to_grow` passes again (break-up on); full core suite passes (2 still ignored)
- [x] Builder check (fresh builder per scenario, largest plot): "test" + 10 piers, 5 dock areas: off → 56 placed, 1 break-up, no problems; undo → extras back next to one added after; redo → taken again. On → rebuilt, 56 added, 2 break-ups, no problems. Optimize Apply (rebuild) → 42 added, no problems.
- [ ] Still ignored: `fewer_connectors` (fewer connectors doesn't always mean fewer now), `planner_extras` (a defence disc's lane at 144 modules)
- [ ] Exits for any race in `leaves_exit`; hub penalties re-measured
