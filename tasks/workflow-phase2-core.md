# Workflow: Phase 2, core library (Rust)

Status: **in progress** (started 2026-10-13)

Goal: a Rust core library that reads and writes X4 plans losslessly, and later the catalogue, snap placement, validator and import-file writer (see [roadmap.md](roadmap.md) Phase 2). Stack: D-6.

## Steps
- [x] Install rustup + stable MSVC toolchain on H: (`RUSTUP_HOME=H:\dev\rustup`, `CARGO_HOME=H:\dev\cargo`)
- [x] Install the missing Windows SDK libraries (10.0.26100 into `D:\Programas\`)
- [x] Write `docs/dev-setup.md` (what is installed, where, and how to build)
- [x] Scaffold the Cargo workspace (`crates/x4sd-core`). No Tauri app yet; that's Phase 3.
- [x] Plan model + strict XML reader + game-style writer (`src/plan.rs`)
- [x] Round-trip tests: the fixture byte for byte; the game files with exact values, and line for line except rotation/quaternion number text
- [ ] Next (roadmap Phase 2): import-file writer, virtual file system + module catalogue, snap placement, validator

## Sub-agents
None.

## Notes / findings log
- 2026-10-13: the user pre-authorized installing tooling, on H:/D: (saved to memory and to CLAUDE.md).
- 2026-10-13: Rust 1.99.0 is installed on H:. The first build failed: from Git Bash, GNU `link` shadows MSVC; from PowerShell, link.exe wasn't found, because the Windows SDK libs were missing (KitsRoot10 = `D:\Programas\` had only Debuggers/Redist). Installed SDK 10.0.26100 with winget, and the build works from PowerShell.
- 2026-10-13: the strict reader accepts every file we have (7 player plans, the TerranScrapyard export, 116 library plans), and all values round-trip exactly.
  - Game-written files: structure and positions are byte-identical. 70+13 rotation/quaternion lines differ in text only.
  - The game's quaternion format = `%.6g`. Its rotations sometimes carry more digits than f32, so they're probably printed from doubles. We decided not to chase byte-exact angles.
  - Fixed: library positions like `-0.0001526` were rounded by the 3-decimal format. The writer now falls back to the shortest exact form.
