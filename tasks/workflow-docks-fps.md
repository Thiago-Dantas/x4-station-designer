# Workflow: FPS counter; docks and piers kept on the outside (user requests)

Status: **done** (2026-10-14); in-game check of ships reaching the docks still open

User, 2026-10-14 (screenshot of a dock area among solar panels and habitat rings):
- "can you add an FPS counter, im noticing its a bit slower now"
- "when auto placing modules we need to take care not to block docks and piers"
- "if possible on every generation we should rework where docks and piers are so they are always on the outermost parts of the station"

## Plan
- [x] **FPS counter** in the builder's 3D view (frames per second, plus frame time).
- [x] **Clearance corridor** for dock areas and piers. It replaces the single line from the origin.
  - The dock's box, swept 3 km out from the station centre through the dock, must be clear of other modules' collision boxes.
  - While placing, no module may enter the corridor of a dock that has one.
- [x] **Rework on every auto-placement run** (Place all, MCP `auto_place`):
  - docks and piers with nothing attached are lifted off first and placed again last, at the nearest spot with a clear corridor (outside the station as it is then), keeping their equipment;
  - docks carrying other modules stay and are protected;
  - if a lifted dock fits nowhere, it goes back where it was (free-standing at the same position).
  - Report: docks moved, docks placed without a clear corridor.
- [x] Tests (targeted, per the user: only the new things): core game-data test of a second placement run (docks moved, still clear, count unchanged), and the FPS counter shown in the builder. Docs.

## Notes
- 2026-10-14: results.
  - **Corridor:** the dock's part-box bounds, translated outward in steps of half its smallest size (at least 25 m), out to 3 km, tested against collision-box bounds with the 1 m tolerance. "Outward" is from the centre of the station's bounds.
  - **"100 solar", then 10 more solar:** both docks lifted and placed again, clear (1.2 km and 3.0 km from the centre), 0 blocked.
  - **TerranScrapyard plus its fix-up modules:** its one free dock moved; the other docks carry modules and stay protected.
  - **Builder:** a scratch CDP check shows "240 fps · 0.2 ms" on a small scene, and a second `auto_place` reports `docks_moved: 2`.
  - The game-data helper `place_and_check` no longer requires the old entries to be unchanged: lifted docks renumber them. It now requires every non-dock module to keep its macro and position.
- **Why it got slower** is still open. The counter will show whether drawing is the cost (fps and ms) or the work after each edit is: validation with mesh checks, cost, analysis and equipment requests.
