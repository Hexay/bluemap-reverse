<div align="center">

# bluemap-reverse

**Turn a BlueMap web map back into a playable Minecraft Java world.**

Point it at a BlueMap site and get a world zip with every dimension, or a WorldEdit schematic.
No server, no Java, no BlueMap install, just one binary.

[![CI](https://github.com/Hexay/bluemap-reverse/actions/workflows/ci.yml/badge.svg)](https://github.com/Hexay/bluemap-reverse/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](#license)
[![Rust 2024](https://img.shields.io/badge/rust-2024%20edition-orange?logo=rust)](https://www.rust-lang.org/)
[![Minecraft 1.18+](https://img.shields.io/badge/minecraft-1.18%2B-62b47a)](#supported-versions)
[![BlueMap 5.27](https://img.shields.io/badge/bluemap-5.27-2c6fbb)](https://bluemap.bluecolored.de/)

[Install](#installation) · [Quick start](#quick-start) · [Accuracy](#accuracy) · [How it works](#how-it-works) ·
[Limitations](#limitations) · [Building](#building-from-source) · [Docs](#documentation)

</div>

---

```console
$ bmr pull https://map.example.com/ -o world.zip
```

Extract `world.zip` into `.minecraft/saves/` and open it.

## Features

- **One command.** `pull` mirrors the site, picks the matching data pack, reconstructs every map and zips
  the result. Downloads are cached and resumable, so re-running costs nothing.
- **Block-accurate.** Each drawn face is matched against BlueMap's own render of every block state. Texture
  orientation, light, tints and game rules (stair corners, fence connections, redstone power, waterlogging)
  settle the remaining details.
- **All dimensions.** Overworld, Nether and End maps go into one world, detected from the map id or BlueMap's
  default sky colour.
- **Biomes.** Recovered from the grass, foliage and water tints BlueMap drew (overworld), from what grows there
  (Nether) and from the island layout (End).
- **Hidden blocks.** Underground and enclosed blocks are filled from evidence and priors. If you know the world
  seed, a same-seed regeneration fills them instead (`--regen`).
- **Seed recovery.** Detects structures in the reconstructed world and cracks the world seed from their start
  chunks ([docs/seed.md](docs/seed.md)).
- **Schematics.** Cuts a Sponge v3 `.schem` for WorldEdit/FAWE out of any world.

## Installation

Download the archive for your platform from the [latest release][latest] (Windows, Linux, macOS on Intel or
Apple silicon), extract it and run `bmr` from that folder. The bundled `packs/` folder covers the
[supported versions](#supported-versions) offline; other packs download on demand.

To build it yourself, see [Building from source](#building-from-source).

## Quick start

```sh
bmr pull https://map.example.com/ -o world.zip            # mirror + check + reconstruct + zip
bmr pull https://map.example.com/ --map world_nether      # only one of the site's maps
bmr pull https://map.example.com/ --schem build.schem     # also a schematic (overworld, else the first map)
bmr schem <world> part.schem --area=x0,z0,x1,z1 --y=60,120 # cut a schematic out of any world
```

### Packs

A pack (~0.3 MB) holds what reconstruction needs for one Minecraft + BlueMap version: block signatures, biome
tints, the block registry and an empty world template. `pull` fingerprints the site's texture list, picks the
matching pack from `./packs`, `packs/` next to the binary or the [online index][packs-release], and downloads it
if needed.

```sh
bmr pack list                    # installed and available packs
bmr pack fetch <mc-version|all>  # download ahead of time
bmr pull … --offline             # never touch the index
```

`pull` compares textures and BlueMap version and explains mismatches (another Minecraft version, mods, resource
packs). It refuses a clearly wrong pack unless you pass `--force`.

### Options worth knowing

| Option | Default | Purpose |
|---|---|---|
| `--map <id>` | all maps | Reconstruct one map. Needed when a site has two maps of the same dimension. |
| `--mask-y`, `--cave-y` | BlueMap's defaults | Map settings the site doesn't publish (Nether roof y 90..127 hidden, overworld caves below y 55). |
| `--concurrency`, `--delay-ms` | 4, 25 ms | Download politeness. |
| `--offline` | off | Use installed packs only. |
| `--force` | off | Use a pack even when it doesn't fit the site. |

Run `bmr <command> --help` for the full list.

## Accuracy

Scored block by block against the original worlds (Minecraft 26.3, BlueMap 5.27). Full history in
[`docs/results/history.jsonl`](docs/results/history.jsonl).

| Test world | Drawn blocks | Drawn, look-alikes allowed | All blocks | Biomes |
|---|--:|--:|--:|--:|
| Superflat with builds | 99.86% | 100% | 99.92% | 100% |
| Vanilla terrain, no seed | 98.94% | 99.99% | 74.4% | 95.7% |
| Vanilla terrain, with seed (`--regen`) | 99.86% | 99.99% | 99.93% | 100% |
| Blocks among neighbours¹ | 99.44% | 99.98% | 97.5% | 99.8% |
| Nether | 99.99% | 99.99% | 82.1% | 73.6% |
| End | 100% | 100% | 97.6% | 100% |

¹ The `context` fixture: stairs, fences, redstone, doors and a jumble of every block.

- **Drawn blocks** are those BlueMap rendered in at least one tile.
- **Look-alikes** render identically, so no tile can tell them apart: waxed vs unwaxed copper, infested vs
  plain stone, a double slab vs its full block, invisible or random properties (note pitch, crop age).
- **All blocks** includes everything BlueMap never drew. Without the seed that part is an educated guess.

## How it works

1. **Mirror.** Download the site's settings, textures and hires/lowres tiles into `work/cache/<site>`.
2. **Decode.** Parse BlueMap's PRBM tile meshes into faces with textures, tints, light and positions.
3. **Invert.** Match each block's faces against signatures learned from BlueMap's render of every block state,
   then apply game rules to pick between candidates.
4. **Fill.** Infer biomes from tints and fill unrendered volume from evidence, priors or a seeded regeneration.
5. **Write.** Emit Anvil region files for each dimension, then zip the world or cut a schematic.

The research behind each step lives in [`docs/research/`](docs/research).

## Limitations

- Only what BlueMap renders can be recovered exactly. Chest contents, sign text, other block entity data and
  entities aren't in the tiles at all.
- Hidden areas (caves under BlueMap's cave cutoff, the Nether roof, sealed interiors) are approximations unless
  you have the seed.
- Look-alike blocks (see [Accuracy](#accuracy)) can't be told apart, so one of the group is picked.
- Modded blocks and resource packs change textures; the pack check will flag them and reconstruction quality
  drops.

## Supported versions

Prebuilt packs, all for BlueMap 5.27:

| Minecraft | Pack |
|---|---|
| 26.3 | `bmr-mc26.3-bluemap5.27.pack` |
| 1.21.11 | `bmr-mc1.21.11-bluemap5.27.pack` |
| 1.21.8 | `bmr-mc1.21.8-bluemap5.27.pack` |
| 1.21.4 | `bmr-mc1.21.4-bluemap5.27.pack` |

Packs for any other 1.18+ version can be built unattended in about two minutes
([docs/development.md](docs/development.md#packs)).

## Building from source

Requires a recent stable Rust toolchain (2024 edition) and a C compiler (MSVC, GCC or Clang) for the vendored
[cubiomes](https://github.com/Cubitect/cubiomes).

```sh
git clone https://github.com/Hexay/bluemap-reverse
cargo build --release
# binary: target/release/bmr
```

The test tooling under `tools/` is plain Python 3 (stdlib only). It downloads its own JDK, Minecraft server and
BlueMap to build fixture worlds and score reconstructions; see [docs/development.md](docs/development.md).

## Documentation

| Document | Contents |
|---|---|
| [docs/plan.md](docs/plan.md) | Goal, architecture, phases, scoring, open questions. **Start here.** |
| [docs/development.md](docs/development.md) | Test loop, fixtures, full command reference, building packs. |
| [docs/seed.md](docs/seed.md) | Seed recovery pipeline. |
| [docs/performance.md](docs/performance.md) | Benchmarking, profiling and performance decisions. |
| [docs/research/01-bluemap-web-format.md](docs/research/01-bluemap-web-format.md) | What a BlueMap site exposes: URLs, PRBM, tiles, textures, culling. |
| [docs/research/02-model-inversion.md](docs/research/02-model-inversion.md) | How BlueMap turns block states into meshes, and how to invert it. |
| [docs/research/03-rust-and-tooling.md](docs/research/03-rust-and-tooling.md) | Crates, BlueMap CLI usage, test-world generation, version pins. |
| [docs/research/04-filling-hidden-data.md](docs/research/04-filling-hidden-data.md) | Recovering what tiles don't contain: seeds, regen, biome inference. |
| [docs/research/seed-recovery.md](docs/research/seed-recovery.md) | Seed recovery from maps: structure cracking, upper bits, sources. |
| [docs/chat-log.md](docs/chat-log.md) | The conversation that started the project. |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Reporting problems, making changes, releasing. |
| [CHANGELOG.md](CHANGELOG.md) | What changed in each release. |

## Responsible use

Only reconstruct maps you own or have permission to reverse. The defaults download gently; keep them that way
on other people's servers.

## Acknowledgements

- [BlueMap](https://github.com/BlueMap-Minecraft/BlueMap), whose renderer this project inverts.
- [cubiomes](https://github.com/Cubitect/cubiomes) (MIT), vendored from the
  [xpple fork](https://github.com/xpple/cubiomes) for biome checks during seed recovery.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option. The vendored cubiomes sources keep their own MIT license
([crates/bmr-cubiomes/vendor/LICENSE](crates/bmr-cubiomes/vendor/LICENSE)).

[packs-release]: https://github.com/Hexay/bluemap-reverse/releases/tag/packs
[latest]: https://github.com/Hexay/bluemap-reverse/releases/latest
