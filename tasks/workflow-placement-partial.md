# Workflow: placement that can't fit everything; blueprint checks (user requests, 2026-10-15)

Status: **done** (2026-10-15); re-tuning of compact layouts stays open in workflow-joined-snaps.md

User: "still failing to place modules sometimes … Placed 1 modules. No room for 5 × 1M6S Luxury Dock Area, 10 × Terran 3-Dock E Pier. Instead of placing, when we cant place for some reason we should prompt the user for confirmation if he wants a partial build, but we should strive to make his build possible by adding more connections. If the user accepts a partial build, we should update the plan to remove things that were placed."
User: "when a plan is made and applied with 'Only modules i have blueprints for' off, and the user turns it on we should check the station for placed modules the user doesnt have blueprint for and alert him on the CHECKS area"

## Repro
- The user's planner (from the builder's local storage): extras 10 × `pier_ter_harbor_03_macro`, 5 × `dockarea_ter_m_station_01_standard_macro`, 1 × `defence_ter_claim_01_macro`; race terran; blueprint limit was on.
- All 7 saved plans and 7 import files place all 16 (scratch `scratch_docks`), also after a full-rebuild Optimize (`scratch_opt_docks`). The station the user had open isn't on disk: asked them to *Save as…* it.
- Builders built since 2026-10-15 morning include the stricter overlap rule (workflow-joined-snaps.md, uncommitted), which refuses spots where modules used to stack.
- The user saved the station as `constructionplan/test.xml` (copy: `research/extracted/profile-samples/test-20261015.xml`, 154 Argon modules). Nothing places on it: all 46 free snaps face into neighbouring modules. A Terran cross fits on none, an Argon cross on 1 (the exit `leaves_exit` kept for Argon pieces), and nothing fits past it. Optimize (keep shape or rebuild) with the 16 modules: no room for 1 or 3 piers (approach zones).

## Steps
- [x] Checks: with the blueprint limit on, list placed modules without a blueprint (`Issue::NoBlueprint`, `ops::issue_list`; `set_blueprints` returns the scene so Checks update at once; MCP `validate_design` too)
- [x] Place all: if not everything fits, ask (partial build?); No → undo; Yes → keep, and the plan drops what was placed (D-21)
- [x] Placement reaches further before giving up: chains of 2-3 struts to a new hub (`Placer::reach`). Small gain (1 more dock with clear space in an overload test); the claytronics and "test" stations are sealed, which it can't fix.
- [x] Checks in the builder (fresh builder per scenario, largest plot): "test" + 4 S, 2 M container storage, 2 Terran piers, race Any: No → 154 modules, plan unchanged; Yes → 163, plan lists 2 M storage, 10 energy, 1 habitat; extras list 2 M storage. Blueprint limit on → 103 "no blueprint" checks; off → 0.
- [x] Docs: usage.md, x4-construction-plans.md (reach), decisions D-21
- Known: after a partial build the plan summary still shows the full plan's cost.
