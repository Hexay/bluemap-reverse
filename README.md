# bluemap_reverse

Reconstruct a Minecraft Java world from a BlueMap web map (the 3D browser viewer) — as close to the original as possible.
Rust (reverser) + Python stdlib (test tooling). Status: end-to-end reversal works; superflat fixture ≈ 99.9% exact state. Next: seeded vanilla terrain.

```
bmr fetch http://127.0.0.1:8100/        # mirror → work/cache/127.0.0.1_8100 (resumable)
bmr obj [--shade]                       # hires tiles → work/obj/<map>/<map>.obj (+mtl, textures) for Blender
bmr check-heights                       # hires top faces vs lowres heightmap (decode sanity check)
bmr score <orig> [recon] --mirror <dir> # block-by-block score over rendered columns; no recon = all-air baseline
bmr probe <world> x,y,z ...             # print block states + biome
bmr reverse --mirror <dir> <out_world>  # reconstruct (needs work/cache/debug + work/worlds/debug, see below)
bmr explain --mirror <dir> x,y,z --original <world>   # why a cell matched / didn't
```
Library prerequisites: `py -3 tools/make_world.py debug && py -3 tools/mirror_fixture.py debug`, plus
`template-void`. Any fixture: `py -3 tools/make_world.py <f> && py -3 tools/mirror_fixture.py <f>`.
Iterate with debug builds (`cargo build`, binary `target/debug/bmr.exe`); deps are optimised in the dev profile.

## Test loop

```
py -3 tools/up.py [fixture] [--force]   # download JDK 25 / MC 26.3 server / BlueMap 5.27, make world, render, serve
```
- `tools/setup.py` → `work/downloads/`; `tools/make_world.py <fixture>` → `work/worlds/<fixture>/world`;
  `tools/render_serve.py <fixture> [--no-render|--no-serve|--force-render]` → `work/bluemap/<fixture>/`, http://127.0.0.1:8100/
- Fixtures: `fixtures/<name>/fixture.json` (server.properties overrides, area to force-load, optional `bluemap` map-config
  overrides) + `commands.txt` (console commands run after the area loads).
- `work/` is git-ignored.

## Docs

- `docs/plan.md` — goal, architecture, phases, scoring, open questions. **Start here.**
- `docs/chat-log.md` — the conversation that started the project.
- `research/01-bluemap-web-format.md` — what a BlueMap site exposes: URL layout, PRBM format, tiles, textures.json, lowres PNGs, what's culled.
- `research/02-model-inversion.md` — how BlueMap turns block states into meshes and how to invert it.
- `research/03-rust-and-tooling.md` — crates, BlueMap CLI usage, test-world generation, version pins.
- `research/04-filling-hidden-data.md` — recovering what tiles don't contain: seed cracking, regen+merge, light/AO/biome inference.

## Only use on maps you own or have permission to reverse.
