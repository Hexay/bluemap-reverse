"""End-to-end check of one Minecraft version's pack: build the superflat fixture in that version, render
and serve it with that BlueMap, `bmr pull` it (the pack must be picked by fingerprint), score against
the original, and load the output in that version's server (writer format + folder layout).

Usage: py -3 tools/check_version.py --mc 1.21.11 [--bluemap 5.27]
"""
import argparse
import json
import subprocess
import sys

from make_world import load_fixture, make_world
from mirror_fixture import bmr_exe, pull_fixture
from paths import DEFAULT, ROOT
from resave_world import resave
from setup import resolve, setup

FIXTURE = "superflat"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--mc", required=True)
    ap.add_argument("--bluemap", default=DEFAULT.bluemap)
    args = ap.parse_args()
    tc = resolve(args.mc, args.bluemap)
    setup(tc)
    make_world(FIXTURE, False, tc)
    original = tc.worlds / FIXTURE / "world"

    out_dir = tc.work / "out" / "check"
    pulled, pull_log = pull_fixture(FIXTURE, tc, out_dir)
    expected = f"bmr-mc{tc.mc}-bluemap{tc.bluemap}.pack"
    # the ranking line marked with an arrow and naming a .pack (the progress lines have arrows too)
    chosen = next((l for l in pull_log.splitlines() if ".pack:" in l and "→" in l.split(".pack:")[0]), "")
    ok_pick = expected in chosen
    print(f"pack picked: {'OK' if ok_pick else 'WRONG'} ({chosen.strip()})")

    report = out_dir / "score.json"
    subprocess.run(
        [bmr_exe(), "score", original, pulled, "--mirror", out_dir / "cache", "--blocks", tc.blocks_json, "--json", report],
        cwd=ROOT, check=True, stdout=subprocess.DEVNULL,
    )
    r = json.loads(report.read_text())
    occ = 100 * r["occupied"]["exact"] / max(r["occupied"]["total"], 1)
    rend = 100 * r["rendered"]["exact"] / max(r["rendered"]["total"], 1)
    print(f"score: occupied {occ:.2f}%, rendered {rend:.2f}%")

    spec, _ = load_fixture(FIXTURE)
    a = spec["area"]
    loads = resave(pulled, [a[0] - 32, a[1] - 32, a[2] + 32, a[3] + 32], tc) == 0
    print(f"server {tc.mc} loads the output: {'OK' if loads else 'FAILED'}")
    sys.exit(0 if ok_pick and loads and occ > 99 else 1)


if __name__ == "__main__":
    main()
