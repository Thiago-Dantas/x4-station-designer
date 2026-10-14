# Development setup

What is installed on the dev machine and how to build. Installs go on H: or D:, not C: (user preference).

## Installed tooling
| Tool | Version | Location | Installed by |
|------|---------|----------|--------------|
| Rust (rustup, cargo, stable MSVC toolchain) | rustc 1.99.0 | `H:\dev\rustup` (`RUSTUP_HOME`), `H:\dev\cargo` (`CARGO_HOME`, `bin` on the user PATH) | rustup-init, 2026-10-13 |
| Visual Studio Community 2026 (C++ x64 tools, MSVC 14.51) | 18.10 | `H:\Program Files\Visual Studio` | already present |
| Windows SDK | 10.0.26100 | `D:\Programas\` (the machine's registered `KitsRoot10`) | `winget install Microsoft.WindowsSDK.10.0.26100`, 2026-10-13. Before that only the SDK's Debuggers/Redist parts were there, and Rust couldn't link. |
| WebView2 runtime | 155 | system | ships with Windows (needed by Tauri) |
| Node.js | 24.18 | system | already present (for bundling the UI in Phase 3) |
| Python | 3.11 | system | already present (research scripts only) |
| gifski | 1.34.0 | `H:\dev\gifski\gifski.exe` (not on PATH) | `win/gifski.exe` from the GitHub release tarball, 2026-10-14 (README GIFs; the `ffmpeg` on PATH is a 2013 build without `palettegen`) |

`RUSTUP_HOME` and `CARGO_HOME` are permanent user environment variables. Terminals opened before the install don't see them or the PATH change.

## Building and testing
Use **PowerShell** (or cmd):
```powershell
npm --prefix ui install            # once: UI dependencies (dev only; not shipped)
npm --prefix ui run build          # the builder UI -> ui/dist (must exist before cargo build)
cargo build -p x4sd --release      # target\release\x4sd.exe: builder + CLI + MCP, ui/dist embedded
cargo test --workspace             # unit, round-trip and game-data tests
cargo test -p x4sd-core -- --nocapture   # also prints the round-trip report
npm --prefix ui run check          # TypeScript type check
```
Smoke tests of the built exe (each runs the real program):
```powershell
python -I research\mcp_smoke.py target\release\x4sd.exe <scratch profile dir>   # MCP, like an agent
python -I research\ui_smoke.py  target\release\x4sd.exe <scratch dir>           # builder, through WebView2's DevTools protocol; saves screenshots
python -I research\link_smoke.py target\release\x4sd.exe <scratch dir>           # live link: an agent edits the open builder (scratch profile)
python -I research\texture_smoke.py target\release\x4sd.exe <scratch dir> [plan]  # textures off vs on: load time, GPU memory, fps
python -I research\snap_pick_smoke.py target\release\x4sd.exe <scratch dir>     # choosing the connecting snap: labels, C/T keys, preview, own-snap cones when moving, Connected by
```
README GIFs (`docs/media/`): `python -I research\record_gifs.py target\release\x4sd.exe <out dir> [build planner agent analysis]` plays each scene in a fresh builder (page fixed at 1440×810, a drawn cursor) and encodes `<out dir>\<scene>.gif` with gifski (`--gifski PATH`, default `H:\dev\gifski\gifski.exe`). Copy the ones you want into `docs/media/`. The `agent` scene needs no other builder open; `analysis` opens the player's TerranScrapyard plan.
- **Native dialogs block CDP scripts** (user remark, 2026-10-14). The builder's prompts (enlarge the plot, unsaved changes, replace an export) are Windows dialogs that CDP can't click, so a script that triggers one waits forever. Avoid them: set the largest plot (10 km per side) before Place all or Optimize, start a fresh builder rather than opening a plan over unsaved changes, and end with `app.terminate()` (no close prompt).

### Builder stack notes
- **Tauri 2.x stable** (`tauri` 2.12, `@tauri-apps/api` 2.12). Tauri 3 was alpha in 2026-10.
  - The crate feature `custom-protocol` is on by default in `crates/x4sd`. That is Tauri's production mode: it embeds `ui/dist`, so a plain `cargo build` gives the self-contained exe without the Tauri CLI.
- The UI is Vite 8 + TypeScript 7 + Three.js 0.186 in `ui/`.
  - The Tauri CLI (`npx tauri`) is only needed to regenerate icons in `crates/x4sd/icons/`.
  - Generated `crates/x4sd/gen/` is git-ignored.
- **Coordinates:** the game is left-handed (Y up) and Three.js right-handed. `ui/src/view3d.ts` mirrors Z, so a position `(x, y, z)` becomes `(x, y, -z)`, and a quaternion `(w, x, y, z)` becomes `(w, -x, -y, z)`.
- **Console window:** `x4sd.exe` stays a console program (for the CLI and MCP). In builder mode it detaches from a console that Windows created just for it (double-click), so a terminal it was started from is kept.
- **UI tests use their own WebView2 profile** (`WEBVIEW2_USER_DATA_FOLDER` = `%TEMP%\x4sd-ui-tests-webview2`), so they neither collide with a builder the user has open nor change its remembered settings. They all use one DevTools port, because WebView2 refuses to start (and Tauri panics) while a previous instance's processes still hold the profile with other arguments.
- **`mcp_smoke.py` and `link_smoke.py` refuse to run while a builder is open:** through the live link they would edit that builder's design.
- **Drag and drop:** `dragDropEnabled: false` in `tauri.conf.json`. Tauri's window-level file-drop handler otherwise swallows HTML5 drag and drop on Windows (palette → view, build-order rows). The UI test checks the handlers with synthetic drag events; the native path can only be checked by hand.
- **UI testing:** WebView2 accepts `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=N`. `research/ui_smoke.py` uses that for clicks, JavaScript evaluation and screenshots, and finds snap cones by their colour.
**Don't build from Git Bash.** Its `/usr/bin/link.exe` (GNU coreutils) shadows the MSVC linker and the build fails with `link: missing operand`.

The round-trip test also checks the game files in `research/extracted/` when they exist (extract them with `tools/x4cat.py`; see [x4-game-data.md](x4-game-data.md)). On a fresh clone it skips them and says so.
