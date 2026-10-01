# Crack the seed from structures, finish with biomes

bluemap-reverse can recover the world seed with no GPU and no Java, in two stages. First, **structure positions pin the lower 48 bits (the "structure seed")**, because every 1.18+ structure's candidate chunk comes from a `java.util.Random` seeded only by `seed mod 2^48`, the region coordinates and a public salt ([cubiomes finders.h](https://github.com/Cubitect/cubiomes/blob/master/finders.h)). Second, **biome tints and the lowres heightmap pick the remaining 16 bits**, since overworld climate noise is Xoroshiro128++ seeded from all 64 bits ([cubiomes biomenoise.c](https://raw.githubusercontent.com/Cubitect/cubiomes/master/biomenoise.c)). Bedrock matters less than the brief assumed. Overworld bedrock is a full-64-bit Xoroshiro oracle with no published inversion, and it sits below BlueMap's default cave cutoff. That leaves it useful only as a verifier. **Nether floor bedrock (y=1–4), by contrast, is a proven 48-bit cracker**, and it is exposed in BlueMap's full-3D nether meshes ([Nether_Bedrock_Cracker](https://github.com/19MisterX98/Nether_Bedrock_Cracker)). End pillar heights give 16 bits for free and cut a blind 2^48 search to 2^32 ([hube12 gist](https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c)). The reference system, SeedcrackerX, finishes this pipeline in **1–5 minutes** ([SeedcrackerX](https://github.com/19MisterX98/SeedcrackerX)), and a BlueMap export carries far more evidence than an in-game session. The work is therefore mostly engineering: turn BlueMap observations into exact structure start chunks, bind the maintained **xpple/cubiomes fork (MIT, 26.3 support)**, and add a tolerant candidate scorer. The existing vanilla-server regen then serves as the final judge.

Throughout, **sourced** numbers carry a citation. **Derived** numbers come from the research notes' own calculations from sourced formulas. **Estimates** have no benchmark behind them and must be measured.

## Structure positions leak about 9 bits each of the lower 48

Every vanilla structure except strongholds uses `random_spread`. The world splits into `spacing`×`spacing`-chunk regions, and each region draws one candidate chunk from `Random(seed + rx·341873128712 + rz·132897987541 + salt)`. Linear spread uses one `nextInt(spacing−separation)` per axis. Monuments, mansions and end cities use a triangular spread, `(nextInt(r)+nextInt(r))/2` ([cubiomes finders.h](https://github.com/Cubitect/cubiomes/blob/master/finders.h), [Minecraft Wiki: Structure set](https://minecraft.wiki/w/Structure_set)). The accept test runs in vanilla order: candidate chunk, then frequency reduction (outposts `legacy_type_1` 1-in-5, buried treasure 0.01, mineshafts 0.004), then the exclusion zone (outposts vs villages, 10 chunks). Biome and terrain checks only happen afterwards ([Pumpkin placement.rs](https://github.com/Pumpkin-MC/Pumpkin/blob/master/crates/pumpkin-world/src/generation/structure/placement.rs)). Biomes, which need the full 64-bit seed, therefore decide only *whether* a candidate succeeds. The position itself is a hard constraint on 48 bits.

The constants have been **identical from 1.19.4 through 26.4-snapshot-1** in the `structure_set` data dumps ([misode/mcmeta](https://github.com/misode/mcmeta)). Only four sets were added after 1.18: ancient cities (1.19), trail ruins (1.20), trial chambers (1.21) and **abandoned_camp, new in 26.3** (spacing 37, separation 8, salt 91231127) ([mcmeta 26.3-data](https://github.com/misode/mcmeta/tree/26.3-data/data/minecraft/worldgen/structure_set)). 1.18.2 has the same spacing and salts, but the outpost frequency, the village exclusion zone and the treasure frequency lived in code rather than JSON ([mcmeta 1.18.2-data](https://github.com/misode/mcmeta/tree/1.18.2-data/data/minecraft/worldgen/structure_set)).

The table below covers the structures a BlueMap render can actually show. Bits are **derived**: 2·log2(r) for linear spread, and the entropy of the triangular distribution otherwise. SeedcrackerX publishes rounded values that agree (9 for temples/igloos/huts, 8 for shipwrecks) ([seedcracker.blog bit calculator](https://seedcracker.blog/structure-bit-calculator/)).

| Structure (surface-visible) | spacing/sep | r | salt | Bits (derived) | Liftable low bits/axis |
|---|---|---|---|---|---|
| Desert pyramid, jungle temple, swamp hut, igloo, outpost | 32/8 | 24 | 14357617–20, 165745296 | 9.17 | 3 |
| Village | 34/8 | 26 | 10387312 | 9.40 | 1 |
| Shipwreck | 24/4 | 20 | 165745295 | 8.64 | 2 |
| Ocean ruin | 20/8 | 12 | 14357621 | 7.17 | 2 |
| Ruined portal (overworld + nether share a set) | 40/15 | 25 | 34222645 | 9.29 | 0 |
| Abandoned camp (26.3+) | 37/8 | 29 | 91231127 | 9.72 | 0 |
| Ocean monument (triangular) | 32/5 | 27 | 10387313 | ~8.95 | – |
| Woodland mansion (triangular) | 80/20 | 60 | 10387319 | ~11.26 | – |
| Nether fortress/bastion | 27/4 | 23 | 30084232 | 9.05 | 0 |
| End city (triangular) | 20/11 | 9 | 10387313 | ~5.80 | – |

Spacing, separation and salt values come from [misode/mcmeta](https://github.com/misode/mcmeta).

Uniqueness needs a little more than 48 bits. **Six good structures give about 54 bits, which leaves an expected 0.016 false candidates. Five give about 45 bits and roughly 8 survivors**, which the 2^16 biome stage then separates cheaply (derived). SeedcrackerX's sourced thresholds are lower because it adds other leaks: cracking starts at "40 bits of liftable structures" or "32 regular bits" plus end pillars ([SeedcrackerX](https://github.com/19MisterX98/SeedcrackerX)). Structures also yield more than their position. Rotation and template choice come from `chunkGenerateRnd(worldSeed, cx, cz)`, also a 48-bit `java.util.Random` ([cubiomes finders.h](https://github.com/Cubitect/cubiomes/blob/master/finders.h)). That gives about 2 extra bits of rotation per structure, plus log2 of the template count. The project's existing block-state template matcher is already the right tool to read those values off hires tiles.

The hard part is turning blocks into an *exact start chunk*. For a fixed-template structure the template origin is tied to the start chunk's minimum corner, and cubiomes places monuments at `chunk<<4` and buried treasure at chunk·16+9 ([cubiomes finders.c](https://github.com/Cubitect/cubiomes/blob/master/finders.c)). Several details come from memory in the notes and are **unverified**: pyramids, jungle temples and huts align to the chunk minimum; monuments start 29 blocks before the chunk's +9 column; a village's start chunk is fixed by its town-center piece. A village whose meeting-point piece isn't identified only bounds the chunk to roughly ±5 chunks. Treat that as a set of candidates, not a point. Strongholds, ancient cities, trial chambers, mineshafts and buried treasure are invisible from the surface and should be ignored.

## Bedrock splits by dimension: the nether floor cracks, the overworld only verifies

In 1.18+ bedrock is a surface-rule `vertical_gradient`. Each position gets a fresh positional RNG from `getOrCreateRandomFactory("minecraft:bedrock_floor").at(x,y,z)`, tested as `nextFloat() < p(y)` ([SurfaceRules.java, 26.2 decompile](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/SurfaceRules.java)). The noise settings decide everything. The **nether and End set `legacy_random_source: true`**, so the factory is `java.util.Random` and depends only on the lower 48 bits. The **overworld sets it false**, so the factory is Xoroshiro128++ and depends on the full 64 bits ([noise_settings/overworld.json](https://github.com/misode/mcmeta/blob/data/data/minecraft/worldgen/noise_settings/overworld.json), [noise_settings/nether.json](https://github.com/misode/mcmeta/blob/data/data/minecraft/worldgen/noise_settings/nether.json)). The nether chain has a verified closed form: `roofSeed = Random(Random(seed).nextLong() ^ 343340730).nextLong()`, with floor constant 2042456806 ([Nether_Bedrock_Cracker layer.rs](https://github.com/19MisterX98/Nether_Bedrock_Cracker/blob/HEAD/bedrock_cracker/src/layer.rs)). The position hash `Mth.getSeed` uses a 32-bit wrapping multiply that a Rust port must mirror ([Mth.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/util/Mth.java)).

**Nether floor bedrock is a primary cracker.** 19MisterX98's Nether_Bedrock_Cracker is Rust and LGPL-3.0. It enumerates 2^36 upper-bit prefixes and resolves the low 12 bits lazily, because LCG output bit k depends only on input bits ≤k. It cross-checks against the other surface and reverses `nextLong` twice to reach the structure seed ([lib.rs](https://github.com/19MisterX98/Nether_Bedrock_Cracker/blob/HEAD/bedrock_cracker/src/lib.rs)). Its example world was cracked from 32 roof blocks at y=123 plus 4 floor blocks at y=4 ([example](https://github.com/19MisterX98/Nether_Bedrock_Cracker/blob/HEAD/examples/seed%20765906787396911863.txt)). MiranCZ's BedrockSeedCracker reports "30 seconds to a couple of minutes" end to end ([README](https://github.com/MiranCZ/BedrockSeedCracker/blob/master/README.md)).

Each observation is worth −log2 P, so rare states carry the most (derived). Bedrock at y=4 and non-bedrock at y=1, both P=1/5, are worth 2.32 bits each. That means **roughly 21+ rare observations on one surface, ideally 25–35, plus 5–10 on the other** (derived).

The **roof is useless from above**, because y=127 is always bedrock. BlueMap's default nether config, however, masks out Y 90–127 and renders everything below it as a full 3D cave mesh (project notes: `docs/research/04-filling-hidden-data.md`). So **floor faces at y=1–4 next to air or lava are in the hires geometry**. One pitfall: a missing face is ambiguous, because the block may be occluded rather than absent. Count a "not bedrock" only where the neighbor cell is known air or lava. If the nether was rendered without the mask, the roof underside at y=123–126 becomes available too. On Paper servers before 1.19.2-213, a bug seeded the intermediate layers with the wrong y. The cracker has a dedicated mode for this ([block_data.rs](https://github.com/19MisterX98/Nether_Bedrock_Cracker/blob/HEAD/bedrock_cracker/src/block_data.rs)).

**Overworld bedrock cannot start a crack.** The derivation is a chain of bijections and constant XORs: seed → Xoroshiro, two `nextLong`s → XOR with the MD5 of `"minecraft:bedrock_floor"` → two more `nextLong`s → a positional Xoroshiro per block. No 48-bit shortcut exists, and no published inversion exists either ([XoroshiroRandomSource.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/XoroshiroRandomSource.java), [Nether_Bedrock_Cracker README](https://github.com/19MisterX98/Nether_Bedrock_Cracker)). **Given the 48-bit structure seed, it is a fine upper-16 filter**: 2^16 candidates × a few Xoroshiro steps each, needing about 10 rare observations (derived). MiranCZ's tool does exactly this, with 512 samples ([README](https://github.com/MiranCZ/BedrockSeedCracker/blob/master/README.md)). The deepslate transition (y=0–8, `random_name: minecraft:deepslate`) uses the same machinery and sits higher, so it is more often exposed in skylit ravines ([underground.json](https://github.com/misode/mcmeta/blob/data/data/minecraft/worldgen/material_rule/overworld/underground.json)). BlueMap's default overworld drops sunlight-0 faces below y=55, though, so expect neither to show up often. Before 1.18, bedrock did not depend on the seed at all ([TerrainFinder bedrock.c](https://github.com/DaMatrix/TerrainFinder/blob/master/bedrock.c)).

Among the other high-entropy features, **End pillars** stand out. The pillar arrangement is keyed by `nextLong() & 0xFFFF` of a Java Random, so reading the 10 pillar heights and cages leaks 16 bits. That cuts the structure-seed space to 2^32, and the 16-bit step itself takes "less than 0.1s" ([hube12 gist](https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c)). The exact geometry constants and 26.x behavior are unverified.

In 1.18+ most decorations (trees, wells, geodes, dungeons) are seeded from the full 64-bit seed through Xoroshiro ([ChunkGenerator.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/chunk/ChunkGenerator.java)). SeedcrackerX dropped dungeon and fungus cracking for this reason ([SeedcrackerX](https://github.com/19MisterX98/SeedcrackerX)). These features are forward-checks only.

Two more features are legacy-seeded and therefore 48-bit verifiers: nether surface patterns (soul sand, gravel, wart) and nether biomes ([RandomState.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/RandomState.java)). The badlands clay-band table is a 192-entry array drawn from one Xoroshiro stream ([SurfaceSystem.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/SurfaceSystem.java)). It carries tens to hundreds of bits for the upper-16 stage (derived). Recovering the full seed from it algebraically is an untested research idea.

## A five-stage pipeline, cheapest leaks first

The recommended pipeline works around one principle: **ask for the full seed only after the 48-bit problem is solved**, and treat every observation as possibly wrong. Rank candidates by how many observations they explain rather than requiring all of them. A single player-built "temple" otherwise eliminates the true seed.

| Stage | Inputs from BlueMap | Method | Bits | Runtime |
|---|---|---|---|---|
| 0. Fingerprint | Pack fingerprint, block palette (pale oak → 1.21.4+, abandoned camps → 26.3+), rendered dimensions | Pick the structure-constant set and cubiomes `MCVersion`; flag pre-1.18 chunks from blending seams | – | trivial |
| 1. Extract | Hires tiles: template matches → (type, start chunk, rotation, template); End pillar heights/cages; nether floor bedrock y=1–4 with exposed neighbors; tint classes; lowres heights | Existing `bmr-invert` template library plus new start-chunk rules per type | – | seconds (estimate) |
| 2a. Structure seed via lifting | ≥3–4 liftable structures (r=24/20) | Enumerate 2^(17+k) low bits (2^20 for r=24), filter with 2k bits per structure, extend survivors by the upper 28 bits | 48 | ms–s (derived: ~2^24 tests with 4 temples) |
| 2b. Structure seed via pillars | End pillars + ≥32 structure bits | 2^16 pillar seed → 2^32 candidates → structure filter | 48 | "under a minute" CPU ([hube12](https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c); pre-1.18 figure) |
| 2c. Structure seed via nether bedrock | ≥25 rare floor observations | Bit-layered 2^36-prefix search | 48 | 30 s – minutes ([MiranCZ](https://github.com/MiranCZ/BedrockSeedCracker/blob/master/README.md)) |
| 2d. Fallback | Only odd-r/triangular structures | Plain 2^48 enumeration, multithreaded | 48 | ~1 day on 8 cores for a comparable 2^48 space ([mc-locate](https://raw.githubusercontent.com/LAOUUUUU/mc-locate/main/README.md)); 1–8 h GPU (estimate) |
| 3. 48-bit verify | Rotation/template bits, triangular and odd-r structures, nether surface/biomes, End island shape | Forward-check each surviving structure seed | +10s | ms |
| 4. Upper 16 bits | Tint classes, surface-block biome classes, ocean/land, snow | Try the `nextLong`-reversal seed (~1 candidate) and the 2 text-seed forms first; then all 2^16 with a log-likelihood biome score | 16 | ~2–3 s single-threaded (estimate) |
| 5. Confirm | Lowres heightmap columns, overworld bedrock/deepslate/clay bands if seen, missing-structure regions, final tile diff | Approximate height (cubiomes) as a soft score → exact regen of the top few with the existing `tools/regen_world.py` path | – | ms/column (estimate) + one regen |

Stage 2a is the default route. Write `r = 2^k·m`. Then `x mod 2^k` equals bits [17, 17+k) of the first LCG state, and those depend only on `seed mod 2^(17+k)`. So low-bit brute force plus layered filtering makes a 2^48 search unnecessary once 3–4 temples or shipwrecks are known (derived from the formula in [cubiomes finders.h](https://github.com/Cubitect/cubiomes/blob/master/finders.h)). Triangular and odd-r structures (monuments, mansions, portals, nether complexes, abandoned camps) cannot be lifted and belong in Stage 3.

Stage 4 has two shortcuts, and both should be tried first. **Random world seeds come from `RandomSource.create().nextLong()`, a legacy LCG** ([WorldOptions.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/WorldOptions.java)). Only 2^48 world seeds are reachable that way, and a structure seed maps to "a single world seed on average" by reversing `nextLong` ([hube12 gist](https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c)). **Text seeds** are `String.hashCode()`, a sign-extended int32 ([Minecraft Wiki: Seed](https://minecraft.wiki/w/Seed_(level_generation))), so only two full seeds fit a given structure seed.

The general 2^16 sweep reuses cubiomes' seeding, `xSetSeed` → two `xNextLong`s XORed with per-noise MD5 constants ([biomenoise.c](https://raw.githubusercontent.com/Cubitect/cubiomes/master/biomenoise.c)). A sourced cost anchor exists: 1M scale-1 `getBiomeAt` calls on 1.21 took 5.683 s single-threaded, about **5.7 µs per query** ([Rohan Sharma](https://rohan-sharma.de/blog/cubiomesmpi-part1/)).

Score biomes by likelihood, not by hard filter. Tints map to *groups* of biomes, and chunk-border voronoi jitter adds noise. Accumulate `log P(observed class | predicted biome)` with a small error term, and accept when the leader beats the runner-up by a wide margin (a design proposal; no published implementation). Sample at 1:4 cell centers far from biome borders and more than about 1 km apart. Use the observed surface y, because 1.18+ biomes are 3D.

Stage 5 needs care with the heightmap. BlueMap's lowres height is the **top visible block, including trees and water**, and cubiomes' `mapApproxHeight` is a 1:4 spline approximation ([generator.h](https://raw.githubusercontent.com/Cubitect/cubiomes/master/generator.h)) that cubiomes-viewer users call "very generous" ([issue #319](https://github.com/Cubitect/cubiomes-viewer/issues/319)). Use it as a tolerant score on flat grass or sand columns only. Leave exact agreement to the vanilla regen the project already runs.

## Build on xpple/cubiomes and a clean-room cracker, not on Java

| Component | License | Version coverage | Status | Use |
|---|---|---|---|---|
| [xpple/cubiomes](https://github.com/xpple/cubiomes) | MIT | 26.3 ("Add 26.3 support", 2026-09-11); terrain 1.14+, abandoned camps 26.3+ | Active, pushed 2026-09-22 ([GitHub API](https://api.github.com/repos/xpple/cubiomes)) | Biomes, approximate/terrain height, structure viability. **Primary dependency** |
| [Cubitect/cubiomes](https://github.com/Cubitect/cubiomes) | MIT | Ends at `MC_1_21_WD` ([biomes.h](https://raw.githubusercontent.com/Cubitect/cubiomes/master/biomes.h)) | Dormant since 2024-11-10 | Reference only |
| [cubiomes crate](https://crates.io/api/v1/crates/cubiomes) (villevilli) | MIT | Wraps upstream; last release 2025-03 | Young, ~5.8k downloads | Skip; write an own `-sys` crate |
| [rusticg](https://crates.io/api/v1/crates/rusticg) | Apache-2.0 | Java Random only | v1.0.1, 2025-10 | Lattice/interval reversal if needed |
| [Nether_Bedrock_Cracker](https://github.com/19MisterX98/Nether_Bedrock_Cracker) | LGPL-3.0 | 1.18+ nether | Last push 2025-03 | Algorithm reference; reimplement cleanly or link dynamically |
| [SeedcrackerX](https://github.com/19MisterX98/SeedcrackerX) | MIT | 1.16.5–26.2 | Active (2.16.x) | Reference design for stages 2–4 |
| [mc-locate](https://github.com/LAOUUUUU/mc-locate) | README says MIT, GitHub says NOASSERTION | Beta 1.7–26.2 | 1 week old, 1 star | Read for the xpple build setup and LattiCG port; do not depend on it |

The stack follows from licensing and version support. Vendor xpple/cubiomes as a submodule behind a project-owned `cc` + `bindgen` `-sys` crate (building it with MSVC is unverified). mc-locate proves the full Rust + xpple-cubiomes + no-Java stack works on Windows ([mc-locate README](https://raw.githubusercontent.com/LAOUUUUU/mc-locate/main/README.md)). Everything cryptanalytic is small enough to own: the LCG, `nextLong` reversal (inverse multiplier `0xdfe05bcb1365`), lifting, the pillar decoder and a clean nether-bedrock filter.

Keep Mojang code out of the published crates. Since 26.1 the game ships unobfuscated under the EULA, and decompiled source may be redistributed only modified or as part of a larger project ([Minecraft Wiki: Obfuscation map](https://minecraft.wiki/w/Obfuscation_map), [minecraft.net](https://www.minecraft.net/en-us/article/removing-obfuscation-in-java-edition)). Reading it to verify constants is standard community practice. Translating it wholesale is the riskier path.

Several pieces must be written. Following the project's layering (not sourced), a new `bmr-seed` crate should hold them rather than growing `bmr-fill`:

- per-version structure-set tables generated from mcmeta, including 1.18.2's in-code frequencies and 26.3's abandoned camp;
- per-type start-chunk extractors with verified template pivots and village town-center identification;
- the lifting and pillar solvers with outlier-tolerant scoring;
- a nether floor bedrock observation extractor with occlusion-aware negatives;
- the `nextLong` and text-seed shortcuts;
- a tint→biome-group confusion model and likelihood scorer;
- heightmap column selection;
- hand-off of the winning seed to the existing `--regen` merge in `bmr-fill/src/regen.rs`.

## Pitfalls that silently kill the true seed

**Custom salts** are the biggest risk on public servers. Spigot and Paper expose per-world salts (`seed-village` 10387312, `seed-feature` 14357617, `seed-monument` 10387313, …) and advise randomizing them against cracking ([Spigot configuration](https://www.spigotmc.org/wiki/spigot-configuration/)). Anti-seed-cracker plugins also rewrite End spikes ([AntiSeedCracker](https://deepwiki.com/akshualy/AntiSeedCracker/3.2.1-end-spike-modification)). The solver must accept per-set salt overrides. If no vanilla-salt solution fits five or more good observations, it should solve for the salt: once two types share a seed, each further salt is a 32-bit search (derived).

**Version mixing** is next. Villages moved from spacing 32 to 34 at 1.18, and the fortress algorithm changed at 1.16 and 1.18 ([cubiomes finders.c](https://github.com/Cubitect/cubiomes/blob/master/finders.c)). Chunks generated before an upgrade keep their old layout, so test pre-1.18-looking regions under both parameter sets.

**Biome tables drift even though noise seeding hasn't.** Pale gardens moved at 1.21.5, sulfur caves arrived in 26.2 and dappled forests in 26.3 ([Minecraft Wiki: World generation/History](https://minecraft.wiki/w/World_generation/History)). Upstream cubiomes mis-predicts these. Either use xpple or exclude those regions.

**Dimension limits**: nether- or End-only maps can never reveal the upper 16 bits. All 2^16 sister seeds reproduce those dimensions exactly ([cubiomes biomenoise.c](https://raw.githubusercontent.com/Cubitect/cubiomes/master/biomenoise.c)). Unless the random-seed or text-seed shortcut applies, report 65,536 equivalent seeds and regenerate only the rendered dimension.

**Seed-origin assumptions**: hosting panels may not generate seeds with `Random.nextLong`, so the shortcut can miss. It is a first guess, not a proof.

**Blind search is not an option**: with "generate structures" off and no nether or End data, only a biome-verified 2^48 or 2^64 search remains. That is infeasible for arbitrary numeric seeds, about 5.8×10^6 CPU-years at 10 µs per seed (derived). It is borderline only for text seeds: 2^32 × 5–20 µs ≈ 0.5–2 h on 16 threads (estimate).

**False structures**: player-built pyramids and rebuilt portals are the most common false positives. Satellite ocean ruins do not mark the start chunk.

## Open questions to settle experimentally

Several claims need a fixture before code depends on them. The seeded fixtures already planned for milestone 9 in `docs/plan.md` are the natural harness.

- **Template origins.** Check the template origin and rotation pivot of every visible structure type against a Mojmap decompile and mcmeta NBT sizes. The notes flag these as memory-derived.
- **BlueMap visibility.** Check whether BlueMap hires tiles contain submerged monuments and shipwrecks, and bedrock faces under nether lava lakes.
- **End pillars on 26.x.** Confirm that 26.x still seeds spikes with `RandomSource.create(seed)` and `& 0xFFFF`, and pin the exact height, radius and cage table.
- **Nether and End seed dependence.** Test a sister-seed pair (same low 48 bits) in the real game to confirm that nether and End generation are identical, including nether surface rules and End islands.
- **Stage 4 cost.** Benchmark per-candidate `setBiomeSeed`/`applySeed` on xpple for 26.3. The 2–3 s sweep is an estimate, and a per-climate-parameter init could cut it further.
- **Height error.** Measure `mapApproxHeight` and xpple's terrain height against the real lowres heightmap: mean and maximum error on flat columns.
- **Tint confusion matrix.** Build it empirically from the existing vanilla fixtures (predicted biome vs BlueMap tint class).
- **Random-seed shortcut.** Confirm `WorldOptions.randomSeed` in 1.18.2 and 1.21.x, and whether common server hosts bypass it.
- **Salt sweep.** Estimate how often a real server map fails the vanilla-salt fit, and whether the per-type salt search is fast enough in practice.

## Conclusion

The seed finder is best seen as a **constraint-satisfaction problem with a trusted oracle at the end**, not a cryptanalysis project. The hard cryptography (LCG lifting, `nextLong` reversal, 16-bit pillar leaks) is small, well understood and cheap. What limits accuracy is observation quality: turning a BlueMap mesh into correct (type, start chunk, rotation) tuples, and reading tints as calibrated biome likelihoods. That is where the project's existing inversion and look-alike scoring give it an edge over in-game crackers, which see one player's surroundings. BlueMap sees hundreds of structures at once, so the budget of 40–54 bits is usually exceeded many times over. That surplus should be spent on robustness: majority-vote scoring, salt solving and version mixing rather than speed.

The one genuine research frontier the notes surfaced is algebraic recovery of Xoroshiro state from overworld artifacts such as clay bands or bedrock. It would crack maps with no structures, but nothing published does it, and the project doesn't need it. With any three or four temples, a nether floor or an End center in view, the seed is minutes away. From there, the existing regen path turns every unmapped chunk into vanilla terrain instead of priors.
