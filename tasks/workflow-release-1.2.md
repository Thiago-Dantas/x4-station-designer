# Workflow: release v1.2.0

Status: in progress (2026-10-15)

User: "publish 1.2" (straight to publish, as for 1.1)

## Steps
- [x] Version 1.2.0: `crates/x4sd/Cargo.toml`, `crates/x4sd-core/Cargo.toml`, `crates/x4sd/tauri.conf.json`, `ui/package.json` (+ lock files)
- [x] Build; `x4sd --version` says 1.2.0; start-up check
- [x] Docs: CLAUDE.md status, roadmap
- [ ] Commit, push
- [ ] Publish release `v1.2.0` (target `main`, not a draft) with `x4sd.exe`
- [ ] Check: releases/latest is v1.2.0, asset digest matches

## Notes
- Release exe SHA-256 171DCABE…5E91CDF7; start-up check loads 287 modules, no update bar.
- Full test suite not re-run (test only what changed); the changes since v1.1.0 were checked on their own (GPU/fps measurements, ui_smoke, snap_pick_smoke, gizmo_checks_smoke).
- Release notes: scratchpad `release-notes-v1.2.0.md`.
