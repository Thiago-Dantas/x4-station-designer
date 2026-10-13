# Workflow: Phase 2, core library (Rust)

Status: **done** (2026-10-13)

Goal: a Rust core library that reads and writes X4 plans losslessly, and later the catalogue, snap placement, validator and import-file writer (see [roadmap.md](roadmap.md) Phase 2). Stack: D-6.

## Steps
- [x] Install rustup + stable MSVC toolchain on H: (`RUSTUP_HOME=H:\dev\rustup`, `CARGO_HOME=H:\dev\cargo`)
- [x] Install the missing Windows SDK libraries (10.0.26100 into `D:\Programas\`)
- [x] Write `docs/dev-setup.md` (what is installed, where, and how to build)
- [x] Scaffold the Cargo workspace (`crates/x4sd-core`). No Tauri app yet; that's Phase 3.
- [x] Plan model + strict XML reader + game-style writer (`src/plan.rs`)
- [x] Round-trip tests: the fixture byte for byte; the game files with exact values, and line for line except rotation/quaternion number text
- [x] Shared error type (`x4sd_core::Error`)
- [x] `vfs` (archive reader), `game` (extensions with id/folder/enabled/save, profile discovery)
- [x] `catalog`: index merge (full `<index>` and `<diff><add sel='/index'>`), module → component → snaps, `required_patches`
- [x] `geometry` (Quat, Transform, `attach`, `fit`), `validate`, `import`
- [x] `tests/game_data.rs` against the real install: 5 tests pass. Clippy is clean.
- [x] `text` (name resolution) + `Catalog::station_modules` (287 modules, all names resolved, 0.12 s in debug)
- [x] The user decided OD-6 → D-7 (live link) and OD-1 → D-8 (MCP + CLI)
- [x] Phase 2 done. Phase 3 steps are in the roadmap.

## Sub-agents
None.

## Notes / findings log
- 2026-10-13: the user pre-authorized installing tooling, on H:/D: (saved to memory and to CLAUDE.md).
- 2026-10-13: Rust 1.99.0 is installed on H:. The first build failed: from Git Bash, GNU `link` shadows MSVC; from PowerShell, link.exe wasn't found, because the Windows SDK libs were missing (KitsRoot10 = `D:\Programas\` had only Debuggers/Redist). Installed SDK 10.0.26100 with winget, and the build works from PowerShell.
- 2026-10-13: the strict reader accepts every file we have (7 player plans, the TerranScrapyard export, 116 library plans), and all values round-trip exactly.
  - Game-written files: structure and positions are byte-identical. 70+13 rotation/quaternion lines differ in text only.
  - The game's quaternion format = `%.6g`. Its rotations sometimes carry more digits than f32, so they're probably printed from doubles. We decided not to chase byte-exact angles.
  - Fixed: library positions like `-0.0001526` were rounded by the 3-decimal format. The writer now falls back to the shortest exact form.
- 2026-10-13: catalogue findings:
  - Extension id ≠ folder. The profile `content.xml` overrides `enabled`.
  - `required_patches` must skip `save="false"` extensions. The game left RecycleShipsAndStations out of "megashipyard terran"'s patches although the plan uses `recyclemodule_macro`.
  - Mod indexes are full `<index>` or `<diff><add>`. `subst_NN.cat` replaces base paths.
  - Load time: 2.3 s debug, 0.55 s release (80 extensions).
- 2026-10-13: validator tolerance raised from 2 to 5 cm. Egosoft's `arg_wharf` has 14 entries 1–3.5 cm off; real misplacements are at least 2.4 m. `fit` uses atan2 for the axis angle (acos lost precision near 0°).
