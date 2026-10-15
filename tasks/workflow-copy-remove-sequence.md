# Copy / Remove sequence like the game (user request, 2026-10-15) — **done**

A **sequence** is a module plus everything attached to it, directly or not (its
subtree; these all come after it in the build order). The old *Remove* already
removed a sequence.

- [x] Core `Design::remove_single`: removes one module; what was attached to it stays where it is, unattached
- [x] Core `Design::copy_sequence`: copies a module's sequence (with equipment) onto a free snap, appended to the build order
- [x] Check `Issue::Unattached`: every module without a link except the first in the build order and those meeting another module at a snap (43 of 58 in the game's and the player's plans do; D-20)
- [x] Builder: Del = remove (single), Shift+Del = remove sequence; Ctrl+C = copy (like middle click); Ctrl+Shift+C / Shift+middle click = copy sequence; buttons
- [x] Unit tests (core), build, ui smoke of the new keys
- [x] Docs: usage.md, decisions (D-20), x4-construction-plans.md, CLAUDE.md, roadmap (README lists no keys)

Verified 2026-10-15: core unit tests (`remove_single…`, `copy_sequence…`), game-data
`validates_player_plans` / `validates_library_plans` (14 library modules placed apart,
the megashipyard's deck), `research/sequence_smoke.py` on the release exe: OK.

Noticed, not fixed (older bug): `#inspector label { display: flex }` overrides `hidden`,
so *Connected by* shows for free-standing modules too.
