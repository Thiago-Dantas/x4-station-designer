# Using x4sd

`x4sd.exe` is one self-contained program (about 15 MB, the builder's UI included; nothing to install; it needs the WebView2 runtime, which comes with Windows 11 and current Windows 10). It finds the game through Steam and the player profile under `Documents\Egosoft\X4\<id>` by itself. Override with `--game-dir` / `X4_GAME_DIR` and `--profile-dir`.

## Visual builder
Run `x4sd` with no arguments (or double-click `x4sd.exe`). It loads the game data in about half a second and opens the builder.
- **Palette (left):** all buildable modules, **grouped and ordered like the game's build menu** (Production, Build, Storage, Habitation, Welfare, Dock, Defence, Processing, Other, Venture; then by name), with search, a category filter and **race chips** (pick one or several races, e.g. Terran + Boron; *All* shows every race; a module's race is its maker race, else the race code in its macro name, so `dockarea_tel_…` is Teladi; modules with neither are *Generic*; remembered), coloured by class. Click a module to *arm* it, or drag it into the view.
  - **Only modules I have blueprints for** (off by default) limits the palette to the modules the player can build. The first time, it opens the profile's `save` folder: pick a save (`.xml.gz`), and its blueprint list is read (about half a second; the reader stops a quarter of the way into the ~1 GB of XML). The planner then also uses only those modules: production, habitats, storage, docks, and the connection structures that *Place all* and Optimize add. When that leaves the planner without a module for a ware (a target or an input it needs), the plan shows a red warning ("No blueprint for a module that makes …") and the status line says so too. Modules you name yourself (planner extras, explicit lists) are still used.
  - Unchecking shows every module again. Checking it again uses the list read last, without reading the save again. *Other save…* picks another save; *Re-read* reads the same one again, e.g. after buying blueprints and saving. The setting and the list are remembered across restarts.
- **First module:** with an empty design, click in the view (or drop the module there): it goes where you click, on the ground plane (rounded to 10 m). *Place as first module* puts it at the origin. Once placed, the view backs off 5 km from it (room for what you attach next; Fit view still frames tightly), and it is selected and placing stops, so its move gizmo and position fields show: to place it exactly (say, inside the plot box), type its position (below).
- **Attaching:** free snaps show as cyan cones. Click a cone (or drop a dragged module on it) to attach the armed module. *Connect by snap* chooses which snap of the new module connects; *Twist* rotates it around the snap axis.
  - Each snap is labelled with the way it faces in the module's own frame (top, bottom, front, back, left, right; the strongest direction), e.g. `top · connectionsnap006`. Not every module has every side: the Terran and Boron disc defence platforms have a top snap, the Argon, Teladi, Paranid and Split discs don't.
  - **Preview:** hovering a cone shows the armed module there, see-through, joined by the chosen snap with the chosen twist. **C** / Shift+C cycles the connecting snap, **T** / Shift+T the twist, and the preview follows.
- **Selection:** click a module. ⟲/⟳ (Shift+R / R) twist it around its snap with everything attached to it, by the **rotation step** (90°, 45°, 30°, 15° or 5°; remembered; the game's own plans use fine twists too). *Remove* (Del) removes it alone, as in the game: what was attached to it stays where it is, unattached, and *Checks* lists it (*not attached to the station*) until you re-attach it with *Move…*. *Remove sequence* (Shift+Del) removes it with everything attached to it.
  - The same step sets the twists offered when attaching (*Placing* panel). The rotate gizmo always turns in 5° steps.
  - A selected **free-standing module** (like the first one) shows a gizmo. *Rotate gizmo* / *Move gizmo* switches it between moving (10 m steps) and rotating around the module's own axes (tilt, yaw, roll) in 5° steps. Everything attached to it turns along and stays on its snaps.
  - A selected free-standing module also shows **X / Y / Z** fields: its position in metres from the station origin (game coordinates). Type a value and press Enter or Tab (or step it by 10 m with the arrows) to move it with everything attached; each change is one undo step, and the view zooms out to the whole plot box so you can see where it sits. The plot box is laid out around the origin by the *Plot* fields (+X 2 km and −X 1 km run from −1000 to +2000 m).
  - *Rotate gizmo* while a module is armed for placing drops the placement and shows the gizmo on the selected module.
  - An **attached module** turns with *Rotate gizmo* too. The gizmo sits on the module, as in the game; the module turns so that it stays on its **joint** (where its snap meets the other module's), with everything on it.
    - Turning around the snap axis is a twist. Any other turn **tilts** it off the snap axis; the details line shows "tilted N°".
    - R / Shift+R keep a tilt. *Straighten* takes it off and keeps the twist.
    - The game's build menu only twists modules around snaps. Whether the game builds tilted joints is not checked yet, so *Checks* lists each one ("tilted N° off its snap").
  - The view follows the gizmo while you drag; the design changes when you let go (one undo step). The checks follow the drag: the plot box turns red or green and *Checks* lists what would be wrong if you let go there.
  - *Build first* makes the module the root and step 1: the links between it and the old root are reversed, so that chain hangs on it and is built next (nothing moves); *Build last* moves it (and everything on it) to the end.
  - *A build stage ends here* closes a build stage after it.
  - **Connected by** (attached modules) changes which of its own snaps joins its parent, in place: same parent snap, same twist, everything on it moves along (a tilt is dropped). **C** / Shift+C cycles it. Snaps that hold one of its own modules aren't offered.
  - **Move…** lifts the module with everything attached to it; click any free snap cone (cones of the moving part are hidden) to re-attach it there. Its own snaps show as orange cones, the connecting one bright: click one to pick it (or use the panel, or C / T as when placing). Hovering a cone previews the moved part there. Esc cancels. If the new spot comes later in the build order, the moved part is re-sequenced after it.
  - **Detach** makes an attached module free-standing where it is.
  - **Right click** a placed module (a click, not a right drag, which pans) for its menu, as in the game: *Copy*, *Copy sequence*, *Remove*, *Remove sequence*, *Move…*, *Detach*. It selects the module; Esc or a click elsewhere closes it.
  - **Copy sequence…** (Ctrl+Shift+C, or Shift+middle click on a module) copies the module with everything attached to it, as in the game. It works like *Move…*: pick the copy's connecting snap (orange cones, C / T), hover a free snap cone to preview, click to place a copy. Each click places another copy; Esc stops. Copies keep their equipment, go to the end of the build order, and don't copy stage ends. The source module's own free snaps aren't offered as targets (its cones pick the connecting snap); the modules attached to it are. **Copy** (Ctrl+C, or middle click) arms just the module's type, like a palette click.
  - **Equipment** (modules with turret and shield slots: most production, storage and defence modules) lists each group, e.g. *2 × M turret* and *1 × M shield*.
    - Pick what goes on it; only equipment that fits those slots is listed. The number box sets how many of the slots to fill.
    - *Apply to all of this type* copies the module's equipment to every module of the same type.
    - *Apply to all modules* uses its turrets and shields on every group where they fit.
- **Build order (left, second tab):** every module in build order, top to bottom, with "End of stage N" dividers.
  - **Drag a row onto another** to move it to that step. A module can't come before the module it attaches to, so impossible moves are refused with a message.
  - Click a row to select the module; double-click it to also point the camera at it (same viewing angle, closer).
  - *Show up to step* previews the station built up to that step; later rows are dimmed.
  - Reordering renumbers the modules; the selection follows the moved module.
- **Planner (left, third tab):** our own take on the x4ep_planner mod. Name the wares to make and how many production modules of each (e.g. Hull Parts × 2), then *Generate plan*.
  - Each target shows **≈ N/h**: what its modules make per hour, before planning. It uses the module the plan would use (race, blueprints), fully staffed when *House the workforce* is on, at the right pane's sunlight. *no blueprint* (red) means the blueprint limit leaves no module that makes it.
  - **Ware box:** type part of a name (e.g. "hull par", "cell"); matches list below it. Arrow keys and Enter, or a click, add the ware with the count next to it.
  - **Other modules box** (below the targets): a defence, storage or ship-building module, a pier or a dock area to add as it is, with a count, e.g. defence platforms, extra storage, ship fabrication or maintenance bays, or the pier or dock area you want (the planner then adds none of that kind of its own). Production modules aren't offered: pick their ware in the ware box. With a *Race* chosen, only that race's modules are listed, plus universal ones (no race, *Generic* in the palette: the plain ship bays, the venture docks). Teladi piers serve only L/XL ships; the Teladi dock area (S/M) is listed with them. Same search as the ware box. Their workforce gets habitats, food and medicine like the rest.
    - **Defence platforms** get side buttons: Front (+Z), Back (−Z), Left (−X), Right (+X), Top (+Y), Bottom (−Y). The count is spread evenly over the sides picked, extras to the first in that order: 6 with Front and Back is 3 + 3; 6 with all six sides is 1 each. No side picked: anywhere.
    - *Place all* puts them last, each on the outside of the station on its side with nothing in front of it, starting from the middle of that face. The sides used are the ones picked when you click *Place all*, so you can change them without planning again.
    - A station with no free snaps on a side (a small, tightly packed one) can't take a platform there. It then goes where nothing is in front of it on that side, else wherever it fits, and the status line says how many to move by hand.
  - **Storage at least:** container, solid and liquid (gases are liquid) capacity the station must have, in m³: `1M`, `500k`, `250000`. Empty: no minimum. Each type gets the larger of this and *Storage hours*; set the hours to 0 to use only these. The largest storage module covers the bulk and the smallest one that covers the rest (200k liquid: one M module of 500k, not an L).
  - *Generate plan* always makes a **new** plan for the design as it is now. Its targets come on top of what the station already has, so generating again after placing adds them again.
  - It lists everything to add:
    - the targets;
    - the production chain for their inputs;
    - habitats for the workforce, and the food and medicine they eat;
    - storage that holds *Storage hours* of the station's production and consumption, and at least *Storage at least*;
    - the other modules;
    - a dock area and a pier if there are none.
  - It also shows the cost and what is left per hour, split into **raw materials to bring in** (mined ones such as ore and ice, and scrap metal; never made) and **to buy**.
  - **Self-sufficient** (shown when it applies) means every made ware is produced on the station; raw materials are always brought in.
  - **Adds to the open design:** the modules already there count as supply, housing and storage. With no wares it fixes the open station's shortfalls.
  - *Race* uses that race's modules where it has them (generic ones otherwise, then any race). *House the workforce* can be turned off.
  - Each row shows **placed/needed** and counts down as you place modules. Click a row to arm that module.
  - **Place all (N)** attaches every module still to place at once (auto-placement).
    - When some find no place, it asks first: **keep a partial build?** *No* leaves the station as it was. *Yes* keeps what was placed: the wares and other modules to add shrink by what was placed, and the plan is made again for the station as it now is, so it lists only what is left, with its cost.
    - Before giving up on a spot, placement also tries a new connection hub at the end of a chain of two or three straight connection structures, out past what is in the way. A station whose free snaps all face into neighbouring modules has no way out: *Optimize* with *Rebuild connection structures* lays the whole station out again instead.
    - Modules go onto free snaps at exact positions, clear of each other, with connection structures of the plan's race added where needed. The largest go first; dock areas, piers and bays go last, where they have clear space.
    - The station may outgrow the plot (up to 20 km per axis); the builder then offers to enlarge it.
    - The station's modules stay where they are, docks, piers and bays too (D-18). New dock areas, piers and bays go last, where their exclusion zones are clear (the space ships use to dock or leave; see Checks); dock areas also need a 3 km corridor out from the station. No module is ever placed in a zone or a dock's corridor, the station's own docks included. The status line warns if a dock or bay has no clear space.
    - What found no room is listed under the button. Ctrl+Z undoes the whole placement.
    - Once everything is placed, the wares and other modules to add are cleared (the plan stays on screen). If something found no room, they stay, for another try.
  - *Buy* on a production-chain row buys that ware instead of making it; the chip above *Generate* undoes it.
    - Either way the plan is redone for the station and the wares it was made for (also once Place all has cleared the lists). What is already placed keeps counting, and nothing is added twice. After Place all, buying a ware doesn't bring the plan back; its modules already placed stay in the design.
  - Sunlight and build method come from the right pane. The settings are remembered.
  - **Optimize station** (the x4ep_planner's *Optimise*, plus a re-layout) works on the open station with the settings above (wares to make are ignored).
    - First it only shows a **plan**: the modules that cover the station's shortfalls, by role. Nothing changes yet. Trim it:
      - *Buy* on a production-chain row buys that ware instead and plans again, so modules that only fed it drop out too (the chip above *Generate* makes it again);
      - ✕ on a habitat, storage or dock row leaves those modules out.
    - **Apply plan** adds what is left, as one undo step. With **Rebuild connection structures** off (default), that is all: the new modules go onto free snaps like *Place all*, and the station's modules stay where they are (D-18). What finds no room is listed under the button.
    - With **Rebuild connection structures** on, Apply plan also re-lays the whole station out **in layers**, rebuilding its connection structures from scratch (often more compact):
      - those that can't carry turrets or shields (habitats, welfare…) in the middle;
      - defensible ones (production, storage, defence) wrapped around them;
      - dock areas, piers and fabrication/maintenance bays outermost, with their approaches and exits clear (both ends of an L/XL bay).
      - equipment stays with each module type;
      - if anything finds no room, nothing changes and the status line says what didn't fit.
      - (MCP `optimize_station` also has a middle mode, `keep_shape`: keep the connection structures and re-place what hangs on them. The builder doesn't use it.)
    - Either way, afterwards the plot is **fitted to the station** (the smallest that holds it), without asking. Like every plot change, undo doesn't restore the old plot.
    - Layers make a larger station than *Place all*, which packs biggest first. TerranScrapyard plus its fixes: 9–11 km per axis.
  - **Fewer connection structures** (off by default, remembered) applies to *Place all* and *Optimize*. A new connection structure is added only when it brings a module 4 km nearer the centre, instead of 2 km. That meant about half as many connection structures (less in-game load, see *In-game load*) but a larger station, measured before 2026-10-15, when placement still stacked modules inside each other; with honest layouts it doesn't always save any yet (re-tuning open). Then: "100 solar, staffed": 37 instead of 81 (11×12×13 km instead of 9×8×8). TerranScrapyard rebuilt by Optimize: 1 instead of 56 (288 modules instead of 343; 14×16×12 km instead of 10×10×10).
- **Right pane sections** fold: click a section's title (▸/▾). Equipment, Build cost, Production, In-game load, Plot and Checks start folded on every start; the controls help at the bottom always shows. *Checks* shows the number of problems in its title. *Selected* and *Placing* open when they apply; the Equipment list inside *Selected* starts folded.
- **Equipment (right):** the whole station; one undo step each.
  - Shows how many turret and shield groups are equipped.
  - *Loadout level* (None, Low, Medium, High) + *Apply* equips every group by our own rule: fitting equipment without missiles, the module's race first, cheapest (Low) to most expensive (High). The game's own loadout level may pick differently.
  - *M turret*, *M shield*, *L turret*, *L shield* + *Equip station* puts those on every group where they fit. Groups none of them fit stay as they were.
  - *Defence modules only* limits both to defence platforms.
  - The equipment offered is every turret and shield players can build that fits some station module (103 with the installed DLCs and mods). The game offers only those you have blueprints for.
- **In-game load (right):** what likely costs the game frame rate. It counts modules (and the connection structures among them, which make nothing), turrets and shields, dock areas and piers, and the station's size in km. These are players' rules of thumb, not measured in game. To lower the load: *Fewer connection structures* in the Planner, only the turrets you need, and no more docks than the trade needs.
- **Build cost (right):** the materials to build the **selected module** (with its turrets and shields), or the **whole station** when nothing is selected.
  - Shows resource amounts and an estimated value at the game's average prices.
  - *Build method* matches the game's Preferred Build Method (Universal, Terran, Boron, …). Most modules have one recipe; shared modules that also have a Terran or Boron recipe change with it, and the others fall back to Universal. The choice is remembered.
  - Not reflected: mods that *replace* ware prices or recipes. Mods that *add* wares or recipes are included.
- **Production (right):** what the whole station makes and uses per hour. It lists:
  - the net per ware: green is a surplus, red a shortfall; hover a row for made and used;
  - workforce housed (per race) against workforce needed, and the workforce bonus;
  - dock areas and piers;
  - storage per cargo type, with how many hours of the station's production and consumption it holds;
  - notes on what is missing (housing, a storage type, a dock).
  - Workers' food and medicine count as use.
  - Raw materials are marked *(raw)*: ore, ice, silicon, nividium, methane, helium, hydrogen, and scrap metal (salvaged). A shortfall of any other ware is noted as "Not self-sufficient".
  - *Sunlight %* is the sector's sunlight, which solar output (energy cells) follows. *Sector* above it lists the game's sectors (base and enabled DLCs) with their sunlight: picking one sets the %; typing a % sets the sector back to *Any*. Both are remembered.
  - The rules and which parts are still hypotheses are in [tasks/workflow-production-planner.md](../tasks/workflow-production-planner.md).
- **Plot (right):** the plot the station must fit, in whole km per side from the station origin (+X, −X, +Y, −Y, +Z, −Z; each axis 1–20 km, as in the game), drawn as a box (green when everything fits, red when not).
  - New designs start at 2 km on every side (4×4×4 km), the game's default; opened plans start on the smallest plot that holds them.
  - *Fit plot to station* sets that minimum.
  - When an edit pushes the station outside the plot, the builder offers to enlarge it, or, if no plot could hold it (over 20 km on an axis), to undo.
  - The plot is not stored in plan files (the game keeps plots per station).
- **Checks (right):** the validator's findings, live, including modules outside the plot and **overlapping modules**. With *Only modules I have blueprints for* on, it also lists every placed module you have no blueprint for ("no blueprint for …"); turning the limit on or off updates the list at once. MCP `validate_design` reports the same. Click one to select the module; double-click it to also zoom the camera to 1 km from the module (from its bounding sphere, same viewing angle).
  - An overlap means two modules' collision meshes intersect. Modules joined at a snap may interlock, and are never counted. See docs/x4-construction-plans.md, "Overlaps".
  - **Blocked approaches and exits:** "entry N: blocks the approach of pier M" (or the exit of bay M, the approach of dock area M). Piers, bays and most dock areas keep exclusion zones free for ships: boxes in front of each dock or bay end, XL 4×4 km, L 1.2×1.2 km, both 100 km long, M 180×180×500 m. A module reaching more than 1 m into one is reported, unless it is joined to that module at a snap. The game imports such plans without a word, but ships may struggle. See docs/x4-construction-plans.md, "Clearance: exclusion zones".
- **Toolbar:**
  - *Open game plan…* lists your plans in two groups: **In the game** (saved plans, `constructionplans.xml`) and **Import files** (the `constructionplan` folder; in the game only once imported there). Each import file has a *Delete* button (asks first; deletes only that file, never the saved plans).
  - *Open file…* / *Save as…* work with plan files. Both start in the game's plan folder, `<profile>\constructionplan\`. *Save as…* proposes the file name the game would give the plan.
  - *Export to game* writes the plan into `constructionplan\`; then click Import in the game's build menu.
    - Import replaces your plan with the same id, after the game asks.
    - **A renamed plan becomes a new plan.** If your saved plans or import files have the plan's id under another name, *Export* and *Save as* give it a new id first. In game it then imports next to the original instead of replacing it. The status line says so.
  - Undo and Redo (Ctrl+Z / Ctrl+Y), and Fit view (F).
  - **● unsaved** (next to the name, and `*` in the window title) appears after changes since the last Save, Export or Open. Closing the window, New and Open ask before discarding them. Changes made by an agent count too.
- **New versions** (D-19): once per start the builder asks GitHub for the latest release. If it is newer than the running one, a bar above the status line offers it: *Download* opens the release page in the browser; *Skip this version* hides it until a later release comes out (skip 1.1 and 1.2 shows again). With no network nothing shows. The CLI and the MCP server don't check.
- **Frame counter** (top right of the view): frames per second, and the milliseconds a frame takes to update and draw. The frame rate stops at the screen's refresh rate, so the milliseconds show the real load. The view draws only when something changes (camera, keys, gizmo, hover, an edit), so it reads **idle** and uses almost no GPU when you aren't moving anything.
- **View:** left drag **or the arrow keys** rotate (left/right around, up/down tilt). Right drag pans. Keys follow the game: **W/S** move forward/back where you look (kept level), **A/D** left/right, **Z/X** straight down/up, **Q/E** turn the camera left/right (like the arrow keys). A turn (keys or left drag) goes around the module surface in the middle of the view, so after flying close with WASD it turns on what you look at; with nothing in the middle it keeps the old pivot. Hold **Shift** to go 3× faster: keys, drags and the wheel (Shift+left drag still turns; plain three.js would pan).
- **Middle click** (or Ctrl+C) a placed module to copy it: it arms that type, like a palette click. With Shift it copies the sequence (*Copy sequence…*). On empty space it takes the selected module. Middle drag still zooms. Keys are smooth while held, with speed following the zoom; the wheel zooms. Modules show their **real game models**, with textures (below) or tinted by module class.
- **Frame rate limit** (toolbar, next to model detail): 30, 60 (default), 120 or 144 fps, or *Screen fps* (the screen's refresh rate). It caps how often the view draws while something moves; lower uses less GPU. Remembered.
- **Model detail** (toolbar): *Fast* (lowest LOD, hull only; for very large stations), *Normal* (default), *Detailed* (finest LOD with all small details). The choice is remembered.
- **Textures** (toolbar, on by default): the game's diffuse textures on the models. Without them, models are tinted by module class. The status bar reports how many loaded and their GPU memory, typically about 10 MB in 0.2 s. Turn them off on weak graphics hardware. The choice is remembered.

## Command line
| Command | What it does |
|---------|--------------|
| `x4sd list-modules [--class C] [--race R] [text]` | Buildable station modules (base, DLCs, enabled mods): macro, class, race, source, name. `text` matches the name or macro. |
| `x4sd snaps <macro>` | A module's snap connections with position and facing direction (module frame, metres) |
| `x4sd show <file>` | Plans in a plan file: id, name, module count, unattached modules, DLC patches, **plot needed**, and the **in-game load** (modules, connection structures, turrets, shields, docks and piers, size) |
| `x4sd validate <file> [--plan P] [--plot +X,-X,+Y,-Y,+Z,-Z]` | Problems the game doesn't catch: unknown modules/snaps, modules off their snap (more than 5 cm), misaligned snaps, overlapping modules, modules blocking a pier's approach or a bay's exit, equipment on a group the module lacks or that doesn't fit, missing DLC patches. With `--plot` (km per side) it also flags modules outside that plot. Exit code 1 if any. |
| `x4sd export <file> --plan P [--new-id] [--overwrite]` | Writes the plan into `<profile>\constructionplan\` for in-game **Import** (build menu, Import button; the folder is rescanned when the menu opens). `--new-id` makes the import add a new plan instead of replacing one with the same id. |

A plan file is either the profile's `constructionplans.xml` (all saved plans) or a single-plan file like the ones the game exports. Exit code 2 means an error (file not found, game not found, ambiguous plan, and so on).

## MCP server for AI agents
`x4sd mcp` speaks MCP over stdio. Register it with an MCP client, for example Claude Code:
```
claude mcp add x4sd -- "C:\path\to\x4sd.exe" mcp
```
(add `--game-dir …` / `--profile-dir …` before `mcp` if they aren't found automatically). The server explains the workflow to the agent in its instructions, including the recipe for requests like "a staffed station with 100 solar panels": `list_wares`, then `auto_place` with `from_plan` (plans and places in one call), then `validate_design` (and `fit_plot` if the station outgrew its plot), then `analyze_station`.
- **Self-sufficient** means every made ware is produced on the station. Raw materials are brought in, mined or salvaged, never planned (user's definition, 2026-10-13). Flows carry `raw`, and the analysis has `self_sufficient`.
- **Checked:** that request plans 137 modules for ≈ 92.8 M Cr, `self_sufficient: true`; only ice is left, to be mined.
  - Placed, it adds 81 connection structures: 218 modules for ≈ 95.6 M Cr on an 8×8×8 km plot, with no overlaps.
- **TerranScrapyard**, fixed with no targets (Terran), is self-sufficient: it still needs ice and scrap metal brought in. `auto_place` puts its 219 new modules onto the existing station.

| Tool | Purpose |
|------|---------|
| `list_modules`, `module_snaps` | Find modules (class/race/text filters) and their snap points |
| `read_blueprints`, `blueprint_filter` | Read the player's blueprints from a save (`save`, default the newest) and limit `list_modules` and the modules `plan_production`, `auto_place` and `optimize_station` choose to them. `plan_production` lists wares that only modules without a blueprint make in `no_blueprint`. `blueprint_filter` with `on` false/true turns the limit off or back on (the save read last, not read again); without `on` it returns the status. It is the same limit as the builder's palette setting. |
| `new_design`, `open_design`, `describe_design`, `rename_design` | Start or load a design; see every module with index, name, position, link and free snaps |
| `add_module` | Place a module on its own (the first one, or a separate part) |
| `attach_module`, `free_snaps` | Connect a module's snap to a free snap of a placed module, optionally twisted |
| `set_twist`, `move_module`, `remove_module` | Rotate around the snap axis, move unattached modules, remove a module with everything attached to it (renumbers 1..n; the builder's Del removes only the module, D-20) |
| `undo`, `redo` | Every edit can be undone |
| `builder_status` | Whether a builder window is open (then all tools act on it, live) |
| `list_wares`, `plan_production` | Wares some module makes; the planner: targets (ware + production modules), `extra_modules` (macro + count, optional `sides`; role `extra`), race, workforce, storage hours, `storage` (minimum m³ per type, e.g. `{"container": 1000000, "liquid": 200000}`), buy list, sunlight. Returns the modules to add with their roles, the analysis with them and the cost (it doesn't place them) |
| `auto_place` | Auto-placement: attaches `modules` (macro + count), or what `from_plan` (plan_production settings) lists, at free snaps without overlaps, adding connection structures. `grow_plot` (default true) lets the station outgrow its plot up to 20 km per axis. `race` sets the connectors' race (default: the plan's, else the modules' most common). `fewer_connectors` (default false): about half the connection structures, a larger station. A module entry (or a `from_plan` `extra_modules` entry) may have `sides` (front, back, left, right, top, bottom): those modules go last, spread over the sides, each on the outside of the station on its side with nothing in front of it. Returns the new modules and connectors (index, macro), what didn't fit, `sides_blocked` (modules with sides that couldn't go on their side), the plot and, with `from_plan`, the plan. One undo step. |
| `list_equipment` | Turrets and shields players can build (kind, size, race, text filters): macro, name, size, race, mark, average price |
| `module_equipment` | A placed module's groups: for each group and kind the slot size and count, what is equipped, and the macros that fit |
| `set_equipment` | Put a fitting macro on one kind of slot in a group (`count`, default all its slots), or remove it |
| `equip_modules` | Equip many modules at once (one undo step): `picks` (each group takes, per kind, the first that fits) or `level` (none, low, medium, high); every module with groups by default, or `modules` / `defence_only`. Reports groups filled, cleared and skipped. |
| `optimize_station` | Optimize the open station: add the modules that fix its shortfalls (plan_production settings), then re-place its modules in layers (no turret/shield slots inside, defensible outside, docks, piers and bays outermost with their exclusion zones clear). `keep_shape` (default true) keeps the connection structures; `fewer_connectors` as in `auto_place`; `modules` adds exactly these instead (e.g. a `plan_production` result with no targets, trimmed by the user). The plot is then fitted to the station. One undo step; nothing changes if anything finds no place. Returns what was added and moved, the plot and the analysis. |
| `copy_equipment` | Copy a module's equipment: `same_type` (default) to every module of its type, or `all` (its macros as picks for every module) |
| `analyze_station` | Production and use per hour (net per ware), workforce needed vs housed and the bonus, storage per cargo type in hours, dock areas and piers, notes on what is missing; `sunlight` in percent |
| `station_load` | In-game load (players' rules of thumb, not measured): modules, connection structures among them, turrets, shields, dock areas plus piers, `size_km` per axis |
| `build_cost` | Resources and estimated value to build one module (with its equipment) or the whole station, for a build method; lists the methods |
| `reattach_module`, `detach_module` | Move a placed module (with everything on it) onto another free snap (`moved_to` gives its new index), or make it free-standing |
| `move_in_build_order`, `build_first`, `build_last`, `set_stage_end` | Reorder the build (renumbers; the result includes `moved_to`) and set stage ends. `describe_design` lists modules in build order with `stage_end`. |
| `set_plot`, `fit_plot` | Set the plot (km per side), or fit it to the station (optionally only growing it). `describe_design` shows the plot, the size needed and whether it fits; `validate_design` includes plot issues. |
| `validate_design` | Same checks as `x4sd validate` (overlaps and equipment included), plus the plot |
| `save_design`, `export_design` | Write a plan file, or put the plan into the game's `constructionplan` folder for Import. A renamed plan gets a new id first (`new_plan: true`), so it imports as a new plan. |

**Live link (D-7):** if the builder window is open, every tool works on the design shown there. You watch the agent's changes appear ("Agent: …" in the status bar) and can drag, twist or undo alongside it. Without a builder, `x4sd mcp` keeps its own design (headless). `builder_status` tells the agent which mode it is in. The link is a local TCP socket on `127.0.0.1:47823`, one JSON line per message; only the first builder window opened listens.

`research/mcp_smoke.py <x4sd.exe> <profile dir>` drives the server like an agent: it builds a small Terran station, plans and auto-places a production chain onto it (then undoes that), validates it and exports it. `research/link_smoke.py` does the same against an open builder and checks that the builder shows the result. `research/blueprint_smoke.py <x4sd.exe> <save> <scratch dir>` checks the blueprint limit: the MCP tools headless, then the builder's checkbox, *Re-read* and a restart.
