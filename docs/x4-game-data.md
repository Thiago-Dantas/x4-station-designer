# X4 game data

## Install layout (verified, version 9.00)
Root: `F:\SteamLibrary\steamapps\common\X4 Foundations`

| Path | What |
|------|------|
| `01.cat` … `09.cat` + `.dat` | Base game archives (`01.dat` is about 10.7 GB) |
| `NN_sig.cat` / `.dat` | Signature archives. Ignore them for data extraction. |
| `extensions/ego_dlc_*` | Official DLCs: `boron`, `pirate`, `split`, `terran`, `timelines`, `ventures`. Each has `content.xml` and `ext_NN.cat/.dat`. |
| `extensions/<mod>` | Installed mods (about 80), same layout. Some ship loose files. |
| `ui/core`, `ui/lualibs` | Loose UI files. Most UI is inside the archives (`ui/addons/**`). |
| `t/` | Loose localisation files (`0002-lNNN.xml`) |
| `X4.exe` | Native engine. Use REA on it only if the data files don't answer a question. |
| `version.dat` | `900` |

## `.cat` / `.dat` archives
- **verified**: every line of a `.cat` file is `<path> <size> <unix mtime> <md5>`, for example
  `libraries/constructionplans.xml 1066417 1779094016 a69f21bbf04be00d85be86cbdd8c193f`
- **verified**: the matching `.dat` holds the file bodies back to back, in `.cat` order. A file's offset is the sum of the sizes listed before it. Tested by extracting 34 files from `08.cat`, `01.cat` and `x4ep_planner/ext_01.cat` with 0 MD5 mismatches.
- **verified**: paths can contain spaces, so split each line from the right.
- **hypothesis**: later catalogs override earlier ones, and extensions patch base files through XML diffs (`<diff><add/><replace/><remove/></diff>`).
- UI scripts ship twice: `.lua` (plain source) and `.xpl` (LuaJIT bytecode, starts with `\x1bLJ`). Read the `.lua`.

### Our extractor: `tools/x4cat.py`
```
python -I tools/x4cat.py list    <file.cat> [glob ...]
python -I tools/x4cat.py extract <file.cat> <outdir> [glob ...]   # checks MD5, exits 1 on mismatch
```
Extract into `research/extracted/<source>/` (git-ignored: game and mod files are not ours to commit). The tool handles one catalog at a time; to find a file, `grep "^<path> " <install>/*.cat`.

### DLC index files (verified)
- Each DLC ships complete `index/macros.xml` and `index/components.xml` (not XML diffs), with entries whose `value` starts with `extensions\ego_dlc_<name>\…`. The files inside the DLC's `ext_NN.cat` are listed without that prefix, so prepend `extensions/ego_dlc_<name>/` when building one virtual file system.
- The DLC index files start with a UTF-8 BOM.
- Some index values double the separator (`assets\\structures\\landmarks\\…`). Collapse repeated `\`/`/` before lookup.
- `research/snapcheck.py` contains a working virtual file system over base `01–09.cat` plus all `ego_dlc_*` catalogs (later catalogs win), and the macro → component → snap resolution. It doesn't cover mods yet.

## What we need from game data
| Need | Source |
|------|--------|
| Station module macros and their component references | **verified**: `index/macros.xml` maps macro → macro file; the macro's `<component ref>` maps through `index/components.xml` → component file (`assets/structures/**`). DLCs have their own `index/`. |
| Snap connections per module (`connectionsnapNNN`, with transforms) | **verified**: `<connection name="ConnectionSnapNNN" tags="snap ">` with `<offset>` (position + optional quaternion) in the component XML. See [x4-construction-plans.md](x4-construction-plans.md#how-connection-names-map-to-module-geometry-verified-for-one-module). |
| Which plan sets a module belongs to, race | Macro XML: `<build><sets><set ref=…>`, `identification@makerrace` |
| Module type, size, DLC | Macro `class` (e.g. `connectionmodule`), the extension it comes from |
| Build cost and resources | `libraries/wares.xml` |
| Display names | `t/0001-l044.xml` (English), referenced by `{page,id}` |
| Turret and shield groups per module | Module component XML (`group` connections) |
| Built-in plans (examples, test data) | `libraries/constructionplans.xml` |

## Installed mods worth studying (prior art)
Folder names don't always match the mod. Check `content.xml` for the real id and name.

| Folder | Mod | Why it's relevant |
|--------|-----|-------------------|
| `x4ep_planner` | **GHOSTau's Station Planner, Designer & Optimiser** (`ws_3777272604`, v2.81, 2026-10-02, tested on 9.00) | **Studied.** It builds stations live through the engine's layout generator, not with exact positions. Findings in [x4-plan-apis.md](x4-plan-apis.md#prior-art-x4ep_planner-ghostaus-station-planner-ws_3777272604). |
| `sn_mod_support_apis` | SirNukes Mod Support APIs (`ws_2042901274`) | Named-pipe IPC between the game's Lua and external processes (option D). Optional dependency of `x4ep_planner`. |
| `x4-mod-ui-extensions` | (not checked) | UI hooking. Useful for an in-game bridge. |
| `tetrino_enable_build_hacking` | (not checked) | Changes build or placement rules. Shows which rules exist. |
| `vas_supply_needs_planner`, `station_storage_allocation` | (not checked) | Station-planning helpers |

Not relevant despite its name: `analyze_build` is "Ship Reverse Engineering" (`ws_1687245083`).

The player profile also has its own `extensions\` folder (`apologizeforattack`, `recycleshipsandstations`).

### `x4ep_planner` contents (verified from `ext_01.cat`, 20 files)
| File | Size | Role |
|------|------|------|
| `ui.xml` | 3 KB | Registers the Lua files with the UI |
| `ui/ep_planner.lua` | 354 KB | Build-menu panel (UI) |
| `ui/ep_model.lua` | 237 KB | Production-chain model |
| `ui/ep_data.lua` | 98 KB | Game-data access through FFI (blueprints, wares, production methods), guarded by `cfn()` and `ensure_cdef()` |
| `ui/ep_stacks.lua`, `ep_zones.lua`, `ep_effects.lua`, `ep_ships.lua`, `ep_text.lua` | 5–23 KB | Helpers |
| `md/x4ep_build_queue.xml` | 98 KB | **Verified**: the "Build This Design" queue. Calls `create_construction_sequence`, then `add_build_to_expand_station`. |
| `md/x4ep_build_bridge.xml` | 9 KB | MD-to-Lua bridge (`raise_lua_event`) |
| `md/x4ep_planner_entry.xml`, `md/x4ep_options.xml` | 9 / 34 KB | Menu entry, Extension Options |
| `t/*` | n/a | Translations |

The mod is open to read: its files are plain Lua and XML in its `.cat`/`.dat`, and a `readme.txt` (full manual) sits next to them.
