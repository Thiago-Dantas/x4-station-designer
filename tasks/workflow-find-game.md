# Workflow: find the X4 install on any machine (done 2026-10-15)

User request (2026-10-15): game files are found only on the dev machine; players have X4 elsewhere.
Scope agreed: Steam anywhere, GOG, a folder picker in the builder that is remembered, dev code uses the same lookup. Decision D-24.

Findings: `find_game_dir()` already read `X4_GAME_DIR`, then `libraryfolders.vdf` under `C:\Program Files (x86)\Steam` only.
The `F:\` path was hardcoded only in `tests/game_data.rs`, `examples/zonecheck.rs` (with the profile id), `research/snapcheck.py`.

- [x] Core `game.rs`: order `X4_GAME_DIR` > saved choice > Steam (registry `SteamPath`, else Program Files) > GOG (registry `GOG.com\Games\*\path`); only folders with `01.cat` count (`is_game_dir`). `winreg` (already in the lock file through Tauri) as a Windows-only dependency.
- [x] Core: `save_game_dir()` writes the choice to `%APPDATA%\x4sd\game-dir.txt` after checking it
- [x] Builder: when the install isn't found (or `--game-dir` / `X4_GAME_DIR` has no `01.cat`), ask, folder dialog, `set_game_dir` command, load again; a wrong folder is asked again with the reason
- [x] MCP: looks again at load time (a folder picked in the builder after the server started)
- [x] `tests/game_data.rs`, `zonecheck.rs` use `find_game_dir()` (and `find_profile_dir()`); `snapcheck.py` reads `X4_GAME_DIR`
- [x] Verify (2026-10-15):
  - lookup on this machine: `SteamPath` read, 5 Steam libraries listed, X4 found on F:; GOG keys read (8 entries of another game); `save_game_dir(C:\Windows)` refused
  - `game_data` test `resolves_base_dlc_and_mod_modules` passes with `X4_GAME_DIR` unset
  - builder with `--game-dir C:\Windows` and a temp `APPDATA` (scratch CDP test, deleted): native ask at start, No → status "X4 install not found"; with the dialogs stubbed at `window.fetch` (Tauri's `invoke` is read-only): ask, open (directory), wrong folder → asked again with "has no X4 game data", real folder → 287 modules loaded, file saved
  - a saved folder (junction to the install) wins over Steam on the next start
  - not tested: X4 bought on GOG (no such install here)
- [x] Docs: usage.md, README.md, x4-game-data.md, decisions.md (D-24), CLAUDE.md, roadmap
