# Seed recovery

Goal: the world seed from what a map shows, so `--regen` can fill everything the tiles don't (see
`research/seed-recovery.md` for the research and sources).

## Use

```
bmr pull https://map.example.com/ --crack-seed            # reconstruct, detect, crack → "seed: <n>", SITE.seed.json
bmr pull https://map.example.com/ --seed <n>              # exact bedrock and deepslate layers (no Java)
python3 tools/regen_world.py <mirror> --seed <n>          # same-seed terrain (needs Java), then pull/reverse --regen
```

Step by step, or on any world: `bmr structures <world> -o obs.json`, then `bmr seed obs.json`.

## Pipeline

| Stage | Status | Input → output |
|---|---|---|
| Structure detection | done: `bmr-seed::detect`, `bmr structures` | reconstructed world → `{set, chunks}` observations |
| Structure seed (lower 48 bits) | done: `bmr-seed::solve` | observations → 48-bit candidates |
| World seed (upper 16 bits) | done: `bmr-seed::upper` + `bmr-cubiomes` | 2^16 seeds ranked by how many observed structures their biomes allow; ties → shortcuts |
| Shortcuts (tie-break) | done: `bmr-seed::world_seed` | text seed (`String.hashCode`) or blank-field random seed (`nextLong`, 2^48 reachable) |
| Villages, igloos, swamp huts, mansions | done: `detect/verified.rs` | bell / dome / hut / plank clusters → placements confirmed by template match; on fixtures only villages occur (2/2) |
| Nether floor bedrock, End pillars | TODO | alternative 48-bit sources for maps without structures |

Observations: `{"mc": "26.3", "structures": [{"set": "shipwrecks", "chunks": [[x, z], …]}]}` — `set` is the
`worldgen/structure_set` name, `chunks` the possible start chunks (`"chunk": [x, z]` also accepted).

## How it works

- **Detection** (`detect/rules.rs`): per structure type, marker blocks (prismarine, netherrack, worked wood
  underwater, wool-stair tents, …) are clustered; the cluster bbox must fit the type's shape, and the start chunk's
  corner sits at a known offset from it (`research/seed-recovery/structure_start_rules.md`). Offsets are snapped to
  multiples of 16 within a small tolerance, which usually leaves one chunk; ambiguous ones keep all candidates.
  Ocean-ruin satellites (small ruins near a big one or each other) never sit on the start chunk and are dropped.
- **Template match** (`detect/verified.rs`): villages (each bell → every town-centre template, bell and rotation
  whose origin lands on a chunk corner), igloos (dome bbox → top template per rotation), swamp huts (7×9 spruce
  bbox → `SwampHutPiece` boxes per orientation) and mansions (every chunk corner under the plank cluster → entrance
  piece at its fixed offset per rotation). A start counts only if enough of the placed template's blocks are there
  (`MIN_SCORE`, calibration in its doc), so house bells and player builds drop out. Templates: `detect/templates.json`,
  regenerated from the server jar by `python3 tools/gen_structure_templates.py`.
- **Lower 48**: each `random_spread` region draws its candidate chunk from `Random(seed + rx·341873128712 +
  rz·132897987541 + salt)`. For a linear set with range r = 2^k·m, the offset mod 2^k is bits [17, 17+k) of the LCG
  state, so the low 17+K seed bits are enumerated and filtered first (lifting), then the rest per survivor. An
  observation matches if any candidate chunk does. `--max-misses` (default one per six) tolerates wrong ones;
  work is bounded by `MAX_WORK` in `solve.rs`.
- **Upper 16**: positions ignore the upper bits, biomes don't. For each of the 2^16 seeds, cubiomes checks whether
  the biomes at up to 64 observed start chunks (each resolved to the chunk the structure seed places it at)
  allow the structure.
- `bmr-cubiomes` vendors xpple/cubiomes @ 18edd56 (MIT, 26.3 support) with a few `bmr patch` edits so MSVC builds it
  (no VLAs); first build ~70 s, cached after.

## Measured (26.3)

- Forward model and cubiomes viability agree with the server on every `/locate`d structure in `fixtures/seed`.
- **Lower 48**: one set alone plateaus (same range → correlated offsets: 24 temples leave 3 candidates, 24 ocean
  ruins 12); mixed ranges resolve fast (~8 across temples, shipwrecks, villages). Temple sets have consecutive
  salts, so two temple types in the same region are nearly redundant.
- **Upper 16**: 10 viability checks leave a handful tied, 20 pick the true seed alone; a typed numeric seed (no
  shortcut) is found by biomes alone.
- **End to end** (`fixtures/structures`: BlueMap render → reverse → detect → crack): monuments, jungle temples,
  outposts, camps, villages (2/2, town centres match 0.82 and 1.0) exact; shipwrecks 16/17, ocean ruins 14/18,
  portals 8/9 (~2.5 candidates each); 50 of 52 observations right; the seed comes out right from them, ~10 s. The
  fixture has no igloos, swamp huts or mansions: those rules are checked on stamped templates (unit tests) only.

## Tests

- `cargo test -p bmr-seed --release` — unit tests plus `tests/truth.rs` on every `fixtures/seed/*.json`.
- `python3 tools/check_seed.py <fixture> <truth>` — end to end on a reversed fixture; per-type detection table.
- New truth: `python3 tools/structure_truth.py <name> [--seed S]` (no chunk gen; a coarse grid misses structures —
  add `--merge --radius 1280 --step 128` near spawn). New fixture from it: `python3 tools/structure_fixture.py <truth>
  <fixture>`, then make_world → mirror_fixture → reverse_fixture.
