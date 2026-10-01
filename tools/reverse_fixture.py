"""Reverse a mirrored fixture and score it against the original world.
Appends a one-line summary to docs/results/history.jsonl (committed) so every change shows up as better/worse.

Usage: py -3 tools/reverse_fixture.py <fixture> [extra bmr reverse args...]
"""
import json
import shutil
import subprocess
import sys
import time

from mirror_fixture import bmr_exe
from paths import DEFAULT, FIXTURES, RESULTS, ROOT, WORK, WORLDS


def pct(acc: dict) -> float:
    return round(100.0 * acc["exact"] / max(acc["total"], 1), 3)


def main() -> None:
    fixture, extra = sys.argv[1], sys.argv[2:]
    mirror = WORK / "cache" / fixture
    out = WORK / "out" / f"{fixture}-rev" / "world"
    shutil.rmtree(out.parent, ignore_errors=True)
    spec = json.loads((FIXTURES / fixture / "fixture.json").read_text())
    dim = ["--dimension", spec["dimension"]] if "dimension" in spec else []
    subprocess.run([bmr_exe(), "reverse", "--mirror", mirror, out, *dim, *extra], check=True, cwd=ROOT)

    RESULTS.mkdir(exist_ok=True)
    report = RESULTS / f"{fixture}.json"
    subprocess.run(
        [bmr_exe(), "score", WORLDS / fixture / "world", out, "--mirror", mirror, "--top", "25", "--json", report,
         "--pack", DEFAULT.pack, *dim],
        check=True, cwd=ROOT,
    )
    r = json.loads(report.read_text())
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True, cwd=ROOT).stdout.strip()
    line = {
        "time": time.strftime("%Y-%m-%dT%H:%M:%S"),
        "commit": commit,
        "fixture": fixture,
        "args": extra,
        "occupied": pct(r["occupied"]),
        "rendered": pct(r["rendered"]),
        "rendered_alike": round(100.0 * r["rendered_alike"] / max(r["rendered"]["total"], 1), 3),
        "exposed": pct(r["exposed"]),
        "solid_iou": round(100.0 * r["solid"]["both"] / max(r["solid"]["original"] + r["solid"]["reconstructed"] - r["solid"]["both"], 1), 3),
        "surface": round(100.0 * r["surface"]["hits"] / max(r["surface"]["total"], 1), 3),
    }
    with open(RESULTS / "history.jsonl", "a") as f:
        f.write(json.dumps(line) + "\n")


if __name__ == "__main__":
    main()
