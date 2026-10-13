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

`RUSTUP_HOME` and `CARGO_HOME` are permanent user environment variables. Terminals opened before the install don't see them or the PATH change.

## Building and testing
Use **PowerShell** (or cmd):
```powershell
cargo test -p x4sd-core            # unit tests + round-trip tests
cargo test -p x4sd-core -- --nocapture   # also prints the round-trip report
```
**Don't build from Git Bash.** Its `/usr/bin/link.exe` (GNU coreutils) shadows the MSVC linker and the build fails with `link: missing operand`.

The round-trip test also checks the game files in `research/extracted/` when they exist (extract them with `tools/x4cat.py`; see [x4-game-data.md](x4-game-data.md)). On a fresh clone it skips them and says so.
