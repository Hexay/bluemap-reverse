#!/usr/bin/env bash
# Structure-detection fixtures on the testbox (RAM-heavy reverse): build, render, reverse, then check seed recovery.
# Run detached: py -3 tools/testbox.py run "nohup bash tools/testbox_structures.sh > structures.log 2>&1 &"
set -uo pipefail
. "$HOME/.cargo/env"
LAND="igloos swamp_huts desert_pyramids villages pillager_outposts"
cargo build --release -p bmr-cli || exit 1
[ "${SKIP_TRUTH:-}" ] || python3 tools/structure_truth.py random_seed --merge --radius 3072 --step 256 --sets $LAND || exit 1
python3 tools/structure_fixture.py random_seed structures-land --radius 192 --sets $LAND || exit 1
for f in structures-numeric structures-land; do
    python3 tools/make_world.py "$f" && python3 tools/mirror_fixture.py "$f" && python3 tools/reverse_fixture.py "$f" || echo "FIXTURE FAILED $f"
done
python3 tools/check_seed.py structures-numeric numeric_seed
python3 tools/check_seed.py structures-land random_seed
echo "chain done"
