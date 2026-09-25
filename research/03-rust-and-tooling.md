# 03 — Rust crates and test tooling (researched 2026-09-25)

Legend: **[V]** verified from a primary source this session; **[?]** unverified / from memory / conflicting sources — check before relying on it.
Crate versions/dates come from the crates.io API (`https://crates.io/api/v1/crates?...`); docs.rs summaries gave conflicting dates, crates.io wins.

## 0. Context that changes everything: MC versioning + world layout (2026)

- MC moved to year-based versions: 1.21.11 (2025-12-09) was the last `1.x`, last on Java 21 [V] https://minecraft.wiki/w/Java_Edition_1.21.11
- **26.1** (2026-03-24, DataVersion 4786) requires **Java 25** and **moved dimension folders** [V] https://minecraft.wiki/w/Java_Edition_26.1
  - Overworld: `dimensions/minecraft/overworld/{region,entities,poi}` (was world root)
  - Nether: `dimensions/minecraft/the_nether/` (was `DIM-1`), End: `dimensions/minecraft/the_end/` (was `DIM1`)
  - `playerdata/` → `players/data/`; `data/` namespaced (`data/minecraft/...`); level.dat slimmed (difficulty, WorldGenSettings etc. moved to `data/minecraft/*.dat`)
  - `.mca` Anvil format itself unchanged.
- **26.3** "Wilderness Bound" released 2026-09-15, DataVersion 5023, Java 25 [V] https://minecraft.wiki/w/Java_Edition_26.3
- **26.4 snapshots**: biomes stored **per-block** instead of 4x4x4 cells [V per wiki chunk-format history, snapshot-only] https://minecraft.wiki/w/Chunk_format — a moving target; don't pin to 26.4.
- Chunk NBT (1.18+ shape, still current): root `DataVersion,xPos,yPos,zPos,Status/status,sections[],block_entities,Heightmaps,block_ticks,fluid_ticks,structures,PostProcessing`; section `Y`, `block_states{palette,data}`, `biomes{palette,data}`, `BlockLight`, `SkyLight`. Index bits = max(4, ceil(log2(palette_len))); **entries never span longs** (since 1.16). Single-entry palette → no `data`. Entities live in separate `entities/` region files (since 1.17). [V] https://minecraft.wiki/w/Chunk_format
  - Wiki says `Status`→`status` rename in 26.4; [?] exact casing per version — read both.

**Code consequence:** abstract "world root + dimension → region dir" (old layout vs 26.1+ layout) in one function.

## 1. World IO crates

| crate | ver (crates.io) | updated | role | notes |
|---|---|---|---|---|
| fastnbt | 2.6.3 | 2026-08-08 | NBT serde (read+write) | MIT/Apache. `to_bytes`/`from_bytes`, `LongArray`/`IntArray`/`ByteArray` wrappers, `borrow::LongArray` zero-copy, `Value`. Most-used, well maintained. https://docs.rs/fastnbt |
| fastanvil | 0.32.0 | 2025-08-17 | region + chunk read | `Region::{create,from_stream,read_chunk,write_chunk,write_compressed_chunk,remove_chunk,iter}` [V source]. `write_chunk` = zlib; reads gzip/zlib/none/LZ4. `CurrentJavaChunk`/`JavaChunk` are **Deserialize-only** — docs: "not suitable for serializing back into a region". https://github.com/owengage/fastnbt |
| mca | 2.1.2 | 2026-03-14 | region read+write, raw bytes | `RegionWriter`; GZip/Zlib/None/LZ4 + custom compression; bring-your-own NBT; claims fastest reader. MIT. https://docs.rs/mca |
| simdnbt | 0.10.0 | 2026-03-28 | NBT (azalea) | borrow (zero-copy) + owned w/ `write`; serde Serialize "experimental"; skips string validation. Fastest reader. https://docs.rs/simdnbt |
| quartz_nbt | 0.2.9 | 2024-03-19 | NBT | stale-ish; no reason to prefer. |
| hematite-nbt | 0.5.2 | 2021-05-15 | NBT | unmaintained. |
| valence_anvil / valence_nbt | 0.1.0 / 0.8.0 | 2023-10 | | Valence project stalled on crates.io since 2023. Avoid. |
| nucleation | 0.10.24 | 2026-09-07 | schematic + world toolkit | MIT, very active (1.1k commits). R/W `.schem` v2/v3, `.litematic`, `.mcstructure`, `.snbt`; world import/export (`from_world_directory`, `save_world`, `WorldSink`). [?] 26.1 layout support, lighting/heightmap generation, doc coverage ~45%. https://github.com/Schem-at/Nucleation |
| rustmatica | 0.5.2 | 2025-02-04 | .litematic R/W | small, focused. |
| mc_schem | 1.1.2 | 2024-04-25 | .schem/.litematic/.nbt R/W | older. |

**Recommendation**
- **Reading (scorer): `fastanvil::Region` + our own thin `fastnbt` serde struct** (only `sections[].Y, block_states{palette,data}`, `DataVersion`, `yPos`), using `fastnbt::borrow`/`LongArray` for speed. Don't depend on `CurrentJavaChunk` semantics — our struct is ~40 lines and survives format drift. If profiling shows NBT parse dominating, swap in `simdnbt::borrow`.
- **Writing (output): `fastnbt` Serialize structs + `fastanvil::Region::create/write_chunk`.** Proven path: **Arnis** (Rust OSM→MC world generator, Apache-2.0) does exactly this with fastnbt 2.6 + fastanvil 0.32 — `src/world_editor/java.rs`: own `Chunk/Section/PaletteItem` structs, `Status: "minecraft:full"`, `DataVersion 3955` (1.21.1), bits=max(4,…), `64/bits` entries per long, optional baked `SkyLight/BlockLight`, 4 heightmaps. https://github.com/louis-e/arnis/blob/main/src/world_editor/java.rs — read it before writing ours.
  - `mca` is an equal alternative for the region layer (LZ4 write, custom compression); fastanvil is fine and pairs with fastnbt.
  - Writing `Status=minecraft:full` without light: set `isLightOn=0` (Arnis `is_light_on`) so the game relights; [?] behavior on 26.x — test by opening the output in a 26.3 server.
  - Also need a `level.dat` (gzip NBT). Simplest: copy one from a vanilla-generated empty world of the pinned version (26.1+ splits settings into `data/minecraft/*.dat` — copy those too) rather than synthesizing.
- **Schematic output**: Sponge `.schem` v3 is trivial to hand-write with fastnbt+flate2 (spec below) — do that rather than pull nucleation. Use nucleation only if we want `.litematic` too / later.
  - Sponge v3 [V] https://github.com/SpongePowered/Schematic-Specification/blob/master/versions/schematic-3.md : gzip NBT, root compound wraps `Schematic{Version:3, DataVersion, Width/Height/Length (u16 → max 65535 per axis), Offset:int[3], Blocks{Palette{"state":idx}, Data: varint byte array, BlockEntities[]}, Biomes{…}, Entities[]}`; index `x + z*W + y*W*L`.
  - Litematica: `Regions{name{Position,Size (may be negative),BlockStatePalette[],BlockStates:long[]}}`; **[?] BlockStates entries DO span longs** (bits=max(2,ceil(log2 n))) — opposite of chunk packing. Verify against rustmatica/Nucleation source before hand-rolling.

## 2. Supporting crates (latest stable per crates.io)

| crate | ver | fit / gotchas |
|---|---|---|
| reqwest | 0.13.5 | Fit. Gotcha: BlueMap stores tiles gzip-compressed (`*.prbm.gz`, `*.json.gz` on disk) and its webserver serves them with `Content-Encoding: gzip` [?]. With reqwest's `gzip` feature on, it auto-decompresses; off, you get raw gzip. Pick one explicitly and test both a `.png` and a `.prbm`. `blocking` feature is simplest (pair with rayon); async needs tokio 1.53. Add retry + concurrency cap. 0.13 changed default TLS backend [?] — irrelevant for http://localhost. |
| flate2 | 1.1.10 | Fit. Use `MultiGzDecoder` (not `GzDecoder`) for safety; `ZlibEncoder` for chunk payloads if not using fastanvil's `write_chunk`. BlueMap storage can also be `zstd`/`deflate`/`none` (file.conf `compression`, default gzip) [V] — force `gzip` or `none` in our test configs; if zstd ever needed, add `zstd` crate. |
| rayon | 1.12.0 | Fit for per-tile decode and per-region scoring/writing. Don't call blocking reqwest inside rayon at huge parallelism — use a bounded pool or async fetch → rayon decode. |
| image | 0.25.10 | Fit for lowres PNG tiles. Disable default features, enable only `png` to cut compile time. Lowres tiles encode height/light in pixel data — decode as raw RGBA8, never let anything colour-manage/resize. |
| clap | 4.6.7 | Fit (derive). |
| serde_json | 1.0.151 | Fit for `settings.json`, map `settings.json`, markers. |
| fastnbt/fastanvil | see §1 | |

Arnis' Cargo.toml uses exactly this stack (reqwest 0.13, rayon 1.10, flate2 1.1, image 0.25, clap 4.6, fastnbt 2.6, fastanvil 0.32) — good sign the versions co-exist.

## 3. BlueMap CLI

- Latest: **v5.27** (hotfix: camera jump crossing 10000-block boundaries), asset `bluemap-5.27-cli.jar`, supports **MC 1.13.2 – 26.3**, release notes list **Java 25** [V] https://github.com/BlueMap-Minecraft/BlueMap/releases/latest
  - [?] GitHub page reported date as "Sep 16, 2024" — can't be right for a 26.3-supporting release; almost certainly 2026-09-16.
  - [?] Install docs still say CLI needs "Java 21 or newer" (plugin Java 25+). Use Java 25 (Temurin 25 zip, portable, no global install) to be safe. https://bluemap.bluecolored.de/wiki/getting-started/Installation.html
- Workflow:
  1. `java -jar bluemap-5.27-cli.jar -c <cfgdir>` once → generates `core.conf`, `webapp.conf`, `webserver.conf`, `maps/*.conf`, `storages/*.conf`.
  2. `core.conf`: `accept-download: true` (downloads the vanilla client jar for resources — needs network on first run; cache it in CI).
  3. `maps/<id>.conf`: `world: "<path>"`, `dimension: "minecraft:overworld"`; `render-mask` (replaces old min/max bounds), `remove-caves-below-y`, `ignore-missing-light-data`, `enable-hires`, `render-edges` [V] https://bluemap.bluecolored.de/wiki/configs/Maps.html
  4. `storages/file.conf`: `root` default `bluemap/web/maps`, `compression: gzip|zstd|deflate|none` (default gzip) [V source]
  5. Render: `java -jar bluemap-cli.jar -c cfg -v 26.3 -r` — **exits when done** unless `-u`/`-w` [V source].
  6. Serve: `java -jar bluemap-cli.jar -c cfg -w` (`-g` generates webapp files; port default **8100**, `ip` default all interfaces, set `127.0.0.1`) [V] https://bluemap.bluecolored.de/wiki/configs/Webserver.html
- Full flag list [V] `implementations/cli/.../BlueMapCLI.java`: `-c/--config`, `-n/--mods`, `-v/--mc-version`, `-l/--log-file`, `-a`, `-w/--webserver`, `-b/--verbose` (log requests — handy to see what the reverser fetches), `-g/--generate-webapp`, `-s/--generate-websettings`, `-r/--render`, `-e/--fix-edges`, `-f/--force-render`, `-m/--maps a,b`, `--markers`, `-u/--watch`, `-V/--version`.
- Headless/CI: CLI is already headless. Pattern: `-r` (foreground, background it — slow) → then `-w` as a background process → poll `http://127.0.0.1:8100/settings.json` until 200 → run reverser → kill. For determinism use `-f` (force full re-render) per test world, and a fresh config/storage dir per world.
- Docker image also exists (`-c /path` for config) [V install page].
- Note: BlueMap needs **light data** by default; worlds must be fully generated/lit (`ignore-missing-light-data: true` otherwise). Our *output* worlds re-rendered by BlueMap (round-trip check) need light or that flag.

## 4. Producing test worlds

**Server jar**: official manifest `https://piston-meta.mojang.com/mc/game/version_manifest_v2.json` → version json → `downloads.server.url` [?] (standard, unchanged for years; verify). Headless run: write `eula.txt` with `eula=true`, then `java -Xmx4G -jar server.jar --nogui`. `--initSettings` writes server.properties and exits [?]. Feed console commands via stdin (`stop` to save+quit) — drive from a small script.

**server.properties** (for test worlds): `level-seed=<fixed>`, `level-type=...`, `generate-structures`, `spawn-protection=0`, `sync-chunk-writes=true`, `online-mode=false` (offline local), `max-world-size` [V] https://minecraft.wiki/w/Server.properties

**Pregeneration**
- **Chunky** (Fabric/Paper/etc.): v1.5.3 supports 26.1–26.3 (Fabric), published 2026-05-04 [V Modrinth API]; 1.4.55 for 1.21.11. Needs a Fabric server (fabric-server-launcher jar + Fabric API) or Paper. Console: `chunky world minecraft:overworld`, `chunky center 0 0`, `chunky radius 256`, `chunky shape square`, `chunky start`, `chunky quiet 5` [V] https://github.com/pop4959/Chunky/wiki/Commands. Wait for completion message, then `stop`.
- Vanilla-only alternative for small areas: `forceload add x1 z1 x2 z2` (≤256 chunks per command [?]) then `stop`. Keeps us on a pure vanilla jar.

**World types**
- **Superflat**: `level-type=minecraft:flat`, `generator-settings={JSON}` for custom layers [V]. Great first target (trivial heightmap, known blocks).
- **Normal**: fixed seed, radius ~256–512 blocks.
- **Debug (every block state)**: preset id `minecraft:debug_all_block_states`; server: `level-type=minecraft:debug_all_block_states` (wiki Talk page confirms it generates a debug world; main wiki page says `level-type=debug` — that's the pre-1.19 value) [? verify empirically on 26.3]. Layout: grid at **Y=70**, barrier floor Y=60; ~29.9k states → ~173×174 grid, blocks spaced every 2 blocks [?] https://minecraft.wiki/w/Debug_mode. No block updates/ticks, so state is exact. Perfect for "which state does this BlueMap model/color map to" lookup tables.
  - [?] Whether BlueMap renders a debug-preset world without issues (light data, 1-block islands) — test.
- **OSM cities via Arnis** (Apache-2.0 tool; data OSM ODbL, attribution required): generate realistic built-up worlds ourselves, no map-licensing issue. https://github.com/louis-e/arnis (writes 1.21.1-era DataVersion — may need to open in server to upgrade).

**Downloadable maps (harder tests)**
- Legally murky: Mojang EULA/usage guidelines govern Minecraft content; builds' copyright belongs to their creators, most download sites carry no license [?]. https://www.minecraft.net/en-us/eula , https://www.minecraft.net/en-us/usage-guidelines
- Known explicit license: **EarthMC** world download = CC BY-NC 4.0 [V search snippet] https://earthmc.net/download — fine for private testing with attribution; don't redistribute in repo.
- Rule: keep downloaded maps **out of git** (download script + checksum), use only for local scoring; prefer self-generated (seeded vanilla, flat, debug, Arnis) for anything committed/CI.

**Version pin recommendation: MC 26.3 (DataVersion 5023) + BlueMap 5.27 + Java 25 + Chunky 1.5.3.** Reasons: latest stable, all tools support it, new layout is the future. Keep the dimension-path abstraction so 1.21.11 worlds (old layout, Java 21) also work for downloaded maps. Avoid 26.4 snapshots (per-block biome change). Record DataVersion in every output.

## 5. Scoring

Existing tools: nothing does block-by-block similarity scoring.
- `region-diff` (Rust, MIT): chunk-level binary diffs for backups, dev'd on 1.21.4 — not a scorer. https://github.com/HairlessVillager/region-diff
- nucleation advertises "structural diffing" of schematics [?] — worth a look for ideas only.
- MCA Selector / Amulet: interactive, not scripted scoring [?].

**Build our own (`score` subcommand), design:**
1. Enumerate chunks present in either world (region header offsets; `Region::iter`). Parallelise per region file with rayon.
2. Decode each section to `[u32; 4096]` of **global interned state ids** (intern `name + sorted properties` string → u32 in a `DashMap`/per-thread map merged later). Unpack via the no-span rule. Missing section = all `minecraft:air`; treat `air/cave_air/void_air` as air.
3. Compare only inside a **scoring mask**: the BlueMap-rendered bounds (render-mask) and optionally only "observable" voxels — e.g. blocks adjacent to air/transparent in the original (what BlueMap can actually show). Report both.
4. Metrics (per region + total, printed as summary, JSON detail to file): exact-state accuracy; block-name accuracy (ignore properties); solid-vs-air IoU; surface accuracy (top non-air per column); confusion top-N (original→ours) for tuning lookup tables; biome accuracy (4×4×4 cells, or per-block on 26.4+).
5. Block entities/entities: out of scope initially; count presence only.
- Cost: 512×512 blocks × 384 high ≈ 100M voxels ≈ 400 MB as u32 — stream per chunk, never materialise the world. Expect seconds per region.
- Round-trip sanity check: render our output world with BlueMap and diff the lowres PNG tiles against the original's (image crate, per-pixel) — cheap visual-equivalence metric independent of block-state ambiguity.

## Open questions to settle empirically (first spike)
1. `level-type=minecraft:debug_all_block_states` works on 26.3 dedicated server?
2. BlueMap 5.27 CLI runs on Java 21 or strictly 25?
3. BlueMap webserver `Content-Encoding` behaviour for stored `.gz` tiles vs reqwest `gzip` feature.
4. Minimal chunk NBT a 26.3 server accepts without regenerating (Status/status casing, needed heightmaps, `isLightOn`).
5. Litematica long-spanning packing (only if we emit .litematic).
