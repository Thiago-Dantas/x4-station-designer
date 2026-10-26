# Workflow: VirusTotal scan of released exes

Status: **done** (2026-10-26; the last item checks itself on the next release)

Request (2026-10-26): scan each released `x4sd.exe` with VirusTotal, connected to the GitHub repo, and link the result from the release.

## Findings
- VirusTotal scans files, not repos; there is no repo-level score like Scorecard. **verified** (VirusTotal API v3 docs: `/files`, `/analyses`)
- `crazy-max/ghaction-virustotal` (v5.0.0) adds the link to the release notes only on a `release` event. Our releases are created by `gh release create` with the workflow's token, which doesn't start other workflows, so the action would never add the link. **verified** (its `src/main.ts` at v5.0.0). So `release.yml` calls the API directly in PowerShell.
- Free API key: 4 requests a minute, 32 MB direct upload limit; `x4sd.exe` is about 16 MB. The key is the repo secret `VT_API_KEY` (set 2026-10-26).
- v1.5.1 scanned locally 2026-10-26: 1 of 70 engines (Trapmine, `malicious.moderate.ml.score`, a machine-learning heuristic), the usual false positive for an unsigned exe.

## Steps
- [x] Repo secret `VT_API_KEY`
- [x] Test the API code locally on the v1.5.1 asset
- [x] `release.yml`: step after publishing: upload, wait for the scan, append the link to the release notes
- [x] actionlint
- [x] Scan the existing releases (v1.1.0 to v1.5.1) and add the links to their notes (all six: 1 of about 70 engines, Trapmine `malicious.moderate.ml.score`)
- [x] Docs: BUILDING.md, SECURITY.md, CLAUDE.md, decisions.md (D-26)
- [x] Commit and push
- [ ] Next tagged release: check the step ran and the link is in the notes
