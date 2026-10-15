# Workflow: piers and a race filter in the planner's Other modules box — **done**

User request (2026-10-15): add piers to the Other modules box, and list only the modules of the race chosen in the planner.

- [x] `ui/src/main.ts`: `EXTRA_CLASSES` gets `pier`; `extraModules()` keeps the planner race's modules and those of no race (the plain ship bays Argon, Paranid and Teladi use); the input's placeholder and tooltip say so
- [x] Checked: a pier given as an other module replaces the planner's own pier (MCP `plan_production`, Teladi: `pier_tel_harbor_03` ×2 as extra, no default pier)
- [x] Checked in the builder over DevTools (blueprint limit off, then restored): Teladi "pier" 3 rows, any race 25, Teladi "bay" 8 (no race), Terran "bay" 14, Teladi "defence" 3; Enter adds the pier to the list. With the blueprint limit on, the user's save has no Teladi piers, bays or defence: the Teladi lists are empty, as expected
- [x] Docs: usage.md (Other modules box), D-15
- [x] Commit and push
