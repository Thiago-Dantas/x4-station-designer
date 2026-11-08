# Workflow: keyboard and focus fixes (after issue #5), release 1.5.3

Status: **done** (2026-11-08)

User, 2026-11-08: asked for other inconsistencies like issue #5, then "ctrl z should not impact number box the same way it shouldnt impact text boxes. fix everything and ship as 1.5.3".

## Found (checked in the builder over DevTools)
1. After Enter in a number box (X / Y / Z, plot, sunlight, planner counts) the box kept focus; Ctrl+Z undid its text, not the move, and leaving the box applied the old value as a new edit.
2. A clicked checkbox or the build-step slider kept focus, and every shortcut (Ctrl+Z, Del, R, C, T, F, Esc, Page Up/Down, WASD/QE/arrows) was ignored until a click in the view.
3. Esc did not clear the palette search (it clears *Find in build order*).
4. Ctrl+R / Ctrl+F / Ctrl+T fired the plain R / F / T shortcuts (from the code).
Not a problem: F5 and Ctrl+R don't reload the page.

## Steps
- [x] Ctrl+Z / Ctrl+Y (Ctrl+Shift+Z) act on the design everywhere, text and number boxes included; never on typing (window key handler, before the text-box early return). Typing not yet applied in a number box stays and applies when the box is left.
- [x] Enter in a number box or the plan name applies it and leaves the box (document keydown: `blur()`, which fires its change)
- [x] Checkboxes and the slider leave focus after a change (document change: `blur()`)
- [x] Esc clears the palette search
- [x] R / F / T / C ignore Ctrl and Alt
- [x] Checked in the builder (release exe, scratch DevTools test, all pass): Ctrl+Z / Ctrl+Y in the search box undo/redo the design and keep the text; Esc clears the search; X = 500 + Enter moves and leaves the box, Ctrl+Z moves back and the field shows 0, Ctrl+Y with the cursor in the box redoes; a clicked *Textures* checkbox leaves focus and Ctrl+Z works after it; Ctrl+R leaves an attached module's twist, R twists it. `npm run check` clean.
- [x] Docs: `docs/usage.md`, roadmap, this file, a note in [workflow-undo-filter.md](workflow-undo-filter.md). `CLAUDE.md`: version only.
- [x] Release 1.5.3: versions, commit (`7fd8e9b`), push, tag; the Release workflow is green and `releases/latest` is v1.5.3 (exe, attestation, VirusTotal link)
