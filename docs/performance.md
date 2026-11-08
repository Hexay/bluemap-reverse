# Performance

Numbers live in `docs/results/bench.jsonl` (one line per benchmark run); this file is method + decisions.

## Tools

| Command | What |
|---|---|
| `python3 tools/bench.py <label> -n 5 --clean work/out/bench -- target/release/bmr reverse … --timings {timings}` | median wall / **CPU** / peak memory + per-stage medians → `docs/results/bench.jsonl` |
| `perf record -F 499 -g --call-graph dwarf target/profiling/bmr reverse …` | Linux sampling profile (`cargo build --profile profiling`); `perf report --no-children` for self time |
| `python3 tools/check_equiv.py [--update]` | output guard: every optimisation must keep both reference reconstructions bit-identical (regen path + prior path) |
| `bmr reverse … ` | prints per-stage time and resident memory after each stage |

- Trust **CPU time** over wall time: background load (other builds, sessions) swings wall time ±50%.
- A/B against an older commit: `git worktree add work/bench-base <rev>` + `cargo build --release --target-dir target/bench-base`.
- Windows: `tools/profile.py` drives samply over ETW (needs the Windows Performance Toolkit and admin; it
  cannot read our Rust PDB, so the profiling build emits an MSVC `/MAP` via `cargo rustc`).

## Decisions (each found by the profiler, verified by check_equiv)

- **Block states interned** (`bmr-world` `StateTable`/`StateId`): the fill, regen lookups and chunk builder
  work on u32 ids; before, 45% of CPU cloned/hashed/compared state strings per cell.
- **Textures interned** (`bmr-invert` `Tex`): face keys are `Copy` and compare integers.
- **FxHash** for integer-keyed maps; **distinct signatures matched once**.
- **Uniform sections store no index array** (reader, builder, writer): most sections are all-air or all-stone.
- **Windowed pipeline** (`bmr-cli/src/window.rs`): one region (512²) at a time with a 32-block halo, so memory
  does not grow with the map. Rules crossing cells stay within the halo; the prior fill also fills halo
  columns (its neighbour searches must see past the border), the regen fill does not. `--no-window` = whole map.
- **Windows run concurrently** (`--window-jobs`, default 4; `reconstruct.rs`), each with its own `StateTable`:
  finished chunks carry block states, not ids, and a window emits only its region, so no merge is needed.
  Before, reverse was nearly serial (CPU ≈ wall on 16 cores). Structures fixture, 2026-11-07, Linux 16 cores:
  1 job 0.92 GB peak, 3 jobs 18 s / 1.5 GB, 6 jobs 9 s / 2.2 GB.
- **mimalloc** global allocator (`bmr-cli`): glibc spent ~25% of reverse CPU in free/heap-trim.
- **Material pass 2 visits each neighbour column once per gap** (`bmr-fill/src/copy.rs`) instead of a 16-ring
  search per cell with a hash lookup and a state clone per probe (was 19% self time). With mimalloc:
  structures fixture single-window 41.4 s → 25.9 s, outputs identical.
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

- 2026-11-08, structures fixture, 4 window jobs: wall 13.6 s and a flat profile (top self: `bmr_fill::complete`
  8.7%, `solid_segments` 8.4%, `evidence::collect` 4.9%, AO 4.6%). The per-window serial stages
  (`invert.gather`'s merge, `fill.segments`, `fill.gaps`) overlap across windows, so speeding one of them up
  mostly helps `--window-jobs 1` (low-memory machines).
