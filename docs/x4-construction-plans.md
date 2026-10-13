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

So **the designer can compute exact offsets from snap geometry**, and the validator can flag linked entries whose snaps don't meet (tolerance about 2 cm to absorb the game's 3-decimal rounding).

Not resolvable from base game and DLC data: `recyclemodule_macro` (in a player plan). It comes from a mod, so the module catalogue must also read enabled extensions. Each snap has a local position and orientation; the second one above faces the opposite way (rotated 180° around Y). Coordinates are consistent with metres. DLC modules resolve through the DLC's own `index/` files.

## Game behaviour around `constructionplans.xml` (verified in game, 2026-10-13)
- **No live reload:** a plan added to the file while the game ran did not appear, even after reopening the build menu.
- **Rewritten from memory:** Save and Export rewrite the whole file from the game's in-memory list at once. Import only adds the plan in memory; the file isn't written until the next Save or Export (importing test C left the file untouched). Any external edit made while the game runs is lost at the next write.
- New plans are written at the top of the file. The plan id = Unix time of the first save.
- Numbers are written with up to 3 decimals and trailing zeros dropped (`530.340` is saved as `530.34`). Odd values such as `-90.00001` show up in rotations.

## `offset` is authoritative (verified in game, 2026-10-13, test C)
We imported TerranScrapyard with one module (the casino, entry 32, no children) moved 1000 m up, while its `connection`/`predecessor` still pointed at entry 8.
- On the build map the casino floated, attached to nothing, and the menu still showed **"Construction Plan Status: Everything is OK."**
- Export and Save kept `y="530.34"` **and** the stale `<predecessor index="8" connection="connectionsnap03"/>`. That was the only change from the original.

So:
- **The game places modules at `offset`.** It does not snap them using `connection`/`predecessor`.
- **The game does not re-derive or check connections.** A plan whose links don't match its geometry is accepted as valid.
- **Our tool has to compute exact offsets from snap geometry**, and our validator has to check that each linked pair of snaps actually meets, because the game won't catch it.
- Still unknown: what `predecessor` is used for (probably build order, the sequence tree), and how a floating module is actually built. REA is no longer needed for this question.

## Unknowns
- Does the game load external edits to `constructionplans.xml` at startup (option A after a restart)? Low priority now that B works.
- How does the game build a module whose position doesn't match its link (floating, like test C or the 31 library cases)?
- Collision and overlap rules: the plan format doesn't stop modules from intersecting. The build menu has `SetConstructionMapCollisionDetection`.
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
