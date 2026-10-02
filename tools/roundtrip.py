"""Render round-trip: relight a fixture's reconstruction in the server, render it with BlueMap using the fixture's
map config, mirror it into work/cache/<fixture>-roundtrip and diff its tiles face by face against the original's
(`bmr diff-render`). Report: docs/results/<fixture>-roundtrip.json.

Usage: py -3 tools/roundtrip.py <fixture> [--recon <world>]
Needs the fixture's mirror (mirror_fixture.py) and reconstruction (reverse_fixture.py, default
work/out/<fixture>-rev/world). The reconstruction is copied first: relighting resaves it in place.
"""
import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

from mirror_fixture import bmr_exe, mirror
from paths import DEFAULT, FIXTURES, RESULTS, ROOT, WORK
from resave_world import resave


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("fixture")
    ap.add_argument("--recon", type=Path, help="reconstructed world (default: work/out/<fixture>-rev/world)")
    args = ap.parse_args()
    fixture, name = args.fixture, f"{args.fixture}-roundtrip"
    spec = json.loads((FIXTURES / fixture / "fixture.json").read_text())
    if spec.get("dimension", "minecraft:overworld") != "minecraft:overworld":
        sys.exit("only overworld fixtures: relighting force-loads the overworld")
    recon = args.recon or WORK / "out" / f"{fixture}-rev" / "world"
    original = DEFAULT.cache / fixture
    for path, how in [(recon, "reverse_fixture.py"), (original, "mirror_fixture.py")]:
        if not path.exists():
            sys.exit(f"no {path}; run tools/{how} {fixture} first")

    world = WORK / "out" / name / "world"
    shutil.rmtree(world.parent, ignore_errors=True)
    shutil.copytree(recon, world)
    if resave(world, spec["area"]):
        sys.exit("relight failed")
    if mirror(fixture, force_render=True, world=world, name=name):
        sys.exit("mirroring the re-render failed")

    RESULTS.mkdir(exist_ok=True)
    report = RESULTS / f"{name}.json"
    sys.exit(subprocess.call([bmr_exe(), "diff-render", original, DEFAULT.cache / name, "--json", report], cwd=ROOT))


if __name__ == "__main__":
    main()
