# Workflow: x4ep_planner functionality in the designer (user request)

Status: **done**: A, B and C done (2026-10-13); the user's in-game check of an auto-placed station is still open

User: "next i want to add x4ep_planner mod functionality".

## What the mod does (from its readme.txt, v2.81)
1. **Plan:** pick target wares and how many modules of each. It works out the whole chain:
   - every intermediate ware and module;
   - the workforce and the habitats that house it;
   - storage (sized for N hours, 6 by default);
   - pier and dock area;
   - the build cost.
   - Settings: race, supply (make or buy, per ware), sunlight, use any race's modules, storage hours.
2. **Optimise:** analyse an existing station. It reports:
   - shortfalls and surpluses per ware;
   - whether the workforce is housed and fed;
   - whether there is a pier or dock;
   - the modules that would fix each shortfall.
3. **Build This Design:** hands the module list to the engine's layout generator (`create_construction_sequence`), which picks the positions. Anything that doesn't fit is listed for placing by hand. It also lists the modules still to place, counting down as you place them.

**Licence:** "copying its code into another mod needs mine first". We write our own from the game files and don't copy its Lua or its data tables.

## Game data we'd use (verified 2026-10-13 from the game files)
- **Module → product:** module macro `<production wares="X"><queue ware="X"/>`, `<workforce max="N"/>`.
  - Example: `prod_gen_energycells_macro` makes `energycells` with 90 workers; `prod_gen_hullparts_macro` makes `hullparts` with 270.
- **Rate:** `wares.xml` `<production time amount method>` plus `<primary>` inputs.
  - Example: hullparts = 294 per 900 s from energycells 80, graphene 40, refinedmetals 280.
  - `<effects><effect type="work" product="0.37"/>` is the bonus with a full workforce (hypothesis: +37%).
- **Workforce:** habitat macro `<workforce race capacity>`.
  - Example: `hab_arg_l_01_macro` holds 1000 argon.
  - Upkeep: ware `workunit_busy` production per method (default: food rations 75 + medical supplies 45 per 600 s for 200 units). The per-worker scale is a hypothesis, to check in game.
- **Storage:** storage macro `<cargo max tags>`. Example: `storage_arg_l_container_01_macro` = 1,000,000 m³ of container cargo.
  - Ware volumes and transport type come from `wares.xml` (`volume`, `transport`).
- **Already in the core:** recipes, build methods and costs (`wares.rs`).

## Proposed pieces (each usable on its own)
- [x] **A. Station analysis** (like Optimise), for the open design:
  - ware flows per hour (produced, used, net), with and without workforce;
  - workforce needed vs. housed, and its food and medicine;
  - storage capacity per cargo type;
  - pier and dock checks.
  - In the right pane and as an MCP tool.
- [x] **B. Production planner** (like Plan): target wares and counts, or a rate. Options:
  - race or build method;
  - make or buy per ware;
  - sunlight;
  - storage hours;
  - workforce on or off.
  - Gives the module list, the net flows and the cost.
  - A "to place" list in the builder that counts down as you place modules.
  - MCP tool.
- [x] **C. Auto-placement** (like Build This Design; plan below): our own layout step that puts the planned modules onto free snaps at exact positions. It adds connectors where needed, avoids overlaps and stays inside the plot. It is the biggest piece.

## Decisions
- 2026-10-13: the user wants all three, in the order A, then B, then C.
- 2026-10-13 (user): **self-sufficient** ignores raw materials. Ore, ice, silicon, methane, helium, hydrogen and the like are mined.
  - The user added: scrap metal is raw too; it is brought in and can't be produced.
  - In the code: wares tagged `minable` or `recycling` (scrap) are `raw`, and `Analysis::self_sufficient()` means only raw wares may be short.
  - The analysis notes "Not self-sufficient" otherwise. The planner summary splits raw materials to bring in from wares to buy. The MCP instructions define it.

## A. Station analysis: plan
- [x] Core: `Module.economy`: production queue (ware, method), workforce max, habitat (race, capacity), cargo (max, tags). `Recipe` gains time, amount, work and sunlight effects; `Ware` gains volume and transport.
- [x] Core `economy.rs`, `analyze(catalog, plan, sunlight)`:
  - ware flows per hour;
  - workforce needed vs housed per race, with its upkeep;
  - storage per cargo type in hours;
  - docks and piers;
  - notes.
- [x] Ops/MCP `analyze_station`; Tauri command
- [x] Builder: right-pane "Production" section (net per ware, workforce, storage, notes) with a sunlight setting
- [x] Tests (game data), ui_smoke, docs

### Rules (game data verified 2026-10-13; the maths are hypotheses until checked against the game's station overview)
- **Production modules:**
  - `<production wares><queue ware method/>`, or `<queue><item ware method/>…` (the scrap recyclers). The method picks the ware's recipe (`default` if absent).
  - Several items are assumed to run in turn: one cycle = the sum of their times.
  - Output per hour = amount × 3600 / cycle × (1 + work × workforce fill) × (1 + (sunlight − 1) × sunlight effect).
  - Inputs per hour = amount × 3600 / cycle: the bonus adds output only.
- **Workforce:**
  - Needed = Σ `<workforce max>` (production and build modules).
  - Housed = Σ habitat `<workforce race capacity>`, assumed full.
  - Fill = min(1, housed / needed).
  - Upkeep per race = `workunit_busy` recipe for that race (argon → `default`) × population / amount × 3600 / time.
  - Example: 1000 argon workers use 2250 food rations and 1350 medical supplies per hour.
- **Effects:** `work` (67 recipes); `sunlight` (energy cells, product 1). `efficiency` appears once, on a non-module ware, and is ignored.
- **Storage:** `<cargo max tags>` (container, solid, liquid, or several). Ware `transport` and `volume` come from `wares.xml`. Hours = capacity / Σ (produced + used) × volume per hour for that type.
- **Not modelled:**
  - welfare modules (`<workforce growthrate>`: growth, not capacity);
  - processing modules (`<products>`, scrap from ships);
  - ship building (build modules only add workforce).

## B. Production planner: plan
- [x] Core `planner.rs`, `plan_production(catalog, existing plan, request)`:
  - Request: targets (ware, number of production modules to add), race (or any), workforce on/off, storage hours (6), wares to buy, sunlight, build method (cost only).
  - It adds to the open design. The modules already there count as supply, housing and storage. With no targets it fixes the open station's shortfalls (the mod's Optimise).
  - Loop until nothing changes:
    1. with workforce on, add habitats until the workforce needed is housed;
    2. analyse;
    3. for each short ware that isn't in the buy list and has a producer, add ceil(shortfall / output per module) producers.
  - Then add storage for N hours per cargo type, plus a dock area and a pier if there are none.
  - Result: the modules to add (with roles: target, chain, habitat, storage, dock), the analysis after the plan (what is left short has to be bought or mined), and the cost of the additions.
- [x] Module choice (only the `factory` build set, which excludes trade-station, Xenon and landmark variants):
  - **Producers:** a single-ware queue that isn't `recycling`. Preference: the chosen race's module, then a generic one (no maker race), then any race; ties by name.
  - **Habitats:** the chosen race's largest. With "any", the largest of all (ties by name).
  - **Storage:** the largest single-type module of the race.
  - **Dock area and pier:** the race's (by maker race, else the race code in the macro name), skipping names with `tradestation` or `venturer`.
- [x] Ops/MCP: `plan_production`, `list_wares` (wares that some module can produce)
- [x] Builder: a left-pane **Planner** tab:
  - targets (ware search plus count), race, workforce, storage hours;
  - Generate;
  - result rows grouped by role, showing placed/needed (counting down as you place); clicking a row arms that module;
  - a per-row "buy instead" for chain wares;
  - the wares left to supply, and the cost.
- [x] Tests (game data), ui_smoke, mcp_smoke, docs

## C. Auto-placement: plan (designed and done 2026-10-13)
Goal: one call places a module list (the planner's) onto the open design at exact snap positions, without overlaps and inside the plot. Then "a staffed station with 100 solar panels" works end to end for agents.

Known facts:
- Snaps carry only `tags="snap"`. Any snap joins any snap, and the game imported such plans (test C).
- `geometry::attach` gives the child transform from (pred, pred snap, child snap, twist).
- `Module::shape` holds oriented part boxes. `plot::entry_bounds` gives axis-aligned bounds. `Design::free_snaps` lists open snaps.
- Build order is entry order, so placing appends in placement order and each parent comes before its child.

Steps:
- [x] **Calibrate overlap:** in the player's plans (made in game), measure how much part boxes of attached neighbours intersect at the joint, and whether unattached pairs ever do. This picks the tolerance, e.g. shrinking boxes by a few metres. Record it in `docs/x4-construction-plans.md`.
  - Result (2026-10-13, game-data test `overlap_calibration`): joints (linked, or touching at a snap) intersect by at most **0.4 m**, so the tolerance is **1 m**.
  - The part boxes (`<size>`) are too loose for unlinked pairs: 29 pairs over 1 m, up to 491 m (a medical-supplies box includes its thin 1.3 km spire, and a habitat stands beside it).
  - So overlaps use **collision boxes** instead: the module's `-collision.xmf` meshes (the xmf parser now reads their declaration-less vertex buffers), split into up to 128 boxes per part (7 halvings, or boxes of 100 m or less). That leaves 14 pairs over 1 m, up to 98 m.
  - Splitting deeper (10 halvings, 25 m) takes 239 s instead of 3 s and still leaves 10 pairs: some shrink (98 m to 45 m: box looseness), others stay (35 m, 19 m, 11 m: the meshes really intersect). The game's build menu has an "Allow module overlap" option (off by default, off in the user's profile now), so these stations were built with overlaps allowed at some point, or the game's check is more lenient.
  - Cost: collision boxes of all 287 modules load in 2.4 s (once); checking the 5 player plans takes 16 ms.
  - **Joints are exempt.** In the game's library plans (116, all races), 228 of 2,658 joined pairs intersect by more than 20 m, up to 130 m: connector collars and defence discs interlock with what they attach to (Argon medical supplies reach 41 m past their snap). Only the player's Terran modules meet within 0.4 m. So two modules whose snap points coincide (linked, or touching) never overlap; other pairs overlap when their collision boxes intersect by more than 1 m. With that rule: 14 overlaps in the player plans, 84 in the library plans.
- [x] Core `layout.rs`: `overlaps(...)`, an oriented-box test (separating axis theorem), with an axis-aligned pre-check per module.
  - `Body` (collision boxes and snap points in the station frame), `overlap(a, b)`, `intersection_depth`, `overlaps(plan, catalog)`. Only boxes inside the other module's bounds are compared.
- [x] Core `auto_place(catalog, design, modules: &[(macro, count)], plot, connector: Option<macro>)`, greedy:
  1. Empty design: start with a connector hub (the race's cross connection structure) at the origin.
  2. Placing order: big modules first; docks and piers last, so they end up on the outside.
  3. For each module, try free snaps sorted by distance from the origin (compact), with each of the module's own snaps and twists 0/90/180/270. Take the first candidate with no overlap inside the plot.
  4. If nothing fits, attach a connector (the race's base connection structure) on the closest free snap that fits, then retry; cap the number of connectors.
  5. Report the modules placed (new indices), the connectors added, and the modules that didn't fit.
  - One undo step for the whole placement (one checkpoint).
  - As built (`layout::auto_place(catalog, design, modules, Options { grow_plot, race })`):
    - hub = the race's factory connection structure with the most snaps (Argon cross 01, Terran cross 01); struts = its straight 2-snap ones, one per length (400, 800, 1600 m for Argon). Race: given, else the most common race of the modules.
    - Options per module, nearest the origin first: directly on any free snap (module or connector), or through a new hub on a free snap (optionally on a strut), counted 2 km further out (`HUB_PENALTY_M`). That trade-off was tried on "100 solar": 0 km gives 124 connectors (4.3% of the cost, 7x8x7 km), 2 km 81 (2.9%, 8x8x8), 4 km 37 (1.4%, 11x12x12).
    - Failures are cached per (snap, module, kind): placing only adds obstacles, so what failed once fails again. Placements stay 1 m inside the plot (f32 rounding).
    - Hub placements with the same snap positions are tried once (the Argon cross is symmetric).
- [x] **Plot:** use the design's plot. With "grow", allow up to the largest plot (20 km per axis), then report the plot needed (the builder already offers to enlarge).
- [x] Ops/MCP:
  - `auto_place` (module list, or `from_plan` with `PlanArgs`, which plans and places in one call);
  - update the MCP instructions: the recipe becomes plan_production, auto_place, validate_design, analyze_station.
  - As built: `ops::auto_place` (`AutoPlaceArgs`: `modules` or `from_plan`, `grow_plot` default true, `race`); the result has `placed`, `connectors`, `unplaced`, `plot`, a `next` hint when the plot must grow, and `plan` with `from_plan`. 31 MCP tools.
- [x] Builder: a Planner-tab **Place all** button that places what is still to place (count − placed), with a status line for what didn't fit.
  - Tauri command `auto_place` runs off the main thread; the usual enlarge-the-plot prompt follows; Ctrl+Z undoes it all.
- [x] Validate: add an overlap issue to `validate` (G4 lists overlaps), so hand-made designs get it too.
  - `Issue::Overlap { index, other, metres }` on the later-built module. The player's plans show 14 (6 in TerranScrapyard), the library plans 84.
- [ ] Tests:
  - [x] a game-data test ("100 solar, staffed": everything placed, no overlaps, validate clean, inside 20 km): `auto_place_planned_stations`, plus a Terran station, a small fixed plot and the TerranScrapyard fix placed onto the existing station; docks checked for open space;
  - [x] mcp_smoke (`auto_place` with `from_plan`, validate, undo) and ui_smoke (Place all, checks clean, Ctrl+Z);
  - [ ] an in-game check by the user: Import and look.

## Notes
- 2026-10-13: A done.
  - **Code:** `economy::analyze` in the core; `ops::analyze_station` adds the notes; MCP `analyze_station`; right-pane Production section with Sunlight % (remembered); `station_analysis` game-data test; ui_smoke and mcp_smoke checks.
  - **Results:**
    - TerranScrapyard: 10,000 of 11,665 workers (bonus 86%), 3 dock areas and 7 piers; hull parts +12,645 per hour; energy cells −758,820 per hour (scrap recyclers, no solar); container storage ≈ 4.3 h.
    - The MCP test station (one Terran habitat) eats 645 Terran MRE and 675 medical supplies per hour.
  - **Open:** check the hypotheses against the game's station overview for a built station:
    - the workforce bonus adds output only;
    - the upkeep per worker;
    - queue items run in turn;
    - habitats are always full.
- 2026-10-13: B done.
  - **Code:** `planner::plan_production` and `producible_wares` in the core; `ModuleInfo.sets`; `economy::analyze_modules` and `module_flows` for counts and single modules; MCP `plan_production` and `list_wares` (30 tools); builder Planner tab; MCP instructions with the planning recipe.
  - **Tests:** the `production_planner` game-data test, mcp_smoke and ui_smoke (generate, arm from a row, place, see 1/1, buy and undo).
  - **Results:**
    - Hull Parts × 2 (any race): 10 production, 2 habitats, 5 storage, 1 dock area, 1 pier, ≈ 12.2 M Cr; methane, ore and ice left to supply.
    - Teladi uses Teladi hull parts with teladianium, and Teladi habitats.
    - TerranScrapyard with no targets, Terran: 180 Terran solar modules for its recyclers, plus food, habitats and storage.
    - "100 solar panels, staffed": 137 modules, ≈ 92.8 M Cr; only ice left.
  - **Choices:**
    - ties go to the shortest macro name (a standard dock area before a luxury one);
    - a macro keeps the role it was first added with;
    - targets always add on top: regenerating after placing adds them again, as in the mod.
  - **Gap (C):** an agent has to attach every planned module by hand, and there is no overlap check.
- 2026-10-13: C done.
  - **Code:**
    - core `layout.rs`: collision-box overlaps and `auto_place`;
    - `Catalog::collision_boxes`, built from the `-collision.xmf` meshes, which the xmf parser now reads;
    - `Issue::Overlap`;
    - `Design::history_len` / `squash_history` (one undo step);
    - `plot::module_bounds` / `bounds_plot`;
    - `ops::auto_place`, MCP `auto_place`, the builder's *Place all*.
  - **Results:** "100 solar, staffed" places 137 modules plus 81 connection structures (2.9% of the cost) on an 8×8×8 km plot in 2.1 s. The dock area and pier face open space.
  - **Changed from the plan:**
    - overlaps use collision boxes, not shrunk part boxes;
    - joints are exempt;
    - modules may attach to any free snap, not only to connectors;
    - a new hub competes with direct placement (2 km penalty) instead of being only a fallback;
    - dock areas and piers prefer open space.
  - **Open (in game):**
    - the game builds an auto-placed station;
    - ships reach its dock area and pier;
    - whether the game minds the overlaps `validate` reports on stations built in game.
