# Recovering the full 64-bit seed from biomes and terrain (Java 1.18+)

## How 1.18+ climate noise is seeded, and why the upper 16 bits matter (sister seeds, 2^16 search, cubiomes cost)

### Takeaway
Overworld biomes and terrain shape in 1.18+ come from Xoroshiro128++ seeded with all 64 bits of the world seed, mixed through a splitmix-style finalizer. So the 2^16 "sister seeds" that share one 48-bit structure seed have unrelated biome and terrain layouts. Testing all 2^16 of them against even a handful of observed biome samples is the standard way to finish a crack, and it takes seconds.

### Cited Findings
- cubiomes `setBiomeSeed(bn, seed, large)` does `xSetSeed(&pxr, seed); xlo = xNextLong(&pxr); xhi = xNextLong(&pxr);`. Each climate DoublePerlinNoise is then seeded with `(xlo ^ C_lo, xhi ^ C_hi)`, where the constants are the MD5 hash of the noise name (this is how Mojang's positional-random `fromHashOf("minecraft:…")` works). Constants in cubiomes (normal / large biomes):
  - temperature `0x5c7e6b29735f0d7f, 0xf7d86f1bbc734988` / large `0x944b0073edf549db, 0x4ff44347e9d22b96`
  - humidity (vegetation) `0x81bb4d22e8dc168e, 0xf1c8b4bea16303cd` / large `0x71b8ab943dbd5301, 0xbb63ddcf39ff7a2b`
  - continentalness `0x83886c9d0ae3a662, 0xafa638a61b42e8ad` / large `0x9a3f51a113fce8dc, 0xee2dbd157e5dcdad`
  - erosion `0xd02491e6058f6fd8, 0x4792512c94c17a80` / large `0x8c984b1f8702a951, 0xead7b1f92bae535f`
  - shift (offset) `0x080518cf6af25384, 0x3f3dfb40a54febd5`
  - weirdness (ridge) `0xefc8ef4d36102b34, 0x1beeeb324a0f24ea`

  — [cubiomes biomenoise.c](https://raw.githubusercontent.com/Cubitect/cubiomes/master/biomenoise.c)
- Octave layouts from `init_climate_seed` (amplitude list; first octave normal/large):
  - shift `{1,1,1,0}`, octave -3
  - temperature `{1.5,0,1,0,0,0}`, -10/-12
  - humidity `{1,1,0,0,0,0}`, -8/-10
  - continentalness `{1,1,2,2,2,1,1,1,1}`, -9/-11
  - erosion `{1,1,0,1,1}`, -9/-11
  - weirdness `{1,2,1,0,0,0}`, -7

  Depth is not a separate noise. It is computed from a spline over (continentalness, erosion, peaks-and-valleys = `-3(| |w| - 2/3 | - 1/3)`, weirdness) plus a y gradient: `off = getSpline(...) + 0.015; d = 1 - 4y/128 - 83/160 + off`. — [cubiomes biomenoise.c](https://raw.githubusercontent.com/Cubitect/cubiomes/master/biomenoise.c)
- `xSetSeed` does `l = seed ^ 0x6a09e667f3bcc909; h = l + 0x9e3779b97f4a7c15`, then the stafford-13 mix (`*0xbf58476d1ce4e5b9`, `*0x94d049bb133111eb`, shifts 30/27/31). `xNextLong` is xoroshiro128++ (`rotl(l+h,17)+l`). Java `setSeed` is `(value ^ 0x5deece66d) & (2^48-1)`, so it only uses 48 bits. — [cubiomes rng.h](https://raw.githubusercontent.com/Cubitect/cubiomes/master/rng.h)
- Structure positions depend only on region coordinates and the lower 48 bits. The README's example finishes a search with `for (upper16 = 0; upper16 < 0x10000; upper16++)`, building full 64-bit seeds and checking biomes. — [cubiomes README](https://raw.githubusercontent.com/Cubitect/cubiomes/master/README.md)
- `getBiomeAt(g, scale, x, y, z)`: with scale=4 coordinates are biome cells; with scale=1 they are blocks, via voronoi, which needs the SHA-256 "sha" seed hash. Vertical scaling always stays 1:4 except when scale==1. Supported genBiomes scales are 1, 4, 16, 64 and (Overworld only) 256. — [cubiomes README](https://github.com/Cubitect/cubiomes); [biomenoise.h](https://raw.githubusercontent.com/Cubitect/cubiomes/master/biomenoise.h)
- For 1.18+, cubiomes samples each point individually. There is little benefit from generating a volume as a whole: "The layered generators for versions up to 1.17 will benefit significantly more from this than the noise-based ones." — [cubiomes README](https://github.com/Cubitect/cubiomes)
- `sampleBiomeNoise(bn, np, x, y, z, dat, flags)` has flags `SAMPLE_NO_SHIFT`, `SAMPLE_NO_DEPTH` and `SAMPLE_NO_BIOME`, so you can get the raw climate vector without the biome-tree lookup. — [biomenoise.h](https://raw.githubusercontent.com/Cubitect/cubiomes/master/biomenoise.h); [DeepWiki xpple/cubiomes](https://deepwiki.com/xpple/cubiomes/3.4-biome-noise-system)
- Benchmark: 1,000,000 `getBiomeAt` queries at scale 1 on 1.21 took 5.683 s single-threaded, about 0.18 M queries/s or about 5.7 µs per query. — [Rohan Sharma, Ultrafast MC biome gen pt 1](https://rohan-sharma.de/blog/cubiomesmpi-part1/)
- SeedCrackerX: each structure-seed candidate stands for 65,536 world seeds, and "at least one biome sample or one spawn reading" narrows it to one seed. The final 16 bits come from a hashed seed (server-sent), decorators, or biomes. Typical total crack time is 1–5 min. — [seedcracker.blog guide](https://seedcracker.blog/how-to-use-seedcrackerx/); [SeedcrackerX README](https://github.com/19MisterX98/SeedcrackerX)
- Seed-cracking primer: bruteforcing the 16 upper bits "using some data from biomes" is the standard final step. — [hube12 seed-cracking.md](https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c)

### Inferences
- Per-candidate cost (my estimate, not measured in any source): `setBiomeSeed` initializes about 2×(4+6+6+9+5+6) = 72 Perlin octaves, each a 256-entry permutation shuffle, so a few µs to tens of µs. Add k samples at about 1–5 µs each at 1:4 (scale 1 adds voronoi). For 2^16 sister seeds with around 5 samples each, that is roughly 65536 × (~20 µs + 5×3 µs) ≈ 2–3 s single-threaded. The per-seed init dominates, and it is trivially parallel.
- Early rejection: order the checks from cheapest and most selective to least. (1) Sample only the climate parameter an observation constrains most tightly, usually continentalness (ocean vs land) or temperature (snowy vs hot). Only that parameter's noise needs initializing, which saves most of the init cost; build a per-parameter init if the library lacks one. (2) Test coarse 1:64/1:256 points far apart, which are roughly independent. (3) Only survivors get full 1:4 biome sampling or height checks.
- Each observation that splits biomes about 50/50 removes about 1 bit, so about 16–20 independent coarse observations isolate one of the 2^16 seeds with margin. BlueMap gives thousands of observations, so noise in the labels matters more than their number (see the discriminative-observations section).
- Use SHA-256 voronoi (scale 1) only for border-precise checks. Compare interior cells at scale 4 so the unknown voronoi jitter doesn't matter.

### Gaps
- No published per-seed benchmark of `setBiomeSeed` / `applySeed` for 1.18+ was found. The 2–3 s figure above is an estimate; benchmark it locally.
- Could not confirm from source whether upstream cubiomes exposes a single-parameter seeding function (I recall `setClimateParaSeed` being used by cubiomes-viewer's climate filters, but did not verify).

## Can the heightmap (terrain shape) test or rank seed candidates?

### Takeaway
Yes. In 1.18+ the surface height is mostly set by the same continentalness/erosion/weirdness noises through the depth, factor and jaggedness splines. Cubiomes already has a 1:4 approximate surface height (`mapApproxHeight`), so a few columns of height make a cheap, strong candidate filter. Exact per-block height needs the full noise router (3D base noise plus the final-density formula). No off-the-shelf "heightmap → seed" tool was found.

### Cited Findings
- `int mapApproxHeight(float *y, int *ids, const Generator *g, const SurfaceNoise *sn, int x, int z, int w, int h);` — "Map an approximation of the Overworld surface height. The horizontal scaling is 1:4." It can also fill biome ids. — [cubiomes generator.h](https://raw.githubusercontent.com/Cubitect/cubiomes/master/generator.h); [web summary](https://github.com/Cubitect/cubiomes/blob/master/README.md)
- Cubiomes notes that 1.18 desert pyramids and jungle temples depend on surface height, which "cubiomes does not provide block-level world generation and cannot check". — [xpple/cubiomes README](https://github.com/xpple/cubiomes)
- The xpple fork (active, used by SeedMapper and similar) lists "Terrain generation (1.14+)", canyon/cave carvers, ore veins and stronghold/loot additions. — [xpple/cubiomes](https://github.com/xpple/cubiomes)
- cubiomes-viewer has an "Approx. surface height" layer. An issue titled "approximate surface height is very generous" exists (#319) and another reports negative Y not displayed (#344). I could not retrieve the text of #319. — [issue #319](https://github.com/Cubitect/cubiomes-viewer/issues/319); [issue #344](https://github.com/Cubitect/cubiomes-viewer/issues/344)
- Minecraft@Home's panorama projects ran "biome and terrain checking code" on candidate seeds, using the rule that two seeds with the same biome at the same place match in terrain shape. The 1.19 panorama seed (-1696067516, text "thewildupdate") was cracked by brute force over text-seed space in under a second of runtime once the text-seed assumption was made. — [MC@Home 1.19 Panorama](https://minecraftathome.com/projects/1-19-panorama.html); [forum summary](https://www.minecraftforum.net/forums/minecraft-java-edition/seeds/3029589-minecraft-home-have-found-the-seed-of-minecrafts)

### Inferences
- Discriminative power: height is continuous, so one column gives several bits (height is roughly ±1 block reproducible exactly, vs a range of about 60–200). A handful of columns spaced more than about 1–2 km apart, placed where continentalness/erosion vary, should isolate one of 2^16 candidates by itself. Height works even where biome tint is ambiguous.
- Cheap path: approximate height (cubiomes spline, 1:4, no 3D noise) as a *soft* score with tolerance of several blocks (it is "generous"). Only the top few candidates get exact regeneration. Watch out for: approximate height ignores jaggedness and the 3D base noise, so peaks and overhangs are unreliable. Use flat-ish plains or ocean-floor-free land columns. Also, BlueMap's lowres heightmap is the top non-air block, including trees and water surface, not the terrain heightmap. Prefer columns where the visible top block is grass/sand/stone, and treat water columns as "sea level 63, floor unknown".
- Exact path: run the vanilla `NoiseBasedChunkGenerator` headless (Java, the game jar via a data-generator/server bootstrap) or a faithful port. Compute only the needed columns with `getBaseHeight(x, z, Heightmap.Types.OCEAN_FLOOR_WG / WORLD_SURFACE_WG)`. This exists in vanilla and avoids generating full chunks. Cost is roughly milliseconds per column, which is fine for re-ranking under 100 candidates but not for 2^32.
- The ocean vs land boundary (continentalness around -0.19 / -0.11) and river valleys (weirdness near 0 → PV "valleys") are the cheapest strong height/biome features to compare.

### Gaps
- No published tool matches an observed 1.18+ heightmap to seeds. MC@Home's code for the panoramas isn't described in detail on the project pages.
- No published accuracy statistics (mean/max error) for `mapApproxHeight` vs real terrain.

## Direct full-seed search without structures: the size of the search space and shortcuts

### Takeaway
A blind 2^64 search is infeasible, but two common seed origins shrink it a lot. Text seeds hash to a signed 32-bit int (2^32 space, feasible in CPU-hours with biome checks). Blank or "random" seeds come from `java.util.Random`-style `nextLong()` (an LCG with 48-bit state), so only 2^48 of the 2^64 values can occur. For those, the 48-bit structure seed extends to the full seed almost instantly without any biome check. The claim that 1.18+ random seeds use all 64 bits via `RandomSupport.generateUniqueSeed` is only true for the *RNG's seed*, not for the world seed.

### Cited Findings
- Text seeds: if the input isn't a valid long, Java `String.hashCode()` is used, which restricts reachable worlds to 2^32 = 4,294,967,296. Numeric input accepts any signed 64-bit value. Since 1.18.2 "0" is a normal seed; earlier it meant random. — [Minecraft Wiki: Seed (level generation)](https://minecraft.wiki/w/Seed_(level_generation))
- "Even a randomly generated world seed will use Java's Random for only 2^48 possible seeds". Because random seeds come from nextLong (two 32-bit nextInt outputs combined), "the 48-bit seed actually corresponds to only a single world seed on average, and can be extended to the 64-bit seed nearly instantly." — [hube12 seed-cracking.md](https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c) (via search summary); [MinecraftForum discussion](https://www.minecraftforum.net/forums/minecraft-java-edition/discussion/2999890-seed-reverse-engineering)
- Java Random LCG: `seed' = (25214903917·seed + 11) mod 2^48`. — [Columbia SeedCracker report](https://www.cs.columbia.edu/~sedwards/classes/2021/4995-fall/reports/SeedCracker.pdf) (via search summary)
- Modern code path: `WorldOptions.randomSeed()` calls `RandomSource.create().nextLong()`. `RandomSource.create()` returns the legacy (Java-LCG) source. — [search snippet](https://nekoyue.github.io/ForgeJavaDocs-NG/javadoc/1.19.3/net/minecraft/util/RandomSource.html) (weak source; confirm in decompiled code)
- MC@Home 1.19 panorama: the seed "came from manually entered text", which reduced the search to the text-hash space. The search code ran in under a second once the region was constrained. — [MC@Home 1.19 Panorama](https://minecraftathome.com/projects/1-19-panorama.html)

### Inferences
- **nextLong reversal (random seeds):** `nextLong() = ((long)nextInt() << 32) + nextInt()`. Let the known lower 48 bits of the world seed be L.
  1. The low 32 bits of L equal the second int `b`, which is the top 32 bits of LCG state s2.
  2. Bits 32..47 of L equal the low 16 bits of `a + (b<0 ? -1 : 0)`, where `a` is the first int (top 32 bits of s1).
  3. Enumerate the 2^16 unknown low bits of s2, step the LCG back once to get s1, check the 16 known bits of `a`, and emit the survivors.

  This costs microseconds and gives about 1 candidate on average. Try this *first*: a hit that also matches biomes confirms a random seed.
- **Text seeds:** the seed is a sign-extended int32, so upper 32 bits are all 0 or all 1. With a known structure seed, check the two values `sext(L & 0xFFFFFFFF)` for consistency (bits 32–47 of L must be all 0 or all 1). Without structures, brute-force 2^32 ints with a climate-only first stage, e.g. one continentalness or temperature sample to match ocean vs land or snow at the observed spawn area. Estimate: 2^32 × ~5–20 µs ≈ 6–24 CPU-hours, so about 0.5–2 h on a 16-thread desktop. That is feasible, and faster with partial noise init or a GPU. Many human-typed numeric seeds (short integers) are also tiny spaces worth trying.
- **Truly arbitrary 64-bit numeric seeds** (typed long numbers, or seeds from tools and hosting panels that don't use `Random.nextLong`) have no shortcut. At about 10 µs per seed a blind 2^64 search takes about 5.8×10^6 CPU-years, so you need the structure seed from structures, end pillars or features first.

### Gaps
- I did not see the decompiled `WorldOptions.randomSeed` directly. Hosting panels (Pterodactyl, Aternos, etc.) may generate seeds some other way, so the 2^48 reachable-set assumption could fail for server-hosted worlds. Verify before relying on it.
- No measured throughput for a 2^32 text-seed biome brute force with cubiomes.

## End-dimension and Nether constraints (maps where the Overworld isn't visible)

### Takeaway
In 1.18+ the Nether (`legacy_random_source: true`) and End biome/island noise are seeded with Java `setSeed(seed)`, which only uses 48 bits. End spikes use `nextLong() & 0xFFFF` of a Java Random. So Nether and End maps can constrain the lower 48 bits (the structure seed) but can never reveal the upper 16. For a Nether- or End-only map you need either the random-seed nextLong trick or a text-seed assumption to get a unique full seed. Otherwise there are 2^16 equally valid seeds, all of which reproduce that dimension exactly (and differ only in the Overworld).

### Cited Findings
- cubiomes `setNetherSeed`: `setSeed(&s, seed)` gives temperature DoublePerlin (octave -7, 2 amps); `setSeed(&s, seed+1)` gives humidity. `setEndSeed`: `setSeed(&s, seed); skipNextN(&s, 17292); perlinInit(...)`. Here `setSeed` is the Java 48-bit LCG init. — [cubiomes biomenoise.c](https://raw.githubusercontent.com/Cubitect/cubiomes/master/biomenoise.c); [rng.h](https://raw.githubusercontent.com/Cubitect/cubiomes/master/rng.h)
- Noise settings `legacy_random_source`: overworld false, nether true, end false. Caves and floating islands presets are true. — [Minecraft Wiki: Noise settings](https://minecraft.wiki/w/Noise_settings)
- End spikes "leak 16 bits of the worldseed for free" via `random.nextLong() & 65535L`, cutting a 2^48 structure-seed search to 2^32. — [hube12 seed-cracking.md](https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c)
- The wiki confirms 10 spikes, heights Y=76–103 and radii 3–6, some caged, but gives no seeding details. — [Minecraft Wiki: End spike](https://minecraft.wiki/w/End_spike)
- SeedCrackerX: find 5+ end cities for the "regular bits", then visit the End center to get the "pillar seed". — [SeedcrackerX README](https://github.com/19MisterX98/SeedcrackerX)

### Inferences
- Pillar arrangement: a shuffle of the 10 (height, radius, cage) triples keyed by a 16-bit value. BlueMap shows pillar tops and cages exactly, which pins those 16 bits (up to rare shuffle collisions). Combine with end-city positions (48-bit) for the structure seed.
- End island shape: the SimplexNoise island field (`skip 17292`) depends on 48 bits, so it can rank structure seeds but not sister seeds.
- The End's "false" `legacy_random_source` affects only noise in its router. The islands function uses the legacy-seeded simplex above. The End has no climate noise and uses fixed biomes by island distance, so it has nothing to give on the upper 16 bits (inference, consistent with SeedCrackerX needing the overworld or a hash for the last 16 bits).

### Gaps
- Did not verify directly from decompiled code that every nether/end density function in 1.18+ ignores the upper 16 bits. It follows from `legacy_random_source` and the cubiomes implementations, but check it on a sister-seed pair with the real game.

## Which biome observations are most discriminative; handling uncertain labels

### Takeaway
Score candidates with a probabilistic likelihood over *climate-parameter-consistent classes*, not a hard biome-ID filter. Tint-derived labels (grass/foliage/water colours) map to groups of biomes. The most informative observations are binary climate splits that are nearly certain from BlueMap data: ocean vs land (continentalness), snowy vs non-snowy (temperature), desert/badlands/mushroom surface blocks, rivers (weirdness near 0), and exact height.

### Cited Findings
- 1.18+ biomes come from a 6D climate point (T, H, C, E, depth, W) matched against a biome parameter table. There is no longer a unique mapping from noise points to biomes (MC-241546). — [biomenoise.h](https://raw.githubusercontent.com/Cubitect/cubiomes/master/biomenoise.h); [DeepWiki](https://deepwiki.com/xpple/cubiomes/3.4-biome-noise-system)
- The 1.19 panorama crack used biome and terrain checks as the candidate filter. — [MC@Home](https://minecraftathome.com/projects/1-19-panorama.html)

### Inferences
- Label taxonomy for scoring, from BlueMap evidence:
  - (a) Exact-surface classes: sand plus water-tint desert vs beach, red sand/terracotta → badlands, mycelium → mushroom fields, snow layer/ice → frozen or snowy, podzol → old-growth taiga/jungle edge, mud/mangrove roots → mangrove.
  - (b) Tint groups, which identify T/H buckets. Grass/foliage colormap lookups depend on the biome's temperature/downfall constants, so biomes sharing constants are indistinguishable.
  - (c) Water-tint groups: warm, lukewarm, cold and frozen ocean, swamp, mangrove.
- Scoring: for each observation o and candidate seed s, compute the predicted biome b(s, x). Add log P(o | b), using a confusion matrix built from the tint→biome ambiguity plus a small error ε (about 0.02–0.05) for chunk-border voronoi jitter and misclassification. Rank by total log-likelihood and accept when the top candidate beats the second by a margin of about 20+ nats. This tolerates mislabeled chunks, pregenerated terrain from other versions, and edited terrain, all of which a hard filter would wrongly reject.
- Choose observation sites far apart (more than 1024 blocks, beyond the scale of the large continentalness octaves) and in cell interiors, away from biome boundaries. Sample at 1:4 cell centres at the surface y (use the observed height for `y/4`, because cave biomes such as lush caves, dripstone and deep dark depend on depth).
- Use the cheap, very reliable splits first as hard-ish gates, with a high-confidence threshold: ocean vs land at deep-ocean centres, frozen vs warm ocean. Put finer classes (forest variants, plateaus, windswept) in the soft score.
- Watch out: BlueMap tint may reflect the biome at y=surface, and 3D biomes at the surface matter. Mountains (peaks, slopes) depend on erosion and PV, so their labels also carry height information.

### Gaps
- No published confusion matrices or likelihood-scoring implementations for tint-only biome observations were found. The scoring scheme above is a design proposal.

## Version dependence (1.18 → 26.x) and cubiomes support

### Takeaway
The climate noises and seeding scheme have been stable since 1.18. What changed between versions is the biome parameter table, i.e. which biome a climate point maps to, plus some terrain and feature details. So a seed test must use the exact version's table. Upstream cubiomes stops at `MC_1_21_WD` (1.21.2–1.21.4 Winter Drop). Later changes (1.21.5 pale-garden placement, 26.x biomes like dappled forest in 26.3, sulfur caves in 26.2) need an up-to-date fork (xpple) or the real game, or you restrict checks to biomes whose placement didn't change.

### Cited Findings
- cubiomes MCVersion values for 1.18+: `MC_1_18_2 (=MC_1_18)`, `MC_1_19_2`, `MC_1_19_4 (=MC_1_19)`, `MC_1_20_6 (=MC_1_20)`, `MC_1_21_1`, `MC_1_21_3`, `MC_1_21_WD` ("Winter Drop, version TBA"), `MC_1_21 = MC_1_21_WD`, `MC_NEWEST = MC_1_21`. Biome ids: deep_dark 183, mangrove_swamp 184, cherry_grove 185, pale_garden 186. — [cubiomes biomes.h](https://raw.githubusercontent.com/Cubitect/cubiomes/master/biomes.h)
- cubiomes-viewer 4.1.2 renamed 1.21.3 → "1.21 WD" and 1.21.2 → 1.21.3 to match the Winter Drop, and fixed spawn for 1.21.2+. Not all 1.21 sub-versions are present. — [cubiomes-viewer releases](https://github.com/Cubitect/cubiomes-viewer/releases) (via search summary)
- World-generation history:
  - 1.18: new terrain generator, height -64..319
  - 1.19: deep dark, then mangrove swamp in 22w14a
  - 1.20 (23w12a): cherry grove
  - 1.21.2 (24w40a): pale garden, only at positive weirdness
  - 1.21.5 (25w02a): pale garden also at negative weirdness, and woodland mansions in pale gardens
  - 26.2 snap1: sulfur caves
  - 26.3 snap1: dappled forests and abandoned camps

  — [Minecraft Wiki: World generation/History](https://minecraft.wiki/w/World_generation/History)
- xpple/cubiomes advertises "Up-to-date biome generation" and "Abandoned camp generation (26.3+)", implying support for current 26.x versions. — [xpple/cubiomes](https://github.com/xpple/cubiomes)
- SeedcrackerX supports MC 1.16.5 through 26.2 (v2.16.1). — [SeedcrackerX README](https://github.com/19MisterX98/SeedcrackerX)

### Inferences
- Use `MC_1_18` for 1.18.x and `MC_1_19_2` for 1.19–1.19.2. For 1.19.3/1.19.4 use `MC_1_19` (the 1.19.3 change was minor). Then `MC_1_20`, `MC_1_21_1`, `MC_1_21_3` (1.21.2–1.21.3) and `MC_1_21_WD` (1.21.4).
  - For 1.21.5+ with upstream cubiomes, ignore pale-garden observations (it would mis-predict pale garden at negative weirdness). All other biomes are still predicted correctly if their parameter entries didn't move.
  - For 26.3+, also ignore areas where dappled forest could appear.
  - Better: use the xpple fork, or validate the top candidates with the real server jar.
- Biome-version detection can come from surface blocks: pale oak/pale moss means 1.21.4+, cherry leaves 1.20+, mangroves 1.19+.

### Gaps
- Exact parameter-table diffs for 1.21.5 and 26.x (which climate ranges moved) were not retrieved. [misode changelog](https://misode.github.io/changelog/?tags=worldgen) is the likely source but is JS-rendered.
- Whether terrain density (splines, final density) changed at all after 1.18 in ways that alter heights for the same seed: not verified. The wiki history lists biome additions but no router changes.
- A secondary blog ([minecraft.how](https://minecraft.how/blog/post/minecraft-world-generation-2026-changes)) claims 26.1 changes terrain and caves. It is low quality and unconfirmed.
