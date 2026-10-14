# Workflow: first release (v0.1.0)

Status: **in progress** (started 2026-10-14). Roadmap item "Release build for players". The GIFs and README are done ([workflow-release-gifs.md](workflow-release-gifs.md)).

User request (2026-10-14): update docs/usage.md and run the full test suite before publishing a release.

## Steps
- [x] docs/usage.md: stale size ("about 1 MB"), "textures are not shown yet", the dev path in the MCP example
- [x] Fresh build: `npm --prefix ui run check`, `npm --prefix ui run build`, `cargo build -p x4sd --release`
- [x] `cargo test --workspace` (unit, round-trip, game-data tests)
- [x] Smoke tests against the release exe: mcp_smoke, ui_smoke, link_smoke, texture_smoke, snap_pick_smoke, camera_smoke, gizmo_checks_smoke, blueprint_smoke
- [x] Fix or report failures
- [x] x4ep_planner audit (user request): no code from the mod; one near-verbatim tooltip phrase reworded (D-17)
- [ ] Open (user's call): version number, license, repo visibility, publishing the GitHub release (`gh` isn't installed)

## Notes
- `cargo test --workspace`: 44 tests. `validates_library_plans` failed: it predates the exclusion-zone check (D-12, f830702) and expected no issue besides gaps and overlaps. The faction library plans put modules in 18 zones (17 of fabrication bays, 1 of a pier); the test now pins those. All pass after that (game-data tests take ~200 s).
- Smoke tests (release exe): all 8 pass. `ui_smoke` failed at first: after *Place as first module* the view now backs off 5 km, where the cross's cones are tiny and `find_color` merged them, so the click missed; the test now presses Fit view before looking for cones.
- x4ep_planner audit (D-17): identifiers and constants, structure, formulas, and an 8-/6-word sequence comparison (`scratchpad/shingles.py`-style: words of the mod's files vs ours). One shared phrase (storage-hours tooltip and doc comments, also MCP's `storage_hours` description) reworded; nothing else.
