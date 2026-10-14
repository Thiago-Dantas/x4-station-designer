# Workflow: palette limited to the player's blueprints (from a save)

**Status:** done (2026-10-14, user request)

## Request
An option to read a save file and offer only the modules the player has blueprints for.
- Off by default: show everything.
- Turning it on the first time: a file dialog in the save folder; read the save, keep only the owned modules.
- Turning it off: show everything again.
- Turning it on again: use the cached result (don't re-read), and offer buttons to pick another save or re-read the current one.

## Research (verified 2026-10-14 on `save_001.xml.gz`, Python scan)
- Saves: `<profile>\save\*.xml.gz` (gzip; the game can also write plain `.xml`). save_001 is 120 MB gzipped, 1.0 GB of XML.
- One `<blueprints>` block per save, of `<blueprint ware="…"/>`: 447 wares, 113 of them `module_*`. It starts at byte 241 M of the XML (line 6.0 M): Python reaches it in 2.5 s, a full read takes 9 s. So the reader stops at `</blueprints>`.
- Blueprint ware → module macro: `wares.xml` `<component ref>` (`Wares::for_macro`).
- The game's build menu lists modules from `C.GetBlueprints(set, moduletype)` (the player's blueprints) and warns about imported plans whose modules lack one (`menu_station_configuration.lua` 1671, 2049–2081). So the filter matches the game.

## User decisions (2026-10-14)
- Scope: palette, planner (plan, Place all, Optimize) **and MCP**.
- The cached list and the on/off setting are remembered across restarts.

## Design
- Core `save.rs`: `read_blueprints(path) -> Vec<ware>`: gzip or plain, line-streamed, stops at `</blueprints>`.
- Filter = lowercase set of owned module macros. It limits what is **offered** (palette, `list_modules`) and what the tools **choose themselves** (planner producers, habitats, storage, docks; auto-placer connection structures). Modules named explicitly (extras, `auto_place` lists, attach) are used as given.
- Planner: `Request.owned`; wares whose only makers lack a blueprint go to `no_blueprint` (not `unknown_targets`).
- Layout: `Options.owned` / `OptimizeOptions.owned` → connection structures.
- ops: `Blueprints {on, save, read_at, modules}` in `Ctx`; ops `ReadBlueprints {save?}` (default newest save) and `BlueprintFilter {on?}`. MCP tools `read_blueprints`, `blueprint_filter`.
- Builder: commands `read_blueprints(file)` (reads without holding the lock) and `set_blueprints(state)`; agent ops emit `blueprints-changed`. UI keeps the state in localStorage (`x4sd.blueprints`) and restores it at start.

## Steps
- [x] Scope question to the user (palette only, or planner/auto-place too)
- [x] Core: `save::read_blueprints` (gzip or plain, streaming, stops at `</blueprints>`), `owned_modules`, `newest_save`; unit tests + game-data test `blueprints_from_save_limit_the_planner`
- [x] Planner (`Request.owned`, `no_blueprint`) and layout (`Options.owned`, `OptimizeOptions.owned`: connection structures)
- [x] ops/app/MCP: `Blueprints` in `Ctx`; builder commands `read_blueprints`, `set_blueprints`, event `blueprints-changed`; MCP `read_blueprints`, `blueprint_filter`
- [x] UI: checkbox, file dialog in `<profile>\save`, cache in localStorage, *Other save…* / *Re-read*, palette, race counts and planner extras limited; plan summary shows "No blueprint for…"
- [x] Verify: `research/blueprint_smoke.py` passes (save_001: 109 of 287 modules; filter off/on without re-reading; Re-read; restart restores it). Release read time 0.43 s.
- [x] Docs: usage.md, x4-game-data.md (Savegames), decisions D-16, CLAUDE.md, roadmap

**Status: done** (2026-10-14). Not checked: the native file dialog itself (it can't be scripted; the first read in the smoke test comes from an agent).
