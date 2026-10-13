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

## Phase 3: Designer app: visual builder plus agent tooling on one shared design
The visual builder is first-class (G6), so it's built alongside the tooling, not after it.
- [x] Decide OD-6: live link, with the builder as the hub (D-7)
- [x] Decide OD-1: both MCP and CLI from one exe (D-8). The UI technology is settled by D-6 (Tauri + WebView2 + Three.js).
Proposed order (each step usable on its own):
- [ ] **Design editing in the core:** operations on an open plan (add a root module, attach a module to a snap with a twist, rotate, move a root, remove with descendants, rename), plus undo/redo. The builder, CLI and MCP all go through these.
- [ ] **CLI** (`crates/x4sd`, one binary): `list-modules`, `validate <file>`, `export <file>` (to `constructionplan\`), `show <file>`
- [ ] **MCP server** (`x4sd mcp`, stdio, official Rust SDK `rmcp`): tools mirroring the design operations, plus list/validate/export
- [ ] **Module shapes for the 3D view:** research bounding boxes or sizes in component XML (`<parts>` / `<size>`). Real meshes (`.xmf`) come later, if ever.
- [ ] **Builder (Tauri + Three.js):** palette (search the 287 modules), 3D view with snap markers, drag a module onto a snap to attach, rotate in steps around the snap axis, validation panel, export button
- [ ] **Live link (D-7):** the builder listens on a local socket. `x4sd mcp` forwards operations to it when it runs, and edits files headless when it doesn't.

## Later: live bridge (option D)
- [ ] Study `sn_mod_support_apis` named pipes
- [ ] A mod that builds an imported plan by id (`add_build_to_expand_station`)
