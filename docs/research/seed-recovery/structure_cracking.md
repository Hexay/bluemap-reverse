# Recovering a Java Edition world seed from observed structure positions (1.18 → 26.x)

Source key (used below):
- [mcmeta] = misode/mcmeta vanilla data dumps, `data` branch = 26.4-snapshot-1 (built 2026-09-22), and `<ver>-data` tags for 1.18.2, 1.19.4, 1.21.11, 26.1, 26.2, 26.3 — https://github.com/misode/mcmeta (files `data/minecraft/worldgen/structure_set/*.json`)
- [cubiomes] = Cubitect/cubiomes `finders.c` / `finders.h` (master) — https://github.com/Cubitect/cubiomes/blob/master/finders.c , https://github.com/Cubitect/cubiomes/blob/master/finders.h
- [pumpkin] = Pumpkin-MC (Rust vanilla-compatible server) `crates/pumpkin-world/src/generation/structure/placement.rs`, a port of vanilla `StructurePlacement` / `ChunkGeneratorStructureState` — https://github.com/Pumpkin-MC/Pumpkin/blob/master/crates/pumpkin-world/src/generation/structure/placement.rs
- [wiki-ss] = https://minecraft.wiki/w/Structure_set
- [scx] = https://github.com/19MisterX98/SeedcrackerX ; [scx-bits] = https://seedcracker.blog/structure-bit-calculator/
- [hube] = KaptainWutax/hube12 "seed-cracking.md" gist — https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c
- [mc@h] = https://minecraftathome.com/projects/packpng.html
- [spigot] = https://www.spigotmc.org/wiki/spigot-configuration/

Items marked **(memory, verify)** are from my knowledge of the deobfuscated Mojang source and were not re-confirmed against a fetched source this session.

---

## 1. Exact placement algorithm in 1.18+ (random_spread, frequency reduction, concentric_rings) and the constants table

### Takeaway
Every vanilla overworld/nether/end structure except strongholds is placed by `random_spread`: the world is cut into `spacing`×`spacing`-chunk regions; one candidate chunk per region is drawn from a `java.util.Random` (48-bit LCG) seeded with `worldSeed + rx*341873128712 + rz*132897987541 + salt`, so the candidate position depends **only on the lower 48 bits of the world seed** (the "structure seed"). Constants have been stable from 1.19.4 through 26.4-snapshot-1; the only placement additions since 1.18 are ancient cities (1.19), trail ruins (1.20), trial chambers (1.21) and **abandoned_camp (new in 26.3)**.

### Cited Findings

**Candidate chunk (random_spread)**
- Region seed and linear draw, as implemented by cubiomes (`getFeatureChunkInRegion`, identical to Java `Random.setSeed` + `nextInt`):
  ```c
  seed = worldSeed + regX*341873128712ULL + regZ*132897987541ULL + salt;
  seed = (seed ^ 0x5DEECE66D);                 // Random.setSeed scramble
  seed = (seed * 0x5DEECE66D + 0xB) & (2^48-1);
  r = spacing - separation;
  if (r not power of 2) { x = (int)(seed >> 17) % r; seed = next; z = (int)(seed >> 17) % r; }
  else                  { x = (int)((r * (seed >> 17)) >> 31); seed = next; z = ...; } // Java pow2 special case
  blockX = (regX*spacing + x) << 4;  blockZ = (regZ*spacing + z) << 4;
  ```
  — [cubiomes finders.h `getFeatureChunkInRegion`/`getFeaturePos`](https://github.com/Cubitect/cubiomes/blob/master/finders.h). (cubiomes ignores Java's `nextInt` rejection loop for non-pow2 bounds; rejection probability ≈ r/2^31, negligible.)
- Triangular spread (monuments, mansions, end cities): two draws per axis, `x = (nextInt(r) + nextInt(r)) / 2`, order x,x,z,z — [cubiomes `getLargeStructureChunkInRegion`](https://github.com/Cubitect/cubiomes/blob/master/finders.h); wiki: "triangular: takes the sum of 2 random numbers between 0 and spacing - separation - 1 (inclusive) then divides that sum by 2" — [wiki-ss](https://minecraft.wiki/w/Structure_set).
- Regions are `floorDiv(chunk, spacing)`; wiki: grid "starting from chunk 0,0 and expanding outward in intervals defined by spacing" — [wiki-ss](https://minecraft.wiki/w/Structure_set). Translating the seed by `moveStructure(seed, dx, dz) = (seed - dx*341873128712 - dz*132897987541) & (2^48-1)` shifts all structures of every type by (dx,dz) regions — [cubiomes finders.h](https://github.com/Cubitect/cubiomes/blob/master/finders.h). (Useful: the constants are the same for all types, so a seed's layout is a lattice-translate of another's.)
- Parameter semantics: `spacing` 0–4096, `separation` strictly < spacing, `salt` non-negative int, `frequency` default 1.0, `frequency_reduction_method` ∈ {default, legacy_type_1, legacy_type_2, legacy_type_3}, `exclusion_zone {other_set, chunk_count}`, `locate_offset` (only affects /locate) — [wiki-ss](https://minecraft.wiki/w/Structure_set).

**Full accept test** (vanilla order: start-chunk → frequency reduction → exclusion zone; biome/terrain checks happen afterwards in structure generation) — [pumpkin `should_generate_structure`](https://github.com/Pumpkin-MC/Pumpkin/blob/master/crates/pumpkin-world/src/generation/structure/placement.rs). Frequency reduction (only evaluated when `frequency < 1.0`):
- `default`: `Random(regionSeed(seed, salt, chunkX, chunkZ)).nextFloat() < f` — note the argument **shuffle**: vanilla calls `setLargeFeatureWithSalt(seed, salt, chunkX, chunkZ)`, i.e. the formula becomes `seed + salt*341873128712 + chunkX*132897987541 + chunkZ` — [pumpkin](https://github.com/Pumpkin-MC/Pumpkin/blob/master/crates/pumpkin-world/src/generation/structure/placement.rs) (`get_region_seed(seed, salt, chunk_x, chunk_z)`). No vanilla set uses `default` with f<1.
- `legacy_type_1` (pillager outposts): `Random((chunkX>>4) ^ ((chunkZ>>4)<<4) ^ seed); nextInt(); nextInt((int)(1/f)) == 0` — [pumpkin](https://github.com/Pumpkin-MC/Pumpkin/blob/master/crates/pumpkin-world/src/generation/structure/placement.rs); same as cubiomes `setAttemptSeed` + `nextInt(5)==0` — [cubiomes finders.c](https://github.com/Cubitect/cubiomes/blob/master/finders.c).
- `legacy_type_2` (buried treasure): `Random(regionSeed(seed, chunkX, chunkZ, 10387320)).nextFloat() < f` — [pumpkin]; cubiomes: `seed = regX*341873128712 + regZ*132897987541 + seed + 10387320; nextFloat < 0.01`, position = chunk*16+9 — [cubiomes finders.c](https://github.com/Cubitect/cubiomes/blob/master/finders.c).
- `legacy_type_3` (mineshafts): `Random(largeFeatureSeed(seed, chunkX, chunkZ)).nextDouble() < f` — [pumpkin](https://github.com/Pumpkin-MC/Pumpkin/blob/master/crates/pumpkin-world/src/generation/structure/placement.rs), where largeFeatureSeed = `setSeed(seed); a=nextLong(); b=nextLong(); setSeed(chunkX*a ^ chunkZ*b ^ seed)` (cubiomes `chunkGenerateRnd`) — [cubiomes finders.h](https://github.com/Cubitect/cubiomes/blob/master/finders.h).
- Exclusion zone: the structure is rejected if any chunk within ±chunk_count of it is an accepted start of `other_set` (pillager outposts vs villages, 10 chunks) — [pumpkin](https://github.com/Pumpkin-MC/Pumpkin/blob/master/crates/pumpkin-world/src/generation/structure/placement.rs); cubiomes "look for villages within 10 chunks" and requires the village to be biome-viable — [cubiomes finders.c ~L1642](https://github.com/Cubitect/cubiomes/blob/master/finders.c).

**Vanilla structure_set table** (identical in 1.19.4, 1.21.11, 26.1, 26.2, 26.3, 26.4-snapshot-1 except where noted; r = spacing − separation) — [mcmeta](https://github.com/misode/mcmeta):

| set | spacing | sep | r | salt | spread | extra |
|---|---|---|---|---|---|---|
| villages | 34 | 8 | 26 | 10387312 | linear | |
| desert_pyramids | 32 | 8 | 24 | 14357617 | linear | |
| igloos | 32 | 8 | 24 | 14357618 | linear | |
| jungle_temples | 32 | 8 | 24 | 14357619 | linear | |
| swamp_huts | 32 | 8 | 24 | 14357620 | linear | |
| pillager_outposts | 32 | 8 | 24 | 165745296 | linear | freq 0.2 legacy_type_1; exclusion villages 10 |
| ocean_monuments | 32 | 5 | 27 | 10387313 | triangular | |
| woodland_mansions | 80 | 20 | 60 | 10387319 | triangular | |
| shipwrecks | 24 | 4 | 20 | 165745295 | linear | |
| ocean_ruins | 20 | 8 | 12 | 14357621 | linear | |
| ruined_portals | 40 | 15 | 25 | 34222645 | linear | (overworld + nether share one set) |
| ancient_cities | 24 | 8 | 16 | 20083232 | linear | from 1.19 |
| trail_ruins | 34 | 8 | 26 | 83469867 | linear | from 1.20 |
| trial_chambers | 34 | 12 | 22 | 94251327 | linear | from 1.21 |
| abandoned_camp | 37 | 8 | 29 | 91231127 | linear | **new in 26.3** (absent in 26.2); 18 biome variants incl. `pale_garden`, `dappled_forest` |
| buried_treasures | 1 | 0 | 1 | 0 | linear | freq 0.01 legacy_type_2; locate_offset [9,0,9] |
| mineshafts | 1 | 0 | 1 | 0 | linear | freq 0.004 legacy_type_3 |
| nether_complexes (fortress+bastion) | 27 | 4 | 23 | 30084232 | linear | |
| nether_fossils | 2 | 1 | 1 | 14357921 | linear | |
| end_cities | 20 | 11 | 9 | 10387313 | triangular | |
| strongholds | concentric_rings | distance 32, spread 3, count 128, preferred_biomes `#stronghold_biased_to`, salt 0 | | | | |

- 1.18.2 data: villages/pillager/buried treasure files have the same spacing/sep/salt, but **no `frequency`/`frequency_reduction_method`/`exclusion_zone`** fields (outpost 1-in-5 and village exclusion, and treasure 0.01, were hard-coded in the structure classes then); no ancient_cities/trail_ruins/trial_chambers — [mcmeta 1.18.2-data](https://github.com/misode/mcmeta/tree/1.18.2-data/data/minecraft/worldgen/structure_set).
- Pre-1.18 differences (cubiomes version switch): villages 32/24-range (i.e. spacing 32, sep 8) through 1.17 → 34/26 from 1.18; ocean ruins 16/8 and shipwrecks 16/8 through 1.15; nether ruined portals 25/15 (separate) in 1.16–1.17; fortress pre-1.16 used a completely different legacy algorithm (`nextInt(3)==0` per 16×16-chunk region); in 1.16–1.17 fortress/bastion choice = `nextInt(5) < 2` after the region draw; pre-1.13 temples/huts/igloos all shared salt 14357617 — [cubiomes finders.c `getStructureConfig`/`getStructurePos`](https://github.com/Cubitect/cubiomes/blob/master/finders.c).
- Nether complex type in 1.18+: at the candidate chunk, `chunkGenerateRnd(seed, cx, cz)` then `nextInt(5) >= 2` → bastion attempt, else fortress; cubiomes treats fortress as "gen where bastions don't (biome dependent)" — [cubiomes finders.c](https://github.com/Cubitect/cubiomes/blob/master/finders.c).
- End cities additionally require distance from origin ≥ 1008 blocks (cubiomes check `x²+z² >= 1008²`) — [cubiomes finders.c](https://github.com/Cubitect/cubiomes/blob/master/finders.c).

**Strongholds (concentric_rings)** — vanilla `ChunkGeneratorStructureState.generateRingPositions`, as ported by [pumpkin](https://github.com/Pumpkin-MC/Pumpkin/blob/master/crates/pumpkin-world/src/generation/structure/placement.rs):
```
rnd = JavaRandom(seed); angle = rnd.nextDouble()*2π; circle=0; posInCircle=0; spread=3
for i in 0..128:
  dist = 4*distance + distance*circle*6 + (rnd.nextDouble()-0.5)*distance*2.5   // chunks, distance=32
  cx = floor(cos(angle)*dist + 0.5); cz = floor(sin(angle)*dist + 0.5)
  forkSeed = rnd.nextLong()                        // per-ring-slot biome-search RNG
  angle += 2π/spread; posInCircle++
  if posInCircle == spread: circle++; posInCircle=0; spread += 2*spread/(circle+1); spread = min(spread, 128-i); angle += rnd.nextDouble()*2π
then per slot: scan quart positions ±28 (=112 blocks) around (cx*4+2, cz*4+2); reservoir-sample a preferred-biome quart with JavaRandom(forkSeed).nextInt(found+1)==0; fallback = (cx,cz)
```
Ring counts from wiki: spread on ring N = `spread*(N^2+3N+2)/6`… and biome search radius 112 blocks — [wiki-ss](https://minecraft.wiki/w/Structure_set).

### Inferences
- The candidate position is a function of `(seed mod 2^48, salt, rx, rz)` only; biomes (which in 1.18+ are Xoroshiro/multinoise from the full 64-bit seed) only decide *whether* the attempt succeeds. So any observed structure = a hard constraint on the 48-bit structure seed, independent of the upper 16 bits.
- Stronghold ring *angles/distances* depend only on the 48-bit seed, but the final snapped chunk depends on biomes (full seed). Strongholds are invisible on BlueMap anyway.
- Because every salt is fixed and public, a server with custom `seed-*` salts (see §6) produces a layout that no vanilla cracker can match — the cracker should support per-set salt overrides.

### Gaps
- I did not pull decompiled Mojang source directly; the frequency-reduction and ring code is taken from Pumpkin's port (which says it matches vanilla) plus cubiomes. Cross-check against a Mojmap decompile (`RandomSpreadStructurePlacement`, `StructurePlacement`, `ChunkGeneratorStructureState`) before shipping.
- 26.x changelogs for abandoned camps (spawn biome rules, surface footprint) were not fetched; only the structure_set JSON was.
- cubiomes' `MC_NEWEST` is still `MC_1_21_WD`; it has no abandoned_camp support.

---

## 2. Which structures are visible from the surface in a BlueMap render, and deriving the start chunk from blocks

### Takeaway
Surface-visible and useful: villages, desert pyramids, jungle temples, swamp huts, igloos (top only), pillager outposts, woodland mansions, shipwrecks (beached or seen through water), ocean monuments and ocean ruins (under water), ruined portals, abandoned camps (26.3+), nether fortresses/bastions (if the nether map is rendered below the roof), end cities and end pillars. Mostly invisible: strongholds, ancient cities, trial chambers, mineshafts, buried treasure, most trail ruins. For fixed-template structures the template's origin corner is tied to the start chunk's min corner, so the region candidate chunk can be derived exactly once the rotation is known.

### Cited Findings
- Monument footprint: cubiomes places monuments with the triangular draw and block pos `chunk<<4` — [cubiomes finders.h `getLargeStructurePos`](https://github.com/Cubitect/cubiomes/blob/master/finders.h).
- Buried treasure is at chunk*16+9 (x and z) — [cubiomes finders.c](https://github.com/Cubitect/cubiomes/blob/master/finders.c); `locate_offset [9,0,9]` — [mcmeta](https://github.com/misode/mcmeta).
- cubiomes `getVariant()` recovers "rotation and bounding box of a structure instance" for supported types from `chunkGenerateRnd(worldSeed, chunkX, chunkZ)` (a `java.util.Random`, 48-bit) — [cubiomes finders.h](https://github.com/Cubitect/cubiomes/blob/master/finders.h). This confirms structure-internal randomness (rotation, variant/template choice) is also a function of the 48-bit seed and the start chunk.
- SeedCrackerX in 1.18+ uses "igloos, desert pyramids, jungle temples, swamp huts, shipwrecks, pillager outposts, and ocean monuments" (the ones whose start chunk it can pin from blocks) — [scx](https://github.com/19MisterX98/SeedcrackerX).

### Inferences (memory, verify against Mojmap source)
- Desert pyramid / jungle temple / swamp hut are `ScatteredFeaturePiece`s whose bounding box min corner is the start chunk's min block (chunkX*16, chunkZ*16); sizes 21×?×21 (pyramid), 12×?×15 (jungle), 7×?×9 (hut); orientation random (nextInt(4)-type draw) → the footprint's min x/z modulo 16 should be 0 → start chunk = floor(minX/16), floor(minZ/16). For non-square footprints the orientation also gives ~2 bits.
- Ocean monument: 58×58 footprint starting at `chunk.getBlockX(9) - 29` → footprint min = chunkX*16 − 20; center block ≈ chunkX*16 + 9. Recover chunk = (minX + 20)/16.
- Villages (jigsaw): the start (town-center / meeting point) piece is placed with its template origin at the start chunk's min corner, rotated about that origin; the rest of the village grows up to 80 blocks away. Ambiguity: you must (a) identify which piece is the town center (bell / well / meeting-point template — distinctive per village type), (b) match its template and rotation, then (c) the rotated origin corner gives the chunk. Without template matching, a village only bounds the chunk to within ~±5 chunks — weak, and should be treated as a set of candidate chunks (OR constraint), not a point.
- Shipwrecks, ocean ruins, ruined portals, igloos, outposts, abandoned camps: template placed at/relative to the start chunk min; with a known template library (same one the project already uses for block-state matching) plus rotation/mirror, the start chunk is exact. Ruined portals may be shifted vertically only; ocean-ruin clusters spawn extra satellite ruins (only the main one marks the start).
- Mansions: large, visible, 2 triangular draws per axis over r=60 — high information but rare (spacing 80 chunks = 1280 blocks).
- End: end cities are visible; end pillars (see §5) are visible if the map covers 0,0.
- Nether: fortresses/bastions only visible if BlueMap renders the nether with a max-Y cut (typical nether configs do). Nether ruined portals share the overworld set's salt/spacing → usable too.
- Observability caveat: BlueMap hires tiles show exact block states, so matching against structure templates (via the project's existing look-alike scoring) is the practical way to get (template, rotation, origin) → start chunk.

### Gaps
- Exact template origins/pivots for igloo, shipwreck, ruined portal, ocean ruin, outpost, abandoned camp, and village start pools per type were not verified this session; they must be taken from the decompiled `*Structure`/`*Pieces` classes and the template NBT sizes (misode/mcmeta `assets` or `data/.../structure/*.nbt`).
- Whether BlueMap renders submerged monuments/shipwrecks in hires tiles depends on the map's render settings (not researched here).

---

## 3. How many structures are needed; information per structure; biome checks

### Takeaway
Each exactly-located structure carries ≈ log2(r²) bits about the 48-bit structure seed (≈ 9 bits for most types; ~7 for ocean ruins, ~6 for end cities, ~11 for mansions). Uniqueness needs a bit more than 48 bits total, i.e. **~6 good structures**, fewer if you add rotation/variant bits or end pillars. Surviving structure-seed candidates are then extended to 64 bits by testing the 2^16 upper-bit values against biomes/terrain, which in 1.18+ depend on the full seed.

### Cited Findings
- SeedCrackerX bit values (log2(offset×offset), rounded): igloo/desert pyramid/jungle temple/swamp hut 9, shipwreck 8, pillager outpost 9 (liftable only), ocean monument 9 (normal only), trial chambers 8 (normal only); cracking starts at ~32 bits (with end pillars, "normal" route) or ~40 bits of liftable structures ("lifting" route) — [scx-bits](https://seedcracker.blog/structure-bit-calculator/); README: "40 bits of liftable structures and 32 regular bits", or e.g. "3 shipwrecks, 1 pyramid and 1 igloo" — [scx](https://github.com/19MisterX98/SeedcrackerX).
- Only lower 48 bits of the seed feed Java `Random`; seeds sharing them ("one of 65536 similar seeds") differ only in content generated by the 64-bit generators — [hube](https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c).
- 1.18+ population/decorator seeds come from a Xoroshiro128++ seeded with the **full** world seed (`xSetSeed(ws)`; a = nextLong|1, b = nextLong|1, `(x*a + z*b) ^ ws`) — [cubiomes finders.c `getPopulationSeed`](https://github.com/Cubitect/cubiomes/blob/master/finders.c).
- cubiomes `isViableStructurePos()` is the biome-validity test that must follow the position test ("Some structure types may fail to produce a valid position… use isViableStructurePos() to test if the necessary biome requirements are met") — [cubiomes finders.h](https://github.com/Cubitect/cubiomes/blob/master/finders.h).

Computed bits per exact observation (my calculation; linear = 2·log2 r, triangular = 2·H((U1+U2)>>1) averaged):

| type | r | bits | low bits "liftable" per axis (v2(r)) |
|---|---|---|---|
| village | 26 | 9.40 | 1 |
| pyramid/jungle/hut/igloo/outpost | 24 | 9.17 | 3 |
| shipwreck | 20 | 8.64 | 2 |
| ocean ruin | 12 | 7.17 | 2 |
| ruined portal | 25 | 9.29 | 0 |
| ancient city | 16 (pow2 → top bits) | 8.00 | 0 |
| trail ruins | 26 | 9.40 | 1 |
| trial chambers | 22 | 8.92 | 1 |
| abandoned camp (26.3+) | 29 | 9.72 | 0 |
| nether complex | 23 | 9.05 | 0 |
| monument (tri) | 27 | ~8.95 avg | – |
| mansion (tri) | 60 | ~11.26 avg | – |
| end city (tri) | 9 | ~5.80 avg | – |

### Inferences
- Rule of thumb: need Σbits ≳ 48 + log2(desired false positives⁻¹) to be unique; with ~9 bits each, 6 structures ≈ 54 bits → expected false candidates ≈ 2^(48−54) ≈ 0.016. With 5 structures (~45 bits) expect ~8 survivors, which the 2^16 biome/terrain stage then disambiguates cheaply — so 5 is workable, 4 (~36 bits → ~4000 survivors × 65536 upper values) is borderline but still fine if the verifier is fast.
- Rotation / template variant of each structure (from `largeFeatureSeed(seed, cx, cz)`) adds ~2 bits (rotation) plus log2(#templates) (shipwrecks ~20 templates, ruined portals ~13 + mirror) per structure — cheap extra filtering at the candidate-verification stage.
- Negative information: a region whose candidate chunk lies in a valid biome and inside the rendered map but shows no structure (and isn't player-cleared) rejects candidates — but requires the biome model (full seed), so use only in the final 64-bit stage.
- BlueMap biome tint colors + lowres heightmap make an excellent 2^16 verifier: for each of 65536 upper-bit values, compute biomes (cubiomes `getBiomeAt` at a few dozen sample points) and compare to observed tint classes; then confirm with terrain height at a few points.
- Beware: a 1.18+ text seed (typed string) has `String.hashCode()` value → world seed in [−2^31, 2^31), i.e. upper 16 bits all-0 or all-1 — only 2 candidates instead of 65536 (memory, verify). Random seeds come from `new Random().nextLong()`, which reaches only 2^48 of the 2^64 values; given a structure seed, the nextLong-reachable world seeds can be computed directly (≈1 on average) — [hube](https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c) (describes `fromNextLong()` reversal with the LCG inverse `0xdfe05bcb1365`). Try these few candidates before the 2^16 sweep.

### Gaps
- No source measured false-positive rates empirically per structure type in 1.18+; the numbers above are information-theoretic.

---

## 4. Cracking methods and costs

### Takeaway
Don't brute-force 2^48 naively (hours on GPU, ~days on CPU). Use **lifting**: structures with even r leak the low bits of the LCG state, so brute-force the low 17+k bits first, filter with all such structures, then extend the upper bits; or use **end pillars** (16 free bits) to cut the space to 2^32. SeedCrackerX-class crackers finish in minutes on a PC. Lattice (LLL) methods apply cleanly to power-of-two / float / double outputs.

### Cited Findings
- SeedCrackerX: "the cracking process starts automatically. This process takes around 1-5 mins"; in 1.18+ "Dungeon cracking, fungus cracking don't work anymore"; supports 1.16.5 through 1.21.11 with a 2.16.1 release for "MC 1.26.2" (as summarised) — [scx](https://github.com/19MisterX98/SeedcrackerX).
- Lifting "begins after roughly 40 bits from structures whose region offsets can be lifted efficiently, first filtering lower seed bits and then testing complete structure-seed candidates" — [scx-bits](https://seedcracker.blog/structure-bit-calculator/).
- End pillars: `nextLong() & 65535` leaks 16 bits → search 2^48 → 2^32; "bruteforcing 16bits is done in less than 0.1s on modern cpu"; 32-bit search "under a minute" on CPU / "under 15s on modern GPU" (as extracted); lattices usable "since technically all of the internal state of the LCG are linked"; nextLong and power-of-two nextInt are directly reversible; LCG multiplier inverse `0xdfe05bcb1365` — [hube](https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c).
- Scale reference: pack.png (Alpha 1.2.2) — full 2^48 search estimated at "nearly 1.5 years" on a single PC, done in "only 3 days" by ~3700 BOINC volunteers, yielding "just under 700K matching seeds" then filtered by terrain height on one PC; filter was sand/dirt beach mixing, not structures — [mc@h](https://minecraftathome.com/projects/packpng.html).

### Inferences (derivation — implementable)
- **Lifting math.** For region seed S = (seed + A·rx + B·rz + salt) mod 2^48, scrambled s0 = S ^ 0x5DEECE66D, s1 = (s0·K + 11) mod 2^48, x = (s1>>17) mod r. Write r = 2^k·m (m odd). Then x mod 2^k = bits [17, 17+k) of s1. Since LCG low bits depend only on low bits, those bits are a function of seed mod 2^(17+k). Same for z with s2. So:
  1. Enumerate L = 2^(17+k) low-bit values (k=3 for the r=24 set → 2^20 ≈ 1M).
  2. Each r=24 structure gives 2k = 6 filter bits; shipwrecks/ocean ruins (k=2) 4 bits over 2^19; villages/trail ruins/trial chambers (k=1) 2 bits over 2^18 — layer them (check k=1 bits at 2^18, extend, etc.).
  3. With n liftable r=24 structures, survivors ≈ 2^20 / 2^(6n); then enumerate the remaining 28 bits (2^28 per survivor) and test full x,z equality. Total ≈ 2^20 + 2^(48−6n)·(cheap) work — with 4 temples this is ~2^24 full tests: milliseconds–seconds.
  4. Pillager outposts are liftable but their frequency check (legacy_type_1) and exclusion zone are extra filters, not position info; monuments/mansions/end cities (triangular) and odd-r sets (portals, nether complexes, abandoned camps) are not liftable → used only in the final filter.
- **End-pillar route**: pillar seed p (16 bits) = low 16 bits of the *second* `next(32)` of `new Random(seed).nextLong()` → bits 16..31 of LCG state 2 given state 1 (which is from seed^K); enumerate 2^32 seeds consistent with p (the 16 fixed bits are linear in the lower 32 state bits, so the free bits can be enumerated directly) and filter with ≥ ~32 structure bits.
- **Naive 2^48**: 2.8·10^14 candidate × (1 region-seed + 2 LCG steps + mod) per structure. At ~10^10–10^11 simple tests/s on a modern GPU that's ~1–8 h; on a 16-core CPU at ~10^9–10^10/s ~8–80 h. (Estimate, no benchmark source.) Lifting makes this unnecessary whenever ≥3–4 r=24/20 structures are known.
- **Lattice**: ancient cities (r=16, power of two) reveal the top 4 bits of s1 and s2 (bits 44–47) → linear-interval constraints on LCG states; combine many such top-bit constraints across regions with LLL (the region offsets A·rx + B·rz are known additive constants, but the `^ 0x5DEECE66D` scramble is XOR on the seed, which is linear only over GF(2) — so apply lattice to *differences of states within one region* or treat the scrambled per-region seeds as unknowns tied by known differences modulo carries; practical crackers prefer lifting + brute force). Ancient cities are underground and rarely visible from BlueMap anyway.
- **Upper 16 bits (1.18+)**: per structure-seed candidate, try the few nextLong-reachable seeds and the two text-seed-shaped seeds first, then all 65536 upper values with biome samples (cubiomes `setupGenerator`/`applySeed`/`getBiomeAt` at scale 4, ~10–100 µs per sample on CPU) — 2^16 × ~20 samples ≈ seconds. Terrain height (BlueMap lowres) as a tiebreaker.

### Gaps
- No primary benchmark found for GPU structure-seed brute force in 1.18+ (the "under 15 s GPU / under a minute CPU" for 2^32 is from the hube12 gist, older versions).
- Columbia University SeedCracker report (https://www.cs.columbia.edu/~sedwards/classes/2021/4995-fall/reports/SeedCracker.pdf) could not be text-extracted by the fetch tool.

---

## 5. Other seed-dependent content: which bits they constrain and whether they're visible

### Takeaway
In 1.18+ nearly all *decorations* (trees, dungeons, wells, geodes, fossils, end gateways) are seeded by Xoroshiro from the **full 64-bit** seed, so they cannot crack the 48-bit structure seed (the reason SeedCrackerX dropped dungeon/fungus cracking) — but they are strong verifiers for the upper 16 bits. Java-Random-only leaks that remain: structure positions and structure internals (rotation/variants), end pillars (16 bits), slime chunks, buried treasure, mineshafts.

### Cited Findings
- 1.18+ population seed uses Xoroshiro seeded with the full world seed; decorator features (end gateways, end islands, desert wells, geodes) in 1.18+ use `xSetSeed(popSeed + salt)`, `xNextFloat() < rarity`, then `xNextIntJ(16)` offsets; salts: desert well 40002 (1.18+; 40013 in 1.16–1.17; 30010 ≤1.15), rarity 1/1000; geode 20002 (1.18+), 1/24; end gateway 40000 (1.18+), 1/700 — [cubiomes finders.c](https://github.com/Cubitect/cubiomes/blob/master/finders.c).
- Dungeons (≤1.17) "can yield between 36 and 55 bits of data with only one floor" and could recover the full lower 48 bits from one floor — [hube](https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c); in 1.18+ "Dungeon cracking, fungus cracking don't work anymore" — [scx](https://github.com/19MisterX98/SeedcrackerX).
- End spikes leak 16 bits via `nextLong() & 65535` — [hube](https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c).
- Slime chunk: `rnd = seed + (int)(cx*cx*0x4c1906) + (int)(cx*0x5ac0db) + (int)(cz*cz)*0x4307a7 + (int)(cz*0x5f24f); rnd ^= 0x3ad8025f; Random(rnd).nextInt(10)==0` — [cubiomes finders.h](https://github.com/Cubitect/cubiomes/blob/master/finders.h). Spigot's default slime salt is 987234911 (spigot-only knob) — [spigot](https://www.spigotmc.org/wiki/spigot-configuration/).
- Buried treasure (48-bit, legacy_type_2) and mineshafts (48-bit, legacy_type_3) formulas in §1 — [pumpkin](https://github.com/Pumpkin-MC/Pumpkin/blob/master/crates/pumpkin-world/src/generation/structure/placement.rs), [cubiomes](https://github.com/Cubitect/cubiomes/blob/master/finders.c).

### Inferences
- **End pillars (best single BlueMap target in the End)** (memory, verify): `SpikeFeature.getSpikesForLevel` uses `RandomSource.create(levelSeed)` (legacy LCG, so 48-bit) → `p = nextLong() & 0xFFFF`, then shuffles indices 0..9 with `Random(p)`; pillar i on the circle (radius 42, angle 2π·i/10 offset) gets height 76 + 3·idx and radius 2 + idx/3, caged if idx is 1 or 2. Reading the 10 pillar heights from BlueMap (obsidian top Y) gives the permutation → brute-force 65536 p values → 16 bits, then 2^32 × structure filter. Heights survive dragon fights (players rarely mine pillars down).
- Trees, desert wells, fossils, geodes (surface-exposed amethyst), end gateways: visible, but 64-bit dependent → use only as final verification.
- Slime chunks, buried treasure, mineshafts: 48-bit but invisible on BlueMap (except badlands mineshaft wood sometimes exposed on mesas — weak).
- Structure internals via `largeFeatureSeed(seed, cx, cz)` (rotation, template choice, village layout): 48-bit, visible → extra filter bits for free once the start chunk is known.

### Gaps
- Not verified this session: that 26.x still uses `RandomSource.create(seed)` + `&0xFFFF` for spikes, and exact spike geometry constants.
- Whether 1.18+ End terrain (end_islands density) is 48-bit-only was not checked; if so, the End surface/heightmap itself would be a 48-bit verifier.

---

## 6. Pitfalls

### Takeaway
The cracker must tolerate wrong/missing observations: mixed-version chunks, player edits, non-vanilla salts/datapacks, and "generate structures" off. Build it as "find the seed consistent with ≥N of M observations" and let the user/version choose parameter sets.

### Cited Findings
- Placement parameters changed across versions (villages spacing 32→34 in 1.18; ocean ruins/shipwrecks 16/8 until 1.15; nether portals separate 25/15 in 1.16–1.17; fortress algorithm changes at 1.16 and 1.18) — [cubiomes finders.c](https://github.com/Cubitect/cubiomes/blob/master/finders.c). Chunks generated before an upgrade keep their old structures, so an upgraded world can mix layouts.
- In 1.18.2, outpost/treasure frequency lived in code rather than structure_set JSON (field absent) — [mcmeta 1.18.2-data](https://github.com/misode/mcmeta/tree/1.18.2-data/data/minecraft/worldgen/structure_set).
- Spigot/Paper servers expose per-world salts (`seed-village` default 10387312, `seed-feature` 14357617, `seed-monument` 10387313, `seed-slime` 987234911, …) and recommend randomising them to prevent seed cracking — [spigot](https://www.spigotmc.org/wiki/spigot-configuration/). Anti-seed-cracker plugins also modify end spikes — [AntiSeedCracker DeepWiki](https://deepwiki.com/akshualy/AntiSeedCracker/3.2.1-end-spike-modification).
- SeedCrackerX warns decorator data in 1.18+ "aren't updated and can give wrong data", and there is "no way to find nether seed" in newer versions — [scx](https://github.com/19MisterX98/SeedcrackerX).
- 26.3 added a new structure set (abandoned_camp) — [mcmeta 26.3-data](https://github.com/misode/mcmeta/tree/26.3-data/data/minecraft/worldgen/structure_set). Its presence/absence in a map is also a version fingerprint.

### Inferences
- Robustness: rank structure-seed candidates by #observations matched rather than requiring all; one misidentified village/player-built "temple" otherwise kills the true seed. Player builds that look like structures (sandstone pyramids, portal ruins rebuilt) are the most common false positives.
- Pre-1.18 chunks: test each observation under both old and new parameter sets; world-border/explored-area shape and 1.18 terrain blending seams in the heightmap hint where old chunks are.
- Salts customised or datapack worldgen (Terralith, Tectonic, structure mods changing spacing): if no vanilla-salt solution exists with ≥5 good observations, try solving per-type salts: with the seed unknown this is harder, but if two types share a solved seed, a third type's salt is a 32-bit search.
- "Generate structures" off → no structure data; fall back to end pillars (still generated? spikes are part of the dragon fight feature — verify) and a 2^48 search over a biome/terrain verifier, which is expensive (Xoroshiro/multinoise per candidate) — essentially infeasible without some 48-bit leak.
- Superflat or single-biome worlds generate structures with the same placement but different biome validity.
- Nether/End-only maps: nether complexes + nether ruined portals give ~9 bits each (48-bit); End gives end cities (~5.8 bits, triangular r=9 → need ~9–10) plus pillars (16 bits) — End maps are very crackable.

### Gaps
- No source consulted on Paper's `feature-seeds` config or on how common custom salts are on public servers.
- Behaviour of 1.18 "blending" for structures straddling old/new chunk borders not researched.
