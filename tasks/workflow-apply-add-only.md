# Workflow: adding modules doesn't move the station (user report)

Status: **done** (2026-10-14)

User, 2026-10-14: "When we generate a plan and apply, we are running the optimizer and changing station layout, even without either of the checkmarks checked."

## Findings
- *Place all* (`layout::auto_place`) lifted the station's dock areas, piers and bays with nothing attached and placed them again outside, on every run.
- Optimize's *Apply plan* always re-laid the station out; *Rebuild connection structures* off only meant "keep the connection structures".

## Decisions (user)
- The flow seen: Generate plan → Place all.
- *Rebuild connection structures* off: Apply plan only adds the modules. On: full rebuild.
- Existing docks, piers and bays stay where they are (D-18).

## Steps
- [x] Core: `auto_place` no longer lifts docks (`lift_candidate`, `docks_moved` removed); `Placer::new` already protects the zones and corridors of those on the station
- [x] Ops/MCP: `docks_moved` dropped from `auto_place`'s result and description
- [x] Builder: Apply plan with Rebuild off = auto-place the plan's modules + fit the plot; on = `optimize_station` with `keep_shape: false`; texts and tooltips
- [x] Tests: `auto_place_planned_stations` (nothing on the station moves, docks included; second run blocks no zone and no dock-area corridor), `optimize_station`, `optimize_keeps_zones_clear` pass
- [x] Check (scratch CDP): TerranScrapyard, Optimize, both boxes off, Apply → "Placed 203 modules and 22 connection structures", none of the 84 modules moved, plot 11×8×10 km, no problems
- [x] Docs: usage.md, x4-construction-plans.md, decisions D-18

## Notes
- A pier's approach follows its own axis (its 100 km zone), not the line out from the station centre: new modules may sit on that line once the pier stays put, and that's fine; the zones stay clear.

## Follow-up: clear the queue after Place all (user request, 2026-10-14)
User: "after we accept a generation plan and click Place All, we should clear the queue of things to add".
- [x] Builder: once Place all has placed everything, the wares and other modules lists are emptied (saved); with something left (no room) they stay, so a retry keeps the sides
- [x] Check (scratch CDP): hull parts × 1 + an L container storage, Generate, Place all → "Placed 16 modules", both lists empty, stored empty, plan rows still shown
- [x] Docs: usage.md
