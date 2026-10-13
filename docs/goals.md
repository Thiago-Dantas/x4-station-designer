# Goals

## Vision
A station designer that runs outside X4. Humans and AI agents use it to design station layouts, validate them against real game data, and put them into the game as normal construction plans.

## Goals
| ID | Goal | Notes |
|----|------|-------|
| G1 | Read and write X4 construction plans with no data loss | Round-trip `constructionplans.xml` byte-for-byte or semantically |
| G2 | Get a plan into the game | Mechanism to be chosen; see [x4-construction-plans.md](x4-construction-plans.md#injection-options) |
| G3 | Know the module catalogue from game data | Module macros, snap connections, race/DLC, size, cost, names |
| G4 | Validate plans before they reach the game | Connection compatibility, missing DLC, unknown macros, overlaps |
| G5 | Agents can drive the designer | CLI and/or MCP server; see open decision OD-1 |
| G6 | Visual designer | Later phase; the UI technology is still open |

## Non-goals (for now)
- Editing savegames
- Placing or building stations in a running game automatically (it may come back later through a mod)

## Proposed architecture (not decided)
```
           +---------------------------+
           |  core library             |
           |  - plan model + XML I/O   |
           |  - game-data catalogue    |
           |  - validator              |
           +-------------+-------------+
                         |
        +----------------+----------------+
        |                |                |
      CLI           MCP server      visual designer
```
The idea: put all logic in one core library and keep the front-ends thin, so the CLI, MCP server and UI can't drift apart. Confirm or reject this in [decisions.md](decisions.md).
