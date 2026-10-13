# X4 game data

## Install layout (verified, version 9.00)
Root: `F:\SteamLibrary\steamapps\common\X4 Foundations`

| Path | What |
|------|------|
| `01.cat` … `09.cat` + `.dat` | Base game archives (`01.dat` is about 10.7 GB) |
| `NN_sig.cat` / `.dat` | Signature archives. Ignore them for data extraction. |
| `extensions/ego_dlc_*` | Official DLCs: `boron`, `pirate`, `split`, `terran`, `timelines`, `ventures`. Each has `content.xml` and `ext_NN.cat/.dat`. |
| `extensions/<mod>` | Installed mods (about 80), same layout. Some ship loose files. |
| `ui/core`, `ui/lualibs` | Loose UI files (Lua). Most UI is probably inside the archives. |
| `t/` | Loose localisation files (`0002-lNNN.xml`) |
| `X4.exe` | Native engine. Use REA on it only if the data files don't answer a question. |
| `version.dat` | `900` |

## `.cat` / `.dat` archives
- **verified**: every line of a `.cat` file is `<path> <size> <unix mtime> <md5>`, for example
  `libraries/constructionplans.xml 1066417 1779094016 a69f21bbf04be00d85be86cbdd8c193f`
- **hypothesis** (community knowledge): the matching `.dat` holds the file bodies back to back, in `.cat` order. A file's offset is the sum of the sizes listed before it.
- **hypothesis**: later catalogs override earlier ones, and extensions patch base files through XML diffs (`<diff><add/><replace/><remove/></diff>`).
- Egosoft's "X Catalog Tool" can extract the archives. A small reader of our own is also easy, given the format above.

## What we need from game data (Phase 1)
| Need | Probable source (verify) |
|------|--------------------------|
| Station module macros and their component references | `index/macros.xml`, `index/components.xml`, `assets/structures/**` |
| Snap connections per module (`connectionsnapNNN`, with transforms) | Module component XML under `assets/structures/**` |
| Module type, race, size, DLC | Macro XML, the extension each one comes from |
| Build cost and resources | `libraries/wares.xml` |
| Display names | `t/0001-l044.xml` (English), referenced by `{page,id}` |
| Turret and shield groups per module | Module component XML (`group` connections) |
| Built-in plans (examples, test data) | `libraries/constructionplans.xml` |

## Installed mods worth studying (prior art)
Folder names don't always match the mod. Check `content.xml` for the real id and name.

| Folder | Mod | Why it's relevant |
|--------|-----|-------------------|
| `x4ep_planner` | **GHOSTau's Station Planner, Designer & Optimiser** (`ws_3777272604`, v2.81, 2026-10-02, tested on 9.00) | **Primary target.** Runs inside the station build menu. "Build This Design" places a whole station (every module connected) onto a plot as one build task. It also places single modules on the cursor, reads an existing station's modules, and ships its own module-geometry data tables. See below. |
| `sn_mod_support_apis` | SirNukes Mod Support APIs (`ws_2042901274`) | Named-pipe IPC between the game's Lua and external processes (option D). Optional dependency of `x4ep_planner`. |
| `x4-mod-ui-extensions` | UI hooking. Useful for an in-game bridge. |
| `tetrino_enable_build_hacking` | Changes build or placement rules. Shows which rules exist. |
| `vas_supply_needs_planner`, `station_storage_allocation` | Station-planning helpers |

Not relevant despite its name: `analyze_build` is "Ship Reverse Engineering" (`ws_1687245083`).

The player profile also has its own `extensions\` folder (`apologizeforattack`, `recycleshipsandstations`).

### `x4ep_planner` contents (verified from `ext_01.cat`, 20 files)
| File | Size | Likely role (hypothesis) |
|------|------|--------------------------|
| `ui.xml` | 3 KB | Registers the Lua files with the UI |
| `ui/ep_planner.lua` | 354 KB | Build-menu panel. Probably hooks `menu_station_configuration` and calls the plan/placement FFI. |
| `ui/ep_model.lua` | 237 KB | Production-chain model |
| `ui/ep_data.lua` | 98 KB | Data tables (module geometry, recipes) |
| `ui/ep_stacks.lua`, `ep_zones.lua`, `ep_effects.lua`, `ep_ships.lua`, `ep_text.lua` | 5–23 KB | Helpers |
| `md/x4ep_build_queue.xml` | 98 KB | Mission Director script. Probably the "Build This Design" placement queue. |
| `md/x4ep_build_bridge.xml` | 9 KB | MD-to-Lua bridge |
| `md/x4ep_planner_entry.xml`, `md/x4ep_options.xml` | 9 / 34 KB | Menu entry, Extension Options |
| `t/*` | n/a | Translations |

The mod is open to read: its files are plain Lua and XML in its `.cat`/`.dat`, and a `readme.txt` (full manual) sits next to them.
