# Workflow: raise the OpenSSF Scorecard score

Status: **open** (created 2026-10-15)

Request (2026-10-15): work through the Scorecard checks after the setup in [workflow-github-ci.md](workflow-github-ci.md) (D-26). First score: **3.9/10**.

## How to check
- Score and checks: `curl -s https://api.scorecard.dev/projects/github.com/Thiago-Dantas/x4-station-designer` (JSON; `checks[].score`, `reason`, `details`). The same results are in the repo's Security tab → Code scanning (tool "Scorecard").
- New score after a change: push to `main` (the workflow runs on every push), or `H:\dev\gh\bin\gh.exe workflow run scorecard.yml`, then wait for the run and read the API.
- Lint workflow files: `H:\dev\actionlint\actionlint.exe` from the repo root. Pin every action by commit SHA with the version in a comment (Pinned-Dependencies is 10 now; keep it).
- Facts below marked *(hypothesis)* are from memory of how Scorecard works; check the Scorecard docs (github.com/ossf/scorecard/blob/main/docs/checks.md) before relying on them.

## Quick wins (no decisions needed)
- [ ] **Security-Policy (0):** `SECURITY.md` at the root: how to report (GitHub private vulnerability reporting is on: Security tab → Report a vulnerability), what's in scope (the exe, CLI, MCP server; it reads game files and writes plan files), supported versions (the latest release).
- [ ] **Dependency-Update-Tool (0):** `.github/dependabot.yml` for `cargo` (root), `npm` (`/ui`) and `github-actions`. Group updates per ecosystem and run monthly to keep PR noise low. Note: Tauri pins gtk-rs 0.18, so a glib bump will keep failing; ignore `glib`/`gtk`-family majors if they spam.
- [ ] **Signed-Releases (0):** Scorecard looks for signature files among the release assets *(hypothesis: `*.sigstore.json`, `*.intoto.jsonl`, `*.sig`, `*.asc`, `*.minisig`)*. In `release.yml`, also upload the attestation bundle (`actions/attest` output `bundle-path`) as `x4sd.exe.sigstore.json`. Only counts from the next tagged release on.
- [ ] **CI (roadmap: "Automated checks (CI) for the parts that don't need the game installed"):** a `ci.yml` on push and pull_request: `cargo test --workspace --locked` (game-data tests skip without the game), `npm --prefix ui ci`, `npm --prefix ui run check`, actionlint. On `windows-latest` (the crates use Windows APIs; check whether Linux works first, it's faster). Minimal `permissions: contents: read`. CI-Tests (-1 now) only scores pull requests, so this helps the score only with PRs.

## Needs checking
- [ ] **SAST (0):** CodeQL *default setup* is on, but it's a repo setting, not a workflow file. Check whether Scorecard counts it with direct pushes to `main` (*hypothesis: it looks at CodeQL check runs on merged PRs and at workflow files*). If not, the fix is CodeQL *advanced setup* (a `codeql.yml` workflow, then default setup is turned off). Ask the user before switching.
- [ ] **Vulnerabilities (8, one finding):** glib 0.18.5 (RUSTSEC, VariantStrIter unsoundness). Linux-only (Tauri's GTK side), not in the Windows exe (`cargo tree -i glib --target all`). Scorecard scans `Cargo.lock` with OSV. Check whether an `osv-scanner.toml` with an ignore entry and a reason is honoured *(hypothesis)*. Separately, the user decides whether to dismiss Dependabot alert #1 as "vulnerable code not used".
- [ ] **Packaging (-1):** check what Scorecard counts as a packaging workflow and whether `release.yml` (`gh release create`) can qualify. Low priority.
- [ ] **Fuzzing (0):** low priority. The plan XML reader takes files from the player and from other people (shared plans), so a `cargo-fuzz` target on `plan` parsing has real value. Check what Scorecard detects for Rust before doing it for the score.

## User decisions (ask before acting)
- [ ] **Branch-Protection (0) and Code-Review (0):** full points need pull requests with reviews, which changes how work lands (now direct pushes to `main`, see the "commit as you go" preference). A middle way: a ruleset on `main` that blocks force pushes and deletion, with no PR requirement. That's a repo setting; ask first.
- [ ] **CII-Best-Practices (0):** the OpenSSF Best Practices badge (bestpractices.dev). The user signs in with GitHub and fills in the questionnaire; the agent can draft the answers from the repo.

## Nothing to do
- **Maintained (0):** needs the repo to be 90+ days old; it rises on its own with activity.
- **Contributors (0):** needs contributors from several organisations.

## Finish
- [ ] Re-read the score; record before/after per check in the notes below
- [ ] Docs: `docs/decisions.md` (extend D-26 or a new entry), `CLAUDE.md` layout rows for new files, `BUILDING.md` if the release or CI steps change, roadmap

## Notes
