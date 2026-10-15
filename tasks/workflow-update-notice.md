# Workflow: new-version notice (user request)

Status: **done** (2026-10-14)

User, 2026-10-14: "make it evergreen, when a new release shows up on github notify the user, only check it once per version per app start, if a user doesnt want to download 1.1 only notify him again when we release 1.2"

## Design (D-19)
- Builder only: the UI fetches `api.github.com/repos/Thiago-Dantas/x4-station-designer/releases/latest` once per start; errors and no network show nothing.
- Shown when the release tag is newer than `getVersion()` (tauri.conf.json) and newer than the skipped tag (`localStorage` `x4sd.skipVersion`).
- Bar above the status line: *Download* (opens the release page, `tauri-plugin-opener`, permission `opener:default`), *Skip this version*.
- Assumption: any later release (1.1.1 too) counts as "the next one" after a skip.

## Steps
- [x] Rust: `tauri-plugin-opener`, capability `opener:default`
- [x] UI: `@tauri-apps/plugin-opener`, banner markup and style, the check in main.ts
- [x] Check (scratch CDP, exe built as 0.9.0 against the real v1.0.0 release): banner shows "1.0.0 is out"; Skip stores v1.0.0 and hides it; next start nothing; skipped v0.9.5 → shown again; Download raises no error
- [x] Docs: usage.md, README, decisions D-19
