# Workflow: piers and a race filter in the planner's Other modules box — **done**

User request (2026-10-15): add piers to the Other modules box, and list only the modules of the race chosen in the planner.

- [x] `ui/src/main.ts`: `EXTRA_CLASSES` gets `pier`; `extraModules()` keeps the planner race's modules and those of no race (the plain ship bays Argon, Paranid and Teladi use); the input's placeholder and tooltip say so
- [x] Checked: a pier given as an other module replaces the planner's own pier (MCP `plan_production`, Teladi: `pier_tel_harbor_03` ×2 as extra, no default pier)
- [x] Checked in the builder over DevTools (blueprint limit off, then restored): Teladi "pier" 3 rows, any race 25, Teladi "bay" 8 (no race), Terran "bay" 14, Teladi "defence" 3; Enter adds the pier to the list. With the blueprint limit on, the user's save has no Teladi piers, bays or defence: the Teladi lists are empty, as expected
- [x] Docs: usage.md (Other modules box), D-15
- [x] Commit and push
- [x] Follow-up (user, 2026-10-15): "universal" modules like *Generic* in the palette (Teladi piers serve only L/XL ships). Universal = no race, already listed with the planner race; dock areas added to the classes, since nearly all are universal. Builder check: Teladi "dock" 18 rows (universal dock areas and S/M bays, Teladi piers), Terran 24; a chosen dock area replaces the planner's own (MCP). Some universal dock areas share a display name (Argon/Teladi "1M6S Standard Dock Area"), as in the palette
- [x] Commit and push the follow-up
- [x] Follow-up 2 (user, 2026-10-15, screenshots): the Teladi dock area showed only under *Generic*: it has no `makerrace`. `ModuleInfo::race` now falls back to the race code in the macro name (`catalog::name_race`, moved from the planner, which already used it), for the palette, the Other modules box, CLI and MCP `list_modules`. 12 modules gain a race (10 dock areas, Argon radar, Boron art gallery). Builder check: Teladi chip + "dock" lists the Teladi dock area and piers; Generic + "dock" only the gen/xen ones; the planner's Teladi list has the Teladi dock area plus universal ones. Tests: lib, game_data plan + equip pass
- [x] Commit and push follow-up 2
