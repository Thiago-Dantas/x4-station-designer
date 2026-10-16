# Workflow: release v1.5.0

Status: **in progress** (2026-10-16)

User: "commit and publish a new version" (2026-10-16)

## Steps
- [x] Version 1.5.0: `crates/x4sd/Cargo.toml`, `crates/x4sd-core/Cargo.toml`, `crates/x4sd/tauri.conf.json`, `ui/package.json` (+ lock files)
- [x] Docs: CLAUDE.md status, roadmap
- [ ] Commit, push
- [ ] Annotated tag `v1.5.0` with the release notes, push the tag (Actions builds and publishes)
- [ ] Check: the Release workflow is green and `releases/latest` is v1.5.0

## Changes since v1.4.0
- Shift-click a range in the build order and drag those modules together
- A drop that would build a module before the one it attaches to lands on the closest step that still keeps that order
- The mouse wheel scrolls the build-order list while a row is dragged
- Page Up builds the selected module first; Page Down builds it last
