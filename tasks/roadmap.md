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
- [ ] **Snap math:** recompute each entry's offset from its predecessor's offset plus both snap transforms (and the plan rotation), and compare with the stored offsets of all player plans (5 plans, about 540 entries). Needs the component XML of every module, including DLC ones. Our designer can't place modules without this.
- [ ] Clean up the test plans in-game: the user deletes "X4SD test B" and "X4SD test C" from the plan list
- [ ] Injection test A after a restart (low priority, since B works)
- [ ] Study `sn_mod_support_apis` named pipes (option D)
- [x] Decide OD-3 (injection method): option B, recorded as D-5

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
