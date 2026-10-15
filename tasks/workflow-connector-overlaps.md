# Workflow: most overlaps are notes, not problems (user report 2026-10-15)

Status: **done** (2026-10-15)

User: "our checks are a bit too strict, see (9.0) Ribbed for Her Player with 205 check failures but in game everything is fine" (screenshot: the game's planner says "Everything is OK").

## Cause (verified 2026-10-15, scratch example)
- All 205 are `Overlap` between connection structures: 204 Terran Cross vs Terran Cross, 1 Argon Cross vs Terran Base. The ribs are crosses on neighbouring arms bent around a ring; the 400 m arms meet tip to tip at 90° or lie side by side. The meshes really intersect (often still at 100 m apart), so the check was right about the geometry.
- "(SS)" variant: 230, of which 229 connector vs connector and 1 Terran Cross through an L Solid Storage.

## Decided (user)
- First: "keep them, as a softer note" (connector vs connector).
- Then, mid-task: the game allows overlaps except for a few components like docks and piers; overlaps are ugly, so placement and Optimize keep avoiding them, but the checks only show an error for docks, piers and ship fabs. D-25.

## Steps
- [x] Core: `validate::DOCKING_CLASSES` (dockarea, pier, buildmodule); `Issue::Overlap` only when one module is of these; else `Issue::MinorOverlap`; `Issue::is_note()`
- [x] CLI `validate`: "ok, N note(s)", `note:` lines after the issues, exit code from issues only
- [x] MCP `validate_design`: `notes` apart from `issues`; server instructions and tool description
- [x] Builder: Checks count and red modules from issues only; notes listed greyed after the issues (tooltip)
- [x] Verified: CLI on the profile (both Ribbed plans ok, 205 / 230 notes, exit 0; megashipyard 5 issues + 1 note); builder over DevTools ((SS): "No problems found", no count, 230 grey notes, no red modules; megashipyard: count (5), the note last); game_data `validates_*` (library: 0 overlaps, 28 notes) and core unit tests pass
- [x] Docs: usage.md, x4-construction-plans.md "Overlaps", decisions D-25, README
