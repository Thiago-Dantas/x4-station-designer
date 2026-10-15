# Roadmap

Master list of milestones. Each active workstream gets its own `tasks/workflow-<topic>.md` with step-by-step state, linked from here.

## Phase 0: Project setup ✅
- [x] CLAUDE.md, docs/, tasks/ skeleton ([workflow-project-setup.md](workflow-project-setup.md))
- [x] Decide on git and hosting: private GitHub repo (D-3)
- [x] `git init`, set repo-local `user.email` = dant12@gmail.com, initial commit
- [x] Pushed to `github.com/Thiago-Dantas/x4-station-designer` (private)

## Phase 1: Research: how X4 handles station plans
Session log: [workflow-phase1-research.md](workflow-phase1-research.md)

- [x] `.cat`/`.dat` extractor `tools/x4cat.py`. The `.dat` layout is verified with MD5s.
- [x] Extract `libraries/constructionplans.xml` and compare its format with the player file. Same format, plus `bookmark`, `fixed`, text-ref names, multiple roots.
- [x] Map `connectionsnapNNN` to module geometry for one module (macro → component → `ConnectionSnapNNN` offsets)
- [x] Find the in-game import/export feature (verified in the menu Lua; folder `constructionplan\` is a strong hypothesis)
- [x] Investigate `x4ep_planner`: it uses the engine's layout generator (`create_construction_sequence`) and then `add_build_to_expand_station`. No exact placement. Its "geometry" tables are only snap counts. See `docs/x4-plan-apis.md`.
- [x] Catalogue the MD actions that take a plan ID (`create_station`, `apply_construction_sequence`, `add_build_to_expand_station`)
- [x] User exported a plan in-game. The folder `constructionplan\` and the file format (a `<plans>` wrapper with one plan) are verified.
- [x] Injection test B: a hand-written file in `constructionplan\` was listed and imported while the game ran, with no restart
- [x] Injection test A, live: the game doesn't re-read `constructionplans.xml` while running. Save and Export rewrite it from memory.
- [x] **Top question answered by test C:** `offset` is authoritative. The game doesn't snap or re-derive links, and calls a plan with a floating module "OK".
- [x] **Snap math solved** (`research/snapcheck.py`): positions and orientations follow from the snap geometry for 99%+ of linked entries in the player and library plans. The Euler convention and the 180°-Y flip plus twist model are verified.
- [x] Decide OD-3 (injection method): option B, recorded as D-5
- [X] Clean up the test plans in-game: the user deletes "X4SD test B" and "X4SD test C" from the plan list
- [ ] Injection test A after a restart (low priority, since B works)

## Phase 2: Core library ✅
Session log: [workflow-phase2-core.md](workflow-phase2-core.md)

- [x] Decide OD-2: Rust + Tauri (D-6)
- [x] Set up the dev toolchain: Rust on H:, Windows SDK on D: (`docs/dev-setup.md`), Cargo workspace. The Tauri CLI moves to Phase 3.
- [x] Plan model plus lossless XML read/write (`crates/x4sd-core/src/plan.rs`). Strict reader; values round-trip exactly on all game files; structure and positions byte-identical to the game's output.
- [x] Import-file writer (D-5): a single-plan file in `<profile>\constructionplan\`, unique `player_<unix time>` id, sanitised file name, no overwrite by default
- [x] Virtual file system plus module catalogue: base, DLCs **and enabled mods** (`recyclemodule_macro` resolves); macro → component → snaps; required `<patches>` (only `save` extensions)
- [x] Snap placement (`geometry::attach`) and measurement (`geometry::fit`). Reproduces all 534 linked placements in the player plans.
- [x] Validator: unknown macros/equipment/snaps, missing predecessors, snap gaps (5 cm) and misalignment (0.5°), missing `<patches>`
- [x] Module display names from `t/` (`text.rs`) and the list of buildable station modules (`Catalog::station_modules`, 287 modules)

## Phase 3: Designer app: visual builder plus agent tooling on one shared design ✅
The visual builder is first-class (G6), so it's built alongside the tooling, not after it.
- [x] Decide OD-6: live link, with the builder as the hub (D-7)
- [x] Decide OD-1: both MCP and CLI from one exe (D-8). The UI technology is settled by D-6 (Tauri + WebView2 + Three.js).
Proposed order (each step usable on its own):
- [x] **Design editing in the core** (`design.rs`, log: [workflow-phase3-app.md](workflow-phase3-app.md)): operations on an open plan (add a root module, attach a module to a snap with a twist, rotate, move a root, remove with descendants, rename), plus undo/redo. The builder, CLI and MCP all go through these.
- [x] **CLI** (`crates/x4sd`, one binary; see `docs/usage.md`): `list-modules`, `validate <file>`, `export <file>` (to `constructionplan\`), `show <file>`
- [x] **MCP server** (`x4sd mcp`, stdio, official Rust SDK `rmcp`): tools mirroring the design operations, plus list/validate/export
- [x] **Module shapes for the 3D view** (`Module::shape`, part boxes): research bounding boxes or sizes in component XML (`<parts>` / `<size>`). Real meshes (`.xmf`) come later, if ever.
- [x] **Builder (Tauri + Three.js):** palette (search the 287 modules), 3D view with snap markers, drag a module onto a snap to attach, rotate in steps around the snap axis, validation panel, export button
- [x] **Live link (D-7):** the builder listens on a local socket. `x4sd mcp` forwards operations to it when it runs, and edits its own design headless when it doesn't.

## Phase 4: Polish and release (proposed, not yet agreed)
- [ ] Try a full round trip in the game: design in the builder, Export, Import in the build menu, build it
- [ ] Builder usability:
  - [x] preview of the armed module on the hovered snap (kept; see [workflow-snap-pick.md](workflow-snap-pick.md))
  - [x] choose the connecting snap: direction labels, C/T keys, own-snap cones when moving, and *Connected by* to change it in place on a placed module (user requests, 2026-10-14)
  - [x] ~~snap labels~~: not needed; the user is happy with the direction labels in *Connect by snap* and the hover preview (2026-10-14)
  - [x] ~~better camera on small designs~~: framing was already fine; the zoom limits made close-up drags erratic, so the user had them reverted (2026-10-14; see [workflow-small-camera.md](workflow-small-camera.md))
  - [x] move unattached modules in the UI: X / Y / Z position fields for a selected free-standing module, to place the first module exactly in the plot (user request, 2026-10-14; see [workflow-position-fields.md](workflow-position-fields.md))
  - [x] live checks while dragging the gizmo: plot box colour and *Checks* follow the drag (user request, 2026-10-14; see [workflow-live-checks.md](workflow-live-checks.md))
- [x] **Equipment** (user request; shields and turrets per module group; see [workflow-equipment.md](workflow-equipment.md)): per-group editing, plus bulk by picks, loadout level or copying; MCP tools; validation. Still to check in game: an equipped plan builds (likely: in-game plans with equipment load correctly).
- [x] Real module meshes from `.xmf` instead of boxes (user request, done first; see [workflow-module-meshes.md](workflow-module-meshes.md))
- [x] Textures on the models as an on/off setting (user request; on by default after measuring; see [workflow-module-meshes.md](workflow-module-meshes.md))
- [x] **Race filter** in the palette (user request): chips for one or several races (e.g. Terran + Boron) or all; remembered; ui_smoke checks Terran + Boron = 85 modules (83 before dock areas took the race in their macro name)
- [x] **Plot size** (user request): plot per side in km, drawn as a box; modules outside are flagged; an enlarge (or undo) prompt; MCP `set_plot`/`fit_plot`; CLI `show` (plot needed) and `validate --plot`. See [workflow-plot-size.md](workflow-plot-size.md).
- [x] **Build order viewer and editor** (user request; see [workflow-build-order.md](workflow-build-order.md)): list with stage dividers, drag to reorder, build first/last, stage ends, timeline slider, MCP tools. Verified in game (user, 2026-10-14, light testing): player builds follow the order.
  - [x] Search in the build order (user request, 2026-10-15): matches highlighted; Enter / Shift+Enter select the next / previous match and frame it in the view. Checked in the builder over DevTools (scratch test).
- [x] Keyboard navigation (user requests): WASD pan, Q/E down/up, arrow keys rotate
- [x] Palette in the game's build-menu category order (user request)
- [x] **x4ep_planner functionality** (user request; our own version from the game files; see [workflow-production-planner.md](workflow-production-planner.md)). Verified in game (user, 2026-10-14, light testing): an auto-placed station builds and ships reach its docks.
  - [x] A. Station analysis: production per hour, workforce, storage, docks (right pane, MCP `analyze_station`)
  - [x] B. Production planner: target wares to a module list, with a "to place" list (Planner tab, MCP `plan_production`, `list_wares`)
  - [x] C. Auto-placement of the planned modules: Planner tab *Place all*, MCP `auto_place`; overlap checks on collision meshes (also in `validate`)
  - [x] Plan files (user request, 2026-10-14): Open/Save as start in the game's plan folder; a renamed plan exports as a new plan (D-11). See [workflow-plan-files.md](workflow-plan-files.md).
  - [x] D. Optimize (user request, 2026-10-14): fix the shortfalls and re-lay the station out in layers (slotless inside, defensible outside, docks outermost), keeping its shape or rebuilding connectors. Optimize shows a plan to trim first, then *Apply plan*; the plot is fitted afterwards (D-14). See [workflow-optimize.md](workflow-optimize.md).
  - [x] F. Planner extras (user request, 2026-10-14): other modules (ship bays, defence platforms…), storage minimums per type, defence platforms on chosen sides of the station (D-15). See [workflow-planner-extras.md](workflow-planner-extras.md). Still to check in game.
  - [x] Other modules box: piers too, and only the planner race's modules (user request, 2026-10-15; see [workflow-planner-piers.md](workflow-planner-piers.md))
  - [x] Defence platforms and piers outermost (user report 2026-10-15: Optimize put a disc deep inside a station): outer layer with a clear corridor out, D-23. See [workflow-outermost.md](workflow-outermost.md).
  - [x] E. Clearance for fabrication bays and piers (user report 2026-10-14): exclusion zones from the game data, `validate` reports blocked ones, auto-place and Optimize keep them clear (D-12). Still to check in game. See [workflow-clearance.md](workflow-clearance.md).
- [x] **In-game load** (user request, 2026-10-14): right-pane readout (modules, connection structures, turrets, docks, size), MCP `station_load`, `show`; Planner option *Fewer connection structures* (D-13). See [workflow-fps-load.md](workflow-fps-load.md). Verified in game (user, 2026-10-14, light testing): these counts track frame rate.
- [x] **Blueprint limit** (user request, 2026-10-14): an option to read a save and offer only the modules the player has blueprints for, in the palette, the planner and MCP; cached and remembered (D-16). See [workflow-save-blueprints.md](workflow-save-blueprints.md).
- [x] Auto-placement keeps room to grow (user report 2026-10-15: a Teladi 50-claytronics station placed 38 of 461 modules; one-snap modules capped every exit). See [workflow-placement-exits.md](workflow-placement-exits.md).
- [x] Copy / Remove sequence like the game: Del removes only the module (what was on it floats, *Checks* warns), Shift+Del the sequence; Ctrl+C copies, Ctrl+Shift+C / Shift+middle click copies the sequence; right-click module menu (user request 2026-10-15, D-20; see [workflow-copy-remove-sequence.md](workflow-copy-remove-sequence.md))
- [x] Build cost in the right pane: selected module or whole station, build method (user request; see [workflow-build-cost.md](workflow-build-cost.md))
- [x] Move placed modules (re-attach to another snap, detach, gizmo for free-standing ones) and warn about unsaved changes (user requests; see [workflow-move-and-unsaved.md](workflow-move-and-unsaved.md))
- [x] Release build for players: README, versioning, a GitHub release with `x4sd.exe` (v1.0.0 published 2026-10-14)
  - [x] GIFs of the main features in `docs/media/` (`research/record_gifs.py`; see [workflow-release-gifs.md](workflow-release-gifs.md))
  - [x] First placement backs the view off 5 km (user request during the recording)
  - [x] README.md with the GIFs, getting started, MCP and CLI
  - [x] Version 1.0.0, MIT licence, social preview image (see [workflow-release.md](workflow-release.md))
  - [x] Draft GitHub release v1.0.0 with `x4sd.exe`
  - [x] v1.1.0 published 2026-10-14: update notice, placement keeps the station's modules ([workflow-release-1.1.md](workflow-release-1.1.md))
  - [x] v1.2.0 published 2026-10-15: view draws on demand, frame rate limit ([workflow-release-1.2.md](workflow-release-1.2.md))
  - [x] v1.3.0 published 2026-10-15: copy / remove sequence, right-click menu, turns on what you look at, planner piers and dock areas ([workflow-release-1.3.md](workflow-release-1.3.md))
  - [x] v1.4.0 published 2026-10-15: game found on any machine, placement breaks the station up and packs rebuilds, docks/piers/defence outermost, partial builds, most overlaps are notes ([workflow-release-1.4.md](workflow-release-1.4.md))
  - [x] The user published it and made the repo public
- [x] Find the game on any machine: Steam libraries anywhere, GOG, a folder picker in the builder that is remembered (D-24, [workflow-find-game.md](workflow-find-game.md))
- [ ] Automated checks (CI) for the parts that don't need the game installed
- [x] Modules no longer stack inside each other: snaps join only facing each other ([workflow-joined-snaps.md](workflow-joined-snaps.md)); Place all asks before a partial build ([workflow-placement-partial.md](workflow-placement-partial.md))
- [ ] Re-tune auto-placement for honest layouts: break up sealed stations, connectors inside for internal modules ([workflow-retune-placement.md](workflow-retune-placement.md))

## Later: live bridge (option D)
- [ ] Study `sn_mod_support_apis` named pipes
- [ ] A mod that builds an imported plan by id (`add_build_to_expand_station`)
