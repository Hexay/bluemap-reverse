# Improvement audit: bluemap-reverse

Read-only audit, 2026-11-07. Sources: docs/architecture.md, README.md, CHANGELOG.md [Unreleased],
docs/results/*.json + history.jsonl + bench.jsonl, docs/performance.md, docs/seed.md,
research/bluemap-5.29-delta.md, a sweep of crates/*, and .github/workflows/*.

**Status 2026-11-08:** done: A1, A2 (nether: basalt deltas; overworld measured, layering already matches),
A3, A4, A5 (`--seed`), A6, A7, F1a (`pull --crack-seed`), F2, F3, F5, F6, R1–R4, C1 (material tests), C8. Open:
F1b (managed regen), F4, F7, R5, C2–C7, C9.

Ranking = impact × (1 / effort). Effort: XS < 2 h, S ≈ half a day, M = 1–3 days, L = a week or more.
"pp" = percentage points of the fixture's occupied voxels (`occupied.total`), taken from the result JSON's
`confusions`. Gains are estimates, worked out from those confusion counts.

## Where accuracy is lost (from docs/results)

| Fixture | Occupied | Biomes | Largest losses (pp of occupied) |
|---|--:|--:|---|
| vanilla (no seed) | 75.35% | 95.74% | air→deepslate 4.08, tuff→deepslate 3.30, water→stone 2.44, andesite/diorite/granite→stone 6.8, water→deepslate 0.80, gravel→deepslate 0.74, bedrock↔deepslate 0.89, deepslate↔stone 0.87, dirt→stone 0.57, stone→sand 0.43 |
| nether | 82.26% | 73.58% | air→netherrack 3.45, netherrack→air 1.93, basalt→netherrack 1.81, bedrock↔netherrack 2.71, soul_sand/soil→netherrack 2.12, blackstone→netherrack 0.98, gravel→netherrack 0.50 |
| end | 97.61% | 100% | obsidian→end_stone 2.39 (the only confusion) |
| context | 97.51% | 99.81% | dirt→bedrock 1.38, grass_block→deepslate 0.66 (together 82% of all misses) |
| vanilla biomes | | | deep_lukewarm_ocean→lukewarm 8 928 cells, beach→plains 6 240, river→plains 4 896 (of 21 216 misses) |
| nether biomes | | | soul_sand_valley→nether_wastes 71 235 of 87 640 misses (81%) |

The decision log (2026-10-02) already shows that most vanilla losses cannot be fixed without the seed: rock
variety blobs, hidden dry caves and buried aquifers leave nothing on the map. With `--regen` the vanilla
fixture scores 99.93%. That makes seed→regen automation the biggest lever for vanilla. Nether and End still
have tractable losses without a seed.

---

## 1. Accuracy

### A1. Nether biomes: spread decided cells in 3D instead of a vertical-only lookup. Impact high, effort S
- **Evidence:** nether.json biomes 244 136 / 331 776 = 73.6%. 81% of the misses are soul_sand_valley→nether_wastes.
  `bmr-fill/src/biomes.rs:86-95`: a cell with no marker evidence looks only straight up/down (±15 cells), then
  falls back to `NETHER[0]` (nether_wastes). `nether_cells` (`:100-121`) decides only the 3×3×3 neighbourhood of
  cells that carry votes.
- **Approach:** do a BFS out from the decided cells, horizontally as well as vertically, bounded like the
  overworld `SPREAD` (`:47`). Also check whether netherrack's 0.2 vote (`:38`) outweighs sparse soul-sand votes
  in SSV walls. Measure on the `nether` fixture.
- **Estimate:** most of the 71k SSV cells sit next to SSV surface cells. Expect roughly 73.6% → 88–92%.

### A2. Biome-conditioned material priors (nether first, then overworld). Impact high, effort M
- **Evidence:** in the nether, basalt→netherrack 1.81pp, blackstone→netherrack 0.98, soul_sand/soil→netherrack
  2.12, magma→netherrack 0.27, gravel→netherrack 0.50. The reverse errors (netherrack→basalt/blackstone/soul_*
  ≈1.65pp) show the fixed depths are both too deep and too shallow. `bmr-fill/src/material.rs:119-137` takes the
  layer depth from the surface block only. The comment at `:126` says "desert sandstone would need the biome".
  The biome map is already computed in the same crate (`biomes.rs`).
- **Approach:** pass `Biomes` into `solid_segments`. In basalt_deltas cells, default to basalt/blackstone at every
  depth instead of netherrack. Use soul_sand_valley-specific soul-soil depth. On the overworld, use
  desert/badlands/beach/ocean-floor depths (stone→sand 0.43pp, dirt→stone 0.57pp, gravel→stone/deepslate
  1.2pp). Learn the per-biome medians from the fixtures, as the existing sand/dirt medians were learned.
- **Depends on** A1 for the nether. **Estimate:** nether +3–5pp occupied, vanilla +0.5–1.5pp.

### A3. End pillar interiors. Impact medium, effort S
- **Evidence:** end.json has a single confusion, obsidian→end_stone 17 442 (2.39pp), which the decision log
  explains as "obsidian inside pillars". `material.rs:49-51` treats End bedrock as built and sends the gap to
  `copy_nearest` (`:173-203`). That function prefers pass-1 terrain columns (end_stone) over observed obsidian
  walls in the same ring.
- **Approach:** for a gap that is enclosed and whose observed horizontal shell at the same y is one non-terrain
  full block, fill it with that block. Prefer `full_cube_at` over `from_pass1` when the gap's own column is
  surrounded by it. Check with `bmr explain` on a pillar first. The same rule also helps thick player walls
  (concrete, wool).

### A4. Superflat-aware floor. Impact medium on flat maps, effort S
- **Evidence:** in context.json, dirt→bedrock 1.38pp and grass_block→deepslate 0.66pp make up 82% of the
  fixture's misses. The fixture is a flat world (`fixtures/context/fixture.json`: 1 bedrock layer).
  `profile.rs:51-55` always puts bedrock at min_y..min_y+2 and deepslate at y ≤ 3, which is wrong for flat
  worlds. Flat creative servers are a common real-world case.
- **Approach:** detect a flat world (constant terrain surface y across the rendered columns, plus the void-facing
  bottom faces the decision log says are emitted). Learn the layer stack from columns where it is exposed (map
  edge, holes), else assume vanilla's classic flat preset. Expose the result as a `Profile` variant.

### A5. Seed-derived bedrock and deepslate gradients without Java. Impact medium, effort M
- **Evidence:** bedrock↔netherrack 2.71pp in the nether. In vanilla, bedrock↔deepslate 0.89pp and
  deepslate↔stone 0.87pp. `profile.rs:48-59` already picks the majority per y, so these losses cannot be fixed
  without the seed. Today, having the seed only helps through `--regen`, which needs Java and a server
  (`tools/regen_world.py`).
- **Approach:** once `bmr seed` has produced a seed, compute vanilla's `vertical_gradient` surface rules
  (positional Xoroshiro random keyed by `bedrock_floor`/`bedrock_roof`/`deepslate`) natively in Rust. This is a
  small, self-contained port and a first step toward Java-free regen.
- **Estimate:** nether +2.7pp, vanilla +1.7pp when a seed is known.

### A6. Overworld ocean and river biomes from regional features. Impact low–medium, effort M, uncertain
- **Evidence:** deep_lukewarm→lukewarm 8 928, beach→plains 6 240 and river→plains 4 896 are 95% of the vanilla
  biome misses (4.3pp). `biomes.rs:137-138` notes that per-column water depth overlaps between the classes.
- **Approach:** try features over a wider window: mean floor depth over 32×32 blocks or distance to the coast
  (deep ocean), the width of a water channel between land (river), and sand bordering ocean (beach, relaxing the
  per-4×4 `sandy_columns` majority at `:162-179`). Run this as a measured experiment and drop it if the
  classes still overlap.
- **Result 2026-11-08 (positive, kept):** among biomes tied on colour, `biomes.rs::water_biome` picks by the
  seabed: sand over 2% of the 20-block window → beach; water over 95% of the 68-block window → ocean, or deep
  ocean when the mean floor is 24+ below sea level; else river. Lukewarm, cold and frozen oceans take the same
  depth cut. Biome accuracy: vanilla 95.737 → 97.029, `biomes` 51.875 → 51.938; occupied unchanged everywhere.
- **Structures not yet scored with this rule** (the run was stopped for memory). Offline, on that fixture's dumped
  tints and surface biomes, the rule gets 17 148 more of 76 092 tinted columns right. The thresholds were chosen
  on that data and vanilla together, so this is not a held-out figure. Cold and frozen oceans were not measured.
- **Limits:** vanilla's 8 928 deep_lukewarm misses remain: its deep columns are not deeper than its shallow ones
  (depth AUC 0.28–0.62), and a lower cut costs vanilla. At 24, 13 717 of 26 664 deep_lukewarm columns on
  structures stay lukewarm; a cut of 22 fixes more there but gives back 13 columns on vanilla.

### A7. Nether roof band (masked y 90..127). Impact medium, effort M, research
- **Evidence:** air→netherrack 3.45pp and netherrack→air 1.93pp. The fixed `roof_from: 108`
  (`profile.rs:35`) scores 88% in the band.
- **Approach:** fit P(rock | y, distance to the nearest drawn rock under the mask, local ceiling slope) on the
  fixture instead of using one cut. The ceiling is truly hidden, so expect at most +1–2pp.
- **Result 2026-11-08 (small positive, kept):** open columns within 3 steps of a column with rock reaching the
  mask lower the cut by `4 - distance` (`columns.rs::roof_drop`). Nether occupied 82.913 → 82.945, exposed 88.063
  → 88.274, solid IoU 94.091 → 94.144; other fixtures unchanged. Measured with
  `crates/bmr-score/examples/roof_band.rs`. Leave-one-quadrant-out on the fixture: +0.033pp in total, positive in
  3 of 4 quadrants (one fixture only: no second nether world exists).
- **Dropped (held-out occupied, per-bucket cuts):** distance buckets −0.019pp, floor height −0.123, biome −0.239,
  distance + biome −0.342; rock fraction within 8 or 24 columns +0.05 but more parameters than the ramp. Cells
  of the band stay ambiguous at any feature tried: expect tenths of a pp at most, not the +1–2pp estimated above.

### Don't pursue
Andesite, diorite, granite and tuff blobs, ores, hidden dry caves and buried aquifers are seed-only. That is
about 20pp of vanilla. Picking stone or deepslate per cell is already the maximum-likelihood guess under the
occupied metric (the decision log, 2026-10-02, reaches the same conclusion). Block-light inference
(roadmap #10) can recover "at most a few hundred" blocks, per the same entry.

---

## 2. Features / UX

### F1. One-command seed → regen. Impact very high (vanilla 75% → ~99.9%), effort M (wrapper) / L (managed server)
- **Evidence:** docs/seed.md:9-15 describes four manual steps across `bmr pull`, `bmr structures`, `bmr seed` and
  `tools/regen_world.py` (Python + Java). README:8 promises "No server, no Java". The regen result is 99.926%
  occupied (history.jsonl line 121). On the `structures` fixture, seed recovery is right from ~50 observations
  in about 10 s.
- **Approach:** (a) S: `bmr pull --crack-seed` runs detection and cracking after reconstruction, prints the seed
  and the exact regen command, and writes the seed into level.dat. (b) M–L: `bmr regen` downloads the vanilla
  server jar and a JRE, pre-generates exactly the rendered chunks, then re-runs the fill with `--regen`. The
  Python `console.py` and `regen_world.py` already show the procedure. Keep it opt-in because it needs Java.

### F2. Infer map render settings from the tiles. Impact high on real sites, effort M
- **Evidence:** `cave_y` and `mask` always come from BlueMap defaults (`bmr-fill/src/profile.rs:27-42`), with
  `--cave-y`/`--mask-y` overrides (`bmr-cli/src/reverse.rs:61-79`). The decision log says "Sites publish neither
  a map's dimension nor its render settings". Every fixture uses the defaults, so the accuracy table does not
  cover customised sites. Customised `remove-caves-below-y`, `min-y`/`max-y` and nether masks are common.
- **Approach:** estimate from the mirror before filling. Dark faces (sunlight 0, blocklight 0) below 55 mean caves
  are rendered. Any face inside 90..127 in the nether means there is no roof mask. The min/max drawn y bounds the
  render range. Print what was inferred and warn when it contradicts the defaults.

### F3. `--area` / `--radius` for `pull` and `fetch`. Impact high for big public maps, effort M
- **Evidence:** `pull` has no spatial limit (`bmr-cli/src/pull.rs:24-53`), while `schem` has `--area`. Mirroring
  is a flood fill over the whole site (`bmr-fetch/src/mirror.rs:120-138`).
- **Approach:** clip the seed tiles and the `discover` frontier to a block box, and pass the same box to
  reconstruction and windowing.

### F4. Test and ship packs for older BlueMap 5.x. Impact medium, effort M (mostly machine time)
- **Evidence:** every shipped pack is BlueMap 5.27 (README:142-149). research/bluemap-5.29-delta.md shows that
  5.28/5.29 need nothing. Older 5.x versions, which many servers still run, are untested.
- **Approach:** `tools/build_pack.py --bluemap <v>` for a few widely deployed 5.x versions, then
  `tools/check_version.py` on each. Record which versions share signatures so that identical packs are not
  shipped.

### F5. Seed detection for villages, igloos, swamp huts and mansions. Impact medium, effort M–L
- **Evidence:** docs/seed.md:24-25 lists these as TODO. Maps without monuments or temples cannot be cracked today.

### F6. Concurrent windows for large maps. Impact medium (wall time), effort M
- **Evidence:** docs/performance.md:46-49 "Next candidates". Windows run serially (`bmr-cli/src/window.rs`), at
  about 7 s wall per 512² region (bench.jsonl `final-current-vanilla-512`). A 10k×10k map would take ~45 min.
  `invert.gather` merges serially, 0.88 s of that 7 s.
- **Approach:** run windows in parallel and merge per-window `StateTable`s, capped by memory (each window is about
  375 MB peak).

### F7. Liquid-inset tolerance in `diff-render`. Impact low, effort XS
- **Evidence:** research/bluemap-5.29-delta.md:24,52-53. The next BlueMap release insets liquid faces by 0.001
  blocks, and the `bmr-prbm/src/diff` QUANT of 1/4096 will stop pairing them.

---

## 3. Robustness

### R1. Pack decode indexes unchecked file data. Impact medium, effort XS–S
- **Evidence:** `bmr-pack/src/format.rs` `decode()` uses `textures[k.tex as usize]` (:117), `body.registry[d.block
  as usize]` (:124), `body.sigs[d.sig]` / `keys[k]` (:126), `body.uv_sets[d.uvs]` (:131) and `keys[k]` (:134).
  A corrupt or third-party `--pack` therefore panics, even though `split()` (`lib.rs:169-181`) validates the
  header with errors.
- **Approach:** make `decode` return `Result` and use `.get(..).context(..)`.

### R2. HTTP response cap of 1 GiB per request. Impact medium, effort XS
- **Evidence:** `bmr-fetch/src/http.rs:13,70,73`. `MAX_BODY = 1 << 30` applies both to the download and to
  decompression. With `--concurrency 4`, a hostile or broken host can push about 4 GiB of allocations. Real PRBM
  tiles are well under a megabyte.
- **Approach:** set a per-kind limit (for example 64 MiB for tiles, less for JSON and PNG).

### R3. Retry policy and single-tile failure. Impact medium, effort S
- **Evidence:** `http.rs:44-59` retries every error three times with 0.5 s and 1 s sleeps. It ignores
  `Retry-After`, treats 429/503 like any other error, and retries permanent 4xx responses. In `mirror.rs:122-137`,
  one tile that keeps failing aborts the whole layer through `?`. The manifest is saved, so the mirror resumes,
  but `pull` dies.
- **Approach:** classify errors as transient or permanent and honour `Retry-After` with exponential backoff.
  Record failed tiles in the manifest and continue with `--max-failures N`.

### R4. Fuzz the parsers that read untrusted input. Impact medium, effort S
- **Evidence:** there is no `fuzz/` directory. The entry points are `bmr_prbm::parse` (parse.rs:22),
  `bmr_compress::decompress_any`, `LowresImage::decode` (lowres.rs:15), `region::read_region_where`, pack
  `split`/`decode`, and textures.json/settings.json. The hand-written guards are good (lz4_block.rs:56-64,
  region.rs:74-77, nbt.rs:104-120), and R1 was found by reading the code, which suggests a fuzzer would find more.
- **Approach:** add `cargo fuzz` targets and a weekly CI job that runs each for a few minutes.

### R5. `check-heights` indexes the lowres image by grid coordinates. Impact low, effort XS
- **Evidence:** `bmr-cli/src/check_heights.rs:55-58` passes `(x - mx, z - mz)` into `img.color()`, which slices
  `rgba` without a bounds check (`lowres.rs:29-31`). A tile whose size differs from settings.json panics. The
  command is hidden.

Panic sweep: non-test code has no TODO, FIXME, `todo!`, `unimplemented!` or `panic!`. The one `unreachable!`
(`bmr-invert/src/matcher.rs:176`) guards a loop invariant. Every non-test `unwrap()` on input bytes sits behind a
length check (region.rs:31/75, lz4_block.rs:51, pack lib.rs:171). The remaining unwraps are `min`/`max` over
non-empty sets or RwLock unwraps (`texture.rs:41-56`), which are fine.

---

## 4. Code health

### C1. Unit tests where the accuracy work happens. Impact high, effort M
- **Evidence:** in bmr-fill (1 746 lines), only `columns_tests.rs` (11), `stairs.rs` (1) and `note_block.rs` (1)
  have tests. `material.rs`, `biomes.rs`, `regen.rs`, `redstone.rs`, `rules.rs`, `profile.rs` and `lib.rs` have
  none, and that is where A1–A5 will land. Also untested: `bmr-pack` `select.rs`/`index.rs` (pack ranking by
  fingerprint), `bmr-fetch` `http.rs`/`mirror.rs`/`store.rs`/`settings.rs`, and `bmr-cli` (2 100 lines, 1 test),
  including `window.rs`, whose halo logic is guarded only by the local `tools/check_equiv.py`.
- **Approach:** add synthetic-column tests for `layer_profile`/`copy_nearest`/biome spread, a test that windowed
  output equals `--no-window` output on a synthetic map, a pack-ranking table test, a `mirror` test against a
  local `tiny_http` server serving fixture bytes, and `assert_cmd` tests for exit codes (`seed` exits 3).

### C2. CI has no accuracy regression gate (see CI gaps below). Impact high, effort M

### C3. Files close to the 300-line cap. Impact low now, effort S each
No `.rs` or `.py` file is over 300 lines; the largest is 294. The ones close to the cap:

| Lines | File |
|--:|---|
| 294 | crates/bmr-invert/src/face.rs |
| 283 | crates/bmr-invert/src/library.rs |
| 269 | crates/bmr-fill/src/columns.rs |
| 268 | crates/bmr-pack/src/format.rs |
| 263 | crates/bmr-score/src/report.rs |
| 255 | tools/context_scenes.py |
| 254 | tools/profile.py |
| 249 | crates/bmr-seed/src/detect/rules.rs |

Split `face.rs` (key/quantisation vs owner/evidence helpers) and `library.rs` before the next feature lands in
them. `material.rs` (203 lines) will grow with A2–A4, so plan for `material/{layers,copy}.rs`.

### C4. Duplicated "default state for a block name". Impact low, effort XS
- **Evidence:** the same closure appears in `bmr-fill/src/material.rs:41-44` and `bmr-fill/src/lib.rs:81-84`, and
  the same lookup in `bmr-world/src/nbt.rs:66`.
- **Approach:** add `BlockRegistry::default_state(name) -> BlockState` to bmr-world.

### C5. Coordinate math scattered across crates. Impact low, effort S
- **Evidence:** there are about 25 inline `div_euclid(16|32|512)` calls in 12 files (bmr-world
  sparse/schem/world_write/chunk, bmr-cli window/schem/probe/explain/reconstruct, bmr-invert tints/library,
  bmr-fill regen, bmr-score compare). `region_of` is defined locally in `bmr-cli/src/window.rs:33`.
- **Approach:** add a `bmr_world::coords` module with `chunk_of`, `region_of`, `section_of` and `local`.

### C6. Dimension ids as bare strings. Impact low, effort S
- **Evidence:** `"minecraft:the_nether"` and similar appear in `bmr-fill/src/profile.rs:29,37`,
  `bmr-cli/src/pull/plan.rs:37` and `bmr-cli/src/main.rs:103,153`.
- **Approach:** parse once into a `Dimension` enum in bmr-world. `Profile::for_dimension` currently treats any
  unknown string as the overworld.

### C7. Three test-layout conventions. Impact low, effort XS
The repo uses `#[path = "x_tests.rs"]` siblings (bmr-invert, bmr-prbm, bmr-fill), `mod tests;` directories
(`bmr-world/src/region/tests.rs`, `bmr-prbm/src/diff/tests.rs`) and inline `mod tests {}` (the rest). Pick one.
Sibling files fit the 300-line rule best.

### C8. Windows-era instructions in docs and error messages. Impact low, effort S
- **Evidence:** docs/performance.md:9-17 (`py -3`, `bmr.exe`, samply + Windows ADK/UAC, MSVC `/MAP`),
  docs/development.md (`py -3` throughout, `target/debug/bmr.exe` at :5), docs/seed.md:14, and error strings in
  `bmr-world/src/world_write.rs:32`, `bmr-world/src/block_entities.rs:96` and `bmr-seed/src/structure_set.rs:120`.
- **Approach:** use `python3`/`uv run`, and Linux `perf`/samply for profiling.

### C9. Stale or inconsistent evidence. Impact medium (decisions rest on it), effort S
- `docs/results/structures.json` dates from 2026-09-26 (history.jsonl:80) and reports 4.59% biomes, from before
  biome recovery and pack format 5. Re-run it.
- The README's "with seed 99.93%" was last measured on 2026-09-26 (history.jsonl:121), before the layered fill
  and AO evidence of 2026-10-02. There is also no `vanilla-regen.json`. Re-run with `--regen`.
- bench.jsonl stops on 2026-09-25, before AO evidence (`bmr-invert/src/ao.rs`) and the layered fill, so their
  cost is unmeasured.
- docs/architecture.md:212 says vanilla biomes are 94.3%; the README and vanilla.json say 95.74%.
- history.jsonl records no biome accuracy, so biome regressions do not show up in the history. The `biomes`
  fixture has no result file.

---

## CI gaps (.github/workflows)

1. **No accuracy gate.** Fixtures need Java, a server and BlueMap, and their worlds are git-ignored. Results are
   produced locally and committed by hand. Options: (a) commit a small cropped mirror plus its truth region (a few
   hundred KB, e.g. part of `context` and `superflat`) and add a `cargo test` that reverses it with the committed
   26.3 pack and asserts score floors; (b) a nightly workflow with `setup-java` and cached jars that runs
   `tools/reverse_fixture.py` on superflat and vanilla and fails on regressions against docs/results.
2. **Committed packs are not validated.** No test loads `packs/*.pack` against `FORMAT = 5`
   (`bmr-pack/src/format.rs:17`) or checks `packs/index.json` fingerprints and sha256 against the files. A format
   bump without a rebuild would ship packs the bundled binary cannot read.
3. **Ignored tests never run.** `block_entities.rs:93` and `structure_set.rs:117` need `work/data` from
   `tools/setup.py`. A nightly job could run setup and then `cargo test -- --include-ignored`.
4. **Python tools are only byte-compiled** (`ci.yml:62-70`). There are no tests and no lint (ruff) for 2 672
   lines, including `context_rules.py`, which ports game rules.
5. **The release does not smoke-test its binaries.** `release.yml` builds and publishes without running
   `bmr --version` or `bmr pack list` against the bundled `packs/`.
6. **Minor:** the MSRV job only runs `cargo check` (`ci.yml:60`). `ci.yml` has no `permissions:` block and no
   `concurrency` cancel. Actions are pinned by tag, not SHA. There is no fuzz job (R4).

---

## Ranked shortlist (impact × effort)

| # | Item | Group | Effort |
|--:|---|---|---|
| 1 | A1 Nether biome 3D spread (73.6% → ~90%) | accuracy | S |
| 2 | R1 + R2 Pack decode bounds, HTTP body cap | robustness | XS–S |
| 3 | F1a `pull --crack-seed` (print the seed and the regen command) | UX | S |
| 4 | A3 End pillar interiors (+2.4pp End) | accuracy | S |
| 5 | A4 Superflat-aware floor (+2pp context, flat servers) | accuracy | S |
| 6 | CI-2 Validate committed packs and index | CI | XS |
| 7 | A2 Biome-conditioned priors (+3–5pp nether) | accuracy | M |
| 8 | F2 Infer cave-y, mask and y-range from tiles | UX/robustness | M |
| 9 | C1 + CI-1 Fill/pack/window unit tests, mini-fixture accuracy gate | code health | M |
| 10 | R3 Retry classification, tolerate failed tiles | robustness | S |
| 11 | F3 `--area` for pull/fetch | UX | M |
| 12 | F1b Managed regen (Java-backed, vanilla → ~99.9%) | feature | L |
| 13 | C9 Re-run stale results; add biomes to history | code health | S |
| 14 | A5 Native bedrock/deepslate gradients from the seed | accuracy | M |
| 15 | R4 Fuzz targets | robustness | S |
| 16 | F4 Older BlueMap 5.x packs | feature | M |
| 17 | C4–C8 Dedup helpers, coords module, Dimension enum, test layout, Linux docs | code health | XS–S each |
| 18 | F6 Concurrent windows | perf | M |
| 19 | A6, A7 Ocean/river biomes, nether roof model | accuracy (research) | M |
