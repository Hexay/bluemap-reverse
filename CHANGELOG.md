# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `bmr --version`.
- Global `-q`/`--quiet`: progress lines off; results, warnings and errors still print.
- `bmr seed` exits with status 3 when it finishes without a world seed (exit codes in `bmr --help`).
- `--data-dir` on `fetch`, `pack list` and `pack fetch`; `BMR_HOME` sets the data dir for all of them.

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

### Fixed

- Release archives include the cubiomes license (`LICENSE-cubiomes`).
- Every command-line flag has help text; rustdoc warnings in bmr-cli.

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
