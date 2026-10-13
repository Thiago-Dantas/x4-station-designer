# Workflow: x4ep_planner functionality in the designer (user request)

Status: **in progress**: A done, B next (started 2026-10-13)

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
- [ ] **B. Production planner** (like Plan): target wares and counts, or a rate. Options:
  - race or build method;
  - make or buy per ware;
  - sunlight;
  - storage hours;
  - workforce on or off.
  - Gives the module list, the net flows and the cost.
  - A "to place" list in the builder that counts down as you place modules.
  - MCP tool.
- [ ] **C. Auto-placement** (like Build This Design): our own layout step that puts the planned modules onto free snaps at exact positions. It adds connectors where needed, avoids overlaps and stays inside the plot. It is the biggest piece.

## Decisions
- 2026-10-13: the user wants all three, in the order A, then B, then C.

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
