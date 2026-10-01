"""End-to-end check of a multi-dimension site: the `dimensions` fixture (one world, overworld + nether + end)
served as BlueMap's three default maps, `bmr pull` without --map must reconstruct all three into one
world. Scores each dimension against the original and loads the output in the server.

Usage: py -3 tools/check_dimensions.py
"""
import json
import subprocess
import sys

from make_world import load_fixture, make_world
from mirror_fixture import bmr_exe, pull_fixture
from paths import DEFAULT, ROOT
from resave_world import resave

FIXTURE = "dimensions"
MAPS = {"overworld": "minecraft:overworld", "nether": "minecraft:the_nether", "end": "minecraft:the_end"}


def main() -> None:
    tc = DEFAULT
    make_world(FIXTURE, False, tc)
    original = tc.worlds / FIXTURE / "world"
    out_dir = tc.work / "out" / "check-dimensions"
    pulled, _ = pull_fixture(FIXTURE, tc, out_dir)
    ok = True
    for map_id, dim in MAPS.items():
        report = out_dir / f"score-{map_id}.json"
        subprocess.run(
            [bmr_exe(), "score", original, pulled, "--mirror", out_dir / "cache", "--map", map_id,
             "--dimension", dim, "--json", report],
            cwd=ROOT, check=True, stdout=subprocess.DEVNULL,
        )
        r = json.loads(report.read_text())
        occ = 100 * r["occupied"]["exact"] / max(r["occupied"]["total"], 1)
        rend = 100 * r["rendered"]["exact"] / max(r["rendered"]["total"], 1)
        good = r["rendered"]["total"] > 0 and rend > 95
        ok &= good
        print(f"{dim:22} occupied {occ:6.2f}%  rendered {rend:6.2f}%  {'OK' if good else 'FAILED'}")

    spec, _ = load_fixture(FIXTURE)
    a = spec["area"]
    loads = resave(pulled, [a[0] - 32, a[1] - 32, a[2] + 32, a[3] + 32], tc) == 0
    print(f"server loads the output: {'OK' if loads else 'FAILED'}")
    sys.exit(0 if ok and loads else 1)


if __name__ == "__main__":
    main()
