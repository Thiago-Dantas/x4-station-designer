# Workflow: equipment (shields and turrets per module group)

Status: **done** (2026-10-14); an in-game check of an equipped plan is still open (likely works: the user's in-game plans with equipment load into the builder correctly)

User: "next lets go with Equipment (shields/turrets per module group) in the model, the builder and MCP" (roadmap Phase 4).

## What we have
- Plan model (`plan.rs`): `Entry.upgrades: Vec<Upgrade { kind, macro_name, path, group, exact }>`, read and written losslessly from `<upgrades><groups><shields|turrets macro group [exact] [path]/>`.
- Build cost (`wares.rs`, `plan_cost`) already counts equipment.
- `validate` flags unknown equipment macros.
- New, 2026-10-13:
  - `Module.equipment_slots` (kind, group, tags, placement), with `equipment_groups()` / `group_slots()`;
  - `Catalog::equipment()`: buildable turrets and shields;
  - `Equipment::fits(slot)`;
  - game-data test `equipment_groups_and_fit`.

## Research (verified 2026-10-13 on the game data and the samples)
- [x] **Slots:** a module component's `<connection group="groupNN" tags="...">`, with `turret` or `shield` among the tags, plus a size (`medium`, `large`) and types (`standard`, `missile`, `advanced`, `hittable`, `combat`). Group names can have trailing spaces, and Boron modules use names such as `group_large_top_01`.
  - Who has groups: 63 of 65 production modules, 55 of 57 storage, 13 of 20 defence, 2 of 2 processing, 1 dock area. No habitats, connectors, piers or build modules.
  - Group shapes, over all modules:

    | Group holds | Groups |
    |---|---|
    | 1 M shield + 2 M turrets | 665 |
    | 2 M shields + 4 M turrets | 19 |
    | 1 L shield + 2 L turrets | 20 |
    | 1 L shield + 1 L turret | 10 |
    | 1 L shield + 4 L turrets | 10 |
    | other shapes, up to 4 L shields + 10 L turrets | 9 |
- [x] **Equipment:** the wares tagged `equipment` and `turret` or `shield`, with a recipe and without `noplayerblueprint`: 216 with the DLCs and mods (123 turrets, 93 shields).
  - Classify by tags, not by the ware `group`: the Boron DLC files its turrets under `weapons`.
  - Fit tags = the tags of the equipment component's connection tagged `component`, without `component`. Examples: a laser turret has `turret medium standard hittable combat`; a shield has `medium shield hittable standard`.
  - Xenon equipment (no `noplayerblueprint`) carries a `xenon` tag, which no player module's slots have.
- [x] **Fit rule:** equipment fits a slot when the slot has every one of its fit tags (so `standard missile` slots take laser and missile turrets).
  - The game decides this in the engine (`IsUpgradeGroupMacroCompatible`).
  - Our rule matches **every** equipped group in the player plans (576) and the library plans (444): 0 misfits.
- [x] **`exact`** = how many slots of that kind in the group are filled (the build menu's `UILoadoutGroupData.count`). Every sample fills all slots.
  - Turrets always carry `exact` (2, 4, 6); shields carry it with 2 slots and omit it with 1.
  - **Hypothesis:** omit `exact` when the count is 1, and allow partial counts (the menu keeps a count per group).
- [x] **`path=".."`** appears only in the library's hand-made plans (all 444 of their equipped groups); the game writes none. We keep what we read and write no `path`.
- [x] **Build menu:** `GetUpgradeGroups` lists (path, group). `GetUpgradeGroupInfo` gives the slot size, count and total, and `GetUpgradeGroupCompatibilities` the type tags. The offer comes from the player's equipment blueprints, which we can't see offline, so we offer every buildable one. The menu also has a station-wide "loadout level" (`GenerateModuleLoadout(level)`).

## Steps
- [x] Research (above), recorded in `docs/x4-construction-plans.md` ("Equipment")
- [x] Core: slots and equipment list, fit rule, test
- [x] Propose the design to the user (editing operations, builder UI, MCP tools) before going further
  - 2026-10-14, user: bulk equipping in **all three** ways (pick per size, loadout level presets, copy from a module), on top of editing one module. By default bulk equipping fills **every module with groups**, with a way to limit it to defence modules.
- [x] Core editing (`Design`), validation of equipment
- [x] Ops/MCP tools (36 tools)
- [x] Builder UI: selection panel *Equipment*, right pane *Equipment*. Generic Tauri command `run_op` runs a shared `ops::Op`; the scene carries per module `equipment: [equipped, total]`.
- [x] Tests, docs
  - game-data tests: `equipment_groups_and_fit` (data and fit rule) and `equip_station` (by hand, copy, picks, levels, undo, validate, plan round trip, cost);
  - mcp_smoke (a Terran disc: groups, set, level, picks, copy, validate, undo);
  - ui_smoke (select a storage module, equip a group, apply to its type, level high fills 172 of 172 groups, picks, 4× undo).
- [ ] In game (user): import an equipped plan and check that it builds with that equipment

## Design (agreed 2026-10-14)
- **Core** (`equipment.rs` for choosing, `Design` methods for editing; each call is one undo step):
  - `Design::set_equipment(index, group, kind, Some((macro, count)) | None)`. The macro must fit every slot of that kind in the group, and the count is 1..=slots (default all).
    - `exact`: turrets always write it; shields omit it only when 1 of 1 slot is filled, as the game writes them. Other counts are written explicitly.
  - `Design::equip_modules(indices, Fill)`. Each group and kind gets a choice:
    - `Fill::Picks(macros)` (pick per size): the first listed macro that fits all its slots;
    - `Fill::Level(None | Low | Medium | High)` (loadout level): our approximation of the game's `GenerateModuleLoadout`. None removes equipment. Otherwise it takes fitting equipment without missiles (missile turrets need ammo supplied), the module's race first; then the cheapest (Low), median (Medium) or most expensive (High) by average price. **Hypothesis**: the game's own picks may differ.
    - It fills all slots, and returns how many groups were filled or skipped (nothing fits).
  - `Design::copy_equipment(from, to)` (same module type: an exact copy of the upgrades). Copy to all modules = `equip_modules` with the source's macros as picks.
  - `validate`: equipment on a group the module lacks, equipment that doesn't fit, more than the slots.
- **MCP:**
  - `list_equipment` (kind, size, race, search);
  - `module_equipment(index)`: each group with kind, size, slots, current equipment and what fits;
  - `set_equipment`;
  - `equip_modules` (picks or level; modules, or all with groups; `defence_only`);
  - `copy_equipment` (`from`, scope `same_type` or `all`).
- **Builder:**
  - Selection panel *Equipment*: per group, a turret choice plus count and a shield choice plus count. Only fitting equipment is listed.
  - Buttons: *Apply to all of this type* and *Apply to all modules*.
  - Right pane *Equipment* for the station:
    - loadout level with Apply;
    - pick per size (M turret, M shield, L turret, L shield) with *Equip station*;
    - *Defence modules only*;
    - how many groups are equipped.

## Notes
- 2026-10-14: results.
  - On a small Argon station (defence disc, 2 food modules, container storage) the bare cost is 2.13 M Cr: picks fill all 56 groups (3.83 M), level low 2.91 M, level high 4.43 M (flak and plasma turrets, Mk2 shields).
  - Group names vary: `group01`, but also `top_left_01` (Argon storage) and `group_large_top_01` (Boron). The builder shows them as "Group 1", "Top left 01".
- 2026-10-14, **flake solved**: the station-wide pick lists offered every buildable turret (216 items), sorted with ties left in hash-map order. Two items are both named "ARG M Beam Turret Mk1": `turret_arg_m_beam_01_mk1` (ships, fits no station slot) and `_02_mk1` (stations). ui_smoke picks the first one, and when that was `_01`, nothing was equipped.
  - The list now holds only equipment that fits some player module's slot (103), sorted to the macro, and same-named items show their macro.
- (Earlier) ui_smoke failed once in 4 runs at the picks step: the status stayed "Equipped 172 groups" for 10 s after *Equip station*. A scratch stress test (30 alternating level/picks clicks on a 5-module station) showed no slow or missed update, so the cause is unknown. The wait is now 30 s; if it comes back, capture the status and any console error then.
