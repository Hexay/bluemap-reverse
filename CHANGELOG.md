# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- The hidden Nether roof comes down next to rock reaching the masked band instead of starting at one fixed height.
- `bmr --version`.
- Seed detection (`bmr structures`, `pull --crack-seed`) matches structure templates for villages, igloos, swamp
  huts and (new) woodland mansions: a candidate start is kept only when the town centre, igloo dome, hut or
  mansion entrance is actually there block for block, so house and player bells, snowy houses and spruce builds
  no longer give wrong observations. Templates come from the server jar (`tools/gen_structure_templates.py`);
  the `--truth` table gains an `unplaced` column and counts only detections that became observations.
- `reverse` and `pull` infer the map's render settings from its tiles: caves drawn below BlueMap's y 55
  cut-off, a Nether roof mask with faces inside it (shrunk or dropped), geometry beyond the build height. They
  print one line with the settings used and warn where they differ from BlueMap's defaults; `--cave-y`,
  `--mask-y`, `--min-y` and `--max-y` still override.
- Global `-q`/`--quiet`: progress lines off; results, warnings and errors still print.
- `bmr seed` exits with status 3 when it finishes without a world seed (exit codes in `bmr --help`).
- `--data-dir` on `fetch`, `pack list` and `pack fetch`; `BMR_HOME` sets the data dir for all of them.
- LZ4-compressed region files (Minecraft 1.20.5+ `region-file-compression=lz4`) are read.
- Sites whose host sends BlueMap's stored tiles as-is (SQL storage via `sql.php`, static hosts) are mirrored with
  any storage compression: deflate, zstd and lz4 alongside gzip (was gzip only; zstd failed).
- Render round-trip: `tools/roundtrip.py` relights and re-renders a fixture's reconstruction with BlueMap, and
  the hidden `bmr diff-render` diffs the two renders face by face (geometry, texture, tint, AO, light).

- Ambient occlusion on the map's faces is read as evidence: it counts the solid blocks around each corner,
  which pins down hidden cells next to visible ones (rock beside dark caves, air pockets).
- `--window-jobs N` (`reverse`, `pull`): region windows are reconstructed N at a time (default 4). With the
  faster fill and allocator below, reverse of the structures test map went from 41 s to about 9–18 s
  (identical output).
- `--seed <n>` (`reverse`, `pull`): with the world seed, unseen bedrock floor/roof and the deepslate transition
  are computed exactly as vanilla's surface rules do (native, no Java; exact on every gradient cell of the
  test worlds). All blocks: vanilla 75.3% → 76.9%, Nether 82.9% → 85.7%.
- `pull --crack-seed`: detects structures in the reconstructed overworld, cracks the world seed and writes
  the report (observations, candidates) to `OUT.seed.json`; replaces the manual `structures` → `seed` steps.
- `--area x0,z0,x1,z1` on `pull` and `fetch`: mirror only the tiles around a block box (plus the reconstruction
  halo) and reconstruct only the columns inside it, so one area of a big public map can be pulled. A later run
  with another area, or none, resumes the same mirror.

### Changed (breaking)

- Downloads go to a per-user data dir (`%LOCALAPPDATA%\bluemap-reverse`, `~/Library/Application Support/bluemap-reverse`,
  `$XDG_DATA_HOME/bluemap-reverse` or `~/.local/share/bluemap-reverse`; `BMR_HOME` or `--data-dir` override):
  site mirrors in `cache/<site>` instead of `work/cache/<site>`, downloaded packs in `packs/` instead of `./packs`.
  Packs bundled next to `bmr` are still found first, then `./packs`.
- `pull --cache <mirror>` is now `--data-dir <dir>` (the mirror goes to `<dir>/cache/<site>`).
- Outputs are always `-o`/`--out`: `bmr reverse <mirror> -o <world>`, `bmr schem <world> -o <file>`,
  `bmr pack lookalikes <pack> -o <file>`, `bmr copy-world <world> -o <out>`; `fetch` and `obj` gain `-o`.
- `pack build --mc-version` is now `--mc`, like `seed` and `structures`.
- `fetch` waits 25 ms before each request per worker by default, like `pull` (was 0).
- `check-heights`, `copy-world` and `probe` are hidden from `bmr --help` (still runnable).

### Changed

- Minimum supported Rust version is declared (1.88) and checked in CI.
- bmr-fetch reports retries and mirror progress through `Options::progress` instead of printing.
- Without `--regen`, unseen space is filled in layers following the evidence down each column (water over rock
  over an air pocket, …) instead of one guess per gap, and below the overworld cave cutoff (y 55) it is rock
  under the last sign of an opening, instead of air down to bedrock. Found by the render round-trip (shafts
  under seabeds, flooded ravines filled with rock). Enclosed air beside known water is water. Vanilla test
  world: all blocks 74.4% → 75.3%.
- Nether biomes are decided per 4×4 column (the nether's climate noise has no height component) and spread
  to columns without markers, instead of per 4×4×4 cell with a vertical search. Nether test world: biomes
  73.6% → 96.0%.
- Flat worlds keep their own layers under builds: where most terrain surfaces sit at one height, the fill
  below them follows the layering observed across the map (superflat dirt and grass) instead of the vanilla
  bedrock band and deepslate. `context` test world: all blocks 97.5% → 99.5%; vanilla unchanged.
- Unseen rock in basalt deltas is basalt (58–71% of it on the Nether test world, at every depth) instead of
  netherrack; End spikes are obsidian down to the floor instead of end stone inside. Nether all blocks
  82.2% → 82.9%, End 97.6% → 100%.
- Faster fill: gaps under builds copy from neighbouring columns in one pass per gap instead of a ring search
  per cell, and `bmr` uses the mimalloc allocator (single window: structures test map 41 s → 26 s).
- Downloads: a response is capped at 256 MiB (was 1 GiB); only timeouts, connection errors, 408, 429 and 5xx
  are retried (4 attempts, honouring `Retry-After` up to 60 s); other 4xx fail at once. A tile that keeps
  failing no longer stops the rest of its layer: the mirror finishes around it, then reports how many
  failed (re-running retries them).

### Fixed

- A corrupt or hand-made pack whose tables point out of range is an error naming the pack instead of a panic.
- Nether caves just under the hidden roof band were filled with rock when one neighbouring block's face was
  drawn and another's missing; a drawn face now wins.
- Flowing lava and water surfaces (sloped) were read as "more liquid above", filling Nether caves with lava up
  to the roof band.
- Fire and spawners lost faces to look-alike neighbours their own geometry spills into, and the cells around
  them were filled with rock; Nether render round-trip extra faces 7.9k → 253.
- Open space under floating blocks (sky islands, builds over void) is no longer filled with rock.
- Release archives include the cubiomes license (`LICENSE-cubiomes`).
- A malformed PRBM tile with an oversized group table is an error instead of an integer overflow.
- Every command-line flag has help text; rustdoc warnings in bmr-cli.
- Structure biome checks reject Minecraft versions newer than cubiomes knows instead of treating them as the newest.
- Structure biome checks treat chunks outside the world border as not viable instead of overflowing.
- Found by the new fuzz targets (`docs/fuzzing.md`):
  - Malformed chunk NBT is an error instead of a panic inside fastnbt (a non-empty list of `End` tags) or a stack
    overflow (deep nesting; capped at Minecraft's 512 levels).
  - A chunk section with an empty block palette is an error instead of a panic when its blocks are read; palettes
    with more entries than cells are refused.
  - A lowres tile or embedded texture PNG whose header declares a huge image is refused before allocating its
    buffer (a 71-byte PNG asked for 3.4 GB).
  - A pack index naming a pack file with a path (`../x.pack`, `/etc/x.pack`) is refused; the download was written
    there.
  - Texture file names in `bmr obj` exports keep only `[A-Za-z0-9._-]`; a backslash or newline in a texture's
    resource path could escape `textures/` on Windows or break the `.mtl` file.
- A site's `settings.json` naming map ids or a data root with `..`, `/`-rooted or drive paths is refused; `fetch`
  wrote the mirror there. Map settings with zero/negative tile sizes, a zero LOD factor, a LOD count over 16 or
  grids that overflow are refused instead of dividing by zero, overflowing or looping for ever, and the start
  position is clamped to the world border.
- Pack files are decompressed with a memory cap (80 MiB of LZMA dictionary, 256 MiB of output): a pack declaring
  a 4 GiB dictionary, or an xz bomb, is an error instead of exhausting memory.

## [0.1.0] - 2026-10-01

First public release.

### Added

- `bmr pull`: mirror a BlueMap site, check the pack fits, reconstruct every map and zip the world in one command.
- Resumable site mirroring with a lowres-seeded flood fill and a download manifest (`bmr fetch`).
- PRBM tile parser, `textures.json` decoding and OBJ export (`bmr obj`); lowres heightmap cross-check
  (`bmr check-heights`).
- Block inversion from a learned signature library, using texture orientation, light, tints and game rules
  (stair corners, connections, note blocks, redstone power, waterlogging) to recover look-alike states.
- Hidden-volume fill from evidence and priors, or from a same-seed regeneration (`--regen`).
- Nether and End support: dimension profiles, roof mask, every map of a site in one world.
- Biome recovery from BlueMap's grass, foliage and water tints (overworld), vegetation (Nether) and island
  layout (End).
- World writer whose output loads in a vanilla server without regeneration; zipped world output.
- Sponge v3 schematic export with area and height selection (`bmr schem`).
- Versioned packs for Minecraft 1.21.4, 1.21.8, 1.21.11 and 26.3 (BlueMap 5.27), an online pack index and
  automatic pack selection by texture fingerprint (`bmr pack`).
- Seed recovery: structure detection, 48-bit structure-seed crack and a biome check for the upper 16 bits
  (`bmr structures`, `bmr seed`).
- Block-by-block scoring against the original world (`bmr score`) and per-cell diagnostics (`bmr explain`).
- Windowed, bounded-memory pipeline with parallel evidence and write stages.
- Test tooling: fixture worlds rendered by a real server and BlueMap, pack builder and version checks.
- Release binaries for Windows, Linux and macOS, with the packs bundled.

[Unreleased]: https://github.com/Hexay/bluemap-reverse/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/Hexay/bluemap-reverse/releases/tag/v0.1.0
