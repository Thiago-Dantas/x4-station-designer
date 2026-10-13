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
| D-7 | 2026-10-13 | **Live link between builder and agents.** The builder app is the hub. An agent's MCP process (its own copy of the exe) connects to the running builder over a local socket and edits the open design, so changes show up live and the user can drag alongside. Without a running builder, the MCP process works on design files headless. | User decision (was OD-6). |
| D-8 | 2026-10-13 | **Both MCP and CLI from one exe:** `x4sd` opens the builder, `x4sd mcp` serves MCP over stdio, `x4sd <command>` for scripts. All call the same core functions. | User decision (was OD-1). |

## Open
| ID | Question | Options / notes |
|----|----------|-----------------|
