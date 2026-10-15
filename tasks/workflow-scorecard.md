# Workflow: raise the OpenSSF Scorecard score

Status: **open** (created 2026-10-15)

Request (2026-10-15): work through the Scorecard checks after the setup in [workflow-github-ci.md](workflow-github-ci.md) (D-26). First score: **3.9/10**.

## How to check
- Score and checks: `curl -s https://api.scorecard.dev/projects/github.com/Thiago-Dantas/x4-station-designer` (JSON; `checks[].score`, `reason`, `details`). The same results are in the repo's Security tab → Code scanning (tool "Scorecard").
- New score after a change: push to `main` (the workflow runs on every push), or `H:\dev\gh\bin\gh.exe workflow run scorecard.yml`, then wait for the run and read the API.
- Lint workflow files: `H:\dev\actionlint\actionlint.exe` from the repo root. Pin every action by commit SHA with the version in a comment (Pinned-Dependencies is 10 now; keep it).
- Facts below marked *(hypothesis)* are from memory of how Scorecard works; check the Scorecard docs (github.com/ossf/scorecard/blob/main/docs/checks.md) before relying on them.

## Quick wins (no decisions needed)
- [x] **Security-Policy (0):** `SECURITY.md` at the root: how to report (GitHub private vulnerability reporting is on: Security tab → Report a vulnerability), what's in scope (the exe, CLI, MCP server; it reads game files and writes plan files), supported versions (the latest release).
- [x] **Dependency-Update-Tool (0):** `.github/dependabot.yml` for `cargo` (root), `npm` (`/ui`) and `github-actions`. Group updates per ecosystem and run monthly to keep PR noise low. Note: Tauri pins gtk-rs 0.18, so a glib bump will keep failing; ignore `glib`/`gtk`-family majors if they spam.
- [x] **Signed-Releases (0):** Scorecard looks for signature files among the release assets (**verified** in checks.md: `*.minisig`, `*.asc`, `*.sig`, `*.sign`, `*.sigstore`, `*.sigstore.json` = 8 points per release; `*.intoto.jsonl` = 10; score = average over the last 5 releases with assets, floored). In `release.yml`, also upload the attestation bundle (`actions/attest` output `bundle-path`) as `x4sd.exe.sigstore.json`. Only counts from the next tagged release on.
- [x] **CI (roadmap: "Automated checks (CI) for the parts that don't need the game installed"):** a `ci.yml` on push and pull_request: `cargo test --workspace --locked` (game-data tests skip without the game), `npm --prefix ui ci`, `npm --prefix ui run check`, actionlint. On `windows-latest` (the crates use Windows APIs; check whether Linux works first, it's faster). Minimal `permissions: contents: read`. CI-Tests (-1 now) only scores pull requests, so this helps the score only with PRs.

## Needs checking
- [x] **SAST (0):** CodeQL *default setup* is on, but it's a repo setting, not a workflow file. Check whether Scorecard counts it with direct pushes to `main` (*hypothesis: it looks at CodeQL check runs on merged PRs and at workflow files*). If not, the fix is CodeQL *advanced setup* (a `codeql.yml` workflow, then default setup is turned off). Ask the user before switching.
- [x] **Vulnerabilities (8, one finding):** glib 0.18.5 (RUSTSEC, VariantStrIter unsoundness). Linux-only (Tauri's GTK side), not in the Windows exe (`cargo tree -i glib --target all`). Scorecard scans `Cargo.lock` with OSV. Check whether an `osv-scanner.toml` with an ignore entry and a reason is honoured *(hypothesis)*. Separately, the user decides whether to dismiss Dependabot alert #1 as "vulnerable code not used".
- [x] **Packaging (-1):** check what Scorecard counts as a packaging workflow and whether `release.yml` (`gh release create`) can qualify. Low priority.
- [x] **Fuzzing (0):** low priority. The plan XML reader takes files from the player and from other people (shared plans), so a `cargo-fuzz` target on `plan` parsing has real value. Check what Scorecard detects for Rust before doing it for the score.

## User decisions (ask before acting)
- [x] **Branch-Protection (0) and Code-Review (0):** full points need pull requests with reviews, which changes how work lands (now direct pushes to `main`, see the "commit as you go" preference). A middle way: a ruleset on `main` that blocks force pushes and deletion, with no PR requirement. That's a repo setting; ask first.
- [x] **CII-Best-Practices (0):** *(user, 2026-10-15: skip for now)* the OpenSSF Best Practices badge (bestpractices.dev). The user signs in with GitHub and fills in the questionnaire; the agent can draft the answers from the repo.

## Nothing to do
- **Maintained (0):** needs the repo to be 90+ days old; it rises on its own with activity.
- **Contributors (0):** needs contributors from several organisations.

## Finish
- [ ] Re-read the score; record before/after per check in the notes below
- [x] Docs: `docs/decisions.md` (extend D-26 or a new entry), `CLAUDE.md` layout rows for new files, `BUILDING.md` if the release or CI steps change, roadmap

## Notes
### 2026-10-15: quick wins and checks
- **Before:** 3.9/10 (Scorecard API, 2026-10-15T20:23Z, commit ee6feaf). Code-Review 0, Packaging -1, SAST 0, Binary-Artifacts 10, Dangerous-Workflow 10, Maintained 0, CI-Tests -1, Token-Permissions 10, Pinned-Dependencies 10, License 10, Security-Policy 0, CII-Best-Practices 0, Signed-Releases 0, Dependency-Update-Tool 0, Vulnerabilities 8 (**two** findings: RUSTSEC-2024-0429 glib and RUSTSEC-2024-0370 proc-macro-error), Branch-Protection 0, Contributors 0, Fuzzing 0.
- Hypotheses checked against docs/checks.md and the Scorecard source (main, 2026-10-15):
  - **SAST, verified (source `checks/raw/sast.go`, `checks/evaluation/sast.go`):** CodeQL counts either as check runs on merged PRs (none here: "no pull requests merged into dev branch") or as a workflow step `uses: github/codeql-action/analyze` (= 10). Default setup has no workflow file, and `upload-sarif` in `scorecard.yml` doesn't match. So with direct pushes, only CodeQL *advanced setup* scores. Waiting on the user's decision.
  - **Vulnerabilities, verified:** checks.md says an `osv-scanner.toml` next to the manifest with an ID and reason is honoured. Both findings are Linux-only (`cargo tree -i glib --target all`; nothing on the Windows host). Added `osv-scanner.toml` (both IDs, with reasons); local `osv-scanner scan source -r .` 2.7.0: before 2 findings, after "No issues found".
  - **Packaging, verified (source `checks/fileparser/github_workflow.go` `IsPackagingWorkflow`):** only registry publishing counts (npm/maven/gradle/gem/nuget/docker push, pypi, goreleaser, `cargo publish`, ko, semantic-release, hex). `gh release create` doesn't. A Windows exe has no fitting registry; -1 is "inconclusive" and isn't counted in the total. Nothing to do.
  - **Fuzzing, verified (checks.md):** Rust counts only through OSS-Fuzz or ClusterFuzzLite (`.clusterfuzzlite/` + a workflow); language-native fuzz functions are detected only for Go, Haskell, JS/TS, Erlang, C#/F#. A `cargo-fuzz` target alone scores nothing. Waiting on the user's decision.
  - **Signed-Releases, verified:** see the item. The existing four releases stay unsigned (v1.4.0 was uploaded by hand, no attestation), so one signed release scores floor(8/5) = 1, and it climbs to 8 after five.
- Done: `SECURITY.md` (f1b0681), `.github/dependabot.yml` (380572b; Dependabot's version-update runs for cargo, npm and actions started fine), release bundle `x4sd.exe.sigstore.json` (30b6f12; first real check is the next tag), `ci.yml` (caad0e5), `osv-scanner.toml` (a9dea3b). osv-scanner 2.7.0 installed on H: (dev-setup).
- SECURITY.md promises an answer within 14 days and a fix/advisory within 90 days (Scorecard's last point wants timeline numbers). The user can change those numbers.
- **User decisions (2026-10-15):** SAST: switch to advanced setup. Branch-Protection: ruleset without PRs. CII badge: skip for now. Fuzzing: skip (low priority).
  - CodeQL: default setup turned off (`gh api -X PATCH .../code-scanning/default-setup -f state=not-configured`), then `codeql.yml` pushed (73973ae): actions, javascript-typescript, python, rust, all `build-mode: none` (the languages default setup had). First run OK, 4 analyses uploaded, 0 alerts.
  - Ruleset 25195674 "main: no force push or deletion" (rules `deletion`, `non_fast_forward`, default branch, no bypass actors) = Branch-Protection Tier 1, 3/10 (checks.md). Direct pushes still work (this commit). Code-Review stays 0 without PRs.
- CI: first run OK (test 4m09s on windows-latest: 51 passed, 2 ignored; game-data tests return early; actionlint 10 s).
