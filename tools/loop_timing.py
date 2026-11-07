"""Time each step of the dev loop (see research/iteration-speed.md): incremental debug rebuild after touching a
crate, reverse+score per fixture, and the test runs. Prints median and spread per step; writes nothing to
docs/results. Run it backgrounded: the full set takes a few minutes, nether alone ~2-4 min in a debug build.

Usage: python3 tools/loop_timing.py [-n 3] [--crates bmr-fill,bmr-invert] [--fixtures context,superflat,vanilla]
                                    [--tests] [--cargo-config KEY=VALUE ...]
"""
import argparse
import os
import re
import statistics
import subprocess
import sys
import time

from paths import ROOT


def timed(cmd: list[str], env: dict | None = None) -> tuple[float, str]:
    t = time.perf_counter()
    r = subprocess.run(cmd, capture_output=True, text=True, cwd=ROOT, env=env)
    if r.returncode:
        sys.exit(f"failed ({r.returncode}): {' '.join(cmd)}\n{(r.stdout + r.stderr)[-3000:]}")
    return time.perf_counter() - t, r.stdout


def report(label: str, xs: list[float]) -> None:
    spread = f"  {min(xs):.1f}..{max(xs):.1f}" if len(xs) > 1 else ""
    print(f"{label:<44} median {statistics.median(xs):6.1f}s{spread}  (n={len(xs)})", flush=True)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("-n", type=int, default=3)
    ap.add_argument("--crates", default="bmr-fill,bmr-invert")
    ap.add_argument("--fixtures", default="context,superflat,vanilla")
    ap.add_argument("--tests", action="store_true", help="also time tools/test.py-style and full workspace tests")
    ap.add_argument("--cargo-config", action="append", default=[], help="passed to cargo --config (experiments)")
    a = ap.parse_args()
    cfg = [x for c in a.cargo_config for x in ("--config", c)]
    build = ["cargo", "build", "-j", "4", "-q", *cfg]
    timed(build)
    for crate in filter(None, a.crates.split(",")):
        xs = []
        for _ in range(a.n):
            os.utime(ROOT / "crates" / crate / "src" / "lib.rs")
            xs.append(timed(build)[0])
        report(f"rebuild after touching {crate}", xs)

    env = {**os.environ, "BMR_EXE": str(ROOT / "target" / "debug" / "bmr")}
    for fixture in filter(None, a.fixtures.split(",")):
        rev, score = [], []
        for _ in range(a.n if fixture != "nether" else 1):
            out = timed([sys.executable, "tools/reverse_fixture.py", "--no-record", fixture], env)[1]
            m = re.search(r"\[reverse ([\d.]+)s, score ([\d.]+)s\]", out)
            rev.append(float(m[1]))
            score.append(float(m[2]))
        report(f"{fixture}: bmr reverse", rev)
        report(f"{fixture}: bmr score", score)

    if a.tests:
        test = ["cargo", "test", "-j", "4", "-q", *cfg]
        timed([*test, "--workspace", "--no-run"])
        os.utime(ROOT / "crates" / "bmr-fill" / "src" / "lib.rs")
        report("tests of bmr-fill + dependents (tools/test.py)", [timed([*test, "-p", "bmr-fill", "-p", "bmr-cli"])[0]])
        report("cargo test --workspace (built)", [timed([*test, "--workspace"])[0]])


if __name__ == "__main__":
    main()
