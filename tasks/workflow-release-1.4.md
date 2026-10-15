# Workflow: release v1.4.0

Status: **in progress**

User: "publish a new version" (2026-10-15)

## Steps
- [ ] Tests: `cargo test --workspace`, `npm --prefix ui run check` (smoke tests not run: full suite only when the user asks)
- [ ] Version 1.4.0: `crates/x4sd/Cargo.toml`, `crates/x4sd-core/Cargo.toml`, `crates/x4sd/tauri.conf.json`, `ui/package.json` (+ lock files)
- [ ] Build; `x4sd --version` says 1.4.0
- [ ] Docs: CLAUDE.md status, roadmap
- [ ] Commit, push
- [ ] Publish release `v1.4.0` (target `main`, not a draft) with `x4sd.exe`
- [ ] Check: releases/latest is v1.4.0, asset digest matches the build
