# Iteration speed (2026-11-07)

Loop: edit `bmr-fill`/`bmr-invert` → `cargo build` (debug) → `tools/reverse_fixture.py <f>` → read the score;
and `cargo test --workspace`. Re-measure with `python3 tools/loop_timing.py --tests` (backgrounded; set
`BMR_OUT` in a worktree).

Machine during the runs: 16 cores at load average 53-68 (another session's release builds and profiles), swap
full, governor `performance`. Wall times move ±30% between runs, so every A/B below ran back to back.

## Before / after

| Step | Before | After |
|---|---|---|
| `cargo build` after a real `bmr-fill` edit | 4.6-5.9 s | same (no profile change paid off, see below) |
| `cargo build` after touching `bmr-invert` | 4.5-6.0 s | same |
| reverse + score `context` | 2.9 s, ~60 lines of output | 2.9 s, one line with deltas vs the last history entry |
| reverse + score context, superflat, vanilla, end | 16.3 s as 4 calls | 6.8 s as 1 call (`-j 4` default) |
| reverse + score `nether` | 142-245 s | same (runtime hot spot, below) |
| tests after a `bmr-fill` edit | `cargo test --workspace` 43-51 s | `tools/test.py` 1.4-2.0 s |
| **edit → build → 4 fixtures → tests** | **~66 s, 6 calls** | **~13 s, 2 calls** |

`cargo test --workspace` stays the acceptance run (CI, CONTRIBUTING). 41 s of its ~45 s is
`bmr-seed/tests/truth.rs` (seed cracking, rayon brute force: 12-42 s per test on this load), which only a
`bmr-seed`/`bmr-cubiomes`/`bmr-world` change can affect.

## Changes

- `tools/test.py`: tests the crates with changed files (vs HEAD, or `--base REF`) plus their workspace
  dependents from `cargo metadata`; a root `Cargo.toml`/`Cargo.lock` change tests everything. Exit code is
  cargo's, no pipe.
- `tools/reverse_fixture.py`: comma-separated fixtures run 4 at a time (reverse is mostly serial: nether ran at
  ~0.8 of one core), `--build` folds in `cargo build -p bmr-cli`, one summary line per fixture with the change
  since its previous history entry and reverse/score seconds; bmr output goes to `<out>/<f>-rev/*.log`
  (`-v` streams it; on failure the log tail is printed). `--no-record` leaves `docs/results` alone, so timing
  runs no longer need a `git checkout docs/results`.
- `$BMR_OUT` (tools/paths.py `OUT`): worktrees share `work/` through a symlink, and `work/out/<f>-rev` collided
  with the main checkout's session. `.gitignore` `/work` (was `/work/`, which misses the symlink).
- `bmr_exe()` picks the newer of `target/release/bmr` and `target/debug/bmr`: a leftover release binary used
  to shadow every debug rebuild silently.
- `tools/loop_timing.py`: the measurement script for this file.

## Measured, not worth changing

- Rebuild after touching `bmr-fill`: `bmr-fill` 0.3 s, `bmr-cli` 3.6 s. In `bmr-cli` (rustc `-Z time-passes`):
  codegen 1.7 s, local ThinLTO 1.3 s, link 0.25 s (mold). Linking is not the cost.
- `debug = "line-tables-only"` (deps `debug = false`): binary 124 → 36 MB, rebuilds 7.7 / 6.2 s vs 4.6 / 6.0 s.
  No gain.
- `lto = "off"` in dev: rebuilds 3.8 / 4.3 s vs 4.3 / 4.5 s, but reverse 30-70% slower (vanilla 3.0 → 4.3 s,
  nether 146 → 244 s). Rejected.
- `opt-level = 3` for `bmr-fill` only: nether 142 → 108 s, vanilla unchanged, a real `bmr-fill` edit
  5.9 → 7.1 s. Not the default; for a nether session:
  `cargo build --config 'profile.dev.package.bmr-fill.opt-level=3'` (switching back recompiles fill + cli, ~10 s).
- `opt-level = 3` for `bmr-seed` in tests: truth.rs 42.5 → 39 s, within noise.
- cubiomes `build.rs`: 26 s of the 62 s cold build of a fresh worktree (new build dir, sccache misses on
  paths); 0.3 s otherwise, and it only reruns when `vendor/` or `shim/` change. The C itself compiles in 5.8 s
  serial / 2.1 s 4-way, so cc's `parallel` feature would save ≤ 4 s once per worktree. Not added.
- Library build in every `bmr reverse` (from `work/cache/debug`): 0.65-0.72 s. Not worth caching.

## Left for the runtime work (crate source, not this task)

- Nether: `fill` is 98% of reverse (230 s of 235 s in one debug run). perf: `bmr_fill::material::solid_segments`
  49% self, `columns::ring` closure 25%, `prior_segments` closure 17%. `material.rs` `copy_nearest` walks
  `ring(r)` for r = 1..=16 per cell with a hash lookup each, serially.
- Mining recent transcripts found a `cargo build … | tail` whose failure the pipe hid; `tools/test.py` and
  `reverse_fixture.py --build` keep cargo's exit code.
