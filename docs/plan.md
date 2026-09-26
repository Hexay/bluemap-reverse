# Plan — bluemap-reverse

Goal: given only a BlueMap web URL, reconstruct the Minecraft Java world **as close to the original as possible**.
Measured by scoring against worlds we own (render → serve → reverse → diff).

Evidence for every claim here lives in `research/01..04`. This file is decisions + order of work.

## Pinned versions

| Thing | Pin | Note |
|---|---|---|
| Minecraft | 26.3 (data version 5023) | Avoid 26.4 snapshots (biome storage change). 26.1+ moved dims to `dimensions/minecraft/<dim>/`. |
| BlueMap CLI | 5.27 | Source read at commit `60e733f`. |
| Java | 25 | Needed by MC 26.x; confirm BlueMap CLI runs on it. |
| Rust | stable, edition 2024 | |

## Core insight

A BlueMap site leaks more than it looks like (research/01, 02):
- `textures.json` gives the **texture name per material index** — face → texture is exact, no image matching.
- Triangles are emitted in block order (x↑, z↑, y↓), stable-sorted by material → faces can be attributed to cells.
- Model variant choice and plant offsets are deterministic position hashes → free verification.
- Per-face sunlight/blocklight, per-vertex AO, biome tint, redstone power are baked in.
- Lowres PNGs encode a heightmap + blocklight for the whole map.

So the core is not "guessing from pictures" but **inverting a deterministic renderer**: port BlueMap's
model renderer to Rust, precompute each block state's face signature, match, then verify by re-rendering.

What is truly gone: fully enclosed blocks, dark faces below `remove-caves-below-y` (overworld default 55),
block-entity NBT (sign text, inventories, banner patterns), entities. Those get filled from priors or the seed.

## Inversion by learned signatures (replaces "port the renderer first")

Instead of porting BlueMap's model renderer before inverting (old phase 5), the library is **learned from
BlueMap itself**: render the vanilla debug world (every state once, isolated), read true states from the
world file and the faces from the tiles → `state → face signature`. Matching can't drift from BlueMap's
real output. The model port is only needed later for round-trip verification (phase 10).

- Face key = quad (not triangle: rotated variants move the diagonal), texture, tinted?, cell-local vertices
  at 1/64. UVs ignored (random rotations). Owner cell = just behind the face along its normal.
- Match: exact signature → offset-normalised (random plant offsets, ±1 jitter) → partial (only boundary
  faces may be missing = culled by a neighbour). Ties: nearest tint (redstone power), then closest to default.
- Overhang: faces outside their block's cell are stored per entry and removed from neighbours after matching.
- Evidence (`bmr-invert/src/evidence.rs`): missing cullable face ⇒ neighbour full opaque (solid); missing liquid
  face ⇒ same liquid or full block; drawn face ⇒ neighbour not a full block (open). Liquid faces are never
  matched by key (their shape depends on neighbours); waterlogged comes from evidence.
- Fill (`bmr-fill`): per-column gaps between observed cells, classified solid/liquid/air from evidence
  (streaming, scales with columns). Above the cave cut-off (`remove-caves-below-y`, default 55, not published
  by the site → `--cave-y`) a solid gap is all full blocks; below it dark cells may be anything.
- With the seed (`tools/regen_world.py` + `--regen`): unseen cells decided per cell, regen vs evidence
  (`bmr-fill/src/regen.rs`); biomes copied; observed blocks adopt regen states that render identically
  (kelp age, leaves distance). Without it: material priors + deep-water floor estimate.
- Irreducible: edits BlueMap cannot see (dark tunnels, buried changes), worldgen order non-determinism
  (ore/stone blobs, seagrass across chunk borders).

## Versions (packs)

- One pack per Minecraft + BlueMap version (`tools/build_pack.py`); `bmr pull` ranks installed packs by
  how much of the site's `textures.json` they know (Minecraft-version fingerprint; 26.3 vs 1.21.11 site:
  100% vs 89.5%), then overlap, then BlueMap version.
- Version-sensitive output, learned per pack: palette encoding (legacy `{Name,Properties}` ≤ 26.2 vs 26.3
  compact) from the debug world; folder layout (`dimensions/` from 26.1) from the template.
- Distribution: `index.json` + packs on the GitHub release `packs`; the index carries each pack's texture
  fingerprint (bitset over a shared name table), so `pull` ranks indexed packs without downloading them and
  fetches only the winner (sha256-verified). Format and index: `bmr-pack/src/format.rs`, `index.rs`.
- Server and BlueMap need different Javas (1.21 server: 21; BlueMap 5.27: 25) — BlueMap's is read from its
  jar's class-file version. Supported: Minecraft 1.18+ (older chunk formats unread).

## Architecture (Cargo workspace)

```
crates/
  bmr-fetch    scrape webroot → local cache (settings, textures.json, hires .prbm, lowres .png)
  bmr-prbm     PRBM parser, tile → world coordinates, OBJ debug export
  bmr-model    Rust port of BlueMap resource-pack loading + ResourceModelRenderer / LiquidModelRenderer
  bmr-invert   signatures, per-cell voting, cull-rule verification, liquids, tints
  bmr-fill     hidden-data priors, biome solve, seed-based regen merge
  bmr-world    Anvil read/write (fastnbt + fastanvil), level.dat template, .schem export
  bmr-score    block-by-block diff vs original, reports
  bmr-cli      `bmr fetch | reverse | score | roundtrip`
tools/         scripts: make test worlds, run BlueMap render/serve, full loop
fixtures/      configs only; worlds and caches are git-ignored
```
Dependency direction: cli → {fetch, invert, fill, score} → {prbm, model, world}. No cycles.

## Scoring (build early — it drives everything)

Report per run, restricted to the rendered area:
- **Exact state accuracy** and **block-name accuracy** over all blocks.
- Same two metrics over **BlueMap-visible blocks only** (the ceiling for pure inversion).
- Solid/air IoU, surface (top-visible) accuracy, biome accuracy per 4×4×4 cell.
- Top-N confusion pairs (e.g. `stone → infested_stone`), per-category breakdown.
- **Round-trip**: render our output with BlueMap and diff tiles face-by-face vs the original render — catches
  errors the block diff can't weigh.

Track scores per fixture in a results file so every change shows up as better/worse.

## Test fixtures (ascending difficulty)

1. **Superflat** + hand-placed builds — first end-to-end loop.
2. **Debug world** (every block state in a grid) — golden test for `bmr-model`: our render must equal BlueMap's
   tiles exactly. Level type string for 26.3 unverified (`debug_all_block_states` vs `debug`).
3. **Vanilla seeded world**, pre-generated area — terrain, caves cutoff, water, biomes.
4. Same seed + player builds on top — tests regen-and-merge.
5. **Arnis**-generated city (OSM → MC, Apache-2.0) — dense builds, no licensing issues.
6. Downloaded showcase maps — local only, never committed, respect licenses.

## Phases

| # | Deliverable | Done when |
|---|---|---|
| 0 | `tools/` scripts: make worlds (server `--nogui`), BlueMap render + serve on 127.0.0.1:8100 | One command yields a live local map from a fresh world |
| 1 | `bmr-fetch`: enumerate tiles (lowres extent + probe; 204 = empty), cache, handle gzip by magic bytes | Full local mirror of a map |
| 2 | `bmr-prbm`: parser + OBJ export | OBJ opens in Blender and matches the web view |
| 3 | `bmr-score` + `bmr-world` read side | Scores an arbitrary world pair |
| 4 | `bmr-world` write side (region files, copied level.dat) | Hand-written blocks load in MC 26.3 without regeneration |
| 5 | `bmr-model` port | Debug-world golden test passes byte-for-byte (tolerance for float rounding) |
| 6 | `bmr-invert` v1: full cubes + common models, voting + cull check | Superflat fixture ≥ 99% visible-state accuracy |
| 7 | invert v2: liquids/waterlogging, connected blocks, variants/offset verify, redstone power, block-entity static models, double chests | Debug + seeded fixtures scored; confusion list reviewed |
| 8 | `bmr-fill` v1: priors (stone/deepslate/bedrock/water), biome solve from tints | Solid/air IoU and biome accuracy reported |
| 9 | `bmr-seed` (docs/seed.md): structures → lower 48 bits, shortcuts/cubiomes → upper 16; regen + tile-diff merge | Seeded+builds fixture: underground accuracy near regen ceiling |
| 10 | Round-trip loop + blocklight/AO inference for hidden light sources and cavities | Round-trip tile diff trending to zero |

Phases 0–4 are plumbing; the accuracy work is 5–10. Don't start 6 before 5's golden test passes.

## Known ambiguities (accept, then use priors)

- Look-alikes: infested vs plain stone, waxed vs unwaxed copper, petrified vs oak slab, double slab vs full block.
- Non-visual properties: leaf `distance`/`persistent`, note block, sapling stage — recompute from game rules or default.
- Culled neighbours: a face's absence only says "neighbour is full opaque", not which one.
- Biome tint is a 5×3×5 blend; biomes sharing colormaps are indistinguishable from tint alone.

## Settled in phase 0 (2026-09-25)

- BlueMap 5.27 CLI renders and serves fine on Temurin 25.
- Built-in webserver: `Accept-Encoding: gzip` → stored gzip + `Content-Encoding: gzip`; no header → raw PRBM;
  explicit `.prbm.gz` → gzip bytes, no header; missing tile → 204; lowres PNG plain.
- 26.3 flat worlds need explicit `generator-settings` layers (empty `{}` logs an ERROR and yields no layers).

## Settled in phase 2 (2026-09-25)

- PRBM layout confirmed against `PRBMWriter.java` v5.27 (`crates/bmr-prbm/src/parse.rs`). Textures: `flipY=false`
  (v=0 is image top); animated UVs span the top square frame.
- `bmr check-heights`: hires up-faces vs lowres lod-1 heights agree on 100% of superflat columns.
- Signs (`oak_sign`) and chests (`entity/chest/normal`) do emit geometry on 26.3 + BlueMap 5.27.
- Void-facing bottom faces of the lowest layer are emitted (bedrock ≈ half of superflat faces).

## Settled in phase 3 (2026-09-25)

- **26.3 chunk palettes changed**: entries are `{id, properties}` (all props) or a bare name = **default state**
  (wrapped `{"": name}` in mixed lists). Decoding needs `blocks.json` defaults — generated by `tools/setup.py`
  (`work/data/reports-26.3/reports/blocks.json`). Legacy `{Name, Properties}` still read. See `bmr-world/src/nbt.rs`.
- Headline metric is **occupied** accuracy (voxels non-air in either world); "all voxels" is ~99% for an empty
  reconstruction. Visible = non-air with an air neighbour, until bmr-model supplies real culling.
- Score only BlueMap-rendered columns (`--mirror`): the rendered area is smaller than the set of full chunks.
- Superflat is too easy: terrain-only regen (`superflat-bare`) already scores 99.27% occupied. Seeded fixture needed.

## Settled in phase 4 (2026-09-25)

- Writer output (26.3 palette shape, no heightmaps/light) loads in a 26.3 server without regeneration:
  `tools/check_writer.py` = copy → resave in server → score (100% except grass→dirt random ticks).
- Output worlds are seeded from `fixtures/template-void` (void: any regenerated chunk scores as missing).
- Server does not recreate missing block entities → writer emits minimal `{id,x,y,z}` per BE block
  (`bmr-world/src/block_entities.rs`; test checks all registry BE types are mapped). 26.3 beds have no BE.

## Block-state coverage (2026-09-26)

- `debug` fixture = every state isolated; `context` fixture (`tools/gen_context.py`) = states among neighbours:
  stair corners, connected fences/walls/panes/bars, redstone, doors, double chests, attached blocks, culling,
  waterlogging, a jumble of every block. `setblock` does not compute neighbour-dependent shapes, so the generator
  ports the game's rules (`tools/context_rules.py`).
- Headline for inversion: `rendered_alike` (`bmr score --pack`), which counts look-alikes as correct
  (`bmr pack lookalikes` lists them).
- States with identical faces are separated, in order, by: tint (redstone power); UVs, when the candidates
  differ in an orientation property (door hinge/open/facing, sign and head rotation, glazed terracotta,
  trapdoors); own block light, when they differ in `lit` (see `bmr-invert/src/lookalike.rs` for why only
  then); then game rules after the fill: stair-corner twin from neighbouring stairs, note block instrument
  from the block below/head above, `powered` from adjacent redstone, leaves distance, waterlogging from
  unseen surrounding water (`bmr-fill/src/{stairs,note_block,redstone,rules,waterlog}.rs`).
- Irreducible (identical render, no rule): waxed vs unwaxed copper, infested vs plain stone, double slab vs
  its full block, note pitch, random ages (fire, kelp, vines), transient states (sculk phase, bed occupied),
  partially hidden blocks whose telling face is culled (wood vs log, dropper vs dispenser).
- Overhang between neighbours that are both unmatched (stacked bars trade cap faces) is stripped by
  `overhang::strip_foreign`; interior faces with a cullface (candle on a cake) by the last-resort
  `candidates_cullable`; a gap reaching the build limit is always air.

## Dimensions (2026-09-26)

- Sites publish neither a map's dimension nor its render settings. `bmr pull` puts every map into its
  dimension of one world (map id, else BlueMap's default sky colour: `pull/plan.rs`) and assumes BlueMap's
  default map for it (`bmr-fill/src/profile.rs`): overworld caves hidden below y 55; nether `render-mask`
  hides y 90..127 (the roof) and nothing is cave-culled; end is fully drawn.
- The nether mask is not air: drawn faces next to it say nothing. Rock under the mask continues up into it;
  over open space the roof starts at y 108 (measured on the `nether` fixture: 88% of those cells right vs ~55%
  all-rock). Soul sand/soil are ~4 deep over netherrack; basalt/blackstone run deep. End bedrock is always
  built (pillars), never a terrain surface.
- `nether` fixture: 82% occupied, 99.99% rendered; `end` 97.6% (misses: obsidian inside pillars).
- Biomes without a seed (`bmr-fill/src/biomes.rs`): overworld from BlueMap's grass/foliage/water tints,
  matched per 4×4 column to a table packs learn from the `biomes` fixture (`bmr-invert/src/tints.rs`; pack
  format 5); nether from surface markers voted per 4×4×4 cell; end by distance from the main island.
  Vanilla 2.5% → 94.3%, nether 0 → 73.6%, end 100%. Cave biomes (underground) take their column's surface
  biome. Every rendered chunk is written, even if empty (else the game generates it with template biomes).

## Open questions — settle with quick experiments in phase 0/1

- Debug world level-type string on a 26.3 server.
- Unknown block states: nothing vs missing-texture model? Exact list of geometry-less vanilla models (beds, signs?).
- Animated textures: UVs span one frame or the strip?
- Minimal chunk NBT a 26.3 server accepts without regenerating.
- Rust cubiomes bindings maturity (else FFI to C cubiomes).

## Ground rules

- Only reverse maps we own or have explicit permission for. Scrape politely (rate limit, cache, never re-fetch).
- Worlds, caches and downloaded maps stay out of git.
- Files ≤ 300 lines; split by responsibility.
