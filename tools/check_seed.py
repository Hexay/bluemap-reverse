"""End-to-end seed recovery on a reversed fixture: detect structures in the reconstruction, crack, compare with the
truth seed. Exit code 0 only if the recovered world seed is the true one.

Usage: py -3 tools/check_seed.py <fixture> <truth_name>
  needs work/out/<fixture>-rev/world (tools/reverse_fixture.py) and fixtures/seed/<truth_name>.json
"""
import json
import subprocess
import sys

from mirror_fixture import bmr_exe
from paths import ROOT, WORK


def main() -> None:
    fixture, truth_name = sys.argv[1], sys.argv[2]
    world = WORK / "out" / f"{fixture}-rev" / "world"
    truth = ROOT / "fixtures" / "seed" / f"{truth_name}.json"
    obs = WORK / "out" / f"{fixture}-structures.json"
    bmr = str(bmr_exe())
    subprocess.run([bmr, "structures", str(world), "--truth", str(truth), "-o", str(obs)], check=True)
    out = subprocess.run([bmr, "seed", str(obs)], capture_output=True, text=True, encoding="utf-8").stdout
    print(out)
    want = json.loads(truth.read_text())["seed"]
    got = next((line.split()[1] for line in out.splitlines() if line.startswith("seed:")), None)
    print(f"{'OK  ' if got == want else 'FAIL'} recovered {got}, true {want}")
    sys.exit(0 if got == want else 1)


if __name__ == "__main__":
    main()
