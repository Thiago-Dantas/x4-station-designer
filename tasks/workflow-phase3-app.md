# Workflow: Phase 3, designer app (builder + CLI + MCP)

Status: **in progress** (started 2026-10-13)

Order agreed with the user (foundation first): core editing operations, CLI, MCP, module shapes, builder, live link. See [roadmap.md](roadmap.md) Phase 3 and decisions D-6/D-7/D-8.

## Steps
- [x] 1. `x4sd-core::design`: an open plan plus operations (add root, attach, set twist, move root, remove subtree, rename, free snaps) and snapshot undo/redo. 4 unit tests (via `Catalog::from_modules`, no game files) + `designed_station_validates` with real Terran modules.
- [x] 2. CLI binary `crates/x4sd` (clap): `list-modules`, `snaps`, `show`, `validate`, `export`. Finds the game through Steam's `libraryfolders.vdf` and the profile by itself. Release exe 1.0 MB. Tried every command against the real data and a scratch profile.
- [x] 3. MCP server `x4sd mcp` (rmcp 3.5.1, stdio): 17 tools over `design`. `research/mcp_smoke.py` builds, validates and exports a Terran station end to end. Exe is 3.8 MB.
- [ ] 4. Module shapes for the 3D view (research the component XML)
- [ ] 5. Builder (Tauri + Three.js)
- [ ] 6. Live link builder ⇄ MCP

## Sub-agents
None.

## Notes / findings log
- 2026-10-13: rmcp 3.5.1 API:
  - `#[tool_router]` impl with `#[tool]` methods taking `Parameters<T: Deserialize + JsonSchema>`. Needs the rmcp feature `schemars`.
  - `#[tool_handler(router = self.tool_router)]` on `impl ServerHandler`. Without `router =` it rebuilds the router each call and leaves the field unused.
  - Tools returning `Result<String, String>` turn `Err` into an `isError` tool result.
  - `ServerConfig::new(caps).with_server_info(Implementation::new(..)).with_instructions(..)`.
  - The MCP revision 2026-07-28 drops the `initialize` handshake; rmcp still accepts it from older clients (our smoke test uses 2025-06-18).
- 2026-10-13: the game is found through `%ProgramFiles(x86)%\Steam\steamapps\libraryfolders.vdf` (6 libraries here; X4 on F:).
