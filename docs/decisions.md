# Decisions

## Decided
| ID | Date | Decision | Why |
|----|------|----------|-----|
| D-1 | 2026-10-13 | Project state lives in `.md` files (`tasks/`, `docs/`), not in session history | This is a long-term project that runs over many sessions |
| D-2 | 2026-10-13 | The game install and the player profile are read-only by default; profile writes need a backup first | Protect the user's game and saved plans |
| D-3 | 2026-10-13 | Git, hosted as a private GitHub repo | User decision (was OD-4) |
| D-4 | 2026-10-13 | Use REA on `X4.exe` to answer whether plan `offset` is authoritative or recomputed from connections | User decision. **Settled without REA** the same day by in-game test C: the offset is authoritative. REA stays available for later questions. |
| D-5 | 2026-10-13 | **Primary injection route = option B**: the tool writes a single-plan file into `<profile>\constructionplan\`, and the player uses Import in the build menu | User decision (was OD-3). Verified in game: vanilla, no restart, never touches `constructionplans.xml`. Option D (a live mod) stays a later add-on. |
| D-6 | 2026-10-13 | **Rust + Tauri.** The core library, CLI and MCP server (official Rust SDK) are in Rust. The visual builder is a web UI (Three.js for 3D) inside Windows' built-in WebView2, bundled into one exe. | User decision (was OD-2; replaces a short-lived TypeScript pick). Meets the requirements: a compiled, self-contained exe of about 10 MB, fast, with nothing for users to install. WebView2 ships with Windows 10/11 (v155 on the dev machine). The snap math lives once in Rust and can be compiled to WASM for drag-time snapping in the UI. The Python scripts in `tools/` and `research/` stay as reference implementations. |

## Open
| ID | Question | Options / notes |
|----|----------|-----------------|
| OD-1 | Agent interface: CLI, MCP, or both? | Both can wrap one core library (see [goals.md](goals.md)). MCP gives agents typed tools; a CLI is scriptable and easy to test. |
| OD-6 | How the visual builder and the tooling share a design | With Tauri (D-6), an agent's MCP client starts its own copy of the exe (stdio). So either (a) that MCP process talks to the running builder over a local socket and changes show up live, or (b) both load and save a shared design file and the builder watches it. Decide at the start of Phase 3. |
