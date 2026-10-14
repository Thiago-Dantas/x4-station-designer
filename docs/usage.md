# Using x4sd

`x4sd.exe` is one self-contained program (about 1 MB; nothing to install). It finds the game through Steam and the player profile under `Documents\Egosoft\X4\<id>` by itself. Override with `--game-dir` / `X4_GAME_DIR` and `--profile-dir`.

## Visual builder
Run `x4sd` with no arguments (or double-click `x4sd.exe`). It loads the game data in about half a second and opens the builder.
- **Palette (left):** all buildable modules, **grouped and ordered like the game's build menu** (Production, Build, Storage, Habitation, Welfare, Dock, Defence, Processing, Other, Venture; then by name), with search, a category filter and **race chips** (pick one or several races, e.g. Terran + Boron; *All* shows every race; modules without a race are *Generic*; remembered), coloured by class. Click a module to *arm* it, or drag it into the view.
- **First module:** with an empty design, click in the view (or drop the module there): it goes where you click, on the ground plane (rounded to 10 m). *Place as first module* puts it at the origin. Raise or lower it afterwards with the move gizmo.
- **Attaching:** free snaps show as cyan cones. Click a cone (or drop a dragged module on it) to attach the armed module. *Connect by snap* chooses which snap of the new module connects; *Twist* rotates it around the snap axis.
- **Selection:** click a module. ⟲/⟳ (Shift+R / R) twist it around its snap with everything attached to it, by the **rotation step** (90°, 45°, 30°, 15° or 5°; remembered; the game's own plans use fine twists too). *Remove* (Del) removes it with everything attached.
  - The same step sets the twists offered when attaching (*Placing* panel) and the rotate gizmo's snap.
  - A selected **free-standing module** (like the first one) shows a gizmo. *Rotate gizmo* / *Move gizmo* switches it between moving (10 m steps) and rotating around the module's own axes (tilt, yaw, roll) by the rotation step. Everything attached to it turns along and stays on its snaps.
  - An **attached module** turns with *Rotate gizmo* too. The gizmo sits on the module, as in the game; the module turns so that it stays on its **joint** (where its snap meets the other module's), with everything on it.
    - Turning around the snap axis is a twist. Any other turn **tilts** it off the snap axis; the details line shows "tilted N°".
    - R / Shift+R keep a tilt. *Straighten* takes it off and keeps the twist.
    - The game's build menu only twists modules around snaps. Whether the game builds tilted joints is not checked yet, so *Checks* lists each one ("tilted N° off its snap").
  - *Build first* moves the module (and the chain it hangs on) to the front of the build order; *Build last* moves it (and everything on it) to the end.
  - *A build stage ends here* closes a build stage after it.
  - **Move…** lifts the module with everything attached to it; click any free snap cone (cones of the moving part are hidden) to re-attach it there. The panel picks which of its snaps connects and the twist; Esc cancels. If the new spot comes later in the build order, the moved part is re-sequenced after it.
  - **Detach** makes an attached module free-standing where it is.
  - **Equipment** (modules with turret and shield slots: most production, storage and defence modules) lists each group, e.g. *2 × M turret* and *1 × M shield*.
    - Pick what goes on it; only equipment that fits those slots is listed. The number box sets how many of the slots to fill.
    - *Apply to all of this type* copies the module's equipment to every module of the same type.
    - *Apply to all modules* uses its turrets and shields on every group where they fit.
- **Build order (left, second tab):** every module in build order, top to bottom, with "End of stage N" dividers.
  - **Drag a row onto another** to move it to that step. A module can't come before the module it attaches to, so impossible moves are refused with a message.
  - Click a row to select the module.
  - *Show up to step* previews the station built up to that step; later rows are dimmed.
  - Reordering renumbers the modules; the selection follows the moved module.
- **Planner (left, third tab):** our own take on the x4ep_planner mod. Name the wares to make and how many production modules of each (e.g. Hull Parts × 2), then *Generate plan*.
  - **Ware box:** type part of a name (e.g. "hull par", "cell"); matches list below it. Arrow keys and Enter, or a click, add the ware with the count next to it.
  - *Generate plan* always makes a **new** plan for the design as it is now. Its targets come on top of what the station already has, so generating again after placing adds them again.
  - It lists everything to add:
    - the targets;
    - the production chain for their inputs;
    - habitats for the workforce, and the food and medicine they eat;
    - storage for *Storage hours* of everything the station makes and uses;
    - a dock area and a pier if there are none.
  - It also shows the cost and what is left per hour, split into **raw materials to bring in** (mined ones such as ore and ice, and scrap metal; never made) and **to buy**.
  - **Self-sufficient** (shown when it applies) means every made ware is produced on the station; raw materials are always brought in.
  - **Adds to the open design:** the modules already there count as supply, housing and storage. With no wares it fixes the open station's shortfalls.
  - *Race* uses that race's modules where it has them (generic ones otherwise, then any race). *House the workforce* can be turned off.
  - Each row shows **placed/needed** and counts down as you place modules. Click a row to arm that module.
  - **Place all (N)** attaches every module still to place at once (auto-placement).
    - Modules go onto free snaps at exact positions, clear of each other, with connection structures of the plan's race added where needed. The largest go first; dock areas and piers go last, where they face open space.
    - The station may outgrow the plot (up to 20 km per axis); the builder then offers to enlarge it.
    - Every Place all also moves the dock areas and piers that have nothing attached to the outside, where a 3 km corridor out from the station is clear. Docks carrying other modules stay. No module is ever placed in a dock's corridor. The status line says how many docks moved, and warns if one has no clear approach.
    - What found no room is listed under the button. Ctrl+Z undoes the whole placement.
  - *Buy* on a production-chain row buys that ware instead of making it; the chip above *Generate* undoes it.
    - Either way the plan is redone for the station it was made for. What is already placed keeps counting, and nothing is added twice. After Place all, buying a ware doesn't bring the plan back; its modules already placed stay in the design.
  - Sunlight and build method come from the right pane. The settings are remembered.
- **Right pane sections** fold: click a section's title (▸/▾). Equipment, Build cost, Production, Plot and Checks start folded on every start; the controls help at the bottom always shows. *Checks* shows the number of problems in its title. *Selected* and *Placing* open when they apply; the Equipment list inside *Selected* starts folded.
- **Equipment (right):** the whole station; one undo step each.
  - Shows how many turret and shield groups are equipped.
  - *Loadout level* (None, Low, Medium, High) + *Apply* equips every group by our own rule: fitting equipment without missiles, the module's race first, cheapest (Low) to most expensive (High). The game's own loadout level may pick differently.
  - *M turret*, *M shield*, *L turret*, *L shield* + *Equip station* puts those on every group where they fit. Groups none of them fit stay as they were.
  - *Defence modules only* limits both to defence platforms.
  - The equipment offered is every turret and shield players can build that fits some station module (103 with the installed DLCs and mods). The game offers only those you have blueprints for.
- **Build cost (right):** the materials to build the **selected module** (with its turrets and shields), or the **whole station** when nothing is selected.
  - Shows resource amounts and an estimated value at the game's average prices.
  - *Build method* matches the game's Preferred Build Method (Universal, Terran, Boron, …). Most modules have one recipe; shared modules that also have a Terran or Boron recipe change with it, and the others fall back to Universal. The choice is remembered.
  - Not reflected: mods that *replace* ware prices or recipes. Mods that *add* wares or recipes are included.
- **Production (right):** what the whole station makes and uses per hour. It lists:
  - the net per ware: green is a surplus, red a shortfall; hover a row for made and used;
  - workforce housed (per race) against workforce needed, and the workforce bonus;
  - dock areas and piers;
  - storage per cargo type, with the hours it holds of everything the station makes and uses;
  - notes on what is missing (housing, a storage type, a dock).
  - Workers' food and medicine count as use.
  - Raw materials are marked *(raw)*: ore, ice, silicon, nividium, methane, helium, hydrogen, and scrap metal (salvaged). A shortfall of any other ware is noted as "Not self-sufficient".
  - *Sunlight %* is the sector's sunlight, which solar output (energy cells) follows. It is remembered.
  - The rules and which parts are still hypotheses are in [tasks/workflow-production-planner.md](../tasks/workflow-production-planner.md).
- **Plot (right):** the plot the station must fit, in whole km per side from the station origin (+X, −X, +Y, −Y, +Z, −Z; each axis 1–20 km, as in the game), drawn as a box (green when everything fits, red when not).
  - New designs start at 2 km on every side (4×4×4 km), the game's default; opened plans start on the smallest plot that holds them.
  - *Fit plot to station* sets that minimum.
  - When an edit pushes the station outside the plot, the builder offers to enlarge it, or, if no plot could hold it (over 20 km on an axis), to undo.
  - The plot is not stored in plan files (the game keeps plots per station).
- **Checks (right):** the validator's findings, live, including modules outside the plot and **overlapping modules**. Click one to select the module.
  - An overlap means two modules' collision meshes intersect. Modules joined at a snap may interlock, and are never counted. See docs/x4-construction-plans.md, "Overlaps".
- **Toolbar:**
  - *Open game plan…* lists your saved plans and import files.
  - *Open file…* / *Save as…* work with plan files.
  - *Export to game* writes the plan into `constructionplan\`; then click Import in the game's build menu.
  - Undo and Redo (Ctrl+Z / Ctrl+Y), and Fit view (F).
  - **● unsaved** (next to the name, and `*` in the window title) appears after changes since the last Save, Export or Open. Closing the window, New and Open ask before discarding them. Changes made by an agent count too.
- **Frame counter** (top right of the view): frames per second, and the milliseconds a frame takes to update and draw. The frame rate stops at the screen's refresh rate, so the milliseconds show the real load.
- **View:** left drag **or the arrow keys** rotate (left/right around, up/down tilt). Right drag pans. Keys follow the game: **W/S** move forward/back where you look (kept level), **A/D** left/right, **Z/X** straight down/up, **Q/E** turn the camera left/right (like the arrow keys).
- **Middle click** a placed module to place more of that type (like a palette click). On empty space it takes the selected module's type. Middle drag still zooms. Keys are smooth while held, with speed following the zoom; the wheel zooms. Modules show their **real game models**, tinted by module class; textures are not shown yet.
- **Model detail** (toolbar): *Fast* (lowest LOD, hull only; for very large stations), *Normal* (default), *Detailed* (finest LOD with all small details). The choice is remembered.
- **Textures** (toolbar, on by default): the game's diffuse textures on the models. Without them, models are tinted by module class. The status bar reports how many loaded and their GPU memory, typically about 10 MB in 0.2 s. Turn them off on weak graphics hardware. The choice is remembered.

## Command line
| Command | What it does |
|---------|--------------|
| `x4sd list-modules [--class C] [--race R] [text]` | Buildable station modules (base, DLCs, enabled mods): macro, class, race, source, name. `text` matches the name or macro. |
| `x4sd snaps <macro>` | A module's snap connections with position and facing direction (module frame, metres) |
| `x4sd show <file>` | Plans in a plan file: id, name, module count, unattached modules, DLC patches, **plot needed** |
| `x4sd validate <file> [--plan P] [--plot +X,-X,+Y,-Y,+Z,-Z]` | Problems the game doesn't catch: unknown modules/snaps, modules off their snap (more than 5 cm), misaligned snaps, overlapping modules, equipment on a group the module lacks or that doesn't fit, missing DLC patches. With `--plot` (km per side) it also flags modules outside that plot. Exit code 1 if any. |
| `x4sd export <file> --plan P [--new-id] [--overwrite]` | Writes the plan into `<profile>\constructionplan\` for in-game **Import** (build menu, Import button; the folder is rescanned when the menu opens). `--new-id` makes the import add a new plan instead of replacing one with the same id. |

A plan file is either the profile's `constructionplans.xml` (all saved plans) or a single-plan file like the ones the game exports. Exit code 2 means an error (file not found, game not found, ambiguous plan, and so on).

## MCP server for AI agents
`x4sd mcp` speaks MCP over stdio. Register it with an MCP client, for example Claude Code:
```
claude mcp add x4sd -- "E:\code\x4-station-designer\target\release\x4sd.exe" mcp
```
(add `--game-dir …` / `--profile-dir …` before `mcp` if they aren't found automatically). The server explains the workflow to the agent in its instructions, including the recipe for requests like "a staffed station with 100 solar panels": `list_wares`, then `auto_place` with `from_plan` (plans and places in one call), then `validate_design` (and `fit_plot` if the station outgrew its plot), then `analyze_station`.
- **Self-sufficient** means every made ware is produced on the station. Raw materials are brought in, mined or salvaged, never planned (user's definition, 2026-10-13). Flows carry `raw`, and the analysis has `self_sufficient`.
- **Checked:** that request plans 137 modules for ≈ 92.8 M Cr, `self_sufficient: true`; only ice is left, to be mined.
  - Placed, it adds 81 connection structures: 218 modules for ≈ 95.6 M Cr on an 8×8×8 km plot, with no overlaps.
- **TerranScrapyard**, fixed with no targets (Terran), is self-sufficient: it still needs ice and scrap metal brought in. `auto_place` puts its 219 new modules onto the existing station.

| Tool | Purpose |
|------|---------|
| `list_modules`, `module_snaps` | Find modules (class/race/text filters) and their snap points |
| `new_design`, `open_design`, `describe_design`, `rename_design` | Start or load a design; see every module with index, name, position, link and free snaps |
| `add_module` | Place a module on its own (the first one, or a separate part) |
| `attach_module`, `free_snaps` | Connect a module's snap to a free snap of a placed module, optionally twisted |
| `set_twist`, `move_module`, `remove_module` | Rotate around the snap axis, move unattached modules, remove a module with everything attached to it (renumbers 1..n) |
| `undo`, `redo` | Every edit can be undone |
| `builder_status` | Whether a builder window is open (then all tools act on it, live) |
| `list_wares`, `plan_production` | Wares some module makes; the planner: targets (ware + production modules), race, workforce, storage hours, buy list, sunlight. Returns the modules to add with their roles, the analysis with them and the cost (it doesn't place them) |
| `auto_place` | Auto-placement: attaches `modules` (macro + count), or what `from_plan` (plan_production settings) lists, at free snaps without overlaps, adding connection structures. `grow_plot` (default true) lets the station outgrow its plot up to 20 km per axis. `race` sets the connectors' race (default: the plan's, else the modules' most common). Returns the new modules and connectors (index, macro), what didn't fit, the plot and, with `from_plan`, the plan. One undo step. |
| `list_equipment` | Turrets and shields players can build (kind, size, race, text filters): macro, name, size, race, mark, average price |
| `module_equipment` | A placed module's groups: for each group and kind the slot size and count, what is equipped, and the macros that fit |
| `set_equipment` | Put a fitting macro on one kind of slot in a group (`count`, default all its slots), or remove it |
| `equip_modules` | Equip many modules at once (one undo step): `picks` (each group takes, per kind, the first that fits) or `level` (none, low, medium, high); every module with groups by default, or `modules` / `defence_only`. Reports groups filled, cleared and skipped. |
| `copy_equipment` | Copy a module's equipment: `same_type` (default) to every module of its type, or `all` (its macros as picks for every module) |
| `analyze_station` | Production and use per hour (net per ware), workforce needed vs housed and the bonus, storage per cargo type in hours, dock areas and piers, notes on what is missing; `sunlight` in percent |
| `build_cost` | Resources and estimated value to build one module (with its equipment) or the whole station, for a build method; lists the methods |
| `reattach_module`, `detach_module` | Move a placed module (with everything on it) onto another free snap (`moved_to` gives its new index), or make it free-standing |
| `move_in_build_order`, `build_first`, `build_last`, `set_stage_end` | Reorder the build (renumbers; the result includes `moved_to`) and set stage ends. `describe_design` lists modules in build order with `stage_end`. |
| `set_plot`, `fit_plot` | Set the plot (km per side), or fit it to the station (optionally only growing it). `describe_design` shows the plot, the size needed and whether it fits; `validate_design` includes plot issues. |
| `validate_design` | Same checks as `x4sd validate` (overlaps and equipment included), plus the plot |
| `save_design`, `export_design` | Write a plan file, or put the plan into the game's `constructionplan` folder for Import |

**Live link (D-7):** if the builder window is open, every tool works on the design shown there. You watch the agent's changes appear ("Agent: …" in the status bar) and can drag, twist or undo alongside it. Without a builder, `x4sd mcp` keeps its own design (headless). `builder_status` tells the agent which mode it is in. The link is a local TCP socket on `127.0.0.1:47823`, one JSON line per message; only the first builder window opened listens.

`research/mcp_smoke.py <x4sd.exe> <profile dir>` drives the server like an agent: it builds a small Terran station, plans and auto-places a production chain onto it (then undoes that), validates it and exports it. `research/link_smoke.py` does the same against an open builder and checks that the builder shows the result.
