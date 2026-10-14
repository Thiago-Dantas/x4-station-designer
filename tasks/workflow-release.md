# Workflow: first release (v1.0.0)

Status: **in progress** (started 2026-10-14). Roadmap item "Release build for players". The GIFs and README are done ([workflow-release-gifs.md](workflow-release-gifs.md)).

User request (2026-10-14): update docs/usage.md and run the full test suite before publishing a release.

## Steps
- [x] docs/usage.md: stale size ("about 1 MB"), "textures are not shown yet", the dev path in the MCP example
- [x] Fresh build: `npm --prefix ui run check`, `npm --prefix ui run build`, `cargo build -p x4sd --release`
- [x] `cargo test --workspace` (unit, round-trip, game-data tests)
- [x] Smoke tests against the release exe: mcp_smoke, ui_smoke, link_smoke, texture_smoke, snap_pick_smoke, camera_smoke, gizmo_checks_smoke, blueprint_smoke
- [x] Fix or report failures
- [x] x4ep_planner audit (user request): no code from the mod; one near-verbatim tooltip phrase reworded (D-17)
- [x] User answers (2026-10-14): version **1.0**; repo goes public later (the user does it); licence: **MIT** (user: "if MIT is permissive go with that"; `LICENSE`, workspace `license = "MIT"`)
- [x] Version 1.0.0: `crates/x4sd/Cargo.toml`, `crates/x4sd-core/Cargo.toml`, `crates/x4sd/tauri.conf.json`, `ui/package.json`
- [x] Repo "icon" (user request): GitHub has no repo icon, so the app icon goes at the top of the README, and a 1280×640 social preview (`docs/media/social-preview.png`) for the user to upload in Settings → General → Social preview
- [x] Release build (`x4sd 1.0.0`, SHA-256 4999BA72…6C4E1D), release notes (scratchpad `release-notes-v1.0.0.md`, also below)
- [x] README: says up front it was vibe-coded for fun with Claude Code (user request)
- [x] Install `gh` on H: (2.102.0)
- [x] The user signs in: `H:\dev\gh\bin\gh.exe auth login`
- [x] Draft release `v1.0.0` (target `main`, tag made on publish) with `x4sd.exe` (SHA-256 5E6E3FB0…CD522D8, matches the upload's digest); notes from scratchpad `release-notes-v1.0.0.md`
- [ ] The user publishes the draft on GitHub
- [ ] The user: make the repo public, then upload `docs/media/social-preview.png` (Settings → General → Social preview; GitHub hides it on private repos)

## Notes
- `cargo test --workspace`: 44 tests. `validates_library_plans` failed: it predates the exclusion-zone check (D-12, f830702) and expected no issue besides gaps and overlaps. The faction library plans put modules in 18 zones (17 of fabrication bays, 1 of a pier); the test now pins those. All pass after that (game-data tests take ~200 s).
- Smoke tests (release exe): all 8 pass. `ui_smoke` failed at first: after *Place as first module* the view now backs off 5 km, where the cross's cones are tiny and `find_color` merged them, so the click missed; the test now presses Fit view before looking for cones.
- x4ep_planner audit (D-17): identifiers and constants, structure, formulas, and an 8-/6-word sequence comparison (`scratchpad/shingles.py`-style: words of the mod's files vs ours). One shared phrase (storage-hours tooltip and doc comments, also MCP's `storage_hours` description) reworded; nothing else.
