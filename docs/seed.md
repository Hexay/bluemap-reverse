# Seed recovery

Goal: the world seed from what a map shows, so `--regen` can fill everything the tiles don't (see
`reports/Minecraft seed recovery from maps.md` for the research and sources).

## Use

```
bmr pull https://map.example.com/ -o site-world           # a folder (or any reconstructed world)
bmr structures site-world -o obs.json                     # structures → observations
bmr seed obs.json                                         # → "seed: <n>"
py -3 tools/regen_world.py <mirror> --seed <n>            # same-seed terrain (needs Java), then pull/reverse --regen
```

## Pipeline

| Stage | Status | Input → output |
|---|---|---|
| Structure detection | done: `bmr-seed::detect`, `bmr structures` | reconstructed world → `{set, chunks}` observations |
| Structure seed (lower 48 bits) | done: `bmr-seed::solve` | observations → 48-bit candidates |
| World seed (upper 16 bits) | done: `bmr-seed::upper` + `bmr-cubiomes` | 2^16 seeds ranked by how many observed structures their biomes allow; ties → shortcuts |
| Shortcuts (tie-break) | done: `bmr-seed::world_seed` | text seed (`String.hashCode`) or blank-field random seed (`nextLong`, 2^48 reachable) |
| Villages, igloos, swamp huts, mansions | TODO | need town-centre / dome / entrance isolation (rules in the research notes) |
| Nether floor bedrock, End pillars | TODO | alternative 48-bit sources for maps without structures |

Observations: `{"mc": "26.3", "structures": [{"set": "shipwrecks", "chunks": [[x, z], …]}]}` — `set` is the
`worldgen/structure_set` name, `chunks` the possible start chunks (`"chunk": [x, z]` also accepted).

## How it works

- **Detection** (`detect/rules.rs`): per structure type, marker blocks (prismarine, netherrack, worked wood
  underwater, wool-stair tents, …) are clustered; the cluster bbox must fit the type's shape, and the start chunk's
  corner sits at a known offset from it (`research_notes/.../structure_start_rules.md`). Offsets are snapped to
  multiples of 16 within a small tolerance, which usually leaves one chunk; ambiguous ones keep all candidates.
  Ocean-ruin satellites (small ruins near a big one or each other) never sit on the start chunk and are dropped.
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
  outposts, camps exact; shipwrecks 16/17, ocean ruins 14/18, portals 8/9 (~2.5 candidates each); the seed comes
  out right from ~50 observations, ~10 s.

## Tests

- `cargo test -p bmr-seed --release` — unit tests plus `tests/truth.rs` on every `fixtures/seed/*.json`.
- `py -3 tools/check_seed.py <fixture> <truth>` — end to end on a reversed fixture; per-type detection table.
- New truth: `py -3 tools/structure_truth.py <name> [--seed S]` (no chunk gen; a coarse grid misses structures —
  add `--merge --radius 1280 --step 128` near spawn). New fixture from it: `py -3 tools/structure_fixture.py <truth>
  <fixture>`, then make_world → mirror_fixture → reverse_fixture.
