# Performance

Numbers live in `results/bench.jsonl` (one line per benchmark run); this file is method + decisions.

## Tools

| Command | What |
|---|---|
| `py -3 tools/bench.py <label> -n 5 --clean work/out/bench -- target/release/bmr.exe reverse … --timings {timings}` | median wall / **CPU** / peak memory + per-stage medians → `results/bench.jsonl` |
| `py -3 tools/profile.py <name> --build --clean work/out/prof -- target/profiling/bmr.exe reverse …` | samply sampling profile (UAC prompt), CPU-weighted self/inclusive top functions; open `work/prof/<name>.json.gz` in profiler.firefox.com for flame graphs |
| `py -3 tools/check_equiv.py [--update]` | output guard: every optimisation must keep both reference reconstructions bit-identical (regen path + prior path) |
| `bmr reverse … ` | prints per-stage time and resident memory after each stage |

- Trust **CPU time** over wall time: this machine's background load (OneDrive, IDE, WSL) swings wall time ±50%.
- A/B against an older commit: `git worktree add work/bench-base <rev>` + `cargo build --release --target-dir target/bench-base`.
- Profiling gotchas: samply needs the Windows Performance Toolkit (ADK) and admin; it cannot read our Rust
  PDB, so the profiling build emits an MSVC `/MAP` via `cargo rustc` (RUSTFLAGS would hit build scripts).

## Decisions (each found by the profiler, verified by check_equiv)

- **Block states interned** (`bmr-world` `StateTable`/`StateId`): the fill, regen lookups and chunk builder
  work on u32 ids; before, 45% of CPU cloned/hashed/compared state strings per cell.
- **Textures interned** (`bmr-invert` `Tex`): face keys are `Copy` and compare integers.
- **FxHash** for integer-keyed maps; **distinct signatures matched once**.
- **Uniform sections store no index array** (reader, builder, writer): most sections are all-air or all-stone.
- **Windowed pipeline** (`bmr-cli/src/window.rs`): one region (512²) at a time with a 32-block halo, so memory
  does not grow with the map. Rules crossing cells stay within the halo; the prior fill also fills halo
  columns (its neighbour searches must see past the border), the regen fill does not. `--no-window` = whole map.
- Parallel: tile parsing, chunk decompress/decode (within a region), regen fill (fold per thread),
  evidence block pass, chunk encode + compress. Deliberately serial: liquid evidence (first-claim wins).

## Pack builds (`tools/build_pack.py` prints time per stage)

Cold new version 693 s → ~100 s, forced rebuild ~80 s (2026-09-26, 22 cores, RAM-starved machine). What mattered:
- BlueMap `render-thread-count` defaults to 1: set to all cores in `render_serve.configure` (render 340 s → ~25 s).
- The server jar's bundler unpacks its libraries into every server folder, and the first JVM to load fresh jars
  pays ~2 min (virus scan; the extraction itself takes 1.5 s). One `-DbundlerRepoDir` for all versions
  (`console.bundler_args`) pays it once.
- Debug and template worlds are generated concurrently (own ports, 2 GB heaps).
- Left: ~40 s of server bootstrap for the worlds, ~20 s reports, ~10 s pack (xz preset 9).

Pack size 592 → ~200 KB (format 3, `bmr pack info` prints the breakdown): states as (block, ordinal) into the
registry, face keys and signatures deduplicated (identical-looking states share one), xz instead of gzip
(on the deduplicated body: gzip 421 KB, xz 217 KB), header compressed (70 → 10 KB).

## Next candidates

- `invert.gather`: per-tile maps are merged into one map serially.
- Run 2+ windows concurrently to recover the wall time windowing costs (per-window `StateTable` merge needed).
