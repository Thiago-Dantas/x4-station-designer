# Workflow: Phase 3, designer app (builder + CLI + MCP)

Status: **done** (2026-10-13), the MVP

Order agreed with the user (foundation first): core editing operations, CLI, MCP, module shapes, builder, live link. See [roadmap.md](roadmap.md) Phase 3 and decisions D-6/D-7/D-8.

## Steps
- [x] 1. `x4sd-core::design`: an open plan plus operations (add root, attach, set twist, move root, remove subtree, rename, free snaps) and snapshot undo/redo. 4 unit tests (via `Catalog::from_modules`, no game files) + `designed_station_validates` with real Terran modules.
- [x] 2. CLI binary `crates/x4sd` (clap): `list-modules`, `snaps`, `show`, `validate`, `export`. Finds the game through Steam's `libraryfolders.vdf` and the profile by itself. Release exe 1.0 MB. Tried every command against the real data and a scratch profile.
- [x] 3. MCP server `x4sd mcp` (rmcp 3.5.1, stdio): 17 tools over `design`. `research/mcp_smoke.py` builds, validates and exports a Terran station end to end. Exe is 3.8 MB.
- [x] 4. Module shapes: part boxes from component `<size>` (detail, collision and invisible parts skipped; a snap-bounds fallback for one partless landmark). All 287 modules have snaps + shape.
- [x] 5. Builder (Tauri 2 + Three.js): palette, 3D boxes, snap cones, click/drag to attach, twist, remove, undo/redo, live checks, open/save/export. One 12.6 MB exe (builder + CLI + MCP). `research/ui_smoke.py` drives it via WebView2 DevTools (open TerranScrapyard, place + attach by clicking a cone, undo).
- [x] 6. Live link builder ⇄ MCP:
  - `ops.rs`: a shared serde-tagged `Op` enum plus `execute`.
  - `link.rs`: 127.0.0.1:47823, JSON lines, greeting `{"x4sd": version}`.
  - The builder runs agent operations on its open design and emits `scene-changed`.
  - `x4sd mcp` forwards when a builder is up, and tells the agent once if the link breaks.
  - New tool `builder_status` (18 tools now).
  - `research/link_smoke.py` confirms an agent-built station appears in the builder (scratch profile). Headless MCP, the UI smoke test and all tests pass.

## Sub-agents
None.

## Notes / findings log
- 2026-10-13: rmcp 3.5.1 API:
  - `#[tool_router]` impl with `#[tool]` methods taking `Parameters<T: Deserialize + JsonSchema>`. Needs the rmcp feature `schemars`.
  - `#[tool_handler(router = self.tool_router)]` on `impl ServerHandler`. Without `router =` it rebuilds the router each call and leaves the field unused.
  - Tools returning `Result<String, String>` turn `Err` into an `isError` tool result.
  - `ServerConfig::new(caps).with_server_info(Implementation::new(..)).with_instructions(..)`.
  - The MCP revision 2026-07-28 drops the `initialize` handshake; rmcp still accepts it from older clients (our smoke test uses 2025-06-18).
- 2026-10-13: builder notes:
  - Tauri 3 is alpha, so we use 2.12. `custom-protocol` on by default embeds `ui/dist`.
  - The game → Three.js conversion mirrors Z: quaternion (w,x,y,z) → (w,-x,-y,z), checked on yaw 90.
  - `x4sd.exe` detaches a console created only for it (double-click).
  - One UI-smoke run timed out on the attach click, and 3 reruns passed. A suspected cause is a leftover WebView2 process holding the debug port; not confirmed.
- 2026-10-13: the game is found through `%ProgramFiles(x86)%\Steam\steamapps\libraryfolders.vdf` (6 libraries here; X4 on F:).
- 2026-10-13 (follow-up, user request): WASD pans like a right drag (screen space, smooth while held, speed scales with the camera distance; ignored while typing). ui_smoke checks it: D shifts the scene about 320 px left, W about 325 px down.
- 2026-10-13: fixed the flaky UI smoke runs, which were test problems.
  - `wait_for` now tolerates the page still loading.
  - The cone click aims at the cone's middle; the old tip click sometimes missed the raycast. 3/3 runs pass.
