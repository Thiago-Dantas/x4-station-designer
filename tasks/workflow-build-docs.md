# Workflow: build instructions and build script (done)

Request (2026-10-15): a separate `.md` with build instructions (README gets at most a mention), a `.bat` that installs the dependencies and builds, and an answer on GitHub's repo safety checks.

- [x] `BUILDING.md` at the root
- [x] `build.bat`: winget installs (MSVC Build Tools + Windows SDK, Rust, Node.js) when missing, then UI + exe build
- [x] README "Building from source" cut to a pointer; link from `docs/dev-setup.md`; `CLAUDE.md` layout rows
- [x] Verify: run `build.bat` on the dev machine (tools present, so it skips installs and builds); winget IDs checked with `winget show`
- [x] Answer: GitHub security features (no repo settings changed)
