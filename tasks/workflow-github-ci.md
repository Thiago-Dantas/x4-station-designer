# Workflow: GitHub security features, Scorecard, release workflow

Request (2026-10-15): turn on GitHub's built-in security features, add OpenSSF Scorecard, and a GitHub Actions build/release. Releases happen only when a new tag is pushed.

- [ ] Repo settings via `gh api`: Dependabot alerts + security updates, secret scanning + push protection, CodeQL default setup, private vulnerability reporting
- [ ] `.github/workflows/scorecard.yml` (upstream example, actions pinned by SHA, publish_results for the badge)
- [ ] `.github/workflows/release.yml`: tag `v*` → check tag = version, build on windows-latest, attest `x4sd.exe`, publish the release (notes from the tag message); manual run builds only
- [ ] Verify: Scorecard run passes; release workflow run by hand (build only) passes; CodeQL default setup run passes
- [ ] README Scorecard badge; docs (BUILDING.md release section, dev-setup, CLAUDE.md, decisions)
