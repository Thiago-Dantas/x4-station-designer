# Workflow: re-tune auto-placement for honest layouts

Status: **open** (not started)

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
- Scratch examples (untracked): `crates/x4sd-core/examples/scratch_docks.rs` (place a module list on plans; `MODS`, `RACE` env), `scratch_free.rs` (free snaps, which hubs fit), `scratch_rebuild.rs` (Optimize with a module list), `scratch_clay.rs`, `scratch_opt_docks.rs`.
- Worktree `../x4sd-prev` (detached, older HEAD) for comparisons.

## Steps
- [ ] Agree the approach with the user
