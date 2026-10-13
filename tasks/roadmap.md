# Roadmap

Master list of milestones. Each active workstream gets its own `tasks/workflow-<topic>.md` with step-by-step state, linked from here.

## Phase 0: Project setup ✅
- [x] CLAUDE.md, docs/, tasks/ skeleton ([workflow-project-setup.md](workflow-project-setup.md))
- [x] Decide on git and hosting: private GitHub repo (D-3)
- [x] `git init`, set repo-local `user.email` = dant12@gmail.com, initial commit
- [ ] Push to `github.com/Thiago-Dantas/x4-station-designer`. Waiting for the user to create the empty private repo.

## Phase 1: Research: how X4 handles station plans
- [ ] Extract `libraries/constructionplans.xml` and compare its format with the player file
- [ ] Find the module macros and component files for a few station modules; map `connectionsnapNNN` to their transforms
- [ ] **Top question:** check whether `offset` is authoritative or derived from predecessor + connection. Answer it with REA on `X4.exe`; it is the first RE target.
- [ ] Injection test A: add a hand-made plan to the profile file (backup first, game closed), launch, check the plan appears
- [ ] Check the game's read and write behaviour for `constructionplans.xml` (startup only? overwritten on exit?)
- [ ] Check for an in-game import/export feature and what `<profile>\constructionplan\` is for (option B)
- [ ] Investigate `x4ep_planner` (GHOSTau's Station Planner, `ws_3777272604`), the main prior art
  - [ ] Extract its `ext_01.cat/.dat` to a research folder outside the install. This also tests the `.dat` layout hypothesis.
  - [ ] Read `readme.txt`, `ui.xml`, `md/x4ep_build_queue.xml`, `md/x4ep_build_bridge.xml`
  - [ ] In `ui/ep_planner.lua`, find which build-menu or FFI calls place modules, connect them and build the plan
  - [ ] Find its module-geometry tables in `ui/ep_data.lua` (how it knows snap points)
  - [ ] Write up the findings in `docs/`
- [ ] Study `sn_mod_support_apis` named pipes (option D)
- [ ] Decide OD-3 (injection method)
- [ ] Use REA on `X4.exe`: start with the `offset` question, then any others the data files can't answer

## Phase 2: Core library
- [ ] Decide OD-2 (language)
- [ ] Plan model plus lossless XML read/write (round-trip test against the player file)
- [ ] `.cat`/`.dat` reader and module catalogue builder
- [ ] Validator (macros, connections, DLC requirements)

## Phase 3: Agent interface
- [ ] Decide OD-1 (CLI / MCP / both)
- [ ] Implement it on top of the core library

## Phase 4: Visual designer
- [ ] Decide OD-5 (technology)
- [ ] 3D view, module placement with snapping, export
