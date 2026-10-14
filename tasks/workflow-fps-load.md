# Workflow: in-game load readout; fewer connection structures (user request)

Status: **done** (2026-10-14); in game it is still unchecked whether these counts track frame rate

User, 2026-10-14, asked "is there a way to optimize stations to have less in game FPS impact?". The answer (community knowledge, all **hypotheses**, nothing measured in game): module count matters most (connection structures add count and produce nothing), then turrets (each one aims and fires), then dock traffic and sprawl (ship pathfinding), then where the station is (other sectors are simulated cheaply). The user picked two follow-ups: "i like both Cost readout and Planner option, do it".

## Design (assumptions, stated to the user)
- **Readout** ("In-game load"): counts that likely drive the game's load, no made-up score.
  - modules, of which connection structures;
  - turrets and shields (from the equipment);
  - dock areas and piers;
  - station size (km per axis, the part-box bounds).
  - Core `perf::load(plan, catalog)`; MCP/ops `station_load`; builder right-pane section; CLI `show` line.
- **Planner option** "Fewer connection structures": auto-placement adds a hub only when it puts the module 4 km nearer the centre (instead of 2 km; see Notes for why not "only where no free snap fits"). Used by *Place all* and *Optimize* (builder checkbox, remembered), MCP `auto_place` / `optimize_station` (`fewer_connectors`). Off by default: it trades connectors for a larger station.

## Steps
- [x] Measure: connectors and station size, "100 solar" and TerranScrapyard rebuild, with and without the option
- [x] Core: `Options::fewer_connectors`, `OptimizeOptions::fewer_connectors`
- [x] Core: `perf::load`
- [x] Ops/MCP: `station_load`; `fewer_connectors` on `auto_place` and `optimize_station`
- [x] Builder: "In-game load" section; Planner checkbox; Tauri `auto_place` arg
- [x] CLI `show`: load line
- [x] Tests (targeted): game-data test of the option (fewer connectors, nothing unplaced, no overlaps) and of `load`; a UI check
- [x] Docs: usage.md, CLAUDE.md, roadmap, this file

## Notes
- 2026-10-14: **hub threshold measured** (game-data test `fewer_connectors`, table in docs/x4-construction-plans.md, "Auto-placement"). With no threshold (hubs only as a last resort), "100 solar" needs 20 connectors but becomes a 20 km snake with a dock blocked. 4 km gives 37 against 81 (11×12×13 km against 9×8×8). The rebuilt TerranScrapyard gives 1 against 56 (288 modules against 343).
- Checks run:
  - game-data test `fewer_connectors`: fewer connectors for both cases, nothing unplaced, no overlaps; `perf::load` matches the plan; "defense station" 288 turrets, 144 shields;
  - `x4sd show` on the samples;
  - a scratch MCP script: `station_load` agrees with `auto_place`, and with `fewer_connectors` 30 solar panels need 13 connectors instead of 18;
  - a scratch CDP check of the builder: the section starts folded, shows TerranScrapyard (84 modules, 16 connection structures), the checkbox is remembered, and Optimize with rebuild plus fewer gives 288 modules with 1 connection structure.
- `research/ui_smoke.py` now expects 6 folded sections (In-game load is new). Not re-run in full (per the user, only new things are tested).
