"""Reverse mirrored fixtures and score them against the original worlds.
Appends a one-line summary per fixture to docs/results/history.jsonl (committed) so every change shows up as
better/worse, and prints that line with the change since the fixture's previous entry plus reverse/score times.
bmr's own output goes to $BMR_OUT/<fixture>-rev/{reverse,score}.log (default work/out); -v streams it instead.

Usage: python3 tools/reverse_fixture.py [--build] [--no-record] [-j N] [-v] <fixture>[,<fixture>...] [bmr reverse args...]
  --build      cargo build -j 4 (debug) first and use target/debug/bmr
  --no-record  leave docs/results alone (timing runs, experiments); the report goes to the out dir
  -j N         reverse N fixtures at once (default 4; reverse is mostly serial, see research/iteration-speed.md)
"""
import json
import os
import shutil
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

from mirror_fixture import bmr_exe
from paths import DEFAULT, EXE, FIXTURES, OUT, RESULTS, ROOT, WORK, WORLDS

METRICS = ["occupied", "rendered", "rendered_alike", "exposed", "solid_iou", "surface"]


def pct(acc: dict) -> float:
    return round(100.0 * acc["exact"] / max(acc["total"], 1), 3)


def summary(r: dict) -> dict:
    return {
        "occupied": pct(r["occupied"]),
        "rendered": pct(r["rendered"]),
        "rendered_alike": round(100.0 * r["rendered_alike"] / max(r["rendered"]["total"], 1), 3),
        "exposed": pct(r["exposed"]),
        "solid_iou": round(100.0 * r["solid"]["both"] / max(r["solid"]["original"] + r["solid"]["reconstructed"] - r["solid"]["both"], 1), 3),
        "surface": round(100.0 * r["surface"]["hits"] / max(r["surface"]["total"], 1), 3),
    }


def previous(fixture: str, extra: list[str]) -> dict | None:
    history = RESULTS / "history.jsonl"
    if not history.exists():
        return None
    for line in reversed(history.read_text().splitlines()):
        h = json.loads(line)
        if h["fixture"] == fixture and h["args"] == extra:
            return h
    return None


def run(cmd: list, log: Path | None) -> None:
    if log is None:
        subprocess.run(cmd, check=True, cwd=ROOT)
        return
    with open(log, "w") as f:
        r = subprocess.run(cmd, stdout=f, stderr=subprocess.STDOUT, cwd=ROOT)
    if r.returncode:
        sys.exit(f"{Path(cmd[0]).name} {cmd[1]} failed ({r.returncode}); last lines of {log}:\n"
                 + "\n".join(log.read_text().splitlines()[-20:]))


def reverse_and_score(exe: Path, fixture: str, extra: list[str], record: bool, verbose: bool) -> tuple[dict, float, float]:
    mirror = WORK / "cache" / fixture
    out = OUT / f"{fixture}-rev" / "world"
    shutil.rmtree(out.parent, ignore_errors=True)
    out.parent.mkdir(parents=True)
    logs = None if verbose else out.parent
    spec = json.loads((FIXTURES / fixture / "fixture.json").read_text())
    dim = ["--dimension", spec["dimension"]] if "dimension" in spec else []
    t0 = time.perf_counter()
    run([exe, "reverse", "--mirror", mirror, "-o", out, *dim, *extra], logs and logs / "reverse.log")
    t1 = time.perf_counter()
    if record:
        RESULTS.mkdir(exist_ok=True)
    report = (RESULTS if record else out.parent) / f"{fixture}.json"
    run([exe, "score", WORLDS / fixture / "world", out, "--mirror", mirror, "--top", "25", "--json", report,
         "--pack", DEFAULT.pack, *dim], logs and logs / "score.log")
    return summary(json.loads(report.read_text())), t1 - t0, time.perf_counter() - t1


def describe(fixture: str, s: dict, prev: dict | None, t_rev: float, t_score: float) -> str:
    def one(k: str) -> str:
        d = s[k] - prev[k] if prev and k in prev else 0.0
        return f"{k} {s[k]:.3f}" + (f" ({d:+.3f})" if abs(d) >= 0.0005 else "")
    return f"{fixture:<10} " + "  ".join(one(k) for k in METRICS) + f"  [reverse {t_rev:.1f}s, score {t_score:.1f}s]"


def main() -> None:
    args = sys.argv[1:]
    build, record, verbose, jobs = False, True, False, 4
    while args and args[0].startswith("-"):
        flag = args.pop(0)
        if flag == "--build":
            build = True
        elif flag == "--no-record":
            record = False
        elif flag == "-v":
            verbose = True
        elif flag == "-j":
            jobs = int(args.pop(0))
        else:
            sys.exit(f"unknown flag {flag} (bmr reverse args go after the fixture)")
    if not args:
        sys.exit(__doc__)
    fixtures, extra = args[0].split(","), args[1:]
    if build:
        subprocess.run(["cargo", "build", "-j", "4", "-q", "-p", "bmr-cli"], check=True, cwd=ROOT)
    exe = ROOT / "target" / "debug" / f"bmr{EXE}" if build and not os.environ.get("BMR_EXE") else bmr_exe()
    prev = {f: previous(f, extra) for f in fixtures}
    commit = subprocess.run(["git", "rev-parse", "--short", "HEAD"], capture_output=True, text=True, cwd=ROOT).stdout.strip()
    print(f"bmr: {exe}")

    def one(fixture: str) -> str:
        s, t_rev, t_score = reverse_and_score(exe, fixture, extra, record, verbose)
        if record:
            line = {"time": time.strftime("%Y-%m-%dT%H:%M:%S"), "commit": commit, "fixture": fixture, "args": extra, **s}
            with open(RESULTS / "history.jsonl", "a", newline="\n") as f:
                f.write(json.dumps(line) + "\n")
        return describe(fixture, s, prev[fixture], t_rev, t_score)

    with ThreadPoolExecutor(max(1, jobs)) as pool:
        for line in pool.map(one, fixtures):
            print(line, flush=True)


if __name__ == "__main__":
    main()
