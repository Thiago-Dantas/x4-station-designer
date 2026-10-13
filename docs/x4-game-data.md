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

### Extensions: discovery, enabling, load order (verified 2026-10-13 unless marked)
- Extensions live in `<game>\extensions\` **and** `<profile>\extensions\` (e.g. `recycleshipsandstations`, which provides `recyclemodule_macro`).
- Each one's `content.xml` has an `id` that **often differs from the folder name** (`analyze_build` → `ws_1687245083`). The **id** is what the profile enables and what plan `<patches>` name. The **folder** is what virtual paths use (`extensions\<folder>\…`, in index values too).
- **Enabled state:** `<profile>\content.xml` has `<extension id="…" enabled="true|false"/>` (79 entries here) and overrides the extension's own `enabled` attribute (`1`/`true`/`0`/`false`; missing means enabled).
- **`save` attribute:** DLCs have `save="1"`. A mod with `save="false"` (RecycleShipsAndStations) was **not** listed in the `<patches>` of a plan that uses its module. So patches list only extensions savegames depend on; a missing `save` attribute is treated as true (hypothesis).
- **Catalogs per extension:** `ext_NN.cat` (files under `extensions/<folder>/`) and `subst_NN.cat` (replace base-game paths, e.g. `libraries/region_lodvalues.xml`, UI files).
- **Mod index files** are either complete `<index>` files or `<diff><add sel='/index'>…</add></diff>` patches. Both just add entries.
- **Load order (hypothesis):** we use base game, then DLCs, then the other enabled extensions alphabetically by id, with later ones winning. Dependencies are not used for ordering yet; this only matters when two mods define the same path.
- **Not handled yet:** diff `replace`/`remove` on index entries, XML diff patches to macro/component files, and loose (unpacked) files in extension folders. None of the installed mods needs them for station modules.

### DLC index files (verified)
- Each DLC ships complete `index/macros.xml` and `index/components.xml` (not XML diffs), with entries whose `value` starts with `extensions\ego_dlc_<name>\…`. The files inside the DLC's `ext_NN.cat` are listed without that prefix, so prepend `extensions/ego_dlc_<name>/` when building one virtual file system.
- The DLC index files start with a UTF-8 BOM.
- Some index values double the separator (`assets\\structures\\landmarks\\…`). Collapse repeated `\`/`/` before lookup.
- The Rust implementation (`crates/x4sd-core`: `vfs`, `game`, `catalog`) covers the base game, DLCs and enabled mods. It loads 80 extensions in about 0.55 s (release build). `research/snapcheck.py` is the original Python version (base + DLCs only).

### Texts (verified)
- English is `t/0001-l044.xml` in the base catalogs (`<language id="44"><page id><t id>`). It also holds the DLC module names; only Ventures ships its own `t/` file. Extension text files may be full documents or diffs that add `<page>`/`<t>` elements.
- A text can be built from other texts: `(Argon Base Connection Structure 02){20202,101} {20104,50001} {20111,5102}` shows as "Argon Base Connection Structure 02".
  - Text in parentheses is a developer comment and is dropped.
  - `{page,id}` references resolve recursively.
  - `\(` and `\)` are literal parentheses.
- Implemented in `crates/x4sd-core/src/text.rs`. All 287 station module names resolve fully.

### Buildable station modules (verified)
Structure macros (`assets/structures/**`) whose `class` is one of `connectionmodule`, `storage`, `production`, `dockarea`, `defencemodule`, `habitation`, `buildmodule`, `pier`, `welfaremodule`, `processingmodule`, `ventureplatform` or `radar`, **and** that have `<properties><build><sets>`. That gives 287 modules on the dev machine (base + DLCs + mods). The other classes under `assets/structures` are landmarks, props and asteroids. The display name is `identification@name`; the race is `identification@makerrace`. Whether the *player* can build a module depends on owned blueprints, which is savegame state and out of scope.

### Module shapes for display (verified)
Component XML describes every part in a `<connection tags="part ...">` with `<parts><part name><size><max x y z/><center x y z/></size>`. `max` is the half-extent and `center` the box centre, both in the connection's frame (the connection `<offset>` places it in the module frame). For example `struct_arg_base_02` `part_main` has max z = 400, spanning its snaps at ±400.
- **Used:** the solid parts (`part_main`, `part_interior`, …).
- **Skipped:**
  - `detail_*` parts (decoration);
  - `collisiononly` boxes;
  - `geometryinvisible` parts (e.g. a pier's 3.1 km "bounding_box" docking volume).
- **Fallback:** `landmarks_par_storage_01_macro` has no parts of its own (it is assembled from child landmark pieces), so it gets a box spanning its snap points.
- Real meshes are in `<component>_data/*-lodN.xmf` (X4's binary mesh format). They could replace the boxes later.
- Implemented as `Module::shape` (`catalog.rs`). All 287 station modules have snaps and a shape.

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
