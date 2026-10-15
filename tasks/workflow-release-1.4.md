# Workflow: release v1.4.0

Status: **done** (2026-10-15)

User: "publish a new version" (2026-10-15)

## Steps
- [x] Tests: `cargo test --workspace`, `npm --prefix ui run check` (smoke tests not run: full suite only when the user asks)
- [x] Version 1.4.0: `crates/x4sd/Cargo.toml`, `crates/x4sd-core/Cargo.toml`, `crates/x4sd/tauri.conf.json`, `ui/package.json` (+ lock files)
- [x] Build; `x4sd --version` says 1.4.0
- [x] Docs: CLAUDE.md status, roadmap
- [x] Commit, push
- [ ] Publish release `v1.4.0` (target `main`, not a draft) with `x4sd.exe`
- [ ] Check: releases/latest is v1.4.0, asset digest matches the build

## Notes
- Tests 2026-10-15, all pass: `cargo test --workspace --release` (24 unit, 25 game-data with 2 ignored for the placement re-tune, 2 round-trip), `npm --prefix ui run check`. Smoke tests not run.
- The user's open builder held `target\release\x4sd.exe`; the user closed it before the build.
- Release exe SHA-256 9E101D30F359FC87813B8E0E1E8EE547626A76C484051DF07B5EB0E9F758F929; start-up check loads 287 modules, no update bar.
- Release notes: scratchpad `release-notes-v1.4.0.md`.
