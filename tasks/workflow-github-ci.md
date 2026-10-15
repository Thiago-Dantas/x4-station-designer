# Workflow: GitHub security features, Scorecard, release workflow (done)

Request (2026-10-15): turn on GitHub's built-in security features, add OpenSSF Scorecard, and a GitHub Actions build/release. Releases happen only when a new tag is pushed.

- [x] Repo settings via `gh api`: Dependabot alerts + security updates, secret scanning + push protection, CodeQL default setup, private vulnerability reporting
- [x] `.github/workflows/scorecard.yml` (upstream example, actions pinned by SHA, publish_results for the badge)
- [x] `.github/workflows/release.yml`: tag `v*` → check tag = version, build on windows-latest, attest `x4sd.exe`, publish the release (notes from the tag message); manual run builds only
- [x] Verify: Scorecard run passes; release workflow run by hand (build only) passes; CodeQL default setup run passes
- [x] README Scorecard badge; docs (BUILDING.md release section, dev-setup, CLAUDE.md, decisions)

## Notes (2026-10-15)
- Runs: Scorecard ok (36 s); CodeQL default setup ok (7 min), no code alerts; Release by hand ok (7.5 min), version check, attest and publish skipped as meant. Not yet seen: a real tag run.
- Scorecard 3.9/10 at first. Zero: Maintained (repo < 90 days), Code-Review and Branch-Protection (direct pushes to main), Security-Policy (no SECURITY.md), Dependency-Update-Tool (no dependabot.yml), SAST (rises as CodeQL scans new commits), CII-Best-Practices, Fuzzing, Signed-Releases, Contributors.
- Dependabot alert #1: glib 0.18.5 (medium, VariantStrIter unsoundness, fixed in 0.20). Only Tauri's Linux GTK side uses it (`cargo tree -i glib --target all`); not in the Windows build. Its automatic update PR failed (Tauri pins gtk-rs 0.18). Left open; the user decides whether to dismiss it.
