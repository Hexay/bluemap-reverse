# bluemap-reverse

Reconstruct a Minecraft Java world from a BlueMap web map (the 3D browser viewer) — as close to the original as
possible: one command turns a site into a world zip (every dimension) or a WorldEdit schematic.
Rust (reverser) + Python stdlib (test tooling).

## What you get

Scored block by block against the original worlds (Minecraft 26.3, BlueMap 5.27; `results/history.jsonl`):

| Test world | Blocks BlueMap drew | … counting look-alikes | All blocks | Biomes |
|---|---|---|---|---|
| Superflat with builds | 99.86% | 100% | 99.92% | 100% |
| Vanilla terrain, no seed | 98.61% | 99.99% | 73.6% | 94.3% |
| Vanilla terrain, with the seed (`--regen`) | 99.84% | 99.99% | 99.93% | 100% |
| Blocks among neighbours (`context`: stairs, fences, redstone, doors, a jumble of every block) | 99.44% | 99.98% | 97.5% | 99.8% |
| Nether | 99.99% | 99.99% | 82.1% | 73.6% |
| End | 100% | 100% | 97.6% | 100% |

- **Drawn blocks** are matched against signatures learned from BlueMap's own render of every block state,
  with texture orientation (door hinges, rotations), light, tints (redstone power) and game rules (stair
  corners, connections, note block instruments, redstone `powered`, waterlogging) settling the details.
- **Look-alikes** render identically, so no tile can tell them apart: waxed vs unwaxed copper, infested vs
  plain stone, double slab vs its full block, invisible or random properties (note pitch, crop/kelp age).
- **Hidden blocks** (underground, inside builds, the nether roof BlueMap masks out) are filled from evidence
  and priors; with the world seed they come from a same-seed regeneration instead. Block entity contents
  (chests, signs) and entities are not in the tiles at all.
- **Biomes** come from the grass, foliage and water tints BlueMap drew (overworld), what grows there (nether)
  and the island layout (end).

## Use it

Needs only `bmr` — no Java, server or BlueMap. `pull` picks the pack (~0.3 MB, one per Minecraft + BlueMap
version: block signatures, biome tints, block registry, an empty world template) whose texture list matches the site (a Minecraft-version fingerprint), among installed packs
(`./packs`, `packs/` next to bmr) and the online pack index, and downloads it if needed (`--offline` to skip;
`bmr pack list`, `bmr pack fetch <mc-version|all>`).
```
bmr pull https://map.example.com/ -o world.zip           # mirror + check + reconstruct + zip; extract into saves/
bmr pull https://map.example.com/ --map world_nether      # only one of the site's maps
bmr pull … --schem build.schem                             # also a WorldEdit schematic (overworld, else first map)
bmr schem <world> part.schem --area=x0,z0,x1,z1 --y=60,120 # cut a schematic out of any world
```
- Every map of the site goes into its dimension of one world (overworld, nether, end: from the map id,
  else BlueMap's default sky colour). Two maps of one dimension are separate worlds: pick one with `--map`.
  Map settings a site does not publish are assumed to be BlueMap's defaults for the dimension (nether roof
  y 90..127 hidden, caves below y 55 hidden in the overworld); override with `--mask-y`, `--cave-y`.
- Re-running is cheap: downloads are cached and resumed (`work/cache/<site>`), nothing is fetched twice.
- Be gentle: defaults are 4 parallel downloads with a 25 ms pause each (`--concurrency`, `--delay-ms`).
- The pack must fit the site: `pull` compares textures and BlueMap version and explains mismatches (other
  Minecraft version, mods, resource packs); it refuses clearly unfitting packs unless `--force`.
- Only reverse maps you own or have permission for.

## Development

New Minecraft/BlueMap version (1.18+), unattended, ~2 min: `py -3 tools/build_pack.py --mc 1.21.11 [--bluemap 5.27]`
→ `packs/bmr-mc<mc>-bluemap<bm>.pack` (prints time per stage). Prove it end to end: `py -3 tools/check_version.py --mc 1.21.11`
(fixture world in that version → BlueMap → `bmr pull` → score → load in that version's server).
Publish: `bmr pack index packs` writes `packs/index.json`; upload it with the packs to the GitHub release
`packs` (the default `--pack-index`). `py -3 tools/check_index.py` tests index → download → pull locally.
```
bmr fetch http://127.0.0.1:8100/        # mirror → work/cache/127.0.0.1_8100 (resumable)
bmr obj [--shade]                       # hires tiles → work/obj/<map>/<map>.obj (+mtl, textures) for Blender
bmr check-heights                       # hires top faces vs lowres heightmap (decode sanity check)
bmr score <orig> [recon] --mirror <dir> # block-by-block score over rendered columns; no recon = all-air baseline
                                        # --pack <p>: also rendered accuracy counting look-alike states as correct
bmr pack lookalikes <pack> out.json     # groups of states BlueMap draws identically (unrecoverable from tiles)
bmr probe <world> x,y,z ...             # print block states + biome
bmr reverse --mirror <dir> <out_world>  # reconstruct (needs work/cache/debug + work/worlds/debug, see below)
bmr explain --mirror <dir> x,y,z --original <world>   # why a cell matched / didn't
bmr reverse … --zip out.zip             # also package the world folder (extracts to <folder>/, drop into saves/)
bmr schem <world> out.schem [--area=x0,z0,x1,z1] [--y=y0,y1] [--no-trim] [--verify]
                                        # Sponge v3 .schem for WorldEdit/FAWE; trims to non-air by default
py -3 tools/check_schem.py out.schem    # independent spec validator (own NBT parser)
```
Library prerequisites: `py -3 tools/make_world.py debug && py -3 tools/mirror_fixture.py debug`, plus
`template-void`. Any fixture: `py -3 tools/make_world.py <f> && py -3 tools/mirror_fixture.py <f>`, then
`py -3 tools/reverse_fixture.py <f> [--regen work/worlds/regen-<f>/world]` (regen: `tools/regen_world.py <f> --seed S`).
Block-state coverage: `debug` (every state, isolated) and `context` (states among neighbours; its commands.txt is
generated by `py -3 tools/gen_context.py`) — reverse both after matcher changes and compare `rendered_alike`.
Dimensions: `nether` and `end` fixtures (a fixture's `dimension`, rendered with BlueMap's default map for it);
`py -3 tools/check_dimensions.py` pulls a three-map site into one world and scores each dimension.
Tools run `$BMR_EXE` if set (e.g. a build in another `--target-dir` while another session uses target/release).

Biomes: the `biomes` fixture (one patch per overworld biome, `py -3 tools/gen_biomes.py`, commands generated per
version) is where packs learn biome tints; `bmr reverse` learns them from work/cache/biomes directly.
## Test loop
Iterate with debug builds (`cargo build`, binary `target/debug/bmr.exe`); deps are optimised in the dev profile.

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
- `docs/performance.md` — benchmarking/profiling tools, method, and the performance design decisions.
- `research/01-bluemap-web-format.md` — what a BlueMap site exposes: URL layout, PRBM format, tiles, textures.json, lowres PNGs, what's culled.
- `research/02-model-inversion.md` — how BlueMap turns block states into meshes and how to invert it.
- `research/03-rust-and-tooling.md` — crates, BlueMap CLI usage, test-world generation, version pins.
- `research/04-filling-hidden-data.md` — recovering what tiles don't contain: seed cracking, regen+merge, light/AO/biome inference.

## Only use on maps you own or have permission to reverse.
