# Workflow: x4ep_planner "Optimize" (user request)

Status: **done** (2026-10-14); an in-game check of an optimized station is still open

User, 2026-10-14: "now i want to add the x4ep_planner optimize functionality. a few things when reordering modules we should strive to do:
* defensible modules should be outwards, modules that cant have turrets/shields should be towards the center
* docks/piers should be at the outermost and unblocked"

## What exists
- The mod's *Optimise* (readme, see workflow-production-planner.md) analyses a station: shortfalls and surpluses, whether the workforce is housed and fed, pier/dock present, modules that fix each shortfall.
- We have that already: the right pane's *Production* (analysis), and the Planner's *Generate plan* with no wares (the modules that fix the shortfalls), plus *Place all*.
- Auto-placement fills the nearest free snaps first, so what is placed first ends up central. Docks and piers go last with a clear corridor and are re-placed on every run (workflow-docks-fps.md).

## Proposed
- Placement order by layer: modules without turret/shield slots first (habitats, welfare, build modules…), then modules with slots ("defensible": production, storage, defence), then docks and piers. Within a layer, biggest first. This applies to every auto-placement (Place all too).
- *Optimize* (open: see questions): analyse, add the fixes, and/or re-lay the whole station out with those layers.

## Questions for the user
- What Optimize does: re-layout plus shortfall fixes, re-layout only, or report and fixes only?
- Re-layout: rebuild connection structures from scratch, or keep the existing ones?

## Decisions (user, 2026-10-14)
- **Optimize = fix + re-layout:** analyse the station, add the modules that fix its shortfalls (the Planner's settings, no target wares), then re-place every module with the layers. Equipment stays with each module type. One undo step.
- **Connection structures: a toggle.**
  - **Keep the shape** (default): keep the connection structures, and the modules that carry them, in place; lift the rest.
  - **Full rebuild:** lift everything and let auto-placement add the hubs and struts it needs.
- **All or nothing** (our choice): if any module finds no place, nothing changes and the report says what didn't fit. Optimize never loses modules.

## Steps
- [x] Agree the design
- [x] Core: layered order in `auto_place` (no slots, then slots, then docks; biggest first within a layer)
- [x] Core: `layout::optimize(catalog, design, fixes, OptimizeOptions { keep_shape, grow_plot, race })`
  - lift: everything (rebuild) or non-connector leaves, repeatedly (keep the shape);
  - auto-place the lifted modules plus the fixes;
  - give each placed module back the equipment of one lifted module of its type;
  - all or nothing; one undo step.
- [x] Ops/MCP `optimize_station` (PlanArgs without targets, `keep_shape` default true). The builder's `run_op` runs off the main thread (optimizing takes seconds).
- [x] Builder: Planner tab *Optimize station* + *Rebuild connection structures* checkbox, status line
- [x] Tests (targeted): TerranScrapyard both ways (nothing lost, no overlaps, docks outside, layers, one undo step); a UI click
- [x] Docs

## Notes
- 2026-10-14: **layers vs. compactness.** In the plain layer order (no slots first, nearest-first), big habitats skip near gaps that smaller defensible modules then fill: defensible ended *nearer* the centre (TerranScrapyard: 1.8 km against 2.3 km for the habitats).
  - **Fix:** defensible modules rank spots by distance from the core's radius (the shell), so they wrap the core. Result: slotless 2.1–2.3 km, defensible 3.7–3.8 km.
  - **Cost:** the shell, and even the layer order alone, makes "100 solar" need 20×20×20 km instead of 8×8×8. So layers are an option (`Options::layered`): Optimize uses it, and *Place all* stays biggest-first (compact).
- Placement now measures "nearest" from the centre of the station as it was, not the origin. That changes nothing for new designs, and helps off-centre stations.
- `run_op` (builder) runs off the main thread: Optimize takes about 2–3 s on TerranScrapyard.
- Checks:
  - game-data test `optimize_station`, both modes: nothing lost, equipment kept, no overlaps, docks clear, layers, one undo step;
  - `auto_place_planned_stations` still gives 8×8×8 km for "100 solar";
  - a scratch CDP check of the builder: Optimize, keep shape (84 to 343 modules) and rebuild (84 to 345), "No problems found" both, undo back to 84.
