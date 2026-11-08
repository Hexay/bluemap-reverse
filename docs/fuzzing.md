# Fuzzing

Every parser that reads input from a BlueMap site, a pack index or a downloaded pack has a
[cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) target under `fuzz/`. The crate is outside the workspace
(it needs nightly and libFuzzer), has its own `Cargo.lock` and builds into `fuzz/target`.

| Target | Entry point | Input |
|---|---|---|
| `prbm` | `bmr_prbm::parse` + `Tile::faces` | hires tile |
| `decompress` | `Format::decompress`, `decompress_any` | first byte picks gzip/zlib/zstd/lz4-java, rest is the stream |
| `lowres` | `LowresImage::decode` + pixel accessors | lowres PNG |
| `region` | `bmr_world::parse_region_where` + `decode_chunk` | `.mca` file |
| `chunk_nbt` | `bmr_world::decode_chunk` + every block/biome accessor | uncompressed chunk NBT |
| `pack` | `Pack::header_from_bytes`, `Pack::from_bytes` | pack file, or a raw postcard body wrapped in a valid header |
| `pack_index` | `Index::parse`, `textures_of`, `verify` | `index.json` |
| `textures_json` | `parse_textures`, `parse_texture_names`, `frame_png`, `file_stem` | `textures.json` |
| `texture_png` | `Texture::frame_png` | one embedded texture PNG |
| `settings_json` | `SiteSettings::parse`, `MapSettings::parse` + grid math | `settings.json` (site or map) |

Targets also assert what callers rely on (e.g. pack and map names stay inside their directory), so an accepted
but unsafe input is a crash too.

## Running

Needs a nightly toolchain and `cargo install cargo-fuzz`. Run one fuzzer at a time:

```
python3 tools/fuzz.py run                        # every target, 180 s each
python3 tools/fuzz.py run region pack --seconds 600
```

Each run uses `fuzz/corpus/<target>` (git-ignored, grows) plus the committed seeds in `fuzz/seeds/<target>`, with
`-rss_limit_mb=1024` and a per-target `-max_len`. Crashes land in `fuzz/artifacts/<target>/`; reproduce one with

```
(cd fuzz && cargo +nightly fuzz run --target x86_64-unknown-linux-gnu <target> artifacts/<target>/<file>)
```

`--target` matters: cargo-fuzz defaults to the triple it was built for, which can be musl (no sanitizers).
Every fix gets a unit test in the crate that owns the parser, with the minimised input inline.

Fuzz builds skip PNG and some other checksums (`cfg(fuzzing)`), so a PNG artifact needs its CRCs fixed before it
reproduces in a unit test.

## Seeds

`python3 tools/fuzz.py seeds [--from CHECKOUT]` regenerates `fuzz/seeds/` from local fixtures (`work/cache`,
`work/worlds/end`, `packs/index.json`): the smallest real tiles, a one-chunk region, trimmed JSON. A worktree has
no `work/`, so point `--from` at the main checkout.

## CI

`.github/workflows/fuzz.yml` runs every target for 180 s each week (Monday 03:17 UTC) and on manual dispatch with a
custom duration. Crash artifacts are uploaded when it fails.
