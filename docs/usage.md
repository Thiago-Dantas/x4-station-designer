# Using x4sd

`x4sd.exe` is one self-contained program (about 1 MB; nothing to install). It finds the game through Steam and the player profile under `Documents\Egosoft\X4\<id>` by itself. Override with `--game-dir` / `X4_GAME_DIR` and `--profile-dir`.

## Command line
| Command | What it does |
|---------|--------------|
| `x4sd list-modules [--class C] [--race R] [text]` | Buildable station modules (base, DLCs, enabled mods): macro, class, race, source, name. `text` matches the name or macro. |
| `x4sd snaps <macro>` | A module's snap connections with position and facing direction (module frame, metres) |
| `x4sd show <file>` | Plans in a plan file: id, name, module count, unattached modules, DLC patches |
| `x4sd validate <file> [--plan P]` | Problems the game doesn't catch: unknown modules/snaps, modules off their snap (more than 5 cm), misaligned snaps, missing DLC patches. Exit code 1 if any. |
| `x4sd export <file> --plan P [--new-id] [--overwrite]` | Writes the plan into `<profile>\constructionplan\` for in-game **Import** (build menu, Import button; the folder is rescanned when the menu opens). `--new-id` makes the import add a new plan instead of replacing one with the same id. |

A plan file is either the profile's `constructionplans.xml` (all saved plans) or a single-plan file like the ones the game exports. Exit code 2 means an error (file not found, game not found, ambiguous plan, and so on).

## MCP server for AI agents
`x4sd mcp` speaks MCP over stdio. Register it with an MCP client, for example Claude Code:
```
claude mcp add x4sd -- "E:\code\x4-station-designer\target\release\x4sd.exe" mcp
```
(add `--game-dir …` / `--profile-dir …` before `mcp` if they aren't found automatically). The server explains the workflow to the agent in its instructions.

| Tool | Purpose |
|------|---------|
| `list_modules`, `module_snaps` | Find modules (class/race/text filters) and their snap points |
| `new_design`, `open_design`, `describe_design`, `rename_design` | Start or load a design; see every module with index, name, position, link and free snaps |
| `add_module` | Place a module on its own (the first one, or a separate part) |
| `attach_module`, `free_snaps` | Connect a module's snap to a free snap of a placed module, optionally twisted |
| `set_twist`, `move_module`, `remove_module` | Rotate around the snap axis, move unattached modules, remove a module with everything attached to it (renumbers 1..n) |
| `undo`, `redo` | Every edit can be undone |
| `validate_design` | Same checks as `x4sd validate` |
| `save_design`, `export_design` | Write a plan file, or put the plan into the game's `constructionplan` folder for Import |

For now the MCP server keeps its own open design (headless). Once the builder exists, it will edit the design open in the builder live (D-7).

`research/mcp_smoke.py <x4sd.exe> <profile dir>` drives the server like an agent: it builds, validates and exports a small Terran station.

Still to come (Phase 3): `x4sd` without arguments opening the visual builder.
