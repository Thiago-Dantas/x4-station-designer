# Workflow: release v1.1.0

Status: **done** (2026-10-14; published 2026-10-15T00:17Z UTC)

User: "release 1.1 with what we have now, no draft, straight to publish"

## Steps
- [x] Version 1.1.0: `crates/x4sd/Cargo.toml`, `crates/x4sd-core/Cargo.toml`, `crates/x4sd/tauri.conf.json`, `ui/package.json` (+ lock files)
- [x] Build: `npm --prefix ui run build`, `cargo build -p x4sd --release`; `x4sd --version` says 1.1.0
- [x] Docs: CLAUDE.md status, roadmap
- [x] Commit, push
- [x] Publish release `v1.1.0` (target `main`, not a draft) with `x4sd.exe`; notes below
- [x] Check: releases/latest is v1.1.0 (authenticated API; the anonymous one was cached for a few minutes), asset digest sha256:6c71023f… matches the build

## Notes
- Release exe SHA-256 6C71023F…E78CDCAE; a start-up check (CDP) loads 287 modules and shows no update bar.
- Full test suite not re-run (standing rule: test only what changed; each change since v1.0.0 was checked on its own).
- Release notes: scratchpad `release-notes-v1.1.0.md`.
