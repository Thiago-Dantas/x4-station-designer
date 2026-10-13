# Workflow: plot size (user request)

Status: **done** (2026-10-13)

User request: "let user define a plot size and work within those constraints, alert user what they want is too big for current plot size and request an increase".

## Game rules (verified in the build menu Lua, 2026-10-13)
Sources: `menu_station_configuration.lua`, `ego_detailmonitorhelper/helper.lua` (`Helper.getPlotData`).
- A plot is a raw size in metres plus a centre offset from the station origin. The menu shows it per side in whole km: `posX = ceil((size.x/2 + centre.x)/1000)`, `negX = floor((size.x/2 − centre.x)/1000)`, the same for Y and Z.
- Sliders step by 1 km. On each axis pos + neg ≤ `maxPlotSize` = **20 km**, and at least 1 km.
- **Minimum plot** for a station: `GetConstructionMapMinimumBuildPlotSize/CenterOffset`, "the smallest kilometer-precision box that can contain the current station", rounded up per side.
- The price comes from `GetBuildPlotPrice(sector, position, x, y, z, owner)` and depends on the sector, so it can't be computed offline.

## Our model
- `Plot { pos: [x,y,z], neg: [x,y,z] }` in whole km.
- Required plot = the station's bounding box (module part boxes in the station frame), rounded up per side.
- The plot is not part of the plan file: opened plans start with a fitting plot, new designs at ±1 km (2×2×2).

## Steps
- [x] Core: `plot.rs` (Plot, limits, required plot), issues for modules outside the plot / plot too small / too big for any plot, `Design.plot` (+ `fit_plot`, grow-only)
- [x] Ops and MCP: `set_plot`, `fit_plot`; plot in `describe_design` / `validate_design`
- [x] Builder: plot panel (6 values, fit button, required vs current), plot box in 3D (green/red), prompt to enlarge (or undo) when the user's edit doesn't fit
- [x] CLI: `show` prints the plot needed, `validate --plot +X,-X,+Y,-Y,+Z,-Z`
- [x] Tests + docs

## Notes
- 2026-10-13: required plots of the player's plans: TerranScrapyard 8x5x9, defense stations 4x4x3/4x4x4, asd1 6x6x6, megashipyard 19x20x15 (at the 20 km limit; built in game, so our bounds aren't larger than the game's).
- 2026-10-13: **bug found and fixed:** quick plot edits ended up with a stale plot (10x10x2 instead of 10x10x10). Tauri replies can arrive out of order, and an older scene overwrote a newer one. Every scene command in `ui/src/api.ts` now runs serially (`serial()`). Since then the intermittent attach-click failure in ui_smoke hasn't come back (3/3 runs), so it was probably the same race.
- 2026-10-13: the plot text is plain ASCII (`8x5x9 km (+X 5 -X 3, ...)`). U+2212 and × broke the Windows console (cp1252) in the MCP smoke test.
- 2026-10-13: an earlier texture_smoke version left `x4sd.textures = off` in the shared WebView2 storage; reset to the default. The user's own `x4sd.quality = detailed` was left as it was.
