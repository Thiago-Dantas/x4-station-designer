# Workflow: build materials / cost in the right pane (user request)

Status: **done** (2026-10-13)

User: "on the right pane lets display build materials required, if the user select a module display that module cost, if nothing is selected display entire station cost".

## Game data (verified 2026-10-13)
- **Module wares:** `libraries/wares.xml`, `<ware tags="module">`, are linked to the module by `<component ref="<macro>"/>`. Each has `<production method="…" time="…"><primary><ware ware="…" amount="…"/>`.
  - Example: `prod_gen_energycells_macro` default = claytronics 260, energy cells 520, hull parts 951.
- **Build methods:** `<wares><production><method id name tags>`.
  - Base: argon, default, paranid, teladi, plus NPC-only processing, recycling, research and xenon (`tags="noplayerbuild"`).
  - DLCs add boron, closedloop, split and terran via `<add sel="/wares/production">`.
- **DLC patches:**
  - `<add sel="/wares">` adds wares (DLC modules, new resources);
  - `<add sel="/wares/ware[@id='X']">` adds children to existing wares: 132 Terran `production method="terran"`, plus owners.
- **After merging:** 325 module wares; 302 with `default` only, 16 default+terran, 3 boron+default+terran, 3 default+xenon, 1 without production.
  - Terran modules' `default` recipe already uses Terran wares (the Terran L habitat: computronic substrate 80, energy cells 593, silicon carbide 165).
  - The build method only changes the 19 shared modules; otherwise the game falls back to `default` (assumed, as in the build menu).
- **Prices:** `<price min average max>`.
- **Equipment** (shields, turrets) are wares with recipes too, found by `component ref`. Plan `<upgrades>` give the macros; `exact` is the turret count.

## Steps
- [x] Core `wares.rs`: load and merge wares (base + extensions: add `/wares`, `/wares/production`, `/wares/ware[@id]`); names via texts; recipe(method) with fallback to `default`; plan or module cost incl. equipment
- [x] Ops/MCP: `build_cost` (module or station, method)
- [x] Builder: right-pane Build cost (selected module or the whole station), build method selector (remembered)
- [x] Tests + docs

## Notes
- 2026-10-13: mods patch wares with `<replace>` (prices, times, recipes); only `<add>` is applied. `faction_fix_pack/libraries/wares.xml` is not valid XML (mismatched tag at line 23), so extension files that fail to parse are skipped.
- 2026-10-13: results:
  - wares load in 20 ms; method names come from the texts (`default` = "Universal");
  - TerranScrapyard ≈ 48.9 M Cr, Medical Supply Production ≈ 157k Cr in the UI;
  - every module in the player's plans has a recipe (the mod's `recyclemodule_macro` too, from its `<wares>` file);
  - the defence disc with its 24 turret/shield groups is 7.9 M versus 0.64 M alone.
- 2026-10-13: `python -I` ignores `PYTHONIOENCODING`, so the UI test scripts reconfigure stdout to UTF-8 (≈ and × on a cp1252 console).
