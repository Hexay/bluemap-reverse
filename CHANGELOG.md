# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project uses
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Release archives include the cubiomes license (`LICENSE-cubiomes`).

### Changed

- Minimum supported Rust version is declared (1.88) and checked in CI.

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
