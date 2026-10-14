# X4 construction plans

What we know about how X4 stores station plans, and the ways we could inject one.

## Where plans live
| Location | Content | Status |
|----------|---------|--------|
| `<profile>\constructionplans.xml` | All of the player's saved plans in one file | **verified**: 4 plans, 459 entries, last written 2026-10-04 |
| `<profile>\constructionplan\` | One file per exported or shared plan, e.g. `TerranScrapyard.xml`. Steam Cloud syncs the folder (`steam_autocloud.vdf`). | **verified in game, 2026-10-13**: the Import dialog names this folder, Export wrote a file there, and a file we wrote there was listed and imported. See [Import/export file](#importexport-file-verified-in-game). |
| `libraries/constructionplans.xml` in `08.cat` (1,066,417 bytes) | The game's built-in plans: 116 plans, 3,553 entries | **verified**: same format as the player file (see below) |

`<profile>` = `%USERPROFILE%\Documents\Egosoft\X4\104494216` (the number is the Steam user id).

## File format (verified from the player's file, game version 9.00)
```xml
<?xml version="1.0" encoding="UTF-8"?>
<plans>
  <plan id="player_1791154201" name="defense station no admin" description="">
    <patches>
      <patch extension="ego_dlc_terran" version="900" name="Cradle of Humanity"/>
    </patches>
    <!-- root module: no connection, no predecessor -->
    <entry index="1" macro="dockarea_ter_m_station_01_hightech_macro">
      <offset>
        <position x="53.566" y="1244.669" z="518.797"/>
      </offset>
    </entry>
    <!-- child module: snaps its own connection onto a predecessor's connection -->
    <entry index="2" macro="defence_ter_disc_01_macro" connection="connectionsnap002">
      <predecessor index="1" connection="connectionsnap001"/>
      <offset>
        <position x="53.565" y="1244.669" z="1518.797"/>
        <rotation yaw="90" roll="90"/>        <!-- or <quaternion qx= qy= qz= qw=/> -->
      </offset>
      <upgrades>
        <groups>
          <shields macro="shield_ter_m_standard_02_mk3_macro" group="group02"/>
          <turrets macro="turret_ter_m_laser_02_mk1_macro" group="group02" exact="2"/>
        </groups>
      </upgrades>
    </entry>
  </plan>
</plans>
```

### Elements and attributes
| Element | Attributes seen | Meaning / notes |
|---------|-----------------|-----------------|
| `plans` | none | Root element |
| `plan` | `id`, `name`, `description` | `id` = `player_<unix timestamp of creation>`. **verified**: `1791154201` = 2026-10-04 22:50 UTC, which matches the file's mtime (19:50, UTC-3) |
| `patches/patch` | `extension`, `version`, `name` | The DLCs or extensions the plan needs. One per plan in the sample. |
| `entry` | `index`, `macro`, `connection` | One module. `index` starts at 1. `macro` = the module's macro name. `connection` = the snap point on *this* module. The root entry has no `connection` (5 entries across 4 plans have none). |
| `entry/predecessor` | `index`, `connection` | The module this one attaches to, and the snap point on *that* module (454 entries) |
| `offset/position` | `x`, `y`, `z` | Position in the station's frame. **hypothesis**: metres. Values step by about 1000 or 1200, which fits module sizes. |
| `offset/rotation` | `yaw`, `pitch`, `roll` (any subset) | Degrees. Zero components are left out (145 seen). |
| `offset/quaternion` | `qx`, `qy`, `qz`, `qw` | Used in place of `rotation` in some entries (44 seen). **hypothesis**: the game writes it when there's no clean Euler form. |
| `upgrades/groups/shields`, `.../turrets` | `macro`, `group`, `exact` (turrets) | Equipment per module hardpoint group. `exact` = how many turrets |

### Import/export file (verified in game)
- **Format:** the same as `constructionplans.xml` with exactly one plan: `<?xml version="1.0" encoding="UTF-8"?>\n<plans>\n  <plan …>…</plan>\n</plans>\n`, LF line endings. The exported `<plan>` block is byte-identical to the saved one. A file we built by hand in this format was accepted.
- **File name:** the plan name with characters outside `[%w_%-%() ]` replaced by `_`, plus `.xml`.
- **Id:** the file keeps the plan's `id`. Import uses that id; if the player already has a plan with it, the menu asks whether to overwrite. Generated plans therefore need unique ids. `player_<unix time>` works; other id patterns are untested.
- **Pickup:** the Import list is rescanned when the build menu opens (and probably by the ↻ icon in the Import dialog), so files written while the game runs show up without a restart.
- **Import copies the plan verbatim** into the player's in-memory list and loads it on the build map. It reaches `constructionplans.xml` at the next Save or Export.
- **Same id, other name** (verified from the build menu's Lua, `menu.getImportablePlans` and `menu.buttonImport`, 2026-10-14):
  - The Import list marks a file "[Imported]" when the player has a plan with its id, whatever the names.
  - Importing it asks to overwrite that plan (shown by its old name), then removes it and imports the file.
  - So a plan renamed outside the game with its id kept looks already imported, and the saved plan keeps its old name until the player imports and confirms. Seen with `megashipyard terran2.xml`.
  - **Our rule (D-11):** before writing a plan, if the player's saved plans or import files have its id under another name, it gets a new id (`import::renew_id_if_renamed`).
    - Ids are `player_<unix seconds>`, unique per run, even when two are made in the same second.
    - The design keeps the new id in every undo state.

### Extra forms seen in the game's library plans (verified)
- `plan@name` / `@description` can be text references such as `{20102,1241}` (page, id in `t/`).
- `entry@bookmark` (165 entries) and `entry@fixed` (2 entries) exist. The build-map struct has matching `bookmarknum` and `isfixed` fields.
- Several entries per plan can lack a `connection`/`predecessor` (164 such entries across 116 plans), so **a plan can have several unconnected roots** (floating modules placed by offset only).
- 3 entries have no `<offset>` at all.

### How `connection` names map to module geometry (verified for one module)
`macro` resolves through `index/macros.xml` to a macro file. That file names its component (`<component ref=…>`), which resolves through `index/components.xml` to the component XML. Snap points are defined there:
```xml
<!-- assets/structures/connectionmodules/struct_arg_base_02.xml -->
<connection name="ConnectionSnap001" tags="snap "><offset><position x="0" y="0" z="400"/></offset></connection>
<connection name="ConnectionSnap002" tags="snap "><offset><position x="0" y="0" z="-400"/>
  <quaternion qx="-0" qy="-1" qz="-0" qw="7.54979E-08"/></offset></connection>
```
Plans write the names in lowercase (`connectionsnap002`), so matching is case-insensitive. The names vary (`connectionsnap03`, `connectionsnap_04`), so don't assume a pattern; look the name up in the component.

## Snap math (verified 2026-10-13 with `research/snapcheck.py`)
Let each module's transform be its plan `offset` (position `P`, orientation `Q`), and let `s = (p, q)` be a snap connection's offset in the module's component (local frame). For an entry with `connection` = `s_c` attached to its predecessor's `s_p`:

```
P_child = P_pred + rot(Q_pred, p_p) - rot(Q_child, p_c)          # the two snap points coincide
Q_child = Q_pred · q_p · Ry(180°) · Rz(θ) · q_c⁻¹                # snaps face each other; θ = free twist
```
- **Euler convention** for `<rotation yaw pitch roll>` (degrees): `Q = Ry(yaw) · Rx(−pitch) · Rz(−roll)`, Hamilton product, applied as `v' = Q v Q⁻¹`. `<quaternion qx qy qz qw>` uses the same convention. All 48 order/sign variants were tried; this one is clearly best.
- **Snap axis** = local +Z of the snap frame. Connecting means turning the child's snap 180° about Y so the two face each other, then optionally twisting it by θ about the snap axis.
- **θ** is mostly 0/90/180/270°, but finer steps exist (3°, 15°, 30°, 45°…), matching the build menu's configurable angle step (`SetConstructionMapBuildAngleStep`).

| Data set | Linked entries | Position within 1 cm | Orientation fits the formula | Misses |
|----------|----------------|----------------------|------------------------------|--------|
| Player plans (5 real + 2 test copies) | 693 | 689 | all 689 | the deliberate test C casino (1000 m), and one TerranScrapyard module 1.2 cm off (in 3 copies; rounding) |
| Game library plans (116) | 3389 | 3358 | all 3358 | 31, all inside about 10 hand-made faction plans (`arg_shipyard`, `arg_wharf`, `tel_shipyard`, …). The same modules and snaps match hundreds of times elsewhere, so it's the plans, not the geometry. |

So **the designer can compute exact offsets from snap geometry**, and the validator can flag linked entries whose snaps don't meet.

### Implementation (`crates/x4sd-core`)
- `geometry::attach(pred, pred_snap, child_snap, twist)` places a child.
- `geometry::fit(...)` measures a stored placement: the gap between snap points, the misalignment of the snap axes, and the twist.
- **Verified:** with each entry's own twist, `attach` reproduces all 534 linked placements of the 5 real player plans within 2 cm.
- `validate::validate(plan, catalog)` reports unknown modules, equipment and snaps; missing predecessors; snap gaps over **5 cm** and axis misalignments over **0.5°**; and `<patches>` the plan lacks.
- On the samples, the validator flags exactly one issue in the player plans (the test C casino, 1000 m). In the library it flags 17 entries that are 2.4–400 m off. 14 more library entries are 1–3.5 cm off (`arg_wharf`), which is why the tolerance is 5 cm and not 2.

Not resolvable from base game and DLC data: `recyclemodule_macro` (in a player plan). It comes from a mod, so the module catalogue must also read enabled extensions. Each snap has a local position and orientation; the second one above faces the opposite way (rotated 180° around Y). Coordinates are consistent with metres. DLC modules resolve through the DLC's own `index/` files.

## Game behaviour around `constructionplans.xml` (verified in game, 2026-10-13)
- **No live reload:** a plan added to the file while the game ran did not appear, even after reopening the build menu.
- **Rewritten from memory:** Save and Export rewrite the whole file from the game's in-memory list at once. Import only adds the plan in memory; the file isn't written until the next Save or Export (importing test C left the file untouched). Any external edit made while the game runs is lost at the next write.
- New plans are written at the top of the file. The plan id = Unix time of the first save.
- Number formats in game-written files (checked against about 2,700 values):
  - **positions:** at most 3 decimals, trailing zeros dropped (`530.340` is saved as `530.34`);
  - **quaternions:** C `%.6g` (`0.707107`, `1.38031e-06`);
  - **rotations:** up to 8 significant digits, sometimes more than an `f32` needs (`-179.99989`). They are probably printed from a double.
- The game's library plans (hand-made) also contain positions with more than 3 decimals (`-0.0001526`).
- Our writer (`crates/x4sd-core`) matches the game byte for byte for structure and positions. It writes angles and quaternions as the shortest text that reads back as the same `f32`. So values always round-trip exactly, and the text differs from the game's on about 1.5% of lines (rotation/quaternion only).

## `offset` is authoritative (verified in game, 2026-10-13, test C)
We imported TerranScrapyard with one module (the casino, entry 32, no children) moved 1000 m up, while its `connection`/`predecessor` still pointed at entry 8.
- On the build map the casino floated, attached to nothing, and the menu still showed **"Construction Plan Status: Everything is OK."**
- Export and Save kept `y="530.34"` **and** the stale `<predecessor index="8" connection="connectionsnap03"/>`. That was the only change from the original.

So:
- **The game places modules at `offset`.** It does not snap them using `connection`/`predecessor`.
- **The game does not re-derive or check connections.** A plan whose links don't match its geometry is accepted as valid.
- **Our tool has to compute exact offsets from snap geometry**, and our validator has to check that each linked pair of snaps actually meets, because the game won't catch it.
- Still unknown: what `predecessor` is used for (probably build order, the sequence tree), and how a floating module is actually built. REA is no longer needed for this question.

## Build order and stages (2026-10-13)
- **Order = entry order (verified in data).** In all 4,000+ entries we have (player and library plans), a predecessor always has a lower `index` than its child. So `index` order is a valid build order: a module comes after the module it attaches to.
  - **Hypothesis:** the builder constructs modules in that order. To check in game, export a plan with a changed order and watch the build.
- **Stages (verified in the Lua and schema).** `entry@bookmark="1"` closes a build stage. The build menu reads it as `bookmarknum` and shows "[Stage N]" plus a "Toggle Bookmark" button, but only when `IsCheatVersion()` (Egosoft's internal build).
  - MD actions can build up to a stage: `add_build_to_expand_station <stage>` ("use 0 to construct whole sequence"), and `create_station` / station state with `<stage>`.
  - 42 library plans use stages, e.g. `arg_claytronics_prefab_08` with 7. Player plans have none.
  - **Unknown:** whether stages matter for a station the player builds through the build menu.
- **Priorities** have no field in the format; a priority is a position in the order. Reordering is safe as long as every module stays after its predecessor (offsets are authoritative, so geometry doesn't change).
- **Implemented:** `Design::move_in_build_order`, `build_first`, `build_last` and `set_stage_end` (core). They're in the builder's Build order tab and the MCP tools. Every reorder is checked so a module never comes before its predecessor, then the plan is renumbered.
- **Re-rooting (2026-10-14).** `build_first` makes the module the root: each link on the path from it to the old root is reversed (the child's `connection` and the predecessor's snap swap roles), so that chain hangs on it, then it and the chain go to the front. Offsets don't change, so the geometry stays the same. Checked on a 307-module player plan: every module still comes after its predecessor, and validation finds no issues before or after.
  - **Hypothesis:** the game accepts a root that isn't at the station origin and builds it first. Offsets are authoritative (test C), and 164 entries in real plans have no `connection`. Still to check in game.
- **Still to check in game:** whether the builder follows the order, and whether stages matter for player builds.

## Plots (verified in the build menu Lua, 2026-10-13)
- **Shape:** a plot is a box around the station origin, measured per side in whole km (+X, −X, +Y, −Y, +Z, −Z). Each axis is at most 20 km (`maxPlotSize`) and at least 1 km. Internally it's a size in metres plus a centre offset (`GetBuildPlotSize` / `GetBuildPlotCenterOffset`).
- **Minimum:** the smallest whole-km box that contains the station (`GetConstructionMapMinimumBuildPlotSize`, rounded up per side in `Helper.getPlotData`).
- **Not in the plan file:** the plot belongs to the station in the savegame.
- **Price:** depends on the sector (`GetBuildPlotPrice`), so it can't be computed offline.
- **Our model** (`crates/x4sd-core/src/plot.rs`):
  - The required plot comes from the module part boxes in the station frame.
  - For the player's plans it gives TerranScrapyard 8×5×9 km, the defense stations 4×4×(3–4), and the megashipyard 19×20×15. The megashipyard sits at the 20 km limit, and it was built in game, so our bounds aren't larger than the game's.
  - `validate_plot` reports modules outside, the plot needed, and stations too big for any plot.

## Tilted joints (builder feature, 2026-10-14; game behaviour is a hypothesis)
- The builder can tilt an attached module about its joint (`Design::rotate_attached`): its snap point stays on the predecessor's, its snap axis turns off the predecessor's, and `connection`/`predecessor` stay.
  - A tilt is the swing part of the joint's turn (swing-twist about the snap axis). `set_twist` keeps it; `straighten` removes it.
- The game's build menu never makes such joints. Offsets are authoritative (test C), so the game should place the module tilted.
- **Hypothesis:** it builds them like any other module. Not checked in game. `validate` reports each as "tilted N° off its snap" (`SnapMisaligned`, 0.5° tolerance).

## Equipment: turrets and shields (verified 2026-10-13, game-data test `equipment_groups_and_fit`)
```xml
<upgrades><groups>
  <shields macro="shield_ter_m_standard_02_mk3_macro" group="group02"/>
  <turrets macro="turret_ter_m_laser_02_mk1_macro" group="group02" exact="2"/>
</groups></upgrades>
```
- **Groups.** A module component declares slots as `<connection group="group02" tags="turret medium standard missile hittable combat">` (or `tags="medium shield hittable standard"`). Plans equip a whole group with one macro per kind; there is no per-slot form.
  - Most production and storage modules have groups, as do defence and processing modules and one dock area. Habitats, connectors, piers and build modules have none.
  - Typical group: 1 M shield + 2 M turrets. Defence modules have L groups, up to 4 shields and 10 turrets.
- **Fit.** Equipment fits a slot when the slot has every tag of the equipment component's `component` connection, `component` itself excepted. For example `turret medium standard hittable combat` fits the slot above.
  - Matches all 1,020 equipped groups in the player and library plans.
- **Available equipment.** Wares tagged `equipment` plus `turret` or `shield`, with a recipe, not `noplayerblueprint`, that fit a slot of some module players build: 103 with DLCs and mods, out of 216 buildable.
  - The rest are ship-only or other sizes. Some share a name with a station variant, e.g. `turret_arg_m_beam_01_mk1` (ships) and `turret_arg_m_beam_02_mk1` (stations), both "ARG M Beam Turret Mk1".
  - Xenon equipment carries a `xenon` tag that no player slot has.
- **`exact`** is the number of slots of that kind filled.
  - Every sample fills all of them. Turrets always write it; shields only with 2 or more slots.
  - **Hypothesis:** absent means 1, and partial counts are allowed (the build menu keeps a count per group).
- **`path=".."`** appears only in the game's hand-made library plans; game-written plans have none.
- **Our editing** (`equipment.rs`, `Design::set_equipment` / `equip_modules` / `copy_equipment`):
  - Writes `exact` the way the game does: always for turrets, for shields unless one of one slot is filled. It keeps any `path` it read.
  - `validate` reports equipment on a group the module lacks, equipment that doesn't fit, and more than the slots.
  - The *loadout level* is our own rule, not the game's: fitting non-missile equipment, the module's race first, cheapest/middle/most expensive. **Hypothesis**: the game's `GenerateModuleLoadout(level)` may pick differently.
  - **Not checked in game yet:** that an imported plan builds with this equipment, and what a partial count (fewer than the slots) does.

## Overlaps (calibrated 2026-10-13, game-data test `overlap_calibration`)
The plan format doesn't stop modules from intersecting. The build menu has an **"Allow module overlap"** setting (`moduleoverlap`, off by default and off in the user's profile; it toggles `SetConstructionMapCollisionDetection`). We need our own overlap check for auto-placement and for `validate`.
- **Collision boxes, not part boxes (verified on data).** The part boxes (`<size>`, used for the plot) are too loose for overlaps: in the player's 5 real plans, 29 unlinked pairs intersect by more than 1 m, up to 491 m. Example: the Terran medical-supplies box includes its thin 1.3 km spire, and a habitat stands next to the spire.
  - So `Catalog::collision_boxes` builds boxes from each colliding part's `-collision.xmf` mesh: every part tagged `part` but not `nocollision`, collision-only boxes included, using the coarser `-collision1.xmf` where it exists.
  - The mesh's triangles are halved along their longest axis up to 7 times (at most 128 boxes per part), or until a box is 100 m or less. 1 to 767 boxes per module; all 287 modules in 2.4 s, once.
  - With these, 14 unlinked pairs in the player plans intersect by more than 1 m (up to 98 m).
  - Halving 10 times (25 m boxes) takes 239 s instead of 3 s and leaves 10. Some depths shrink (98 m → 45 m); others stay at 35, 19, 11 m.
  - **Corrected 2026-10-14:** those were box looseness too, not real intersections. The user confirmed TerranScrapyard was built and exported in game. On the collision-mesh triangles (below), none of the 14 pairs intersect. Axis-aligned boxes stay loose around curved and diagonal geometry such as the Terran habitat rings, however finely they are split.
- **Joints interlock by design (verified on data).** Where Terran modules meet at a snap in the player's plans, their boxes intersect by at most 0.4 m (471 pairs).
  - In the game's 116 library plans (all races), 228 of 2,658 joined pairs intersect by more than 20 m, up to 130 m. Connector collars and defence discs reach into what they attach to; Argon medical supplies reach 41 m past their own snap.
  - So **two modules whose snap points coincide (within 10 cm) never overlap**, linked or merely touching.
- **Rule** (`layout::overlap`, `layout::overlaps`, 2026-10-14). Two modules that aren't joined at a snap overlap when two things hold:
  - a pair of their collision boxes intersects by more than **1 m** (separating axis theorem over the 15 box axes);
  - their collision meshes (`-collision.xmf`, full detail) really intersect: an edge of a triangle of one passes through the inside of a triangle of the other. Faces or edges that only touch don't count.
  - The mesh test (`meshes_intersect`) walks a bounding-volume tree of each mesh (leaves of up to 8 triangles; `Catalog::collision_mesh`, built on first need), then tests triangles.
  - **Verified:** a medical-supplies module shifted 30 m into a copy of itself intersects; at 5 km it doesn't (unit and game-data tests).
  - Player plans: **0** overlaps (the 14 box pairs are all clear on the meshes). The only exception is test C's casino, moved 1 km up, which now sits inside two modules and is reported.
  - Library plans: 28 of the 84 box pairs intersect on the meshes. These are hand-made NPC stations, spawned by script rather than built in the build menu.
  - Time for the 5 player plans: 264 ms the first time (loading meshes), 17 ms after that.
  - **Auto-placement keeps the box test alone.** It is stricter, so what it places has no overlaps either way.
  - Checking takes 16 ms for the 5 player plans, and 5 ms for a 239-module station.
- `validate` reports them as "entry N: overlaps entry M" on the later-built module.

## Clearance: exclusion zones (research 2026-10-14)
The red volume the build menu draws in front of a pier while you move it is an **exclusion zone**: space a module keeps free for ships to dock or leave. Research tool: `cargo run -p x4sd-core --release --example zonecheck -- <plans.xml>...` (or `-- --list`).
- **Where they are (verified, component XML).** A zone is a component connection tagged `exclusionzone <class>`, with an offset (position + orientation) and nothing else, e.g. on the Terran 3-Dock E Pier (`pier_ter_harbor_03`):
  `<connection name="Con_exclusionzone001" tags="exclusionzone ship_xl">` at (−700, 0, 3000), turned so its +Z points along −X.
  - `Catalog`: `Module::clearance` (tag + frame), `Catalog::exclusion_size(tag)`.
  - The engine checks the tags: X4.exe has the message `Macro '%s' connection '%s' has missing exclusion zone tag(s)` (verified, string in the exe).
- **Size per class (verified, `libraries/parameters.xml`, `<exclusionzones>`; no DLC or installed mod changes it):**

  | Tag | x | y | z (length) |
  |---|---|---|---|
  | `ship_xs`, `ship_s` | 70 | 70 | 400 |
  | `ship_m` | 180 | 180 | 500 |
  | `ship_l` | 1200 | 1200 | 100 000 |
  | `ship_xl` | 4000 | 4000 | 100 000 |
  | `radar_arg_dish_01` | 1600 | 1400 | 910 |

  So L and XL zones run 100 km: in practice, out of the station and past any plot. The construction map draws them as an outline (`<construction><exclusionzone width="10" color="holomap_construction_exclusionzone">`).
- **Which modules have them (verified, `--list` over the 287 station modules):**
  - every **pier**: one `ship_xl` zone per dock, starting just past the dock and pointing away from the pier's snaps. 3-Dock E Piers: three zones 3 km apart along the spine, all towards −X (the snaps are at +X and ±Y). 1-Dock Piers: one, about 1.9 km out (Terran `pier_ter_harbor_05`: 0.3 km). T and 4-dock piers: one per arm.
  - every **fabrication and maintenance bay**: L and XL bays have **two**, out of both open ends of the bay (±Z; XL at ±1200 m, the snaps at ±X). S/M bays have one to three `ship_m` zones; Boron bays one at −Z.
  - most **M dock areas** (`ship_m`, 500 m straight up, +Y), the L/XL venture dock, the scrap processors, and the Argon radar dish. Habitats, production, storage and connection structures have none.
- **Shape (hypothesis, strongly supported by calibration):** a **box `x` by `y` wide (full widths), starting at the connection and running `z` along its +Z.**
  - The XML gives no shape. REA couldn't analyse the 55 MB X4.exe in time, and its strings don't say.
  - Calibration with `zonecheck` (other modules' collision boxes reaching more than 1 m into a zone):

    | Plans | Zones | Box, full widths | Box, twice as wide |
    |---|---|---|---|
    | User's game-built plans (TerranScrapyard, megashipyard terran, asd1, two defence stations) | 144 | **4**, all one case (below) | 81 |
    | Game library plans (base + DLCs: 267 of 275; our reader rejects 8) | 1538 | 14, in 7 hand-made faction plans | 296 |
    | Our Optimize output `megashipyard terran2` | 101 | 39 | 85 |

  - The user's **megashipyard terran**, built in the build menu with 15 3-Dock E Piers stacked 400 m apart plus L/XL bays, has **no** intrusion. The game's own placement seems to keep these zones clear, and with exactly these sizes.
  - The 4 game-built cases are TerranScrapyard's two pairs of 1-Dock Piers joined back to back at a snap: each one's arm reaches 117 m into the other's zone, where the zone starts. So modules **joined at a snap** to the zone's module seem exempt (as for overlaps).
  - Library exceptions: `arg_equipmentdock`, `par_equipmentdock` (a pier and storage 1.3–6 km out in front of L/XL maintenance bays), `tel_shipyard`, `yak_wharf`, `xen_wharf`, and two radar dishes. These are hand-made stations spawned by script, like the library's overlaps and snap misfits.
  - The user describes the in-game zone as a frustum, much wider than the pier, made of overlapping volumes, one per dock. A 4 km wide, 100 km long box seen in perspective looks like that, and XL zones 3 km apart and 4 km wide overlap by 1 km.
- **What the game does about them:**
  - **Build-menu status (verified, `menu_station_configuration.lua`):** the plan errors and warnings check builder, resources, blueprints, a pier and a dock, venture parts, illegal wares. Nothing checks zones.
  - **Import (verified, user, 2026-10-14):** `megashipyard terran2` (39 blocked zones by this model) imports and builds with no errors, and the status says "Everything is OK".
  - **Moving a module in the build menu (user's observation; not retested):** a module that blocks a zone turns red, like a collision.
  - **The engine's layout generator (verified, string in X4.exe; x4ep_planner logs):** `ConstructionSequenceGenerator %s! After %u failed connection attempts, %u collisions, %u exclusion zone intersections`. It treats a zone intersection like a collision and gives up.
- **Our rule (D-12, implemented 2026-10-14):** `layout::zones` builds the boxes, `layout::blocked_zones` lists modules reaching more than 1 m into another's zone (not its own, not one joined to it at a snap), and `validate` reports "entry N: blocks the approach of pier M" / "blocks the exit of bay M". Auto-placement keeps every zone clear (see "Auto-placement").
  - Game-data test `clearance_zones`: the user's game-built plans block nothing; `megashipyard terran2` (our first Optimize) has 652 blocker/zone pairs.
- **Ships leaving a bay:** the space is the bay's two zones (L: 1.2 km square, XL: 4 km square, out of both ends). L bays also have a `launchpos` and a `todock` connection. We use the zone sizes instead of measuring the largest ship of each size: the game sizes the space itself.

## Auto-placement (2026-10-13)
`layout::auto_place` attaches a module list (for example a production plan's) to a design at exact snap positions, without overlaps and inside the plot.
- **Connectors:**
  - The hub is the race's buildable connection structure with the most snaps (Argon or Terran cross 01).
  - The struts are its straight 2-snap structures, one per length (Argon 400, 800, 1600 m).
- **Order:** the largest modules first; dock areas and piers last.
  - With `Options::layered` (Optimize), the order goes in layers instead. Modules without turret/shield slots come first. Defensible ones come next, ranked by distance from the core's radius (the farthest slotless module), so they wrap the core rather than fill gaps in it. Dock areas and piers come last.
    - Layers cost compactness. "100 solar, staffed" needs a 20×20×20 km plot in layers, against 8×8×8 biggest-first. So *Place all* stays biggest-first.
  - "Nearest" is measured from the centre of the station as it was (2026-10-14), so an off-centre station grows around itself; for a new design that is the origin.
- **Placement:** each module goes onto the free snap nearest the origin where it fits, with the most compact of its own snaps and the four twists. Alternatively it goes onto a new hub (optionally on a strut) on a free snap, when that is 2 km nearer.
  - **Fewer connection structures** (`Options::fewer_connectors`, 2026-10-14): 4 km instead of 2 km. Game-data test `fewer_connectors`:

    | Case | Threshold | Connection structures | Plot |
    |---|---|---|---|
    | "100 solar, staffed" | 2 km (default) | 81 | 9×8×8 km |
    | | 4 km | 37 | 11×12×13 km |
    | | 6 km | 25 | 16×16×16 km |
    | | 8 km | 20 | 20×20×20 km |
    | | none (hubs only where no free snap fits) | 20 | 20 km snake, a dock blocked |
    | TerranScrapyard rebuilt (Optimize) | 2 km | 56 (343 modules) | 10×10×10 km |
    | | 4 km | 1 (288 modules) | 14×16×12 km |

    4 km roughly halves them while the station stays well inside the largest plot; past that the station sprawls (also a load, see "In-game load").
  - **Docks stay reachable** (user request, 2026-10-14):
    - A dock area or pier needs a **clear corridor**: its own box swept 3 km straight out from the station's centre, clear of other modules' collision boxes.
    - No module (or connection structure) may enter the corridor of a dock already on the station.
    - **Every run reworks docks.** It first lifts off the dock areas and piers with nothing attached and places them again last, at the nearest spot with a clear corridor. They keep their equipment.
    - Docks that carry other modules stay where they are, and their corridors are protected.
    - A lifted dock that fits nowhere goes back to its old position, free-standing.
    - The run reports `docks_moved` and `docks_blocked` (placed without a clear corridor because there was no such spot).
    - **Hypothesis:** a 3 km corridor straight out is enough room for ships to reach them. Not checked in game.
  - **Clearance** (D-12, 2026-10-14; see "Clearance: exclusion zones"): modules that need clear space (`layout::outer`: dock areas, piers, and every module with exclusion zones, such as fabrication and maintenance bays) form the outer layer, go last, and are lifted and re-placed on every run like docks.
    - They are placed where all their zones are clear of modules not joined to them; their zones are then protected like corridors.
    - The 3 km corridor stays for dock areas and piers whose zones are all shorter than it (M dock areas: 500 m up). Without it a dock area sat among the "100 solar" panels with only a clear column above it. Piers don't need it (100 km zones); requiring it too left 6 modules of the megashipyard without a spot.
    - Game-data test `optimize_keeps_zones_clear`, the user's megashipyard terran plus its 147 fixes: keep shape 483 entries, 19×20×12 km, 6 s; rebuild 351 entries, 11×12×16 km, 7–11 s. Both: no zone blocked, nothing lost, no overlaps. The builder shows "No problems found" for both.
    - **Why *Rebuild connection structures* made bays worse before:** bays have no turret/shield slots, so they were in the slotless core layer; a rebuild places biggest-first within a layer, so the XL bay went first, onto the hub.
- **Sides** (D-15, 2026-10-14; `Options::sides`): modules given sides of the station (front +Z, back −Z, left −X, right +X, top +Y, bottom −Y; meant for defence platforms) are spread over them and placed after everything else, docks included.
  - Spots rank by distance from the axis out through that side, less how far out they are: the middle of that face first.
  - Three passes: (1) all of the module beyond the station's centre plane on that side, and a clear **lane**: its box extended straight out from its face to 3 km past the station's bounds, entered by no other module; (2) a clear lane anywhere; (3) anywhere it fits. Passes 2 and 3 count in `sides_blocked`. A placed module's lane stays clear, so a second one doesn't go in front of the first.
  - Game-data test `planner_extras`: "100 solar" + an L fabrication bay + 6 Argon discs puts one disc in the middle of each face (e.g. front at z 3,200 m on a station reaching 3,352 m), or 3 + 3 on front and back. On "2 hull parts" (29 modules, 1 hub) the station has only a handful of free snaps and none on some sides: 2 of 6 discs are off their side (reported). Nothing is ever in front of a disc.
- **Plot:** with *grow*, it stays within the largest plot (20 km per axis) and reports the plot needed. Placements keep 1 m inside the plot, so rounding to stored positions can't push a module out.
- **Undo:** the whole placement is one undo step.
- **Results** (game-data test `auto_place_planned_stations`):

  | Case | Modules | Connection structures | Plot | Time |
  |---|---|---|---|---|
  | "100 solar, staffed" | 137 | 81 (2.9% of the cost) | 8×8×8 km | 2.1 s |
  | Terran hull parts | 18 | 1 | 6×4×5 km | 0.4 s |
  | TerranScrapyard's fix | 219, onto the existing station (its one free dock moved outside) | 21 | 10×10×11 km | 1.1 s |
  | "100 solar" again, plus 10 solar | 10; both docks lifted and placed again, clear | 9 | 8×8×8 km | 0.9 s |

- **Optimize** (`layout::optimize`, user request 2026-10-14):
  - It lifts the station's modules: keeping the shape, all but the connection structures and the modules carrying them; rebuilding, everything.
  - It places them again, together with the shortfall fixes, in layers, and gives each module back the equipment of one lifted module of its type.
  - All or nothing, one undo step. Unknown modules make it refuse.
  - TerranScrapyard plus its 219 fixes, Terran:

    | Mode | Time | Plot | Mean distance: no slots / defensible / docks |
    |---|---|---|---|
    | Keep shape | 2.8 s | 11×11×10 km | 2.3 / 3.8 / 3.7 km |
    | Rebuild | 1.9 s | 9×10×10 km | 2.1 / 3.7 / 3.0 km |

  - Both modes: nothing lost, equipment kept, no overlaps, docks clear.
- **Not checked in game yet:**
  - that the game builds these stations (modules attached to modules, connectors);
  - that ships reach the docks.

## In-game load (hypothesis, 2026-10-14)
What likely costs the game frame rate, from players' experience; **nothing here is measured** in game:
- **Module count** matters most: each module is an object the game draws and checks for collisions. Connection structures add to it and make nothing.
- **Turrets:** each aims and fires on its own, which costs most in fights.
- **Traffic and sprawl:** docking ships, and ships steering around a sprawling station (pathfinding).
- **Location:** stations in other sectors run on a cheaper simulation, so the cost is mostly felt in the same sector.

`perf::load` counts these (modules, connection structures, turrets and shields as slots filled, dock areas plus piers, size). It feeds the builder's *In-game load* section, MCP `station_load` and `x4sd show`. Checked on the samples: TerranScrapyard 84 modules (16 connection structures); "defense station" 288 turrets on 144 entries (`exact`), 144 shields. A way to verify: build a sprawling and a compact variant of one station in game and compare the frame rate in its sector.

## Unknowns
- Does the game load external edits to `constructionplans.xml` at startup (option A after a restart)? Low priority now that B works.
- How does the game build a module whose position doesn't match its link (floating, like test C or the 31 library cases)?
- What the game does with overlapping modules in an imported plan (build them anyway?), and how strict its own collision check is. Our rule is in "Overlaps" above.
- How does the game handle invalid entries (unknown macro, mismatched connection, missing DLC)?
- Are ids without the `player_` prefix accepted on import?

## Injection options
None has been tested in-game yet. Ranked by current evidence:

| Option | How | Evidence | Pros | Cons / risks |
|--------|-----|----------|------|--------------|
| **B. Plan file plus in-game import** (works) | Write a plan XML into `<profile>\constructionplan\`, then use Import in the build menu | **Verified in game**: a hand-written file was listed and imported while the game ran | Vanilla, no mods, no restart. Never touches the master file. | The player clicks Import. Id clashes prompt an overwrite. |
| A. Edit the profile file | Add a `<plan>` to `<profile>\constructionplans.xml` while the game is closed | **Doesn't work live** (verified). After a restart: untested. | No click needed | The game must be closed. Needs a backup. The game rewrites the file from memory on Save, Export and Import. |
| D. Live bridge through a mod | MD `add_build_to_expand_station` / `apply_construction_sequence` / `create_station` with a plan ID, triggered over IPC (e.g. `sn_mod_support_apis` pipes) | The MD actions are **verified** in `common.xsd` | Agents could act on the running game, and even start the build | Needs a mod. Most work. Later phase. Builds on A or B for getting the plan in. |
| C. Ship as an extension | A mod that patches `libraries/constructionplans.xml` | Weak: the plan menu lists only `source == "local"` plans unless the game is a cheat build (**verified** in Lua) | Survives game writes | Plans wouldn't show in the player's list. Only useful through MD by ID (D). |

**Prior art:** `x4ep_planner` builds stations live, but it lets the engine generate the layout from a list of macros instead of injecting exact positions. Details in [x4-plan-apis.md](x4-plan-apis.md#prior-art-x4ep_planner-ghostaus-station-planner-ws_3777272604).
