# Workflow: GIFs for the GitHub page (release prep)

Status: **done** (five GIFs; README.md written and links them) (started 2026-10-14). Part of the roadmap item "Release build for players".

User request: record GIFs that show the builder's features, for the GitHub page. Decisions (user, 2026-10-14): all four scenes; GIFs live in `docs/media/` in the repo and the README links them.

## Approach
- `research/record_gifs.py` drives the release `x4sd.exe` over DevTools (`research/cdp.py`), plays one scripted scene per GIF and grabs JPEG frames with `Page.captureScreenshot` in a loop on a second DevTools connection.
- Clicks are synthetic, so a fake cursor is injected into the page while recording.
- Frames → GIF with gifski (installed on H:). About 3–5.5 MB each.

## Steps
- [x] Install gifski on H:, record in docs/dev-setup.md
- [x] Recorder core: screencast frame capture, fake cursor, encode
- [x] Scene 1, snap-building: new design, place a module, attach via snap cones, twist
- [x] Scene 2, planner → auto-place → Optimize
- [x] Scene 3, agent live edit (MCP over the live link) with a caption
- [x] Scene 4, open a game plan + analysis panels + build-order timeline
- [x] Review every GIF (frames look right, size OK)
- [x] Docs: CLAUDE.md layout row, docs/dev-setup.md, roadmap
- [x] Scene 5 (user request): Optimize on the saved "megashipyard terran" (332 modules, the in-game save, not the 434-module import file of the same id) with *Rebuild connection structures* and *Fewer connection structures*
- [x] README.md (user request): features, the five GIFs, getting started, MCP, CLI, status (what's unconfirmed in game), building from source

## Notes
- Page fixed at 1440x810 with `Emulation.setDeviceMetricsOverride` (at 1280 the toolbar wraps once "unsaved" and the patches text show); GIFs 1080 wide. Capture runs at ~25 fps; GIFs are resampled to 12 fps.
- The fps counter in the view is hidden while recording (it measures the capture, not the app). Textures on, detail Normal.
- **User feedback (2026-10-14):** the first run built outside the default 4 km plot and hit the native enlarge prompt: the build scene now sets a 10 km plot. After *Place as first module* the view framed the module too tightly: `placeFirst` now backs off 5 km (`View3D.frame(extra)`), user confirmed it "looks great". Fit view (F) is unchanged.
- Cone picking: zoomed out, the hub's cones sit within 60 px of each other, so `cdp.find_color`'s radius blob merged them and the click landed between cones. `Scene.cone()` takes connected blobs and picks the one nearest the view centre.
- Planner scene: the planner remembers its targets in the test profile; setup opens the tab (they render then) and removes them.
- Range inputs ignore synthetic mouse drags: the build-order timeline is stepped from JS with the cursor kept on the thumb.
- The start-up status line names the profile folder (user name, profile id): setup clears it before recording.
- Results (2026-10-14): build 30 s / 3.5 MB, planner 21 s / 3.0 MB, agent 23 s / 3.8 MB, analysis 32 s / 5.5 MB, all 1080×608 at 12 fps, in `docs/media/`. The planner scene stops at Place all + Production (no Optimize). The analysis scene's Open dialog shows the player's plan names.
- Optimize scene: the ~6 s Apply plan wait is cut from the GIF (`Recorder.pause`, timestamps shifted so the GIF doesn't jump). Result: 332 modules (137 connection structures) → 349 (7), 147 modules added for shortfalls, plot fitted 11×12×17 km. gifski quality 60 for this scene (8.2 MB at 80).
