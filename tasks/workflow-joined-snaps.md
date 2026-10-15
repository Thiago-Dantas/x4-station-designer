# Workflow: modules placed inside each other (user report 2026-10-15)

Status: **done** (2026-10-15): fix committed; re-tuning moved to workflow-retune-placement.md (user: commit now, re-tune separately)

User: "looks like we have a regression the optimizer placement, we are placing ship fabs inside piers. to repro open megashipyard terran 642 modules, optimize with rebuild connection structures, set to buy energy cells, it should be a re-layout only"

## Cause (verified 2026-10-15, scratch examples)
- `Body::joined` counted two modules as joined when *any* snap points coincided, so `overlap` skipped them. Placement can leave a free snap on top of another module's free snap. A module attached there lands inside the module that owns the other snap, and the two count as "joined".
- Not new: same result on the commit before bb37d67. On HEAD, "100 solar" (Place all) has 1184 module pairs whose collision meshes cross, e.g. a ring of four cross hubs whose last hub sits exactly on hub #1 at the origin.
- The megashipyard (rebuild, no fixes): 1061 such pairs before the fix.

## Fix
- [x] `joined` needs coinciding snaps that face each other (+Z opposite, as `attach` makes them).
- [x] Placer: two free snaps that meet (same place, facing) close a loop; both leave the free list (Optimize on the megashipyard 28 s → 24 s; before the fix 16 s with modules stacked).
- [x] Megashipyard: 481 re-placed, connection structures −161 +7; no overlaps, no blocked zones, no mesh-crossing pair that isn't linked.
- [x] Unit test: facing the same way is not joined.
- [x] game_data: optimize_station, auto_place_planned_stations, overlap_calibration, validates_player_plans, validates_library_plans, clearance_zones, optimize_keeps_zones_clear, attach_reproduces_game_placements pass.
- [ ] Failing now (layouts tuned on stacked results):
  - `fewer_connectors`: "100 solar" is 20×20×19 km (was 9×8×8 stacked); fewer connectors gives 41 against 36.
  - `auto_place_keeps_room_to_grow`: Teladi 50 claytronics leaves pier, 7 quantum tubes, 1 storage unplaced; a module in a dock area's zone.
  - `planner_extras`: a defence disc's lane is blocked (144 modules, 20×20×20 km).
- [x] Decided (user): commit now, re-tune separately. The 3 failing tests are `#[ignore]`d with a pointer to workflow-retune-placement.md.
- [x] Docs: x4-construction-plans.md "Overlaps" (facing rule), "Auto-placement" (old numbers flagged).
- [x] usage.md *Fewer connection structures* numbers flagged as pre-fix; re-measuring is part of the re-tune.
- [x] Calibration unchanged: `overlap_calibration`, `validates_library_plans`, `validates_player_plans` print the same as on HEAD.

## User notes (2026-10-15)
- Builder matches the game for the megashipyard as saved. Entries 562/565/567/575 ("N m away from its linked snap") look wrong in game too: probably the planner mod; we're correct, keep them as failures. Entry 173 (observation deck, not attached) floats in game: game quirk, keep reporting. Recorded in x4-construction-plans.md.

## Double-click a check zooms to 1 km (user request, 2026-10-15)
- [x] `View3D.frameModule(index, gap)`: camera `gap` m out from the module's bounding sphere, same viewing angle; Checks rows: click selects, double click (`detail === 2`) also zooms 1 km.
- [x] Checked in the builder (scratch CDP, megashipyard): double click on "entry 173" and "entry 562" selects and zooms to each.
- [x] usage.md

## Scratch
- `crates/x4sd-core/examples/scratch_megayard.rs`, `scratch_solar.rs` (delete before commit); worktree `../x4sd-prev` at HEAD for comparisons.
- Copy of the profile: `research/extracted/profile-samples/constructionplans-20261015-mega.xml`.

## Also uncommitted in layout.rs (2026-10-15)
- Placement reach: chains of 2-3 struts before each fallback (workflow-placement-partial.md, D-21). Committed together with the facing rule once the user decides.
