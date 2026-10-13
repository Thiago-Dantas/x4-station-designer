# Decisions

## Decided
| ID | Date | Decision | Why |
|----|------|----------|-----|
| D-1 | 2026-10-13 | Project state lives in `.md` files (`tasks/`, `docs/`), not in session history | This is a long-term project that runs over many sessions |
| D-2 | 2026-10-13 | The game install and the player profile are read-only by default; profile writes need a backup first | Protect the user's game and saved plans |
| D-3 | 2026-10-13 | Git, hosted as a private GitHub repo | User decision (was OD-4) |
| D-4 | 2026-10-13 | Use REA on `X4.exe` to answer whether plan `offset` is authoritative or recomputed from connections | User decision. It is the first RE target. |

## Open
| ID | Question | Options / notes |
|----|----------|-----------------|
| OD-1 | Agent interface: CLI, MCP, or both? | Both can wrap one core library (see [goals.md](goals.md)). MCP gives agents typed tools; a CLI is scriptable and easy to test. |
| OD-2 | Implementation language | Python (fast XML and data work) or TypeScript (shares code with a web UI and the MCP SDK). Decide after Phase 1. |
| OD-3 | How plans get into the game | See [x4-construction-plans.md](x4-construction-plans.md#injection-options). Decide after the Phase 1 experiments. |
| OD-5 | Visual designer technology | Web (Three.js) or desktop. Not needed until Phase 4. |
