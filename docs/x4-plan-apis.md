# X4 construction-plan APIs (Lua UI and Mission Director)

What the game exposes for plans, read from its own scripts (game version 9.00). All sources are files we extracted with `tools/x4cat.py`.

## Lua UI (station build menu)
Source: `ui/addons/ego_detailmonitor/menu_station_configuration.lua` in `08.cat`. Plain source; the `.xpl` file next to it is LuaJIT bytecode of the same thing. The functions are FFI exports of `X4.exe`, declared in the menu's `ffi.cdef` block.

### Plan library: verified from the menu's source
| Function | Use in the menu |
|----------|-----------------|
| `GetNumConstructionPlans()` / `GetConstructionPlans(UIConstructionPlan*)` | Lists plans as `{name, id, source, deleteable}`. The menu shows only `source == "local"` (the player's plans) unless `IsCheatVersion()`. |
| `SaveMapConstructionPlan(holomap, source, id, overwrite, name, desc)` | Save button. New plans use `source="local"`, `id="player"`; the game then makes `player_<unix time>`. |
| `RemoveConstructionPlan(source, id)` | Delete. Also used before re-importing a plan with the same id. |
| `ExportMapConstructionPlan(holomap, filename, id, overwrite, name, desc)` | **Export button.** `filename` = plan name with characters outside `[%w_%-%() ]` replaced by `_`. It also saves the plan into the player's list, so exporting a listed plan asks "This will overwrite the existing construction plan … in your list" (verified in game). |
| `GetNumImportableConstructionPlans()` / `GetImportableConstructionPlans(UIConstructionPlanInfo*)` | **Import list** as `{filename, name, id}`. Rescanned every time the title bar is built, and by a "reload importable" button. |
| `ImportMapConstructionPlan(filename, id)` | **Import button.** Adds the file's plan to the player's library. |
| `IsConstructionPlanValid(id, &numinvalidpatches)` / `GetConstructionPlanInvalidPatches` | Flags plans that need missing, old or disabled extensions (the `<patches>` block) |
| `AreConstructionPlanLoadoutsCompatible(id)` | Warning tooltip on plans whose equipment doesn't fit |

**Where import/export files live (verified in game):** `<profile>\constructionplan\`. File format: [x4-construction-plans.md](x4-construction-plans.md#importexport-file-verified-in-game).

### Build map: the in-game editor
`ShowConstructionMap`, `AddMacroToConstructionMap`, `GetBuildMapConstructionPlan2` (returns `UIConstructionPlanEntry2` = `{idx, macroid, componentid, offset (UIPosRot), connectionid, predecessoridx, predecessorconnectionid, isfixed, bookmarknum}`), `ShuffleMapConstructionPlan2`, `SetConstructionSequenceFromConstructionMap`, `ResetConstructionMapModuleRotation`, `SetConstructionMapBuildAngleStep` and others. The entry struct mirrors the plan XML one to one.

## Mission Director (MD) actions
Source: `libraries/common.xsd` in `08.cat` (the documented script schema).

| Action | Plan attribute | What it does (schema docs) |
|--------|----------------|----------------------------|
| `create_station` | `constructionplan` = plan ID string or construction sequence | Creates a station |
| `apply_construction_sequence` | `sequence` = construction sequence or plan ID | Appends a plan to an existing station (its plan may be empty) |
| `add_build_to_expand_station` | `constructionplan` = plan ID string or sequence | Adds a normal build task (builder ship, resources) to expand a station |
| `create_construction_sequence` | `macros`, `connectors`, `base`, `constructionbias`, `failsafe`, `timeout` | The **engine's own layout generator**: it connects a list of module macros randomly. Runs on a worker thread unless `<immediate>` is given. |
| `remove_from_construction_sequence` | `sequence` | Removes entries that use given macros, plus their descendants |

So once a plan is in the player's library (by import or a file edit), a small mod can build it by ID with `add_build_to_expand_station` or `apply_construction_sequence`.

## Prior art: `x4ep_planner` (GHOSTau's Station Planner, `ws_3777272604`)
**Verified from its source** (`md/x4ep_build_queue.xml`, `ui/ep_*.lua`):
- **It doesn't lay out modules itself.** "Build This Design" sends a flat list of module macros (one per module) to `create_construction_sequence`, so the engine's generator chooses positions and connections. Then `add_build_to_expand_station constructionplan="$Seq"` queues the result as one build task.
- It runs the generator asynchronously (no `<immediate>`; a big bill froze the game for minutes otherwise) and waits for `event_object_construction_sequence_created`. It allows one queue per station and uses `failsafe="false"`: the default `true` drops modules that don't fit into free space, unconnected.
- Modules that don't fit are retried in batches ("lifting"), then reported for the player to place by hand.
- The Lua side uses only read-only FFI calls (blueprints, wares, production methods, workforce) plus `ShuffleMapConstructionPlan2` and `GetBuildMapConstructionPlan2`. It never calls import/export or save. The Lua side and MD talk through `raise_lua_event` and an MD bridge cue.

**What that means for us:** the mod shows that a station can be fed into a live game from a script and built. But it gives up exact layout, which is the whole point of a designer. Our designer needs exact positions, so we inject a full plan (option A or B) and, optionally, build it by plan ID from MD.
