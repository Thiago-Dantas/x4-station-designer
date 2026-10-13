# Workflow: Project setup (Phase 0)

Status: **done** (2026-10-13)

Goal: create the project skeleton (CLAUDE.md, docs/, tasks/) with first-pass, verified knowledge of how X4 stores station plans.

## Steps
- [x] Inspect workspace (empty, not a git repo)
- [x] Inspect game install: `F:\SteamLibrary\steamapps\common\X4 Foundations` (version.dat = 900)
- [x] Inspect player profile: `%USERPROFILE%\Documents\Egosoft\X4\104494216\`
- [x] Read the structure of `constructionplans.xml` (4 plans, 459 entries)
- [x] Confirm `.cat` index line format and find `libraries/constructionplans.xml` in `08.cat`
- [x] Write `CLAUDE.md`
- [x] Write `docs/goals.md`, `docs/decisions.md`, `docs/x4-construction-plans.md`, `docs/x4-game-data.md`
- [x] Write `tasks/roadmap.md`
- [x] User pointed to workshop mod `3777272604`. It is installed as `extensions/x4ep_planner`. Its contents are documented and the investigation was added to the roadmap.

- [x] User decision: the `offset` question will be answered with REA (D-4)
- [x] User decision: git with a private GitHub repo (D-3). `gh` is not installed, so the user creates the repo on github.com. Commits use the repo-local email dant12@gmail.com.
- [x] `git init -b main` and initial commit
- [x] Added remote `https://github.com/Thiago-Dantas/x4-station-designer.git`. The stored GCM login had expired, so the user did the first push from their own terminal, which refreshed it.

## Sub-agents
None.
