# Workflow: start maximized, collapsible right pane (user requests)

Status: **done** (2026-10-14)

User, 2026-10-14: "can the program always start maximized? also the right pane is becoming cluttered, can we make it so we can collapse the right pane sections, start with them collapsed".

## Steps
- [x] Window starts maximized (`tauri.conf.json`, `maximized: true`; 1500×920 stays the size when restored).
- [x] Right pane: clicking a section title collapses or expands it (a chevron shows which).
  - Equipment, Build cost, Production, Plot, Checks and Controls start collapsed on every start (not remembered, as asked).
  - *Placing* and *Selected* appear only when needed and start open. The *Equipment* list inside *Selected* starts collapsed.
  - The *Checks* title shows how many problems there are (red), so collapsing it hides none.
- [x] ui_smoke: sections start collapsed and expand on click; the rest still passes. Docs (usage.md).

## Notes
- 2026-10-14, user: "controls doesnt need to be a section, leave it in plain view". The controls help lost its heading and always shows; five sections fold.
