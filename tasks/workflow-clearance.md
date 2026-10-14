# Workflow: clearance for fabrication bays and piers

Status: **proposed** (2026-10-14). Nothing checked yet. The user asked to write the tasks down and run them in a new session.

## What the user saw (2026-10-14)
- Optimize on `megashipyard terran`, saved as `megashipyard terran2` (probably `<profile>\constructionplan\megashipyard terran2.xml`; the plan uses a mod module, so load the catalog with the real profile). In game it imports and builds with no errors.
- **Fabrication bays** (`buildmodule_ter_ships_l_macro`, `buildmodule_ter_ships_xl_macro`, …) are boxed in by other modules. Big ships would have trouble leaving them. With *Rebuild connection structures* on, it looks even worse.
  - Likely cause: `layout::layer` puts modules without turret/shield slots in the core, and build modules have none, so Optimize puts them in the middle on purpose. Nothing models the space a bay needs.
- **Terran 3-Dock E Piers** face the wrong way. In game, large-ship piers show an approach volume (red wireframe frustum from the pier outward) while you move them.
  - Our dock rule only sweeps the dock's box *radially* away from the station centre (`layout::corridor`, `DOCK_CLEARANCE_M`). It ignores which way the pier's approach actually faces, so a pier can pass while its approach points along or into the station.

## Steps
### A. Research (game data first; REA only if the XML/Lua can't answer; tag findings verified/hypothesis)
- [ ] Piers and dock areas: find the data behind the in-game approach frustum. Candidates: component XML connections or tags of the pier (`dockingbay`, `dock_l`/`dock_xl`, approach/waypoint/path connections), dock-area macros, `menu_station_configuration.lua` (how it draws or checks the approach, and whether a blocked approach is a warning or an error). Record its shape and size: origin, direction, length, opening angle or cross-section, per pier type (1-dock, 3-dock E/T, …) and per race.
- [ ] Fabrication bays (all `buildmodule_*` ship bays S/M/L/XL, plus equipment docks and wharf/shipyard parts): find where ships spawn and leave (build/launch connections, paths, doors) and the space they need. Use the largest ship of the bay's size (ship macros' bounding boxes) as the check size.
- [ ] Does the game itself refuse or warn about blocked approaches or bay exits? Look in the build menu's Lua for strings and FFI calls. "Everything is OK" on the user's import suggests it doesn't block them.

### B. Model
- [ ] `catalog`: a clearance volume per module (pier approach, bay exit) in the module frame. Unit test on a few known macros.
- [ ] `validate`: report modules that intrude into another module's clearance volume ("blocks the approach of pier #N" / "blocks the exit of bay #N"). Calibrate it on the user's saved, game-built plans (TerranScrapyard, megashipyard terran) before trusting it, the way the overlap rule was calibrated (D-9).

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
