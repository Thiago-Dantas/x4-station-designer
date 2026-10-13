# Using x4sd

`x4sd.exe` is one self-contained program (about 1 MB; nothing to install). It finds the game through Steam and the player profile under `Documents\Egosoft\X4\<id>` by itself. Override with `--game-dir` / `X4_GAME_DIR` and `--profile-dir`.

## Visual builder
Run `x4sd` with no arguments (or double-click `x4sd.exe`). It loads the game data in about half a second and opens the builder.
- **Palette (left):** all buildable modules, with search, a class filter and **race chips** (pick one or several races, e.g. Terran + Boron; *All* shows every race; modules without a race are *Generic*; remembered), coloured by class. Click a module to *arm* it, or drag it into the view.
- **First module:** with an empty design, click *Place as first module* (or drop it anywhere in the view).
- **Attaching:** free snaps show as cyan cones. Click a cone (or drop a dragged module on it) to attach the armed module. *Connect by snap* chooses which snap of the new module connects; *Twist* rotates it around the snap axis.
- **Selection:** click a module. ⟲/⟳ (Shift+R / R) twist it by 90° with everything attached to it; *Remove* (Del) removes it with everything attached.
- **Plot (right):** the plot the station must fit, in whole km per side from the station origin (+X, −X, +Y, −Y, +Z, −Z; each axis 1–20 km, as in the game), drawn as a box (green when everything fits, red when not).
  - New designs start at 2×2×2 km; opened plans start on the smallest plot that holds them.
  - *Fit plot to station* sets that minimum.
  - When an edit pushes the station outside the plot, the builder offers to enlarge it, or, if no plot could hold it (over 20 km on an axis), to undo.
  - The plot is not stored in plan files (the game keeps plots per station).
- **Checks (right):** the validator's findings, live, including modules outside the plot. Click one to select the module.
- **Toolbar:**
  - *Open game plan…* lists your saved plans and import files.
  - *Open file…* / *Save as…* work with plan files.
  - *Export to game* writes the plan into `constructionplan\`; then click Import in the game's build menu.
  - Undo and Redo (Ctrl+Z / Ctrl+Y), and Fit view (F).
- **View:** left drag orbits; right drag **or WASD** pans (W/S up and down, A/D left and right on screen, smooth while held, speed follows the zoom); the wheel zooms. Modules show their **real game models**, tinted by module class; textures are not shown yet.
- **Model detail** (toolbar): *Fast* (lowest LOD, hull only; for very large stations), *Normal* (default), *Detailed* (finest LOD with all small details). The choice is remembered.
- **Textures** (toolbar, on by default): the game's diffuse textures on the models. Without them, models are tinted by module class. The status bar reports how many loaded and their GPU memory, typically about 10 MB in 0.2 s. Turn them off on weak graphics hardware. The choice is remembered.

## Command line
| Command | What it does |
|---------|--------------|
| `x4sd list-modules [--class C] [--race R] [text]` | Buildable station modules (base, DLCs, enabled mods): macro, class, race, source, name. `text` matches the name or macro. |
| `x4sd snaps <macro>` | A module's snap connections with position and facing direction (module frame, metres) |
| `x4sd show <file>` | Plans in a plan file: id, name, module count, unattached modules, DLC patches, **plot needed** |
| `x4sd validate <file> [--plan P] [--plot +X,-X,+Y,-Y,+Z,-Z]` | Problems the game doesn't catch: unknown modules/snaps, modules off their snap (more than 5 cm), misaligned snaps, missing DLC patches. With `--plot` (km per side) it also flags modules outside that plot. Exit code 1 if any. |
| `x4sd export <file> --plan P [--new-id] [--overwrite]` | Writes the plan into `<profile>\constructionplan\` for in-game **Import** (build menu, Import button; the folder is rescanned when the menu opens). `--new-id` makes the import add a new plan instead of replacing one with the same id. |

A plan file is either the profile's `constructionplans.xml` (all saved plans) or a single-plan file like the ones the game exports. Exit code 2 means an error (file not found, game not found, ambiguous plan, and so on).

## MCP server for AI agents
`x4sd mcp` speaks MCP over stdio. Register it with an MCP client, for example Claude Code:
```
claude mcp add x4sd -- "E:\code\x4-station-designer\target\release\x4sd.exe" mcp
```
(add `--game-dir …` / `--profile-dir …` before `mcp` if they aren't found automatically). The server explains the workflow to the agent in its instructions.

| Tool | Purpose |
|------|---------|
| `list_modules`, `module_snaps` | Find modules (class/race/text filters) and their snap points |
| `new_design`, `open_design`, `describe_design`, `rename_design` | Start or load a design; see every module with index, name, position, link and free snaps |
| `add_module` | Place a module on its own (the first one, or a separate part) |
| `attach_module`, `free_snaps` | Connect a module's snap to a free snap of a placed module, optionally twisted |
| `set_twist`, `move_module`, `remove_module` | Rotate around the snap axis, move unattached modules, remove a module with everything attached to it (renumbers 1..n) |
| `undo`, `redo` | Every edit can be undone |
| `builder_status` | Whether a builder window is open (then all tools act on it, live) |
| `set_plot`, `fit_plot` | Set the plot (km per side), or fit it to the station (optionally only growing it). `describe_design` shows the plot, the size needed and whether it fits; `validate_design` includes plot issues. |
| `validate_design` | Same checks as `x4sd validate` |
| `save_design`, `export_design` | Write a plan file, or put the plan into the game's `constructionplan` folder for Import |

**Live link (D-7):** if the builder window is open, every tool works on the design shown there. You watch the agent's changes appear ("Agent: …" in the status bar) and can drag, twist or undo alongside it. Without a builder, `x4sd mcp` keeps its own design (headless). `builder_status` tells the agent which mode it is in. The link is a local TCP socket on `127.0.0.1:47823`, one JSON line per message; only the first builder window opened listens.

`research/mcp_smoke.py <x4sd.exe> <profile dir>` drives the server like an agent: it builds, validates and exports a small Terran station. `research/link_smoke.py` does the same against an open builder and checks that the builder shows the result.
