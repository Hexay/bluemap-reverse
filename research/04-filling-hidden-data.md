# 04 — Filling hidden/missing data

Scope: what BlueMap tiles do NOT contain, and how to recover or plausibly synthesize it.
Confidence tags: **[verified]** = checked in source/docs this session; **[likely]** = strong
community knowledge, not re-verified; **[uncertain]** = needs an experiment.

## 0. What the tiles actually give us (inputs to everything below)

- Hires PRBM per-face attributes: `position`, `normal`, `color` (tint), `uv`, `ao` (per vertex),
  `blocklight`, `sunlight` (one value per face, written 3x) **[verified]**
  (`core/.../map/hires/PRBMWriter.java`, https://github.com/BlueMap-Minecraft/BlueMap).
  Face light = light level of the block the face points *into* (air/transparent neighbor) **[likely]**.
- Cave removal: faces with sunlight 0 below `remove-caves-below-y` (overworld default 55) are
  dropped; optional `cave-detection-ocean-floor` (template: -5), `cave-detection-uses-block-light`
  (default false) **[verified]** (`config/maps/map.conf`;
  https://bluemap.bluecolored.de/community/CaveRendering.html).
- Nether default: `remove-caves-below-y -10000` + render-mask subtracting Y 90–127 (ceiling removed).
  End default: `-10000`, no mask **[verified]** (`common/.../config/BlueMapConfigManager.java`).
  => End and Nether (below Y90) maps are *full 3D visible-surface* meshes including caves.

### Publicly exposed metadata (passive reads only)
- `<webroot>/settings.json`: `version` = **BlueMap** version (not MC), maps list, UI options **[verified]**
  (`common/.../WebFilesManager.java`).
- `maps/<id>/settings.json`: name, sorting, hires/lowres grid, startPos, sky/void color,
  ambient/sky light, view toggles **[verified]** (`MapSettingsSerializer.java`). **No seed, no MC version.**
  BlueMap reads `world_gen_settings.dat` only for dimension type, never publishes the seed **[verified]**.
- MC version bracketing: `maps/<id>/textures.json` lists every texture resource BlueMap loaded —
  presence of e.g. `pale_oak_*` (1.21.4+), `cherry_*` (1.20+), trial-chamber blocks (1.21+),
  `copper_golem_statue` (25w/1.21.9+) bounds the version; non-`minecraft:` namespaces reveal mods
  **[likely — confirm textures.json contents in 01/02 research]**.
- Worldgen fingerprints: biomes/blocks that only exist in some versions; Terralith/Tectonic-style
  terrain = datapack → vanilla seed regen will not match.
- `markers.json` (POIs, sometimes "spawn"), live `players.json`: fine to read, don't poll aggressively.
- Out-of-band: server websites/Discord often post seed + version; `/seed` is op-only by default.

## 1. Seed recovery

### Background (Java edition)
- Structure placement (random-spread structures) uses only the **lower 48 bits** ("structure
  seed") via `setLargeFeatureWithSalt` + java LCG. Feature/decoration seeds are also LCG-derived
  (48 bits). Biomes/noise terrain (1.18+) use Xoroshiro over the **full 64 bits** **[likely]**
  (https://gist.github.com/hube12/368e7331e497b17e092e8ca4ba206b3c).
- Upper 16 bits: brute-force 65,536 candidates against biome/terrain samples — trivial CPU work.
- Random (not typed) seeds come from `new Random().nextLong()`, so the 64-bit seed is a function of
  a 48-bit state: given a structure seed, check if it is "nextLong-reachable" (SimpleReversal) —
  often gives the world seed with **zero** biome work **[likely]** (same gist).
- 48-bit GPU brute force: ~40e9 seeds/s on a GTX 1070-class GPU => 2^48 in ~2 h per filter pass
  (https://www.minecraftforum.net/forums/minecraft-java-edition/seeds/3053023-seed-crack). Lattice
  reversal methods reduce most structure/decorator cracks to seconds–minutes.

### Tools / libraries
| Tool | Use for us | Notes |
|---|---|---|
| cubiomes (C) https://github.com/Cubitect/cubiomes | biomes, structure attempt pos + viability, strongholds, 1.18+ | no block-level terrain; 1.18 temples/mansions can false-positive due to surface-height check |
| Rust: `cubiomes` / `cubiomes-sys` crates https://docs.rs/cubiomes | call from our Rust tool | bindings maturity **[uncertain]** |
| SeedcrackerX https://github.com/19MisterX98/SeedcrackerX | reference for which signals/bits; 1.16.5–26.2 | in-game mod; 1.18+ uses structures (igloo, pyramid, jungle temple, swamp hut, shipwreck, outpost, monument), e.g. "3 shipwrecks + 1 pyramid + 1 igloo"; End: 5+ end cities |
| KaptainWutax SeedUtils/FeatureUtils/mc_feature_java, SeedCracker https://github.com/KaptainWutax/SeedCracker | reference impls (Java, largely unmaintained) | pre-1.18 focus |
| Nether_Bedrock_Cracker (Rust) https://github.com/19MisterX98/Nether_Bedrock_Cracker | 1.18+ nether bedrock → structure seed (nether uses legacy LCG) | needs floor+roof samples |
| BedrockSeedCracker https://github.com/MiranCZ/BedrockSeedCracker | full 64-bit via nether 48 + overworld bedrock 16 | needs ~512 OW + 128+128 nether blocks |
| fnseedc resource list https://github.com/SeedFinding/fnseedc | index of techniques | |

### Which signals survive a BlueMap top-down scrape
Ranked by usefulness:
1. **Surface structures (overworld)** — villages (jigsaw; start chunk ≈ center/meeting-point piece),
   desert pyramid, jungle temple, swamp hut, igloo top, pillager outpost, ocean monument (prismarine
   visible through water), shipwrecks in shallow water, ruined portals, woodland mansion.
   Each gives the region-chunk offset (~8–10 bits each for spacing 24–34). 5–6 good structures ⇒
   48-bit structure seed. Watch for player-built lookalikes and destroyed structures.
2. **End map (fully rendered)** — the 10 obsidian spikes' heights/radii encode
   `Random(worldSeed).nextLong() & 0xFFFF` (16 bits cheaply); end city positions (≥5 per
   SeedcrackerX) give the structure seed. **[likely]** Great if the End map is published.
3. **Upper 16 bits via biomes** — our biome inference (§3) is coarse (tints collapse many biomes),
   but ocean-vs-land, desert/badlands (sand/terracotta surface), snowy (snow layer), jungle,
   mushroom fields, cherry, swamp tint are distinct. Tens of such points pin 16 bits easily
   with cubiomes `getBiomeAt`. Alternative: compare regenerated heightmaps (§2) for the ≤65536
   candidates — expensive per candidate but only if biomes ambiguous.
4. **Trees/decorators** — tree trunk positions/heights in one chunk leak the decoration seed
   (pack.png approach, Minecraft@Home: ~3,700 BOINC volunteers × 3 days in 2020 for an older
   version, https://minecraftathome.com/projects/packpng.html). Modern lattice tree crackers
   are far cheaper, but version-specific feature order/salts make 1.18+ implementations
   scarce **[uncertain]**; use only if no structures.
5. **Bedrock** — overworld bedrock (Y -64..-60) is invisible under default cave removal; nether
   floor is mostly under netherrack/lava and roof is masked out by default. Only viable on maps
   with non-default config **[likely]**.
6. **Dungeons/ores/geodes/mineshafts** — below Y55 → culled unless skylit (ravines, open caves);
   dungeon floors (36–55 bits from one dungeon) are only usable if exposed. Opportunistic.

Cost for 1.18+ with structures: lattice/GPU 48-bit step minutes–~2 GPU-hours worst case, then
65,536-candidate biome check in seconds on CPU. Terrain-only (no structures, no End) cracking
of 1.18+ is an open-ended research problem — treat as infeasible for this project **[uncertain]**.

Worked precedent: Minecraft@Home recovered title-screen/pack.png seeds purely from images
(https://minecraftathome.com/projects/1-18-panorama.html) — our data (exact block positions)
is strictly richer than screenshots.

## 2. Regenerate + merge

Pipeline:
1. Determine MC version bracket (§0) and exact version by trial: generate a probe area with each
   candidate version, compare heightmap/biomes. Noise worldgen changed 1.18→1.18.2→1.19 (deep dark,
   mangroves)→1.20 (cherry)→1.21 (trial chambers)→1.21.4 (pale garden); structure sets also changed.
2. Headless server (vanilla jar, `level-seed`, same datapacks), pre-generate the scraped bounding
   box with Chunky; or use the vanilla `--forceUpgrade`-free fresh world.
3. **Render the regenerated world with the same BlueMap version + config + resource packs** (BlueMap
   CLI). Now both sides are identical PRBM tiles → diff face-sets per tile. This is the key trick:
   avoids re-implementing culling/cave-removal logic in the comparison.
4. Classify per block column / per chunk: `unchanged` (faces match) → take generated blocks for
   the whole column including underground; `modified` → reconstructed surface from scrape +
   generated underground below the deepest observed modification (minus a safety margin);
   `unknown` (not rendered, e.g. `min-inhabited-time`) → generated.
5. Merge at chunk level with our own Anvil writer, or MCA Selector
   (https://github.com/Querz/mcaselector) for chunk import/filters.

Diff heuristics / modification signals: blocks that are never natural in that biome (planks,
glass, wool, rails, torches → nonzero blocklight in open terrain), missing trees, flattened
columns, water/lava flow changes, farmland/paths, crops. Use chunk-level morphological dilation so
partially edited chunks keep underground from gen but surface from scrape.

Pitfalls:
- **Chunk-order nondeterminism**: features crossing chunk borders (trees, some jungle/large
  features) can differ between generations of the same seed — MC-55596
  (https://bugs-legacy.mojang.com/browse/MC-55596, affects ≥1.7.9–1.20.1). Expect small natural
  diffs; tolerate them (don't classify as player edits unless clustered/non-natural blocks).
- **Upgraded pre-1.18 chunks**: old chunks keep old terrain; below Y0 new terrain + blending
  (`blending_data`), bedrock Y0–4 replaced by deepslate
  (https://minecraft.fandom.com/wiki/Java_Edition_1.18). Regenerating with 1.18+ will NOT
  reproduce those chunks' surface; need original old version for them, and blending borders are
  order-dependent. Detect by: world-border ring of mismatch + old-style terrain (1.17 mountains).
- **Datapacks/mods** (Terralith, Tectonic, custom dims): vanilla regen fails; diff rate will be
  ~100% everywhere → fall back to §3.
- **Spigot/Paper structure salts**: `spigot.yml` `seed-village`, `seed-feature`, etc. and Paper
  `feature-seeds` can change structure/feature placement (https://docs.papermc.io/paper/reference/spigot-configuration/).
  Reportedly custom spigot seeds ineffective since 1.18.2 **[uncertain]**. If structures don't
  crack cleanly but biomes do, suspect this.
- Server-side world border/pregen with different version than later chunks → mixed-version world.
- Structure contents (chests loot) are generated from loot seeds — regenerated chests contain
  *original* loot only for unlooted chests; can't know which were looted.

## 3. Non-seed heuristics (custom/modded or seed not found)

- **Fill below surface**: overworld 1.18+: stone above Y0, deepslate at/below Y0 (transition
  noise Y0..8), bedrock -64..-60; granite/diorite/andesite/tuff blobs and ores can be sprinkled
  statistically, or simply omitted (honest "unknown" fill).
- **Sunlight on faces**: a face with sunlight < 15 facing up means something opaque/overhang
  above; sunlight = 15 in a face below ground ⇒ open to sky. In rendered caves (Nether/End or
  `-10000` maps) sunlight 0 + blocklight>0 ⇒ nearby light source.
- **Blocklight inversion**: blocklight decreases by 1 per block (Manhattan through non-opaque
  space). Given many face samples, solve for sources: local maxima of 14/15 ⇒ torch(14),
  glowstone/lantern/sea-lantern/shroomlight/jack-o-lantern(15), lava(15), soul torch(10),
  redstone torch(7), end rod(14), glow lichen(7), amethyst cluster(5). Level value narrows block
  type; position is the argmax of consistent BFS fields. Hidden sources (inside walls? no — light
  must propagate through air) imply **hidden air cavities**: a blocklight gradient on a surface
  face whose source isn't visible ⇒ there is an air path + source behind. Works best for
  interiors of builds that are culled.
- **AO**: per-vertex AO tells occupancy of the 3 neighbor cells at each face corner (vanilla
  smooth-lighting rule) ⇒ reveals solidity of blocks adjacent to the visible face, including
  diagonal neighbors that have no visible face. Use to confirm/deny hidden blocks one layer
  deep **[likely — confirm BlueMap AO formula in source]**.
- **Biome inference from tints**: `color` attribute = biome tint (grass/foliage/water) multiplied
  in; BlueMap blends biomes over a radius and applies swamp noise (fixed 5.25). Invert: tint →
  (temperature, downfall) via `grass.png`/`foliage.png` colormap lookups; fixed-tint biomes
  (swamp, badlands, dark forest noise, cherry, mangrove) are distinguishable; water tint separates
  warm/lukewarm/cold/frozen oceans, swamp. Resolution ~4x4 biome cells blurred by blend radius;
  many biomes map to the same tint → keep a candidate set + surface-block priors (sand, snow,
  mycelium, podzol, terracotta).
- **Ocean floor / water**: water surfaces are rendered; with `cave-detection-ocean-floor` the
  seabed is present. Otherwise fill water down to nearest known floor by interpolation.
- **Lowres tiles**: color + height per pixel ⇒ fill unrendered hires gaps with a heightfield.

## 4. Block entities / entities visible in BlueMap

Resource-extension blockstates shipped by BlueMap **[verified]** (`core/src/main/resourceExtensions/
assets/minecraft/blockstates/`): all colored `*_banner`/`*_wall_banner`, `*_shulker_box`,
`chest`, `trapped_chest`, `ender_chest`, `sign` (all wood variants), mob/player heads & skulls,
`decorated_pot`, `conduit`, `copper_golem_statue` (all oxidation), `bell`, `cake`, water/lava,
bubble_column. Added in 5.2 (https://github.com/BlueMap-Minecraft/BlueMap/releases/tag/v5.2).

| Block entity | Rendered? | Recoverable |
|---|---|---|
| Chest / trapped / ender | static model (single/double, left/right) | type, facing, single vs double, position; **contents never** |
| Shulker box | yes, per color | color + facing; contents no |
| Sign / hanging sign | yes, **blank** — text not rendered (open issue #146, https://github.com/BlueMap-Minecraft/BlueMap/issues/146) | wood type, rotation/wall/hanging; text no |
| Banner | base color only; patterns parsed but `TODO` not rendered **[verified, BannerBlockEntity.java]** | base color, rotation; patterns no |
| Heads/skulls | yes | type & rotation; player-head skin — profile parsed, whether skin texture rendered **[uncertain]** |
| Bed | not in resourceExtensions list; 5.18/5.21 notes mention 26.x bed changes (vanilla may now ship bed models) **[uncertain]** | color, facing, head/foot if rendered |
| Decorated pot | yes | sherd patterns: probably default model only **[uncertain]** |
| Lectern/furnace/hopper/etc. | normal block models | facing/state; inventories no |
| Spawner | block model | mob type no |

Entities: core has entity render pass + `entitystates` (`missing.json` only) — vanilla mobs/
item frames/paintings/armor stands are rendered only with the BlueMapEntities addon
(https://github.com/BlueMap-Minecraft/BlueMapEntities) **[likely]**. If present, item frames and
paintings (variant from texture) are recoverable as entities.

Unrecoverable: container inventories, sign text, command blocks, spawner data, lectern books,
jukebox discs, beehive bees, entity NBT (villager trades, pet owners), player data, scoreboards.

## 5. Ranked approaches (accuracy gain ÷ effort)

| # | Approach | Gain | Effort | Notes |
|---|---|---|---|---|
| 1 | Out-of-band seed/version (site, Discord, textures.json bracket) | very high | trivial | always try first |
| 2 | Structure-seed crack from visible surface structures + cubiomes upper-16 biome check | very high (unlocks §2) | medium (port or shell out to cubiomes; reuse SeedcrackerX logic) | vanilla worlds only |
| 3 | Regen w/ headless server + BlueMap CLI re-render + tile face-diff merge | very high (whole underground, ores, caves, unmodified terrain exact) | medium-high | chunk-order noise, upgraded chunks, datapacks |
| 4 | Naive fill (stone/deepslate/bedrock by Y, water down to floor) | medium (solid, plausible world) | low | baseline for everything |
| 5 | Block entity static models → correct block ids + states | medium (chests/signs/banners exist & face right) | low | parser just maps models |
| 6 | Biome inference from tints + surface blocks | medium (correct grass/water colors, mob spawns) | medium | coarse; many-to-one |
| 7 | End-map pillar/end-city crack | high if End map exists | low-medium | independent seed confirmation |
| 8 | Blocklight inversion → hidden light sources & cavities | low-medium (interiors of builds) | high | nice-to-have |
| 9 | AO-based one-layer neighbor solidity | low | medium | verify formula first |
| 10 | Tree/decorator/dungeon lattice cracking | high if 2 fails | high | only when no structures |
| 11 | Bedrock cracking | high | low (tools exist) | only if bedrock is rendered (non-default configs) |

Open questions to settle by experiment: exact BlueMap AO/light-per-face semantics; textures.json
contents; bed/player-head rendering in current BlueMap; Rust cubiomes binding quality.
