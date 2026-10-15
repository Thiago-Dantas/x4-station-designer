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

GitHub Actions runs both on every push to `main` and on pull requests ([.github/workflows/ci.yml](.github/workflows/ci.yml)), without the game, and lints the workflow files with actionlint.

## Releases
Releases are built by GitHub Actions ([.github/workflows/release.yml](.github/workflows/release.yml)), only when a tag `vX.Y.Z` is pushed:
1. Set the version in `crates/x4sd/Cargo.toml`, `crates/x4sd-core/Cargo.toml`, `crates/x4sd/tauri.conf.json` and `ui/package.json` (and their lock files), commit, push.
2. Tag with the release notes as the tag message, and push the tag:
   ```powershell
   git tag -a v1.5.0 -F release-notes.md
   git push origin v1.5.0
   ```
The workflow stops if the tag doesn't match the versions in `Cargo.toml` and `tauri.conf.json`. Otherwise it builds `x4sd.exe` on a clean Windows runner, attests it (a signed record that this workflow built it from that commit) and publishes the release with the exe and the attestation bundle `x4sd.exe.sigstore.json`. Anyone can check a downloaded exe with `gh attestation verify x4sd.exe -R Thiago-Dantas/x4-station-designer` (add `--bundle x4sd.exe.sigstore.json` to use the downloaded bundle instead of fetching it). Running the workflow by hand from the Actions tab only builds, as a check.

## Troubleshooting
- **`link: missing operand`**: you built from Git Bash. Its `/usr/bin/link.exe` hides the MSVC linker. Build from PowerShell or cmd.
- **`LINK : fatal error LNK1181: cannot open input file 'kernel32.lib'`**: the Windows SDK is missing. Install it (see the table) and build again.
- **`cargo build` fails on a missing `ui/dist`**: build the UI first (`npm --prefix ui run build`).
