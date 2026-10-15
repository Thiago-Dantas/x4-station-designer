# Workflow: spire GIF

Status: **done** (2026-10-15)

User request: a GIF of the builder making a spire: 20 km plot (10 km each side), six Terran
Cross Connection Structure 01 stacked, each by its bottom snap and twisted 15° from the one
below, then Terran L Living Quarters on every side snap of the crosses (4 "stairs").

- [x] `spire` scene in `research/record_gifs.py` (plot set before recording, crosses by
      connectionsnap005 with T for 15°, 24 LQs at twist 0, bottom level first)
- [x] LQs go straight to cross snaps: the side cones are mapped once before any habitat;
      the ones hidden behind the spire come from a camera fitted to the visible ones (worst miss 5.6 px)
- [x] Shorter: plays 1.5× fast (`SPEED`), 28.7 s, 5.6 MB
- [x] gifski gets relative frame names (the full paths overflowed the Windows command line)
- [x] Docs: dev-setup.md scene list
