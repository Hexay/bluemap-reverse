# Origin chat log — 2026-09-25

Condensed record of the conversation that started this project (Claude Code session, Opus 5.5).

## 1. Does a BlueMap "world reverser" exist?

**User:** Check online if there exists a bluemap world reverser — BlueMap takes a Minecraft world and makes it visible in 3D online, but I want a way to get a world download from just having the website.

**Findings:** No existing tool reconstructs a world from a BlueMap web map. Searched BlueMap FAQ, 3rd-party addon list, GitHub `bluemap` topic, general web. Closest prior art:
[BlueRender](https://github.com/DedInc/bluerender/) — headless Python renderer that fetches tiles from a live BlueMap server and includes a PRBM parser (renders images only).

Why none exists — the browser only gets render output:
- Hires tiles: gzipped PRBM meshes (position/normal/color/uv/ao/light), no block IDs; faces between solid blocks culled.
- Lowres tiles: top-down images.
- No block-entity NBT, entities, biomes-as-data, redstone state.

Feasible approach: scrape tiles like the viewer → parse PRBM → snap faces to the block grid → identify blocks via UV → texture (`textures.json`) → block state → write schematic / region files. Result is lossy. Only use against maps you own or have permission for.

## 2. Test strategy and language

**User:** Find a public world download, stand up our own BlueMap site with it, run the reverser, check output == original map. Easy testing, all our own stuff. Which language — JS, Python or Rust?

**Answer:** Rust.
- `fastnbt`/`fastanvil` read the original world (scoring) and write the output (write support to be verified).
- Heavy per-face geometry work across thousands of tiles → `rayon`; `reqwest` + `flate2` for fetch/decompress.
- Strong types help with tile/chunk/block coordinate transforms.
- Runner-up Python (BlueRender parser, `amulet-core`); skip JS unless in-browser.

Test-loop refinements:
1. Output will never be byte-identical (hidden blocks, NBT) → score, not equality: % of visible blocks correct, split by block-type-only vs full block state.
2. Start with a self-generated world (no licensing issues, controllable), add showcase maps later.
3. BlueMap CLI renders and serves the webapp itself: render → serve → reverse against localhost → score.

## 3. Project goal set

**User:** The goal is to get as close as we can to an original. Created `C:\Users\hexay\bluemap_reverse` — put this chat, documentation, further research and planning there.

→ Research delegated to `docs/research/01..04-*.md`; synthesized plan in `docs/plan.md`.
