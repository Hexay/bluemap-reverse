# Seed recovery from bedrock and other position-hashed worldgen randomness (Java 1.18 to 26.x)

Main source for code: a decompiled 26.2 Mojang-mapped tree ([Renekovski/26.2-mcp](https://github.com/Renekovski/26.2-mcp)), cross-checked against a 1.18.1 tree ([jacobo-mc/mc_1.18.1_src](https://github.com/jacobo-mc/mc_1.18.1_src)), cubiomes, and the 19MisterX98 cracker source. Worldgen JSON comes from [misode/mcmeta](https://github.com/misode/mcmeta) at data version 26.4-snapshot-1.

## Q1. Exact 1.18+ bedrock generation and its seed dependence

### Takeaway
Bedrock is placed by a surface-rule `vertical_gradient`. At every (x,y,z) it creates a fresh RNG from `getOrCreateRandomFactory("minecraft:bedrock_floor" | "minecraft:bedrock_roof").at(x,y,z)` and tests `nextFloat() < p(y)`. The **Nether and End use `legacy_random_source: true`**: the RNG is java.util.Random, and the result depends only on the **low 48 bits** of the world seed. The **Overworld uses Xoroshiro128++**: the result depends on the **full 64-bit seed** through a 128-bit derived state.

### Cited Findings
- **Rule JSON (26.4 snapshot).** Surface rules now live in a `worldgen/material_rule` registry. The noise settings reference them (`"material_rule":"minecraft:nether"`).
  - `bedrock_floor.json` = `condition{ vertical_gradient{ random_name:"minecraft:bedrock_floor", true_at_and_below:{above_bottom:0}, false_at_and_above:{above_bottom:5} } -> bedrock }`.
  - `bedrock_roof.json` = `condition{ not{ vertical_gradient{ random_name:"minecraft:bedrock_roof", true_at_and_below:{below_top:5}, false_at_and_above:{below_top:0} } } -> bedrock }`.
  - The nether sequence begins `["minecraft:bedrock_floor","minecraft:bedrock_roof", …]`. The overworld sequence begins `["minecraft:bedrock_floor","minecraft:overworld/copper_ore_vein","minecraft:overworld/iron_ore_vein", …]`. So in 26.4 ore veins are also material rules.
  - Sources: [bedrock_floor.json](https://github.com/misode/mcmeta/blob/data/data/minecraft/worldgen/material_rule/bedrock_floor.json), [bedrock_roof.json](https://github.com/misode/mcmeta/blob/data/data/minecraft/worldgen/material_rule/bedrock_roof.json), [material_rule/nether.json](https://github.com/misode/mcmeta/blob/data/data/minecraft/worldgen/material_rule/nether.json), [material_rule/overworld.json](https://github.com/misode/mcmeta/blob/data/data/minecraft/worldgen/material_rule/overworld.json)
- **Random-source flags.** Noise settings `legacy_random_source`: overworld = `false`, nether = `true`, end = `true`. Sea level is 63 in the overworld and 32 in the nether. — [noise_settings/overworld.json](https://github.com/misode/mcmeta/blob/data/data/minecraft/worldgen/noise_settings/overworld.json), [noise_settings/nether.json](https://github.com/misode/mcmeta/blob/data/data/minecraft/worldgen/noise_settings/nether.json)
- **Gradient condition code (26.2).** — [SurfaceRules.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/SurfaceRules.java)
  ```java
  if (blockY <= trueAtAndBelow) return true;
  else if (blockY >= falseAtAndAbove) return false;
  double probability = Mth.map(blockY, trueAtAndBelow, falseAtAndAbove, 1.0, 0.0);
  RandomSource random = randomFactory.at(blockX, blockY, blockZ);
  return random.nextFloat() < probability;
  ```
  Here `randomFactory = ruleContext.randomState.getOrCreateRandomFactory(randomName)`. In 1.18.1 the same lookup went through `SurfaceSystem.getOrCreateRandomFactory` ([1.18.1 SurfaceRules](https://github.com/jacobo-mc/mc_1.18.1_src/blob/HEAD/src/main/java/net/minecraft/world/level/levelgen/SurfaceRules.java)).
- **Gradient probability.** The wiki gives the probability between anchors as `(false_at_and_above − Y)/(false_at_and_above − true_at_and_below)`. — [Surface rule wiki](https://minecraft.wiki/w/Surface_rule)
- **RandomState seeding chain (26.2).** — [RandomState.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/RandomState.java)
  - `this.random = settings.getRandomSource().newInstance(seed).forkPositional();`
  - `getOrCreateRandomFactory(name) = this.random.fromHashOf(name).forkPositional()` (cached per name).
  - It also creates `aquiferRandom = random.fromHashOf("minecraft:aquifer").forkPositional()` and `oreRandom = random.fromHashOf("minecraft:ore").forkPositional()`.
  - Noises: `Noises.instantiate` does `NormalNoise.create(this.random.fromHashOf(noiseId), params)`.
- **Legacy positional RNG (Nether and End).** — [LegacyRandomSource.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/LegacyRandomSource.java)
  - `forkPositional()` returns `new LegacyPositionalRandomFactory(this.nextLong())`.
  - `fromHashOf(name)` returns `new LegacyRandomSource(name.hashCode() ^ seed)`.
  - `at(x,y,z)` returns `new LegacyRandomSource(Mth.getSeed(x,y,z) ^ seed)`.
- **Xoroshiro positional RNG (Overworld).** — [XoroshiroRandomSource.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/XoroshiroRandomSource.java), [RandomSupport.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/RandomSupport.java)
  - `new XoroshiroRandomSource(long seed)` builds `Xoroshiro128PlusPlus(RandomSupport.upgradeSeedTo128bit(seed))`.
  - `forkPositional()` returns `new XoroshiroPositionalRandomFactory(nextLong(), nextLong())`.
  - `at(x,y,z)` returns `new XoroshiroRandomSource(Mth.getSeed(x,y,z) ^ seedLo, seedHi)`.
  - `fromHashOf(name)` takes the MD5 of the UTF-8 name. hashLo is bytes 0..7 and hashHi is bytes 8..15, both big-endian (`Longs.fromBytes`). It returns `new XoroshiroRandomSource(hashLo ^ seedLo, hashHi ^ seedHi)`, with no mixing.
  - `nextFloat() = (nextLong() >>> 40) * 5.9604645E-8F`.
  - Constants: `GOLDEN_RATIO_64 = -7046029254386353131L` (0x9E3779B97F4A7C15) and `SILVER_RATIO_64 = 7640891576956012809L` (0x6A09E667F3BCC909).
  - `upgradeSeedTo128bitUnmixed(s)`: `lo = s ^ SILVER`, `hi = lo + GOLDEN`, then `mixStafford13` on each half.
- **Xoroshiro reference implementation (cubiomes).** — [cubiomes rng.h](https://github.com/Cubitect/cubiomes/blob/master/rng.h)
  - `mixStafford13`: `z=(z^z>>>30)*0xBF58476D1CE4E5B9; z=(z^z>>>27)*0x94D049BB133111EB; z^z>>>31`.
  - Xoroshiro128++ `nextLong`: `n = rotl(l+h,17)+l; h ^= l; lo = rotl(l,49) ^ h ^ (h<<21); hi = rotl(h,28)`.
- **Position hash.** — [Mth.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/util/Mth.java)
  ```java
  long seed = x * 3129871 ^ z * 116129781L ^ y;
  seed = seed * seed * 42317861L + seed * 11L;
  return seed >> 16;
  ```
  Gotcha: `x * 3129871` is a 32-bit int multiply that wraps. The Rust cracker mirrors this with `x.wrapping_mul(3129871)` as i32. — [block_data.rs](https://github.com/19MisterX98/Nether_Bedrock_Cracker/blob/HEAD/bedrock_cracker/src/block_data.rs)
- **Nether chain in closed form (verified by the cracker's unit test):**
  - `rand = Random(worldSeed).nextLong()`
  - `roofSeed = Random(rand ^ 343340730).nextLong() & MASK48`
  - `floorSeed = Random(rand ^ 2042456806).nextLong() & MASK48`
  - The constants are `"minecraft:bedrock_roof".hashCode()` and `"minecraft:bedrock_floor".hashCode()`.
  - The test asserts that world seed 765906787396911863 gives roof 191924403737289 and floor 18240473916414.
  - Source: [layer.rs](https://github.com/19MisterX98/Nether_Bedrock_Cracker/blob/HEAD/bedrock_cracker/src/layer.rs), [lib.rs](https://github.com/19MisterX98/Nether_Bedrock_Cracker/blob/HEAD/bedrock_cracker/src/lib.rs)
- **Paper bug.** PaperMC before 1.19.2-213 seeded intermediate bedrock layers with the wrong y (0 for the floor and 122 for the roof). This produced "pillars" of bedrock, and the cracker has a dedicated `Paper1_18` mode for it. — [modes.rs / block_data.rs](https://github.com/19MisterX98/Nether_Bedrock_Cracker/blob/HEAD/bedrock_cracker/src/block_data.rs), [Jorian Woltjer blog](https://jorianwoltjer.com/blog/p/stories/part-2-the-new-liveoverflow-minecraft-hacking-server)

### Inferences
- **Per-layer bedrock probability** (from `Mth.map` and the anchors):
  - Overworld floor (minY = −64): y=−64 always; −63: 4/5; −62: 3/5; −61: 2/5; −60: 1/5; ≥−59 never.
  - Nether floor (minY = 0): y=0 always; 1: 4/5; 2: 3/5; 3: 2/5; 4: 1/5.
  - Nether roof: `below_top(k)` resolves to `minY + height − 1 − k`, so for height 128 it is 127 − k. The cracker's `layer − 122` agrees with this. The gradient is true at ≤122 and false at ≥127, and the rule is negated, so bedrock appears when `nextFloat() ≥ (127−y)/5`. That gives y=123: 1/5, 124: 2/5, 125: 3/5, 126: 4/5, 127: always.
- **Full nether formula.** `bedrock(x,y,z) = [ next24( ((getSeed(x,y,z) ^ S) ^ 0x5DEECE66D) & 2^48−1 ) / 2^24 ] <cmp> threshold(y)`.
  - `next24(s)` = `((s*0x5DEECE66D + 0xB) & 2^48−1) >>> 24`.
  - S is roofSeed or floorSeed. Only the low 48 bits of S matter, and S depends only on `worldSeed & 2^48−1`, because `new Random(seed)` masks to 48 bits.
- **Full overworld formula.**
  - `(L1,L2) = two nextLong() of Xoroshiro(mix(seed^SILVER), mix(seed^SILVER+GOLDEN))`
  - `(B1,B2) = two nextLong() of Xoroshiro(md5lo("minecraft:bedrock_floor")^L1, md5hi(...)^L2)`
  - `bedrock(x,y,z) = (Xoroshiro(getSeed(x,y,z)^B1, B2).nextLong() >>> 40) * 2^-24 < (−59−y)/5`
  - The 64-bit seed maps to a 128-bit (B1,B2), and every step is a bijection or an XOR with a constant. So overworld bedrock depends on all 64 bits, and no 48-bit shortcut exists.
- **Deepslate uses the same machinery.** The overworld deepslate transition is `vertical_gradient{random_name:"minecraft:deepslate", true_at_and_below:{absolute:0}, false_at_and_above:{absolute:8}}` ([underground.json](https://github.com/misode/mcmeta/blob/data/data/minecraft/worldgen/material_rule/overworld/underground.json)). Deepslate vs stone at y=1..7 is a Xoroshiro, full-64-bit positional oracle, equivalent to bedrock. It is higher up (y 0–8 vs −64..−60), so it may be exposed somewhat more often in skylit ravines.

### Gaps
- I did not verify every intermediate version (1.18.2 to 1.21.x) line by line. The 1.18.1 and 26.2 code paths agree, and the 1.18.1 `getOrCreateRandomFactory` lived on `SurfaceSystem`. I found no source reporting a change to bedrock seeding after 1.18 (other than the Paper bug).

## Q2. Existing bedrock crackers: algorithm, data needed, timings; pre-1.18

### Takeaway
The only practical 1.18+ bedrock cracker is **19MisterX98's Nether_Bedrock_Cracker** (Rust, with a GUI; xpple wrapped it as a Fabric mod). It brute-forces the 48-bit legacy state of one nether surface (floor or roof) using a bit-layered pruning tree, cross-checks against the other surface, and reverses `nextLong` twice to reach the 48-bit **structure seed**. For the upper 16 bits, **MiranCZ/BedrockSeedCracker** checks all 2^16 candidates against **overworld** bedrock. No published tool cracks Xoroshiro overworld bedrock from scratch. Before 1.18, bedrock was **seed-independent**.

### Cited Findings
- **Scope.** The README says the tool works on 1.18+ "since bedrock became seed dependent in that release". It advises using y=4 and y=123 because bedrock is rarer there. It states "The overworld uses a different RNG, so the calculations used here are not applicable." There are two output modes, world seeds or structure seeds, plus a seed-list filter mode. — [Nether_Bedrock_Cracker README](https://github.com/19MisterX98/Nether_Bedrock_Cracker)
- **Search structure (lib.rs).**
  - Threads split `1<<36` upper-bit prefixes, each shifted left by 12.
  - `checks.run_checks(upper_bits)` runs over a filter tree built for lower-bit counts 12 down to 0.
  - Each layer splits on the next lower bit (`run_checks(upper) ; run_checks(upper + split)`).
  - A block check is `((upper_bits ^ pos_hash) * 0x5DEECE66D + offset) & MASK48 < condition`. Here `pos_hash = (getSeed>>16 masked) ^ multiplier`, and the bounds encode the float threshold times 2^48.
  - Unknown low bits are handled by widening the acceptance interval by `lower_bits_mask * multiplier` ("jiggle room").
  - Checks are grouped 8 at a time and sorted by filter power (expected discarded seeds).
  - Source: [lib.rs](https://github.com/19MisterX98/Nether_Bedrock_Cracker/blob/HEAD/bedrock_cracker/src/lib.rs), [block_data.rs](https://github.com/19MisterX98/Nether_Bedrock_Cracker/blob/HEAD/bedrock_cracker/src/block_data.rs), [layer.rs](https://github.com/19MisterX98/Nether_Bedrock_Cracker/blob/HEAD/bedrock_cracker/src/layer.rs)
- **Surface choice and recovery (layer.rs).** `get_filter_power` estimates survivors as `2^48 * Π(pass probability)`, and the surface with fewer survivors becomes the primary filter. `CrossComparison.run` then:
  1. `reverse_next_long(state)` (via the `next_long_reverser` crate), then XOR with the primary hash;
  2. derives the other surface's seed and checks those blocks;
  3. applies `reverse_next_long` again to get the structure seed;
  4. in WorldSeed mode, applies `reverse_next_long(structure_seed)` then `next_long(prev)` to emit world seeds.

  Source: [layer.rs](https://github.com/19MisterX98/Nether_Bedrock_Cracker/blob/HEAD/bedrock_cracker/src/layer.rs)
- **World-seed mode only works for random seeds.** Random world seeds are generated as `RandomSource.create().nextLong()`, and `RandomSource.create(long)` returns `new LegacyRandomSource(seed)`. So a random seed is a java.util.Random `nextLong` output and is determined by its low 48 bits. — [WorldOptions.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/WorldOptions.java), [RandomSource.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/util/RandomSource.java)
- **Example data.** The example world (seed 765906787396911863) was cracked from 32 bedrock blocks at y=123 plus 4 at y=4, listing only positive "Bedrock" observations. — [example file](https://github.com/19MisterX98/Nether_Bedrock_Cracker/blob/HEAD/examples/seed%20765906787396911863.txt)
- **Mod wrapper.** xpple/NetherBedrockCracker is a Fabric mod wrapping the Rust library. For the overworld it points users to SeedCrackerX. — [xpple/NetherBedrockCracker](https://github.com/xpple/NetherBedrockCracker)
- **MiranCZ/BedrockSeedCracker** (Fabric, Java 1.18+):
  - It cracks the structure seed (48 bits) from nether bedrock, then "goes through all 2^16 combinations for the 16 upper bits and checks it against the overworld bedrock".
  - It collects "512 pieces for the overworld and 128 floor and roof each for the nether".
  - It takes "30 seconds to a couple of minutes".
  - Source: [BedrockSeedCracker README](https://github.com/MiranCZ/BedrockSeedCracker/blob/master/README.md)
- **Overworld search infeasibility.** Jorian Woltjer's write-up (LiveOverflow server) used known-seed bedrock to *locate* coordinates, not to crack a seed. He found the nether feasible thanks to its 1:8 scale and 16×16 chunk structure, but judged an overworld-wide pattern search infeasible ("14 TRILLION" chunks). — [Jorian Woltjer blog](https://jorianwoltjer.com/blog/p/stories/part-2-the-new-liveoverflow-minecraft-hacking-server)
- **Pre-1.18 bedrock.** The chunk RNG was seeded with `(chunkX*341873128712 + chunkZ*132897987541) ^ 0x5DEECE66D`, independent of the world seed. Tools like TerrainFinder therefore use bedrock only to find *coordinates*. — [DaMatrix/TerrainFinder bedrock.c](https://github.com/DaMatrix/TerrainFinder/blob/master/bedrock.c), [silversquirl bedrock-finder-118](https://silversquirl.github.io/bedrock-finder-118/)

### Inferences
- **Information per observation** is `−log2 P(observation)`.
  - Rare-state observations carry the most: bedrock at y=4/123 or −60 (P=1/5), and "not bedrock" at y=1/126 or −63 (P=1/5), are 2.32 bits each.
  - Common states carry little: P=4/5 is 0.32 bits, and P=2/5 or 3/5 is 1.32 or 0.74 bits.
  - The primary surface needs ≳48 bits plus margin, roughly ≥21 rare observations, ideally 25–35. The other surface needs about 5–10 more bits to cut the remaining candidates.
  - Negative observations (a known non-bedrock at y=123 etc.) are as useful as positive ones. They are the "OTHER" block type in the cracker input.
- **Why the nether is easy.** Bit k of the LCG output depends only on input bits ≤k. This allows top-down enumeration of 2^36 prefixes with lazy resolution of the low 12 bits. The practical cost is ~2^36 × (few checks), which is minutes on a desktop CPU.
- **Overworld from scratch** means brute-forcing 2^64 seeds, and each costs ~5 Xoroshiro steps plus MD5 constants (precomputable).
  - At ~10^10–10^11 candidates/s on a GPU that is years, so it is infeasible without a 48-bit prior.
  - An algebraic attack would need to reconstruct the 128-bit (B1,B2) from thresholded top-24-bit outputs of `rotl(h^B1 + B2, 17) + (h^B1)`, across many known h. This is a nonlinear (ARX) problem. I found no published attack.
- **Overworld given the low 48 bits:** 2^16 candidates × (seed→L1,L2→B1,B2 + ~N block checks) is microseconds to milliseconds. About 16 bits plus margin suffices, e.g. ~10 rare observations, far fewer than MiranCZ's 512.
- **Recommended pipeline for this project:**
  1. Get the 48-bit structure seed from nether bedrock if visible, or else from structures (SeedcrackerX-style) or other legacy-seeded nether data (see Q4).
  2. Get the upper 16 bits by checking overworld Xoroshiro-derived observables: biomes via cubiomes, surface noise, bedrock/deepslate if visible.
  3. If the world used a random seed, `reverse_next_long(structure_seed)` gives about one 64-bit candidate directly. Always test this "random seed" hypothesis first.

### Gaps
- No published timing for Nether_Bedrock_Cracker as a function of block count; only MiranCZ's "30 s to minutes". The ~2^36 figure is my reading of the loop bounds.
- I did not find sources for Kinomora, a "mc-bedrock-cracker" crate, or Minecraft@Home bedrock work, and cannot confirm they exist.

## Q3. Nether roof visibility from above; how many positions suffice

### Takeaway
From above, the nether roof is useless: **y=127 is 100% bedrock**, so a top-down view only ever shows a uniform bedrock plane. The informative layers 123–126 are hidden underneath and visible only from below (the ceiling underside) or through holes, and there are no holes at y=127. BlueMap's default nether map also masks out Y 90–127, but otherwise renders the full 3D cave surface below 90. That makes **nether floor** faces (y=1..4) adjacent to air or lava potentially present in the hires geometry.

### Cited Findings
- **Roof rule.** Roof bedrock is certain at y=127 (the gradient is false at and above `below_top:0`, and the rule is negated). — [bedrock_roof.json](https://github.com/misode/mcmeta/blob/data/data/minecraft/worldgen/material_rule/bedrock_roof.json)
- **Useful layers.** The cracker's advice is to collect y=4 and y=123 blocks (the rarest bedrock layers). Its example uses roof y=123 data, which is collected in-game from below. — [Nether_Bedrock_Cracker README](https://github.com/19MisterX98/Nether_Bedrock_Cracker)
- **BlueMap nether defaults.** `remove-caves-below-y -10000` plus a render-mask subtracting Y 90–127, "so End and Nether (below Y90) maps are full 3D visible-surface meshes including caves". For the overworld, faces with sunlight 0 below `remove-caves-below-y` (default 55) are dropped. — [research/04-filling-hidden-data.md](C:/Users/hexay/bluemap_reverse/research/04-filling-hidden-data.md) (project notes, citing BlueMap configs), [research/01-bluemap-web-format.md](C:/Users/hexay/bluemap_reverse/research/01-bluemap-web-format.md)

### Inferences
- **How to read bedrock off a nether map.** With default nether settings, check for bedrock faces at y=1..4 in the hires geometry. Their presence or absence at y=1..4 in exposed columns, such as where lava-lake bottoms or air caverns touch the floor, is exactly the cracker's input. A missing bedrock face at an exposed y=1 or y=2 position means "not bedrock", which is a strong negative observation.
- **Masked vs full maps.** If a map was rendered without the Y 90 mask, the ceiling underside (y=122 is never bedrock; 123..126 are) is rendered as cave surface. Then roof observations become available as well.
- **Occlusion risk.** One exposed face tells you that block is bedrock (or netherrack/other). Absence of a face is ambiguous: the block may be occluded rather than be non-bedrock. Only count positions whose neighbor cell is known air or lava.
- **Sufficiency:** ≈25–35 rare-layer observations on one surface plus ~5–10 on the other (see the Q2 information math). The example used 36 positives.
- **The End** has no bedrock floor, so End bedrock is not usable.

### Gaps
- Whether BlueMap renders bedrock faces under lava in the tile geometry, as opposed to visually, should be verified against this project's own mesh-inversion notes. I did not verify it from BlueMap source here.

## Q4. Other seed-dependent positional/noise features visible from the surface

### Takeaway
Every surface-rule noise (`noise_threshold`), the badlands clay-band array, the surface-depth jitter, and the powder-snow/iceberg randomness derive from `RandomState.random`.
- **In the overworld** they are Xoroshiro and depend on the full 64-bit seed. They are good **filters for the 2^16 upper-bit search**, but they are not invertible primary crackers.
- **In the nether** they are legacy and depend only on the low 48 bits: soul sand/gravel/netherrack/wart patches and nether biomes. They are usable as filters over structure-seed candidates.

The badlands band array is especially attractive. It is a 192-entry table fully determined by one Xoroshiro stream, and it is visible on badlands cliffs.

### Cited Findings
- **SurfaceSystem noises and bands (26.2).** — [SurfaceSystem.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/SurfaceSystem.java)
  - Noises: `clay_bands_offset`, `surface`, `surface_secondary`, `badlands_pillar`, `badlands_pillar_roof`, `badlands_surface`, `iceberg_pillar`, `iceberg_pillar_roof`, `iceberg_surface`.
  - `clayBands = generateBands(noiseRandom.fromHashOf("minecraft:clay_bands"))`. Here `noiseRandom` is `RandomState.random`, the root positional factory.
  - `getBand(x,y,z) = clayBands[(y + round(clayBandsOffsetNoise(x,0,z)*4) + 192) % 192]`.
  - `generateBands` fills 192 entries with TERRACOTTA, then places orange every `nextInt(5)+1`, then applies `makeBands` for yellow (width 1), brown (2) and red (1). Each call uses `nextIntBetweenInclusive(6,15)` bands, width `base+nextInt(3)`, start `nextInt(192)`. Finally 9–15 white bands are placed with `nextInt(16)+4` spacing and optional light-gray neighbors via `nextBoolean`.
  - `getSurfaceDepth = (int)(surfaceNoise(x,0,z)*2.75 + 3.0 + noiseRandom.at(x,0,z).nextDouble()*0.25)`.
  - Powder snow and iceberg columns use `noiseRandom.at(x,0,z)` (`2+nextInt(4)`, `seaLevel+18+nextInt(10)`, `nextDouble()`).
- **Noise names in the rules.** The 26.4 material rules use `minecraft:surface`, `surface_swamp` and `gravel_layer` (plus more in the per-biome files) in the overworld. The nether uses `patch`, `nether_state_selector`, `netherrack`, `nether_wart` and `soul_sand_layer`. — [material_rule/overworld/surface.json](https://github.com/misode/mcmeta/blob/data/data/minecraft/worldgen/material_rule/overworld/surface.json), [material_rule/nether.json](https://github.com/misode/mcmeta/blob/data/data/minecraft/worldgen/material_rule/nether.json)
- **Nether biome noise.** It uses `NormalNoise.createLegacyNetherBiome(new LegacyRandomSource(seed + 0 / seed + 1))` when `legacy_random_source` is true. Terrain with legacy init also uses `LegacyRandomSource(seed)`; otherwise it uses `random.fromHashOf("minecraft:terrain")`. — [RandomState.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/RandomState.java)
- **Structure placement.** `ChunkGenerator` still uses `new WorldgenRandom(new LegacyRandomSource(0L))` for structure-related seeding, and SeedcrackerX 1.18+ cracks from structures. — [ChunkGenerator.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/chunk/ChunkGenerator.java), [SeedcrackerX README](https://github.com/19MisterX98/SeedcrackerX/blob/master/README.md?plain=1)

### Inferences
- **Legacy sources are 48-bit.** Any quantity derived from a LegacyRandomSource has at most 48 bits of seed dependence. This covers all nether surface-rule noises, nether biomes, nether bedrock and End terrain. Nether surface patterns such as soul sand/gravel/blackstone/basalt placement and biome layout can therefore confirm or reject structure-seed candidates. They cannot supply the upper 16 bits.
- **Clay bands as a strong overworld filter.**
  - The ordered colors of a vertical badlands band sequence at one (x,z) give up to 192 symbols from a small alphabet. Choosing columns where `|offset noise*4| < 0.5` removes the offset; alternatively, the offset can be fitted as an unknown integer shift per column.
  - This is tens to hundreds of bits of information, enough to pin the upper 16 bits after the low 48 are known.
  - In principle it also constrains the 128-bit state `(md5lo("minecraft:clay_bands")^L1, md5hi^L2)`. `(L1,L2)` are two consecutive Xoroshiro128++ outputs from `upgradeSeedTo128bit(seed)`. Xoroshiro128++ state is invertible from two consecutive full outputs, and mixStafford13 is a bijection. So recovering the band-generator state would algebraically give the full 64-bit seed.
  - But the generator only exposes bounded `nextInt` values: 32-bit lemire reduction of the low 32 bits. A partial-output state-recovery attack on Xoroshiro128++ is research-grade; I found no existing implementation.
- **Surface noises need a candidate seed.** Surface noises (`surface`, `surface_secondary`, badlands pillars, icebergs, swamp water patches, gravel/coarse-dirt/podzol/powder-snow patches) are NormalNoise (Perlin octaves) whose permutation tables are seeded from Xoroshiro forks. They are only practical as a forward check given a candidate seed, not for inversion.
- **Surface-depth jitter is nearly invisible.** It changes the grass/dirt-to-stone depth by at most 1, and only on rounding. Its only surface signature is subtle dirt/stone boundaries on cliff faces. Low value.
- **Aquifer and ore randoms** are `fromHashOf("aquifer")` and `fromHashOf("ore")`, both Xoroshiro in the overworld. They are mostly invisible, but lava/water aquifer fills in skylit ravines are forward-checkable.

### Gaps
- There is no known public tool that inverts overworld surface noise or clay bands to a seed. The clay-band state-recovery idea is my untested inference.
- I did not enumerate the per-biome `biome_surface/*.json` noise names exhaustively.

## Q5. Decoration/population-seeded features (trees, flowers, etc.) and tree cracking

### Takeaway
Since 1.18, feature decoration uses **Xoroshiro** seeded from the full 64-bit world seed. The classic java.util.Random lattice/tree/dungeon crackers (TreeCracker, LattiCG dungeon cracking, pack.png-era tree cracking) therefore no longer reduce to a 48-bit LCG problem. SeedcrackerX explicitly dropped dungeon and fungus cracking for 1.18+. Structures remain 48-bit legacy and are the standard lower-48 source.

### Cited Findings
- **Decoration RNG (26.2).**
  - `ChunkGenerator.applyBiomeDecoration` uses `new WorldgenRandom(new XoroshiroRandomSource(RandomSupport.generateUniqueSeed()))`, then `setDecorationSeed(level.getSeed(), originX, originZ)`, then `setFeatureSeed(decorationSeed, index, step)` per feature.
  - `WorldgenRandom.setSeed` delegates to the wrapped source's `setSeed`, which for Xoroshiro means `upgradeSeedTo128bit`.
  - `setDecorationSeed`: `setSeed(seed); xs = nextLong()|1; zs = nextLong()|1; result = chunkX*xs + chunkZ*zs ^ seed; setSeed(result)`.
  - `setFeatureSeed`: `setSeed(decorationSeed + index + 10000*step)`.
  - `setLargeFeatureWithSalt`: `x*341873128712 + z*132897987541 + seed + salt`.
  - Sources: [ChunkGenerator.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/chunk/ChunkGenerator.java), [WorldgenRandom.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/WorldgenRandom.java)
- **SeedcrackerX 1.18+.** "Dungeon cracking, fungus cracking don't work anymore". It uses structures (igloo, desert pyramid, jungle temple and swamp hut at 9 bits each, shipwreck at 8, outpost, monument), needing "40 bits of liftable structures and 32 regular bits". It also reports no nether method in 1.18+ (before the bedrock cracker existed). — [SeedcrackerX README](https://github.com/19MisterX98/SeedcrackerX/blob/master/README.md?plain=1)
- **pack.png and 1.18 panorama.** pack.png (pre-1.18) was cracked by Minecraft@Home using tree crackers on trees in one population chunk. The 1.18 panorama seed was cracked by cortex (Oct 17 2021) after "recent changes that messed with the way features get seeded/generated, and lots of code digging". — [Minecraft@Home pack.png](https://minecraftathome.com/projects/packpng.html), [1.18 Panorama](https://minecraftathome.com/projects/1-18-panorama.html), [ResetEra](https://www.resetera.com/threads/the-seed-for-minecrafts-pack-png-found.297059/)
- **Tree crackers and lattice tools.** TreeCracker (MCRcortex), treecrackerPOS (polymetric), and LattiCG (JavaRandom seed reversal via LLL and branch-and-bound). — [SeedFinding/fnseedc](https://github.com/SeedFinding/fnseedc)

### Inferences
- **Tree cracking in 1.18+ is expensive.**
  - A chunk's feature seed is a 64-bit value, with trees drawn from `Xoroshiro(upgradeSeedTo128bit(featureSeed))`. Tree data can at best constrain that 64-bit feature seed, which needs a ~2^64 brute force, or ~2^32 given 32 bits of prior knowledge.
  - Mapping back to the world seed goes through `setDecorationSeed`. That function uses Xoroshiro `nextLong`s of the world seed, so it is also full-64.
  - So tree-based cracking in 1.18+ is only practical as a **filter over a small candidate set**, e.g. 2^16 upper-bit candidates after structures or nether bedrock give the low 48. Presumably that is what the 1.18 panorama effort did, but the page gives no method, so this is unconfirmed.
- **Forward checks are cheap.** For our use, visible tree, flower and grass placement in a chunk is a cheap forward check once a candidate 64-bit seed exists. With the feature index/step order known for the version, it gives many bits per chunk.

### Gaps
- I found no written description of the 1.18 panorama cracking method (the credits doc was not fetched), and no public 1.18+ tree cracker.

## Q6. Seed-independent position randomness (useless for cracking)

### Takeaway
Block-model random offsets and rotations are pure functions of block position and carry **no seed information**. They are useful only for locating coordinates, as in the 1.18 panorama grass-rotation work. Pre-1.18 bedrock is in the same category.

### Cited Findings
- **Pre-1.18 bedrock** used the chunk-coordinate-only seed `(x*341873128712 + z*132897987541)^0x5DEECE66D`. — [TerrainFinder bedrock.c](https://github.com/DaMatrix/TerrainFinder/blob/master/bedrock.c)
- **Coordinate finding.** The 1.18 panorama project used cloud analysis and "grass texture rotations" to find coordinates before seedcracking. — [1.18 Panorama](https://minecraftathome.com/projects/1-18-panorama.html)
- **`Mth.getSeed(x,y,z)`** itself is seed-free. It is XORed into the seeded factory state only inside `at()`. — [Mth.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/util/Mth.java), [XoroshiroRandomSource.java](https://github.com/Renekovski/26.2-mcp/blob/HEAD/src/net/minecraft/world/level/levelgen/XoroshiroRandomSource.java)

### Inferences
- **Classify each observable by its RNG root.**
  - Uses `Mth.getSeed` alone, or client-side model offsets/rotations: seed-free.
  - Uses a `RandomState`/`LegacyRandomSource` root with legacy flag true (Nether, End): 48-bit.
  - Uses a Xoroshiro root (overworld noises, bedrock, deepslate, clay bands, aquifers, ores, decoration): full 64-bit.

### Gaps
- None beyond those listed above.
