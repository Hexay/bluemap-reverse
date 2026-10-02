# Architecture — bluemap-reverse

Goal: given only a BlueMap web URL, reconstruct the Minecraft Java world **as close to the original as possible**.
Measured by scoring against worlds we own (render → serve → reverse → diff).

Evidence for the claims here lives in `docs/research/01..04`. This file holds the design, the decisions and
their history.

## Pinned versions

| Thing | Pin | Note |
|---|---|---|
| Minecraft | 26.3 (data version 5023) primary; packs for 1.21.4, 1.21.8, 1.21.11 | Avoid 26.4 snapshots (biome storage change). 26.1+ moved dims to `dimensions/minecraft/<dim>/`. |
| BlueMap CLI | 5.27 | Source read at commit `60e733f`. |
| Java | 25 | Needed by MC 26.x and BlueMap 5.27. |
| Rust | 1.88+, edition 2024 | `rust-version` in `Cargo.toml`, checked in CI. |

## Core insight

A BlueMap site leaks more than it looks like (research/01, 02):
- `textures.json` gives the **texture name per material index** — face → texture is exact, no image matching.
- Triangles are emitted in block order (x↑, z↑, y↓), stable-sorted by material → faces can be attributed to cells.
- Model variant choice and plant offsets are deterministic position hashes → free verification.
- Per-face sunlight/blocklight, per-vertex AO, biome tint, redstone power are baked in.
- Lowres PNGs encode a heightmap + blocklight for the whole map.

So the core is not "guessing from pictures" but **inverting a deterministic renderer**.

What is truly gone: fully enclosed blocks, dark faces below `remove-caves-below-y` (overworld default 55),
block-entity NBT (sign text, inventories, banner patterns), entities. Those get filled from priors or the seed.

## Inversion by learned signatures

The signature library is **learned from BlueMap itself** rather than from a Rust port of its model renderer:
render the vanilla debug world (every state once, isolated), read true states from the world file and the faces
from the tiles → `state → face signature`. Matching can't drift from BlueMap's real output. A renderer port is
only needed for render round-trip verification (see Roadmap).

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
  jar's class-file version. Packs can be built for Minecraft 1.18+ (older chunk formats unread).

## Crates

```
crates/
  bmr-compress  gzip, zlib, zstd, lz4-java blocks: BlueMap storage + region chunk decompression
  bmr-fetch     scrape webroot → local mirror (settings, textures.json, hires .prbm, lowres .png)
  bmr-prbm      PRBM parser, tile → world coordinates, OBJ export, render diff
  bmr-world     Anvil read/write (fastnbt), block registry, level.dat template, .schem export
  bmr-invert    signature library, face matching, evidence, look-alikes, tints
  bmr-fill      hidden-volume fill, game rules, biome recovery, seed-based regen merge
  bmr-pack      versioned pack format, index, fingerprint-based selection
  bmr-score     block-by-block diff vs original, reports
  bmr-seed      structure detection, structure-seed crack, upper-bit search (docs/seed.md)
  bmr-cubiomes  FFI to vendored cubiomes (biome checks for the seed's upper bits)
  bmr-cli       the `bmr` binary: pull, fetch, reverse, score, pack, seed, schem + debug commands
tools/          Python: build fixture worlds, render/serve with BlueMap, build packs, run the scoring loop
fixtures/       configs only; worlds and caches are git-ignored
```

Dependency direction (no cycles):
- `cli` → everything below
- `pack` → `invert`, `fetch`, `prbm`, `world`
- `fill` → `invert`, `world`
- `invert` → `fetch`, `prbm`, `world`
- `seed` → `cubiomes`, `world`
- `score` → `world`
- `fetch`, `world` → `compress`
- `compress`, `prbm`, `cubiomes` are leaves

## Scoring

Report per run, restricted to the rendered area (`bmr score`, `bmr-score/src/report.rs`):
- **Exact state accuracy** and **block-name accuracy** over occupied voxels (non-air in either world).
- Same metrics over **BlueMap-visible blocks only** (the ceiling for pure inversion), and `rendered_alike`,
  which counts look-alikes as correct.
- Solid/air IoU, surface (top-visible) accuracy, biome accuracy per 4×4×4 cell, top-N confusion pairs.

Scores per fixture are tracked in `docs/results/` so every change shows up as better/worse.

## Test fixtures

Defined in `fixtures/*/fixture.json`; built by a real server + BlueMap (`tools/`). In ascending difficulty:

1. **Superflat** + hand-placed builds — the first end-to-end loop.
2. **Debug world** (`minecraft:debug_all_block_states`) — source of the signature library.
3. **Context** — states among neighbours (see "Block-state coverage").
4. **Vanilla seeded world**, pre-generated area — terrain, cave cut-off, water, biomes; **vanilla-edited** adds
   player builds on top to test regen-and-merge.
5. **Nether**, **End**, **biomes**, **structures** — dimension profiles, biome tables, seed recovery.

## Roadmap

| # | Deliverable | Status |
|---|---|---|
| 0 | `tools/`: make worlds, BlueMap render + serve on 127.0.0.1:8100 | Done |
| 1 | `bmr-fetch`: enumerate tiles (lowres extent + probe; 204 = empty), cache, gzip by magic bytes | Done |
| 2 | `bmr-prbm`: parser + OBJ export | Done |
| 3 | `bmr-score` + `bmr-world` read side | Done |
| 4 | `bmr-world` write side (region files, copied level.dat) | Done |
| 5 | Port of BlueMap's model renderer | Replaced by learned signatures |
| 6 | `bmr-invert` v1: full cubes + common models | Done |
| 7 | invert v2: liquids, connected blocks, variants, redstone power, block entities, double chests | Done |
| 8 | `bmr-fill`: priors, biome recovery from tints | Done |
| 9 | `bmr-seed`: structures → lower 48 bits, cubiomes → upper 16; regen + merge | Done |
| 10 | Render round-trip (re-render our output, diff tiles face-by-face) + blocklight/AO inference for hidden light sources and cavities | Measuring (`tools/roundtrip.py`); inference open |

## Known ambiguities (accept, then use priors)

- Look-alikes: infested vs plain stone, waxed vs unwaxed copper, petrified vs oak slab, double slab vs full block.
- Non-visual properties: leaf `distance`/`persistent`, note block, sapling stage — recompute from game rules or default.
- Culled neighbours: a face's absence only says "neighbour is full opaque", not which one.
- Biome tint is a 5×3×5 blend; biomes sharing colormaps are indistinguishable from tint alone.

## Decision log

### Rendering and serving (2026-09-25)

- BlueMap 5.27 CLI renders and serves fine on Temurin 25.
- Built-in webserver: `Accept-Encoding: gzip` → stored gzip + `Content-Encoding: gzip`; no header → raw PRBM;
  explicit `.prbm.gz` → gzip bytes, no header; missing tile → 204; lowres PNG plain.
- 26.3 flat worlds need explicit `generator-settings` layers (empty `{}` logs an ERROR and yields no layers).

### PRBM (2026-09-25)

- PRBM layout confirmed against `PRBMWriter.java` v5.27 (`crates/bmr-prbm/src/parse.rs`). Textures: `flipY=false`
  (v=0 is image top); animated UVs span the top square frame.
- `bmr check-heights`: hires up-faces vs lowres lod-1 heights agree on 100% of superflat columns.
- Signs (`oak_sign`) and chests (`entity/chest/normal`) do emit geometry on 26.3 + BlueMap 5.27.
- Void-facing bottom faces of the lowest layer are emitted (bedrock ≈ half of superflat faces).

### World reading and scoring (2026-09-25)

- **26.3 chunk palettes changed**: entries are `{id, properties}` (all props) or a bare name = **default state**
  (wrapped `{"": name}` in mixed lists). Decoding needs `blocks.json` defaults — generated by `tools/setup.py`
  (`work/data/reports-26.3/reports/blocks.json`). Legacy `{Name, Properties}` still read. See `bmr-world/src/nbt.rs`.
- Headline metric is **occupied** accuracy (voxels non-air in either world); "all voxels" is ~99% for an empty
  reconstruction. Visible = non-air with an air neighbour.
- Score only BlueMap-rendered columns (`--mirror`): the rendered area is smaller than the set of full chunks.
- Superflat is too easy: terrain-only regen (`superflat-bare`) already scores 99.27% occupied. Seeded fixture needed.

### World writing (2026-09-25)

- Writer output (26.3 palette shape, no heightmaps/light) loads in a 26.3 server without regeneration:
  `tools/check_writer.py` = copy → resave in server → score (100% except grass→dirt random ticks).
- Output worlds are seeded from `fixtures/template-void` (void: any regenerated chunk scores as missing).
- Server does not recreate missing block entities → writer emits minimal `{id,x,y,z}` per BE block
  (`bmr-world/src/block_entities.rs`; test checks all registry BE types are mapped). 26.3 beds have no BE.

### Block-state coverage (2026-09-26)

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

### Dimensions (2026-09-26)

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

### Render round-trip (2026-10-02)

- `tools/roundtrip.py` copies a reconstruction, resaves it in the server (bmr writes no light), renders it with
  the fixture's map config and diffs it against the original mirror (`bmr diff-render`, `bmr-prbm/src/diff/`).
  Faces pair by geometry; faces looking into columns the original didn't render are skipped (the
  reconstruction ends there, so BlueMap draws its outer walls). Reports in `docs/results/<fixture>-roundtrip.json`.
- `superflat`: every face pairs, identical except sunlight on the underside of the bedrock floor (light below
  the world, unseen).
- `vanilla-edited`: 99.82% of original faces pair, 87% identical; above the cave cutoff (y 55) almost nothing
  is missing or extra. The ~98k extra faces below it were mostly air gaps the fill ran down to bedrock under
  seabeds (one open cell at the top made the whole gap air). Fix: air gaps turn solid below their lowest
  evidence and the cutoff (`bmr-fill/src/columns.rs`, `dark_below`): extra faces 98k → 59k, vanilla occupied
  74.4% → 75.2%, solid IoU 94.1% → 95.2%; superflat loses 4 cells (dark air inside builds now reads solid).
- Left: 31k bedrock undersides at y -64 and sunlight on 11.6% of paired faces are relight artifacts of the
  round trip (light below the world), not fill errors. AO 1.3% and blocklight 0.15% (hidden light sources)
  are the inputs for the inference half of phase 10.
- `nether`: 96.7% identical, 41k extra faces at y 48..95 (open caves under the roof mask filled solid, lava
  filled up to the mask). Two evidence bugs (`bmr-invert/src/evidence.rs`, `face.rs`): a cell with both a
  drawn face towards it and a missing one voted solid (a drawn face is proof, a missing one may be an imperfect
  match: `open` now overrides `solid`); and a flowing liquid's sloped surface wasn't seen as facing up, so the
  cell above read as more liquid. After: 98.7% identical, 7.9k extra, blocklight 2.9% → 0.9%.
- Then overhang (`bmr-invert/src/overhang.rs`): a floor fire's side planes land in the air beside it and match
  as wall fire, whose model "overhangs" back, so the fake fire stripped the real fire's sides; the bare fire
  then claimed its neighbours solid. Cells made only of explained overhang no longer claim; when two such
  cells explain each other (a spawner's inner faces in all six neighbours), the one showing more of its
  signature is the real block. `strip_foreign` tries dropping duplicate planes before own faces. Nether:
  98.9% identical, extra 7.9k → 253 (as many missing: shape differences, not wrong blocks); occupied 82.26%.
  The fill also keeps void under floating blocks as air when nothing below the cave cutoff was seen (the
  first `dark_below` cut `debug` from 12% to 0.6% occupied). `end`: 99.99% identical.
- `bmr explain --mirror <m> x,z` prints a column's matched blocks and fill evidence.

### Seed recovery bindings

- No mature Rust cubiomes bindings; `bmr-cubiomes` compiles the vendored C sources (xpple fork) and wraps the
  few calls `bmr-seed` needs.

## Ground rules

- Only reverse maps we own or have explicit permission for. Scrape politely (rate limit, cache, never re-fetch).
- Worlds, caches and downloaded maps stay out of git.
- Files ≤ 300 lines; split by responsibility.
