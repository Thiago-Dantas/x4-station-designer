# Workflow: Ctrl+Z resets the filter text (GitHub issue #5)

Status: **done** (2026-11-08)

Report (issue #5): "When you ctrl-z a thing you did it also resets the last text that was in the filter textbox." A comment adds that the typing counts as an undo step.

## Cause
The builder's key handler runs its own undo/redo on Ctrl+Z / Ctrl+Y, but it did not block the browser's default action. WebView2 keeps one text-undo stack for the whole page, so the same key press also undid the last typing in a text box, even after the box lost focus.

## Steps
- [x] `ui/src/main.ts`: `preventDefault()` on Ctrl+Z and Ctrl+Y in the window key handler. Inside a text box the handler still returns early, so Ctrl+Z there undoes the typing as before. (1.5.3 changed that: undo/redo act on the design in boxes too; see [workflow-keys-focus.md](workflow-keys-focus.md).)
- [x] Checked in the builder over DevTools (scratch test): type in the palette search, leave the box, place a module, Ctrl+Z. The 1.5.1 exe cleared the search and removed the module. The fixed exe removes the module and keeps the text.
- [x] Docs: `docs/usage.md`, this file, the roadmap line. `CLAUDE.md` has nothing to add.
