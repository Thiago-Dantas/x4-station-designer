# Workflow: release v1.5.1

Status: **in progress** (2026-10-18)

User: "commit everything and publish a new version to github" (2026-10-18)

## Steps
- [x] Version 1.5.1: `crates/x4sd/Cargo.toml`, `crates/x4sd-core/Cargo.toml`, `crates/x4sd/tauri.conf.json`, `ui/package.json` (+ lock files)
- [x] Docs: CLAUDE.md status, roadmap
- [ ] Commit, push
- [ ] Annotated tag `v1.5.1` with the release notes, push the tag (Actions builds and publishes)
- [ ] Check: the Release workflow is green and `releases/latest` is v1.5.1 (`x4sd.exe`, `x4sd.exe.sigstore.json`)

## Changes since v1.5.0
- The Planner tab scrolls when the plan is taller than the pane, so the module list, *Generate plan* and the hint stay reachable
- The ware and other-module suggestion lists stay on screen while that tab scrolls, and open upward when they would run past the bottom of the window
