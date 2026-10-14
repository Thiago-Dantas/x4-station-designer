# Workflow: clearance for fabrication bays and piers

Status: **in progress** (2026-10-14): A (research) under way.

## What the user saw (2026-10-14)
- Optimize on `megashipyard terran`, saved as `megashipyard terran2` (probably `<profile>\constructionplan\megashipyard terran2.xml`; the plan uses a mod module, so load the catalog with the real profile). In game it imports and builds with no errors.
- **Fabrication bays** (`buildmodule_ter_ships_l_macro`, `buildmodule_ter_ships_xl_macro`, …) are boxed in by other modules. Big ships would have trouble leaving them. With *Rebuild connection structures* on, it looks even worse.
  - Likely cause: `layout::layer` puts modules without turret/shield slots in the core, and build modules have none, so Optimize puts them in the middle on purpose. Nothing models the space a bay needs.
- **Terran 3-Dock E Piers** face the wrong way. In game, large-ship piers show an approach volume (red wireframe frustum from the pier outward) while you move them.
  - Our dock rule only sweeps the dock's box *radially* away from the station centre (`layout::corridor`, `DOCK_CLEARANCE_M`). It ignores which way the pier's approach actually faces, so a pier can pass while its approach
   points along or into the station.
- From the user's screenshots (the next session can't see them):
  - **Builder, keep shape:** the selected XL bay (#164, attached to #57, twist 90°, 1 free snap) sits low in the middle, inside a lattice of connection structures and production modules.
  - **Builder, rebuild:** the XL bay is **entry #3, attached straight to #1, 0 free snaps**: placed first, at the very core, and buried.
    - The 3-dock E piers stand vertically along the cube's edges, arms parallel to the station's faces.
  - **Builder, from above:** several piers are stacked close and parallel across the top, and more stand along the sides. They likely **block each other's** approaches, not just face the station.
  - **In game** (the user's station "Asteroid Belt Scrapyard", planner, a Terran 3-Dock E Pier being moved): the approach volume is a large red frustum.
    - It starts at the pier and extends to one side, **at right angles to the pier's arm**, away from the station.
    - It's **much wider than the pier**, and made of **several overlapping volumes** one per dock.
    - The plan status still said "Everything is OK". red is the normal display colour for the approach, the game shows a blocked approach by making the module itself red failing the plan.

## Steps
### A. Research (game data first; REA only if the XML/Lua can't answer; tag findings verified/hypothesis)
- [x] Piers and dock areas: find the data behind the in-game approach frustum. Candidates: component XML connections or tags of the pier (`dockingbay`, `dock_l`/`dock_xl`, approach/waypoint/path connections), dock-area macros, `menu_station_configuration.lua` (how it draws or checks the approach, and whether a blocked approach is a warning or an error). Record its shape and size: origin, direction, length, opening angle or cross-section, per pier type (1-dock, 3-dock E/T, …) and per race.
- [x] Fabrication bays (all `buildmodule_*` ship bays S/M/L/XL, plus equipment docks and wharf/shipyard parts): find where ships spawn and leave (build/launch connections, paths, doors) and the space they need. Use the largest ship of the bay's size (ship macros' bounding boxes) as the check size.
- [x] Does the game itself refuse or warn about blocked approaches or bay exits? Look in the build menu's Lua for strings and FFI calls. "Everything is OK" on the user's import suggests it doesn't block them.

- Findings (2026-10-14), details in docs/x4-construction-plans.md, "Clearance: exclusion zones":
  - Zones are component connections `exclusionzone ship_<class>`; sizes per class in `libraries/parameters.xml` (XL 4000×4000×100 000 m, L 1200×1200×100 000, M 180×180×500). On every pier (one per dock), every bay (L/XL: two, out of both ends), most M dock areas, scrap processors, radar dish.
  - Shape not in the data; REA timed out on X4.exe. Calibration (`examples/zonecheck.rs`): a box, full widths, from the connection along +Z fits the user's game-built plans (4 intrusions in 144 zones, all back-to-back 1-dock piers joined at a snap) and the library (14 in 1538).
  - The build-menu status never checks zones; an import with blocked zones says "Everything is OK"; the engine's layout generator counts zone intersections as failures.
  - Largest-ship sizes not measured: the game's own zone sizes per class replace them.
- [x] Asked the user (2026-10-14), D-12: box as measured; modules joined at a snap to the zone's module are exempt; both ends of L/XL bays must be clear; validate reports a problem.

### B. Model
- [x] `catalog`: a clearance volume per module (pier approach, bay exit) in the module frame. Unit test on a few known macros.
- [x] `validate`: report modules that intrude into another module's clearance volume ("blocks the approach of pier #N" / "blocks the exit of bay #N"). Calibrate it on the user's saved, game-built plans (TerranScrapyard, megashipyard terran) before trusting it, the way the overlap rule was calibrated (D-9).

- B done (2026-10-14): `Module::clearance`, `Catalog::exclusion_size`, `layout::zones`, `layout::blocked_zones`, `Issue::BlocksZone`. Game-data test `clearance_zones`: zone directions of the 3-Dock E Pier and the XL bay; the player's game-built plans block nothing; `megashipyard terran2` has 652 blocker/zone pairs ("entry 20: blocks the exit of bay 163").

### C. Placement and Optimize
- [ ] `auto_place` / `optimize`:
  - orient piers and bays so their clearance volume points out of the station;
  - protect those volumes like the dock corridors are protected now;
  - rework lifted piers and bays on each run, as docks are now.
- [ ] Layers: treat fabrication bays (and other modules that need clearance) as an outer layer next to the docks, not as slotless core modules.
- [ ] Find out why *Rebuild connection structures* makes the bays worse (connector choices around big modules?) and fix it, or document the reason.

### D. Checks (targeted, per the standing "test only new things" rule)
- [ ] Game-data test: Optimize `megashipyard terran` (both modes) leaves every pier approach and bay exit clear, with no overlaps and nothing lost.
- [ ] Builder screenshot of the result (CDP, as in `research/ui_smoke.py`).
- [ ] Docs: x4-construction-plans.md (clearance volumes, verified/hypothesis), usage.md (new checks), decisions.md (clearance rule). CLAUDE.md if module roles change.
- [ ] Commit and push.

### E. In game (user)
- [ ] Import the re-optimized megashipyard and look at the pier approach volumes while moving a pier: none red.
- [ ] Watch an L/XL ship leave a fabrication bay.
