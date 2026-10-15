# Building x4sd

How to build `x4sd.exe` (the builder, CLI and MCP server in one file) from source. Windows 10 or 11, x64.

## Quick way: `build.bat`
From a clone of the repo, run:
```bat
build.bat
```
It installs whatever is missing with `winget`, then builds. Installers ask for administrator rights. The result is `target\release\x4sd.exe`, a single self-contained exe.

What it installs, only when missing:

| Tool | winget package | Why |
|------|----------------|-----|
| Visual Studio Build Tools, C++ workload (includes the Windows SDK) | `Microsoft.VisualStudio.BuildTools` | Rust's MSVC linker and the Windows libraries |
| Windows SDK (when Visual Studio is there but the SDK is not) | `Microsoft.WindowsSDK.10.0.26100` | `kernel32.lib` and the other Windows libraries |
| Rust (rustup, stable MSVC toolchain) | `Rustlang.Rustup` | the program |
| Node.js LTS | `OpenJS.NodeJS.LTS` | bundling the builder UI |

The first build downloads the Rust crates and npm packages and takes a few minutes. If a tool was just installed and the script says it is still not on `PATH`, open a new terminal and run `build.bat` again.

## Manual way
1. Install the tools from the table above: Rust stable with the MSVC toolchain, the MSVC C++ build tools with a Windows SDK, and Node.js 22 or newer. The WebView2 runtime is needed to run the builder; it comes with Windows 11 and current Windows 10.
2. In **PowerShell** or **cmd** (not Git Bash, see below), from the repo root:
   ```powershell
   npm --prefix ui ci              # UI dependencies, once
   npm --prefix ui run build       # builder UI -> ui\dist (must exist before cargo build)
   cargo build -p x4sd --release   # target\release\x4sd.exe, with ui\dist embedded
   ```

## Tests
```powershell
cargo test --workspace     # unit, round-trip and game-data tests
npm --prefix ui run check  # TypeScript type check
```
The game-data tests need X4: Foundations installed. They find it like the builder does (`X4_GAME_DIR` overrides) and skip when it isn't there. The smoke tests that drive the built exe are in [docs/dev-setup.md](docs/dev-setup.md).

## Troubleshooting
- **`link: missing operand`**: you built from Git Bash. Its `/usr/bin/link.exe` hides the MSVC linker. Build from PowerShell or cmd.
- **`LINK : fatal error LNK1181: cannot open input file 'kernel32.lib'`**: the Windows SDK is missing. Install it (see the table) and build again.
- **`cargo build` fails on a missing `ui/dist`**: build the UI first (`npm --prefix ui run build`).
