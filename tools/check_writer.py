"""Writer acceptance test: copy a fixture world through bmr's writer, load + resave it in a real server,
score against the original. Expect ~100% (grass under solid blocks may decay to dirt during the resave).

Usage: py -3 tools/check_writer.py [fixture=superflat]
"""
import json
import shutil
import subprocess
import sys

from mirror_fixture import bmr_exe
from paths import ROOT, WORK, WORLDS
from resave_world import resave


def main() -> None:
    fixture = sys.argv[1] if len(sys.argv) > 1 else "superflat"
    original = WORLDS / fixture / "world"
    copy = WORK / "out" / f"{fixture}-copy" / "world"
    shutil.rmtree(copy.parent, ignore_errors=True)
    subprocess.run([bmr_exe(),"copy-world", original, "-o", copy], check=True, cwd=ROOT)
    area = json.loads((ROOT / "fixtures" / fixture / "fixture.json").read_text())["area"]
    # 2-chunk margin: worldgen finishes chunks beyond the force-loaded area
    area = [area[0] - 32, area[1] - 32, area[2] + 32, area[3] + 32]
    if resave(copy, area):
        sys.exit(1)
    mirror = WORK / "cache" / fixture
    subprocess.run([bmr_exe(),"score", original, copy, "--mirror", mirror, "--top", "5"], check=True, cwd=ROOT)


if __name__ == "__main__":
    main()
