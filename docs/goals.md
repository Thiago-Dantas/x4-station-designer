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
| G6 | Visual builder with drag and drop | **First-class, built alongside the tooling** (user requirement, 2026-10-13). Users can drag and drop and use the tooling on the **same design**. How the two share a design is OD-6. |

## Constraints (user, 2026-10-13)
- **Fast:** quick start-up and a responsive builder
- **Compiled, self-contained distribution:** users don't download runtimes or a pile of libraries

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
All logic (plan model, snap placement, validation, import-file writing) lives in one core library. The visual builder, the CLI and the MCP server are thin front-ends over it, so a drag in the UI and an agent's tool call perform the same operations and can't drift apart. The language is OD-2.
