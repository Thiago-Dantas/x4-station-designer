# Workflow: plan file paths and renamed plans

Status: **done** (2026-10-14); an in-game check of importing a renamed plan is still open

User request:
- "Open File and Save As should default to the correct filesystem paths where the xmls should be."
- "When I open a game plan and rename it, it's only renaming the xml, not the plan name in game."
  - Decision (user, 2026-10-14): **a renamed plan becomes a new plan.** Writing it gives it a new id, so in game it imports next to the original instead of replacing it.

Cause of the rename issue: Export kept the plan's id. In the profile, `megashipyard terran2.xml` had the id of the saved `megashipyard terran`, so the game listed it as "[Imported]" and the saved plan kept its old name.

## Steps
- [x] Core: `import::plan_files` (saved plans + import files); `import::renew_id_if_renamed` (the id belongs to another name there → new id); `Design::set_id` (every undo state, so undo never brings the old id back). Unit test.
- [x] Builder `save_design` / `export_design` and MCP `save_design` / `export_design` renew the id and say so (`new_plan`).
- [x] Builder: *Open file…* starts in `<profile>\constructionplan\`; *Save as…* proposes `<profile>\constructionplan\<plan name>.xml` (the game's file name).
- [x] Checks: unit test; the builder's dialog defaults (captured over CDP); export of a renamed plan into a temporary profile.
- [x] Docs: usage.md, x4-construction-plans.md, decisions.md.

## Notes
- `new_plan_id` was `player_<unix seconds>`: a rename and an export within a second gave the same id. Now ids are unique per run (one second ahead when needed).
- Checks:
  - unit test `import::tests::renamed_plan_becomes_a_new_plan`: same name keeps the id; renamed gets a new one, also within the same second; the new id is then stable; undo keeps it.
  - Scratch CDP check of the builder with a temporary profile (a copy of the saved plans):
    - *Open file…* starts in `<profile>\constructionplan`;
    - *Save as…* proposes `defense station renamed_ v2.xml` there;
    - renamed `defense station` exported as a new plan (new id, status says so); a second export kept that id; undo restored the name.
  - The dialogs were stubbed by wrapping `window.fetch`: Tauri's `invoke` is read-only, and its IPC is a fetch to `http://ipc.localhost/<command>`.
