# Workflow: defence platforms and piers outermost — **done**

User report (2026-10-15, screenshot): on the "test" station, an Optimize re-layout put the Terran Disc Defence Platform deep inside the station. "Large piers and defense modules should always be on the outermost."

## Cause
`layout::layer` put defence modules (class `defencemodule`) in layer 1 with production and storage, so Optimize put them in the shell around the core. Only dock areas had a clear corridor out; piers relied on their 100 km approach zone.

## Steps
- [x] Repro test `optimize_puts_defence_and_piers_outside`: "test" + 5 dock areas, 10 piers, 2 discs, Optimize (rebuild). Before: 4 of 13 piers/defence modules had a module on the line out (a disc behind production)
- [x] Fix (D-23): `layout::outermost` = dock areas, piers, defence modules: outer layer and the 3 km corridor, in Optimize and Place all; a module placed on a side keeps its lane instead (else `planner_extras` got worse). After: 0 of 13
- [x] Piers: now need the corridor too (the 2026-10-14 megashipyard problem is gone with break-up: `optimize_keeps_zones_clear` passes)
- [x] Other placement tests: all pass; ignored `fewer_connectors` and `planner_extras` fail exactly as before. Cost: more connection structures (24 → 38 on the repro, megashipyard 17 → 25)
- [x] Wording: status line, MCP descriptions, ops; docs: x4-construction-plans.md, usage.md, decisions (D-23), README; roadmap
- [x] Commit and push

Not checked in the builder over DevTools: the change is in the core (covered by the test); the UI change is one status-line string.
