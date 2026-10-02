"""Render round-trip: relight a fixture's reconstruction in the server, render it with BlueMap using the fixture's
map config, mirror it into work/cache/<fixture>-roundtrip and diff its tiles face by face against the original's
(`bmr diff-render`). Report: docs/results/<fixture>-roundtrip.json, or <fixture>-<map>-roundtrip.json per map
for a multi-dimension fixture.

Columns within INSET blocks of the original render's edge are left out. The relit world ends there, next to void
chunks the server generates around it, so sky light leaks in sideways and lights caves and the sea: the original
world itself, written and relit the same way, differed from the site on 120k faces near the edge, and on 84
texture variants once they were left out (2026-10-02).

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

from make_world import fixture_dimensions
from mirror_fixture import bmr_exe, mirror
from paths import DEFAULT, FIXTURES, RESULTS, ROOT, WORK
from render_serve import TEMPLATES
from resave_world import resave, written_area

# sky light travels at most 15 blocks
INSET = 16


def map_ids(dimensions: list[str]) -> list[str | None]:
    """Map ids render_serve.configure gives a multi-dimension fixture's maps; None = the mirror's only map."""
    return [None] if len(dimensions) == 1 else [Path(TEMPLATES[d]).stem for d in dimensions]


def relight_area(world: Path, dimensions: list[str], fallback: list[int]) -> list[int]:
    """Every written chunk plus one chunk around: the server only fully lights chunks whose neighbours are
    loaded, and the reconstruction reaches past the fixture area (it covers the whole original render)."""
    areas = [a for d in dimensions if (a := written_area(world, d))] or [fallback]
    x0, z0 = min(a[0] for a in areas) - 16, min(a[1] for a in areas) - 16
    return [x0, z0, max(a[2] for a in areas) + 16, max(a[3] for a in areas) + 16]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("fixture")
    ap.add_argument("--recon", type=Path, help="reconstructed world (default: work/out/<fixture>-rev/world)")
    args = ap.parse_args()
    fixture, name = args.fixture, f"{args.fixture}-roundtrip"
    spec = json.loads((FIXTURES / fixture / "fixture.json").read_text())
    dimensions = fixture_dimensions(spec)
    recon = args.recon or WORK / "out" / f"{fixture}-rev" / "world"
    original = DEFAULT.cache / fixture
    for path, how in [(recon, "reverse_fixture.py"), (original, "mirror_fixture.py")]:
        if not path.exists():
            sys.exit(f"no {path}; run tools/{how} {fixture} first")

    world = WORK / "out" / name / "world"
    shutil.rmtree(world.parent, ignore_errors=True)
    shutil.copytree(recon, world)
    if resave(world, relight_area(world, dimensions, spec["area"]), dimensions=dimensions):
        sys.exit("relight failed")
    if mirror(fixture, force_render=True, world=world, name=name):
        sys.exit("mirroring the re-render failed")

    RESULTS.mkdir(exist_ok=True)
    failed = 0
    for map_id in map_ids(dimensions):
        report = RESULTS / (f"{fixture}-{map_id}-roundtrip.json" if map_id else f"{name}.json")
        select = ["--map", map_id] if map_id else []
        print(f"--- {map_id or fixture}", flush=True)
        failed |= subprocess.call(
            [bmr_exe(), "diff-render", original, DEFAULT.cache / name, *select, "--inset", str(INSET), "--json", report],
            cwd=ROOT,
        )
    sys.exit(failed)


if __name__ == "__main__":
    main()
