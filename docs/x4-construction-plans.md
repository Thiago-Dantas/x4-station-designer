# X4 construction plans

What we know about how X4 stores station plans, and the ways we could inject one.

## Where plans live
| Location | Content | Status |
|----------|---------|--------|
| `<profile>\constructionplans.xml` | All of the player's saved plans in one file | **verified**: 4 plans, 459 entries, last written 2026-10-04 |
| `<profile>\constructionplan\` | Empty apart from `steam_autocloud.vdf` | **hypothesis**: target folder for exported or imported single plans |
| `libraries/constructionplans.xml` in `08.cat` (1,066,417 bytes) | The game's built-in (NPC/faction) plans | **verified** that it exists in the catalog; contents not read yet |

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

## Unknowns (answer in Phase 1)
- When does the game read `constructionplans.xml`? Only at startup, when the plan menu opens, or on profile load?
- Does the game overwrite the file on exit or when a plan is saved? That would wipe external edits made while it runs.
- **Top question:** is `offset` authoritative, or does the game recompute it from `predecessor` + `connection`? This decides whether we must compute exact positions ourselves. **Plan:** answer it with REA on `X4.exe`, by finding the code that loads plan entries. It is the first target when reverse engineering starts.
- How does the game handle invalid entries (unknown macro, mismatched connection, missing DLC)?
- Do `connectionsnapNNN` names match the connections in each module's component XML?
- Is there an in-game import/export feature tied to the `constructionplan\` folder?
- What format does `libraries/constructionplans.xml` use (faction plans), and can an extension add plans to it?

## Injection options
All of these are **hypotheses** until tested.

| Option | How | Pros | Cons / risks |
|--------|-----|------|--------------|
| A. Edit the profile file | Add a `<plan>` to `<profile>\constructionplans.xml` while the game is closed | Simplest. Uses the exact format the game writes. | The game may overwrite it. Needs a backup and the game closed. |
| B. Plan file plus in-game import | Drop a single-plan XML into `<profile>\constructionplan\` and import it in-game | No edits to the master file | Depends on the import feature existing and on its file format |
| C. Ship as an extension | A mod that patches `libraries/constructionplans.xml` (XML diff) | Survives game writes. Can be shared. | Plans may show up as faction plans, not player plans. Needs a game restart. |
| D. Live bridge through a mod | Lua UI or MD script plus IPC (e.g. named pipes from `sn_mod_support_apis`) | Agents could work with the running game | Most work. Depends on mods. Later phase. |

**Prior art for in-game placement:** `x4ep_planner` (GHOSTau's Station Planner, `ws_3777272604`) already places a whole connected station onto a plot from the build menu as one build task. This is **verified** from its description; how it does it is not known yet. Its Lua and MD files should show which build-menu API calls place modules and connect them. That tells us what option D would need, and maybe whether the game recomputes `offset` from connections. See [x4-game-data.md](x4-game-data.md#x4ep_planner-contents-verified-from-ext_01cat-20-files).
