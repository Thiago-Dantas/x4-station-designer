# Workflow: real module models in the builder

Status: **geometry done** (2026-10-13). Textures still to decide with the user (step 6).

User request (2026-10-13): "nobody wants to see blocks and squares, users will want to see the actual models for each station module". This comes before the Phase 4 in-game round trip.

Approach: geometry first (real meshes with shading, tinted by class), then decide on textures and materials with the user.

## Steps
- [x] 1. Find the mesh files of a module (component part → `<component>_data/<part>-lodN.xmf`)
- [x] 2. Reverse-engineer `.xmf` (research parser in Python). Check that the decoded bounds match the part's `<size>` in the component XML.
- [x] 3. Rust `xmf` parser in `x4sd-core`, plus the parts/LOD each module uses, with tests against all 287 modules
- [x] 4. Builder: binary mesh transfer (Tauri), shared geometry per module, boxes as fallback
- [x] 5. Screenshots / UI smoke test; docs
- [x] 6. Ask the user about textures and materials. Answer: add them, as an on/off **setting** (performance unknown).
- [ ] 7. Textures:
  - [ ] UVs from `.xmf`
  - [ ] material library (base + DLC diffs) → diffuse map
  - [ ] gzip DDS (BC1/3/4/5) → WebGL compressed textures, max 1024 px by skipping mips
  - [ ] toolbar toggle, remembered, default off until measured
  - [ ] measure load time and memory
- [ ] 8. Maybe normal maps (BC3/BC5 with game-specific channel packing)

## Sub-agents
None.

## Notes / findings log
- 2026-10-13: the `.xmf` layout is documented in docs/x4-game-data.md.
  - Bounds match the component `<size>` exactly.
  - Normals are D3DCOLOR (B,G,R order) and triangles are clockwise.
  - The usage field needs masking to its low byte (Paranid dock `anim_escalator` has junk in the upper bytes).
- 2026-10-13: part rules:
  - `parent` chains and `ref` parts (piers) are resolved.
  - Effect materials are filtered out.
  - Detail levels come from the tags. Cost per module: Fast about 6k, Normal about 22k, Detailed about 57k triangles.
- 2026-10-13: texture facts:
  - Textures are gzip-packed DDS: full 2048×4096 (5.6–11 MB as BC) and `-small` 128×256 (22–44 KB).
  - Formats are BC1, BC3, BC4 and BC5.
  - `libraries/material_library.xml` maps `collection.material` to maps (`diffuse_map`, `normal_map`, `smooth_map`, `metal_map`, plus paint/dirt/detail layers). DLCs add to it with `<diff><add sel="/materiallibrary">`.
  - Station meshes use texcoord FLOAT2 (usage index 0; one mesh also has index 2).
- 2026-10-13: the builder draws real models, with one shared geometry per module type sent as binary over Tauri IPC. Boxes are a fallback (only `landmarks_par_storage_01_macro` has no own geometry). Screenshots look right: Terran habitats are rings, the cross is a disc hub. The UI smoke test passes.
