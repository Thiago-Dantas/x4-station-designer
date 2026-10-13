# Workflow: Phase 1 research (how X4 handles station plans)

Status: **in progress** (started 2026-10-13)

Goal: answer the Phase 1 questions in [roadmap.md](roadmap.md) and record the answers in `docs/`.
User constraint: keep sub-agents to a minimum (token budget).

## Steps
- [x] Write a small `.cat`/`.dat` extractor (`tools/x4cat.py`) that checks MD5s. The `.dat` layout is now verified.
- [x] Add `.gitignore` for extracted data (game and mod files are not ours to commit)
- [x] Extract `x4ep_planner` and read how it places modules
- [x] Extract base-game files: `libraries/constructionplans.xml`, `menu_station_configuration.lua`, macro and component indexes, `common.xsd`, `md.xsd`
- [x] Map `connectionsnapNNN` to module component connections (one module, `struct_arg_base_02`)
- [x] Write the findings in `docs/` (new: `x4-plan-apis.md`; updated: `x4-construction-plans.md`, `x4-game-data.md`)
- [ ] Next session: the cheap `offset` check (see roadmap), then REA if still needed (D-4)

## Sub-agents
None, at the user's request to save tokens.

## Notes / findings log
- 2026-10-13: the build menu has Export/Import/Reload of plans (`ExportMapConstructionPlan`, `ImportMapConstructionPlan`, `GetImportableConstructionPlans`). This makes option B the lead.
- 2026-10-13: the plan list shows only `source == "local"` plans (non-cheat builds). This makes option C weak.
- 2026-10-13: `x4ep_planner` doesn't inject exact layouts. It feeds macro lists to the engine generator.
- 2026-10-13: MD can build a plan by ID (`add_build_to_expand_station`, `apply_construction_sequence`, `create_station`).
- 2026-10-13 14:41: the user **saved** (not exported) the plan "TerranScrapyard" (`player_1791913306`, 84 entries, 4 roots, terran + pirate DLC) with the game running. Verified: the game rewrites `constructionplans.xml` at once on Save, the plan id = Unix time of the save, the new plan goes to the top, and `constructionplan\` stays empty.
- 2026-10-13 14:42: **live test A in progress.** Backed up to `constructionplans.xml.bak-20261013-1442`, then injected a copy of TerranScrapyard as `player_1791913365` "X4SD test A" at the top, with the game running. Waiting for the user to report whether it shows in the build menu's plan list. Then the user exports TerranScrapyard (test B).
  - Restore if needed: copy the `.bak-20261013-1442` file over `constructionplans.xml`. It doesn't contain "X4SD test A".
  - **Result:** "X4SD test A" did **not** show up after reopening the build menu, so the game doesn't re-read the file while it runs. Still open: does it load after a restart? Until then the user must not Save a plan, because that rewrites the file from the game's memory and drops test A.
- 2026-10-13: the user's screenshot of the Import dialog confirms the folder. Tooltip: "Import shared construction plans from your constructionplan folder." Body: "Import community created construction plans from the 'constructionplan' folder in your personal X4 folder." There's no visible refresh button.
- 2026-10-13: **test B in progress.** Wrote the guessed file `constructionplan\X4SD test B.xml` (`<plans>` wrapper with one `<plan id="player_1791913400" name="X4SD test B">`, a copy of TerranScrapyard). Asked the user to Export TerranScrapyard (real format), reopen the build menu, and check the Import list.
  - **Result: B works.** Test B was listed and imported. The export `TerranScrapyard.xml` differs from our file only in the id and name line, and the imported block is identical apart from id and name. Export also saved to the list (overwrite prompt). The saved/exported/imported writes at 14:48:18 rewrote `constructionplans.xml` and dropped test A (expected).
- 2026-10-13: side finding: snap names aren't always 3 digits (`connectionsnap03` on entry 8 of TerranScrapyard). Matching must not assume `NNN`.
- 2026-10-13: **test C (offset question) in progress.** `constructionplan\X4SD test C.xml` (`player_1791913500`) = TerranScrapyard with entry 32 (casino, `welfare_gen_casino_01_macro`, no children, attached via `connectionsnap002` to entry 8's `connectionsnap03`) moved from y = -469.66 to y = 530.340 (+1000 m). Asked the user to import it, view it, and Save it.
  - If the casino sits in its normal place and the re-saved y is about -469.66, the game recomputes from connections. If it floats 1 km up and the saved y = 530.34, the offset is authoritative.
  - **Result (visual, user screenshot):** the casino floats, attached to nothing, and the build menu still says "Construction Plan Status: Everything is OK". **The offset is authoritative on the build map**, and the game doesn't validate that connected snaps line up.
  - Importing test C did NOT rewrite `constructionplans.xml` (mtime still 14:48:18), so Import is in-memory only. Corrected in the docs.
  - Pending: the user Saves test C, so we can see whether the game keeps y = 530.34 and the stale `predecessor` link.
  - **Result (file):** the user Exported test C at 15:01:36. The saved list and the export both keep `y="530.34"` and `<predecessor index="8" connection="connectionsnap03"/>`; the only line changed from the original is that position. **The offset is authoritative, and links are neither re-derived nor validated.** D-4 is settled without REA.
  - The game wrote `530.340` as `530.34` (trailing zero dropped).
- 2026-10-13 15:02: cleanup. Deleted our `X4SD test B.xml` and `X4SD test C.xml` from `constructionplan\` (the user's `TerranScrapyard.xml` export stays). Copied samples into `research/extracted/profile-samples/` (git-ignored): `TerranScrapyard.xml`, the test C game export, and `constructionplans-20261013-1501.xml`. The backup `constructionplans.xml.bak-20261013-1442` is still in the profile.
  - Still to do: the user deletes the "X4SD test B" and "X4SD test C" plans from the in-game list.
- [ ] Next: the snap-math verification (roadmap)
