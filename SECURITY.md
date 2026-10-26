# Security policy

## Reporting a vulnerability
Please report vulnerabilities privately through GitHub: open
https://github.com/Thiago-Dantas/x4-station-designer/security/advisories/new
(Security tab → *Report a vulnerability*). Don't open a public issue for a security problem.

Include what you found, how to reproduce it (a plan file or steps), and which version you used.

This is a one-person hobby project. I aim to answer within 14 days, and to publish a fix and a
GitHub security advisory within 90 days. Please keep the details private until the fix is released
or the 90 days have passed; after that, disclosure is up to you. I'll credit you in the advisory
unless you'd rather not be named.

## Supported versions
Only the latest release gets security fixes. Fixes come out as a new release; the builder shows a
notice when one is available.

## Scope
`x4sd.exe` is one program: the visual builder, the command line tool (CLI) and the MCP server for AI agents. In scope:
- **Reading files from others:** plan files (`constructionplans.xml`, shared plans and import files),
  savegames and the game's data files. A crafted file that crashes the program, writes outside the
  folder it should, or runs code is a vulnerability.
- **Writing files:** the plan files it writes into the player's profile folder.
- **The live link** between the builder and the MCP server: a TCP socket on `127.0.0.1:47823`.
- **The new-version check:** one request to the GitHub API when the builder starts.
- **The release build:** `x4sd.exe` is built and attested by GitHub Actions
  ([.github/workflows/release.yml](.github/workflows/release.yml)). Check a download with
  `gh attestation verify x4sd.exe -R Thiago-Dantas/x4-station-designer`. Each release's notes link
  to a VirusTotal scan of its `x4sd.exe`.

Out of scope: the game itself, other mods, and what an AI agent decides to do with the MCP tools it
has been given.
