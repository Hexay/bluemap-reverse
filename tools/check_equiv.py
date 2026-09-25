"""Output-equivalence guard for performance work: reconstruct, then require 100% identical voxels and
biomes against a stored reference reconstruction. Optimisations must not change results.

Usage: py -3 tools/check_equiv.py [--update] [--bin target/release/bmr.exe]
Case: vanilla mirror + regen (exercises inversion, evidence, regen fill, biomes, writer).
"""
import argparse
import json
import shutil
import subprocess
import sys

from paths import ROOT, WORK

CASE = ["--mirror", "work/cache/vanilla", "--regen", "work/worlds/regen-vanilla/world"]
REF = WORK / "ref" / "vanilla-regen" / "world"
OUT = WORK / "out" / "equiv" / "world"


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--update", action="store_true", help="store this build's output as the reference")
    ap.add_argument("--bin", default="target/release/bmr.exe")
    args = ap.parse_args()
    bmr = str(ROOT / args.bin)
    shutil.rmtree(OUT.parent, ignore_errors=True)
    subprocess.run([bmr, "reverse", *CASE[:2], str(OUT), *CASE[2:]], check=True, cwd=ROOT, stdout=subprocess.DEVNULL)
    if args.update:
        shutil.rmtree(REF.parent, ignore_errors=True)
        shutil.copytree(OUT.parent, REF.parent)
        print(f"reference updated: {REF}")
        return
    report = WORK / "out" / "equiv" / "score.json"
    subprocess.run([bmr, "score", str(REF), str(OUT), "--top", "5", "--json", str(report)],
                   check=True, cwd=ROOT, stdout=subprocess.DEVNULL)
    r = json.loads(report.read_text())
    bad = r["all"]["total"] - r["all"]["exact"]
    bad_biomes = r["biomes"]["total"] - r["biomes"]["hits"]
    if bad or bad_biomes or r["all"]["total"] == 0:
        print(f"NOT EQUIVALENT: {bad} voxels, {bad_biomes} biome cells differ; top: {r['confusions'][:5]}")
        sys.exit(1)
    print(f"equivalent: {r['all']['total']} voxels, {r['biomes']['total']} biome cells identical")


if __name__ == "__main__":
    main()
