"""Output-equivalence guard for performance work: reconstruct each case, then require 100% identical
voxels and biomes against stored reference reconstructions. Optimisations must not change results.

Usage: py -3 tools/check_equiv.py [--update] [--bin target/release/bmr.exe]
Cases: vanilla + regen (inversion, evidence, regen fill, biomes, writer) and vanilla-edited without the
seed (prior fill: neighbour searches, deep-water floors) — the two fill paths differ in what crosses
window borders.
"""
import argparse
import json
import shutil
import subprocess
import sys

from paths import ROOT, WORK

CASES = {
    "vanilla-regen": ["--mirror", "work/cache/vanilla", "--regen", "work/worlds/regen-vanilla/world"],
    "vanilla-edited-prior": ["--mirror", "work/cache/vanilla-edited"],
}


def run_case(bmr: str, name: str, args: list[str], update: bool) -> bool:
    ref = WORK / "ref" / name / "world"
    out = WORK / "out" / f"equiv-{name}" / "world"
    shutil.rmtree(out.parent, ignore_errors=True)
    subprocess.run([bmr, "reverse", *args[:2], str(out), *args[2:]], check=True, cwd=ROOT, stdout=subprocess.DEVNULL)
    if update:
        shutil.rmtree(ref.parent, ignore_errors=True)
        shutil.copytree(out.parent, ref.parent)
        print(f"{name}: reference updated")
        return True
    report = out.parent / "score.json"
    subprocess.run([bmr, "score", str(ref), str(out), "--top", "5", "--json", str(report)],
                   check=True, cwd=ROOT, stdout=subprocess.DEVNULL)
    r = json.loads(report.read_text())
    bad = r["all"]["total"] - r["all"]["exact"]
    bad_biomes = r["biomes"]["total"] - r["biomes"]["hits"]
    if bad or bad_biomes or r["all"]["total"] == 0:
        print(f"{name}: NOT EQUIVALENT: {bad} voxels, {bad_biomes} biome cells differ; top: {r['confusions'][:5]}")
        return False
    print(f"{name}: equivalent ({r['all']['total']} voxels, {r['biomes']['total']} biome cells)")
    return True


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--update", action="store_true", help="store this build's outputs as the references")
    ap.add_argument("--bin", default="target/release/bmr.exe")
    args = ap.parse_args()
    bmr = str(ROOT / args.bin)
    results = [run_case(bmr, name, case, args.update) for name, case in CASES.items()]
    sys.exit(0 if all(results) else 1)


if __name__ == "__main__":
    main()
