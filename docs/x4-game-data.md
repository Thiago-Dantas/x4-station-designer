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
Structure macros (`assets/structures/**`) whose `class` is one of `connectionmodule`, `storage`, `production`, `dockarea`, `defencemodule`, `habitation`, `buildmodule`, `pier`, `welfaremodule`, `processingmodule`, `ventureplatform` or `radar`, **and** that have `<properties><build><sets>`. That gives 287 modules on the dev machine (base + DLCs + mods). The other classes under `assets/structures` are landmarks, props and asteroids. The display name is `identification@name`; the race is `identification@makerrace`, else the race code in the macro name (`_arg_`, `_tel_`…; 2026-10-15, user report: the Teladi dock area showed only under *Generic*). That gives 10 dock areas, the Argon wide-area sensor array and the Boron art gallery a race; the planner already used this fallback. Whether the *player* can build a module depends on owned blueprints, which is savegame state and out of scope.

**Build-menu categories** (verified: `leftBar` in `menu_station_configuration.lua`, names from page 1001), in the game's order:

| Category | Our macro classes |
|---|---|
| Production Modules | `production` |
| Build Modules | `buildmodule` |
| Storage Modules | `storage` |
| Habitation Modules | `habitation` |
| Welfare Modules | `welfaremodule` |
| Dock Modules | `dockarea`, `pier` |
| Defence Modules | `defencemodule` |
| Processing Modules | `processingmodule` |
| Other Modules | `connectionmodule`, `radar` |
| Venture Modules | `ventureplatform` (only when venture features are on) |

The menu files `moduletypes_radar` under Other (verified in the Lua). The class-to-category mapping otherwise follows the module classes (inferred). The builder's palette uses this order.

### Module shapes for display (verified)
Component XML describes every part in a `<connection tags="part ...">` with `<parts><part name><size><max x y z/><center x y z/></size>`. `max` is the half-extent and `center` the box centre, both in the connection's frame (the connection `<offset>` places it in the module frame). For example `struct_arg_base_02` `part_main` has max z = 400, spanning its snaps at ±400.
- **Used:** the solid parts (`part_main`, `part_interior`, …).
- **Skipped:**
  - `detail_*` parts (decoration);
  - `collisiononly` boxes;
  - `geometryinvisible` parts (e.g. a pier's 3.1 km "bounding_box" docking volume).
- **Fallback:** `landmarks_par_storage_01_macro` has no parts of its own (it is assembled from child landmark pieces), so it gets a box spanning its snap points.
- Implemented as `Module::shape` (`catalog.rs`). The boxes are now only a fallback; the builder draws the real meshes (below).
- They also give the plot a station needs. For overlaps they are too loose (they cover thin spires and the space beside them); overlaps use boxes built from the collision meshes (below).

### Module meshes: `.xmf` (reverse-engineered and verified 2026-10-13)
**Files.** Each part has meshes at `<geometry>/<part>-lodN.xmf`, LOD 0 being the finest. `<geometry>` is the component's `<source geometry="...">` folder (verified 2026-10-14): usually `<component path>_data`, but a mod can borrow another component's. RecycleShipsAndStations' `recyclemodule` uses `assets\structures\production\prod_gen_dronecomponents_data`, and the builder showed it as a plain box until this was read. That mod's macro has no build sets, so its display name comes from the macro's `<identification name>` (`Catalog::display_name`): "Recycle Module". Main hull parts usually have LOD 0–3, detail parts only LOD 0. `-collision.xmf` and `-collision1.xmf` are collision meshes and aren't drawn.

**Collision meshes (verified 2026-10-13).** Same container; the vertex buffer has **no declaration** (0 elements) and 12-byte items: float3 positions only, plus an index buffer. No materials.
- Example: `prod_ter_medicalsupplies` `part_main-collision.xmf` has 37,636 vertices and 75,040 triangles.
- Where `-collision1.xmf` exists it is a coarser copy with the same bounds (37,424 triangles in that example).
- Almost every part has one, including `nocollision` detail parts and `collisiononly` boxes.
- All 287 station modules' collision meshes parse. Used for overlap checks (`Catalog::collision_boxes`; see docs/x4-construction-plans.md, "Overlaps").

**Format.** Little endian, Direct3D 9 conventions:
- **Header, 64 bytes:**
  - `XUMF`, then a u8 version (3), a u8 big-endian flag, a u16 header size (64);
  - u8 buffer count, u8 buffer-description size (188), u8 material count, u8 material size (136), u16 0;
  - u32 vertex count, u32 index count, u32 primitive type (4 = triangle list), zero padding.
- **Buffer descriptions, 188 bytes each:**
  - u32 type (0 vertex, 30 index), u32 usage index, u32 data offset, u32 compressed, u32 0;
  - u32 format (index buffers: 30 for 16-bit, 31 for 32-bit), u32 compressed size, u32 item count, u32 item size, u32 sections;
  - 16 bytes reserved;
  - u32 element count, then 16 × (u32 `D3DDECLTYPE`, u32 usage). In the usage, the **low byte** is the `D3DDECLUSAGE` and the next byte the usage index. The upper bytes can hold leftover data (e.g. `0x0DEC0000`).
- **Materials, 136 bytes each:** u32 first index, u32 index count, char[128] name (e.g. `generic.generic_p1_hulltexture`). They split the index buffer into per-material ranges.
- **Data:** each buffer sits at `data start + offset` (data start = after the materials), zlib-compressed when flagged. The decompressed size equals count × item size.

**Vertex layout.** A typical vertex is position (FLOAT3), normal (D3DCOLOR), tangent (D3DCOLOR), texcoords (FLOAT2, up to 3) and colours (D3DCOLOR, up to 2), 32–44 bytes in all.
- **Normals:** a D3DCOLOR is stored as bytes B, G, R, A, so `n = (byte2, byte1, byte0) / 127.5 − 1`. Verified: unit length, and consistent with the geometry.
- **Coordinates and winding:** coordinates are the game's (left-handed). Triangles are **clockwise**: right-handed face normals come out opposite to the stored ones on 5,330 of 5,330 vertices. Mirroring Z for Three.js makes them counter-clockwise.

**Verification.** The decoded bounds equal the component's `<size>`, e.g. `struct_arg_base_02` part_main ±138.89 / ±74.87 / ±400, and every index is in range. All 286 modules with own parts decode.

**Which parts are drawn** (`Module::parts`, `Catalog::module_model`):
- **Included:** every `<connection tags="part ...">` except `collisiononly` and `geometryinvisible`. Animation parts are included, because they hold real geometry: rings, cranes, signs.
- **Placement:**
  - A connection's `parent="<part name>"` puts the part on top of its parent's placement; the chain is composed.
  - `<part ref="<component>.<part>">` borrows another component's part. Its meshes are in that component's `_data` folder (e.g. piers use `xref_dockingbay_gen_xl_pier_01.part_main`).
- **Detail level:** from the tags (`detail_l` 1, `detail_m` 2, `detail_s` 3, `detail_xs` 4, main 0). Parts named `*interior*` count as 3.
- **Skipped materials:** names containing `fx`, `lightcone`, `decal`, `effects.`, `gradient`, `laser`, `flare` or `pattern_hex`. These are glows, light cones and decals that the game draws as overlays.

**Cost** (all 286 modules; detail parts only exist at LOD 0 and can be as heavy as the hull):

| Builder setting | LOD, detail parts | Triangles, all modules | Per module |
|---|---|---|---|
| Fast | LOD 3, main hull only | 1.8 M | about 6k |
| Normal | LOD 2, up to `detail_m` | 6.4 M | about 22k |
| Detailed | LOD 1, all details | 16.3 M | about 57k |

`research/xmf.py` is the Python research parser.

### Textures (verified 2026-10-13)
**Material lookup.** A mesh material name such as `generic.generic_p1_hulltexture` is `collection.material` in the material libraries:
- `libraries/material_library.xml` (`p1` materials);
- `libraries/material_library_1.xml` (the newer `p2*` collections: `p2generic`, `p2argon`, …; 204 materials);
- the DLCs' `libraries/material_library.xml`, as `<diff><add sel="/materiallibrary">`.

A material lists `BitMap` properties: `diffuse_map`, `normal_map`, `smooth_map`, `metal_map`, plus layers for paint, dirt and detail. Values are game paths without extension.

**Files.** `<path>.gz` is a gzip-packed DDS; `<path>-small.gz` is a 128×256 version. Full ones are 2048×4096 with 13 mips.
- Formats seen: BC1, BC3, BC4 (single channel, e.g. aged-hull diffuse maps) and BC5 (some normal maps).
- Block-compressed data goes to the GPU as is (WebGL `s3tc` and `rgtc` extensions).
- **Mip levels above 1024 px are skipped,** so a texture costs 0.2–0.7 MB instead of 5–11 MB.

**In the builder:**
- Only diffuse maps are used.
- BC4 is shown as grey via a small shader patch.
- Game UVs and DDS data both use Direct3D's top-left origin, uploaded unflipped.
- Normal, smoothness and metal maps are not used yet. Normal maps use game-specific channel packing (BC3 or BC5).

**Cost (measured):**

| Plan | Modules | Textures | GPU memory | Load time | Frame rate |
|---|---|---|---|---|---|
| TerranScrapyard | 84 | 18 | 9.3 MB | 0.2 s | no change (240 fps) |
| megashipyard terran | 332 | 18 | 10.5 MB | 0.2 s | no change (240 fps) |

The megashipyard first dropped from 240 to 68 fps with textures, because there was one draw call per module per material. Drawing **instanced per module type** fixed that, in both modes.

### Wares, recipes and build methods (verified 2026-10-13)
- **Module wares:** a module or equipment ware in `libraries/wares.xml` names its macro in `<component ref>`. It has one `<production method="…">` recipe per build method (`<primary><ware ware amount>`) and a `<price min average max>`.
  - Example: Energy Cell Production (default) = claytronics 260, energy cells 520, hull parts 951.
- **Build methods:** `<wares><production><method id name tags>`. Players can pick argon, default ("Universal"), paranid, teladi, boron, closedloop, split and terran; processing, recycling, research and xenon are `noplayerbuild`.
- **DLC patches:** `<add sel="/wares">` (new wares), `<add sel="/wares/production">` (methods) and `<add sel="/wares/ware[@id='X']">` (extra recipes, e.g. 132 Terran ones).
- **After merging:** 302 of 325 module wares have only `default`. Terran modules' `default` already uses Terran wares. 19 shared modules also have terran/boron recipes; otherwise the game falls back to `default`.
- **Mods:** they patch wares with `<replace>` too (prices, production times, a few recipes). Our reader (`crates/x4sd-core/src/wares.rs`) applies only `<add>`. It skips extension files that aren't valid XML: `faction_fix_pack` ships a broken `wares.xml`.
- **Totals:** the player's plans at average prices: TerranScrapyard ≈ 48.9 M Cr, defense station ≈ 95 M Cr (mostly turrets and shields; one disc alone 0.64 M, with its 24 groups 7.9 M), megashipyard ≈ 315 M Cr.

### Production, workforce and storage (data verified 2026-10-13; maths are hypotheses)
- **Production modules:** the macro's `<production wares="X"><queue ware="X" method="…"/>` names the ware and the recipe method (`default` if absent).
  - The scrap recyclers queue several wares: `<queue><item ware method="recycling"/>…`.
  - `<workforce max="N"/>` is the staff it needs, e.g. energy cells 90 and hull parts 270.
- **Rate:** the ware's `<production time amount method>` in `wares.xml`, inputs in `<primary>`.
  - `<effects>`: `work` (output bonus at full staff, e.g. energy cells 0.43, 67 recipes) and `sunlight` (energy cells, product 1).
  - Example: 175 energy cells per 60 s = 10,500 per hour; 15,015 fully staffed (hypothesis: the bonus adds output, not input).
- **Habitats:** `<workforce race="argon" capacity="1000"/>`. Workers eat the `workunit_busy` recipe of their race (argon uses `default`).
  - Example: 200 workers per 600 s eat food rations 75 and medical supplies 45, so 1000 argon workers use 2250 and 1350 per hour (scale is a hypothesis).
- **Welfare modules:** `<workforce growthrate>` (growth only).
- **Storage:** `<cargo max tags>`, with tags `container`, `solid`, `liquid` or several. Ware `transport` and `volume` (m³) come from `wares.xml`.
- **Processing modules:** `<products><ware amount/>` (scrap from ships).
- **Reader:** `crates/x4sd-core/src/economy.rs` (`ModuleEconomy` in `catalog.rs`).
- **Raw materials** are brought in, never made by a player station. Two `wares.xml` tags mark them (verified 2026-10-13 in the base game and DLC files):
  - `minable` (mined): ore, ice, silicon, nividium (solid), and methane, helium, hydrogen (liquid);
  - `recycling` (salvaged scrap): scrap metal, Kha'ak scrap metal, raw scrap, raw Kha'ak scrap.
  - The scrap wares' only recipes are `method="processing"` with `tags="noplayerbuild"`. The user confirmed that scrap metal is brought in (2026-10-13).
  - `Ware::raw` in `crates/x4sd-core/src/wares.rs`.
- **Build sets** (`<build><sets><set ref>`): `factory` is the set for player stations.
  - Trade-station storage (`tradestation_argon`), Xenon modules (`factory_xenon`) and landmarks lack it.
  - Some factory-set docks and piers are still trade-station or venturer variants.
  - The planner only uses `factory` modules (`crates/x4sd-core/src/planner.rs`).
- **Maker races:** most producers are generic (no `makerrace`). Food, medicine and race-specific wares have one module per race, e.g. medical supplies ×6. Habitats, storage and piers exist per race, but most dock areas have no maker race; their race shows only in the macro name (`dockarea_arg_…`).
- **Player's plans:** TerranScrapyard: 10,000 of 11,665 workers housed (bonus 86%), hull parts +12,645 per hour, energy cells −758,820 per hour (the scrap recyclers).

### Savegames: the player's blueprints (verified 2026-10-14)
- **Files:** `<profile>\save\*.xml.gz` (`save_NNN`, `autosave_NN`, `quicksave`). They are gzip; the game can also write plain `.xml`. `save_001` is 120 MB gzipped and 1.0 GB of XML. The header has `<info><save name date/><game version build start …/><player name location money/>`.
- **Blueprints:** one `<blueprints>` block of `<blueprint ware="…"/>` lines, one per line. It is the player's: a full scan of `save_001` found exactly one, 447 wares, 113 of them `module_*`, the rest paint, clothing, equipment and so on.
  - It starts at byte 241 M of the XML (line 6.0 M), so a reader can stop after it. Python reaches it in 2.5 s and reads the whole file in 9 s; our reader (`crates/x4sd-core/src/save.rs`, release build) reaches it in 0.43 s.
  - A module ware maps to its macro through `<component ref>` in `wares.xml` (all 113 did). 109 of them are palette modules; the other 4 (`recyclemodule_macro` and three showroom dock areas) have no `factory` build set and aren't offered anyway.
- **The game offers only blueprinted modules:** the station build menu fills its module list from `C.GetBlueprints(set, moduletype, "")`, the player's blueprints (`menu_station_configuration.lua` line 1671). It also warns about plans with modules the player has no blueprint for (lines 2049–2081).

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
