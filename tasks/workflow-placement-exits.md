# Workflow: auto-placement gets stuck (keep an exit open) — **done**

User report (2026-10-15): a new Teladi station with a 50-module claytronics target placed 38 of 461 modules,
then "No room for" everything else.

## Cause (verified with a debug trace)
Modules with one snap (here `prod_gen_water`, `prod_gen_microchips`, `prod_gen_claytronics`) are dead ends. Nearest-first
placement put them on the station's outward snaps until the only free snaps left were the side snaps of the small
Teladi hubs (200 m arms), which overlap the big neighbours. With no snap a hub fits on, nothing could grow,
although the station was 3 km across and could reach 20 km.

## Steps
- [x] Reproduce headless (MCP `plan_production` + `auto_place`, Teladi, workforce, 6 h): 38 placed, 18 connectors
- [x] Trace why: every free snap blocked (temporary debug example, removed)
- [x] Fix in `layout.rs`: a placement may not close the station's last free snap a hub fits on (`Placer::leaves_exit`); without such a spot, place anyway
- [x] Verify: game-data test `auto_place_keeps_room_to_grow` (461 placed, also with fewer connectors; no overlaps, valid); MCP repro through the rebuilt exe; `auto_place_planned_stations` gives exactly the same results as before
- [x] Docs: docs/x4-construction-plans.md, "Auto-placement", *Room to grow*; roadmap line. CLAUDE.md unchanged (no new module or path)
- [x] Commit and push
