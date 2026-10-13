# Workflow: Phase 3, designer app (builder + CLI + MCP)

Status: **in progress** (started 2026-10-13)

Order agreed with the user (foundation first): core editing operations, CLI, MCP, module shapes, builder, live link. See [roadmap.md](roadmap.md) Phase 3 and decisions D-6/D-7/D-8.

## Steps
- [x] 1. `x4sd-core::design`: an open plan plus operations (add root, attach, set twist, move root, remove subtree, rename, free snaps) and snapshot undo/redo. 4 unit tests (via `Catalog::from_modules`, no game files) + `designed_station_validates` with real Terran modules.
- [x] 2. CLI binary `crates/x4sd` (clap): `list-modules`, `snaps`, `show`, `validate`, `export`. Finds the game through Steam's `libraryfolders.vdf` and the profile by itself. Release exe 1.0 MB. Tried every command against the real data and a scratch profile.
- [ ] 3. MCP server `x4sd mcp` (rmcp, stdio)
- [ ] 4. Module shapes for the 3D view (research the component XML)
- [ ] 5. Builder (Tauri + Three.js)
- [ ] 6. Live link builder ⇄ MCP

## Sub-agents
None.

## Notes / findings log
