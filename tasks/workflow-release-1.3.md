# Workflow: release v1.3.0

Status: **done** (2026-10-15)

User: "run the full test suite then once all passes make a release" (2026-10-15)

## Steps
- [x] Full suite: `cargo test --workspace`, `npm --prefix ui run check`, every `research/*_smoke.py` on the release exe
- [x] Version 1.3.0: `crates/x4sd/Cargo.toml`, `crates/x4sd-core/Cargo.toml`, `crates/x4sd/tauri.conf.json`, `ui/package.json` (+ lock files)
- [x] Build; `x4sd --version` says 1.3.0
- [x] Docs: CLAUDE.md status, roadmap
- [x] Commit, push
- [x] Publish release `v1.3.0` (target `main`, not a draft) with `x4sd.exe`
- [x] Check: releases/latest is v1.3.0, asset digest matches the build

## Changes since v1.2.0
- Copy / Remove sequence like the game, right-click module menu, *not attached* check (D-20)
- Turns (Q/E, arrows, left drag) go around what is in the middle of the view
- Planner Other modules: piers and dock areas, filtered by the planner race; universal modules
- Auto-placement keeps room to grow; module race falls back to the macro name's race code

## Notes
- Full suite 2026-10-15, all pass: `cargo test --workspace` (24 unit, 23 game-data, 2 round-trip), `npm --prefix ui run check`, smoke tests mcp, ui, link, snap_pick, camera, gizmo_checks, sequence, turn_pivot, texture, blueprint (save_001).
  - ui_smoke expected Terran + Boron = 83 modules; now 85, because dock areas without a maker race take the race in their macro name since f2d4fdd (e.g. `dockarea_ter_m_station_01_standard_macro`). Test updated.
  - link_smoke must not share its scratch folder with mcp_smoke (both export "MCP smoke station").
- Release exe SHA-256 A15B65F3…E39709769; start-up check loads 287 modules, no update bar.
- Release notes: scratchpad `release-notes-v1.3.0.md`.
