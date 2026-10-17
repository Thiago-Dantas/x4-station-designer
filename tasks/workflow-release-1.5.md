# Workflow: release v1.5.0

Status: **done** (2026-10-16)

User: "commit and publish a new version" (2026-10-16)

## Steps
- [x] Version 1.5.0: `crates/x4sd/Cargo.toml`, `crates/x4sd-core/Cargo.toml`, `crates/x4sd/tauri.conf.json`, `ui/package.json` (+ lock files)
- [x] Docs: CLAUDE.md status, roadmap
- [x] Commit, push (`617ddd9`)
- [x] Annotated tag `v1.5.0` with the release notes, push the tag (Actions builds and publishes)
- [x] Check: the Release workflow is green and `releases/latest` is v1.5.0 (`x4sd.exe`, `x4sd.exe.sigstore.json`). CI, Scorecard, and CodeQL on `617ddd9` are green.

## Changes since v1.4.0
- Shift-click a range in the build order and drag those modules together
- A drop that would build a module before the one it attaches to lands on the closest step that still keeps that order
- The mouse wheel scrolls the build-order list while a row is dragged
- Page Up builds the selected module first; Page Down builds it last
