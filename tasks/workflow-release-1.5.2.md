# Workflow: release v1.5.2

Status: **done** (2026-11-08)

User: "publish a 1.5.2 release with the fix" (2026-11-08)

## Steps
- [x] Version 1.5.2: `crates/x4sd/Cargo.toml`, `crates/x4sd-core/Cargo.toml`, `crates/x4sd/tauri.conf.json`, `ui/package.json` (+ lock files)
- [x] Docs: CLAUDE.md status, roadmap
- [x] Commit, push (`de81471`)
- [x] Annotated tag `v1.5.2` with the release notes, push the tag (Actions builds and publishes)
- [x] Check: the Release workflow is green and `releases/latest` is v1.5.2 (`x4sd.exe`, `x4sd.exe.sigstore.json`, VirusTotal link in the notes)

## Changes since v1.5.1
- Ctrl+Z (and Ctrl+Y) undo design edits only: they no longer also undo the last typing in the palette search or another text box (GitHub issue #5)
