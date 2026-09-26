"""Build a bmr pack for any Minecraft + BlueMap version, unattended: download the toolchain (right Java
from Mojang's metadata), generate reports, the debug and void-template worlds, render + mirror the debug
world with that BlueMap, then `bmr pack build`. Resumable: finished steps are skipped (--force redoes worlds).

Usage: py -3 tools/build_pack.py --mc 1.21.11 [--bluemap 5.27] [--force]
Output: packs/bmr-mc<mc>-bluemap<bluemap>.pack
Supports Minecraft 1.18+ (older chunk formats are not read).
"""
import argparse
import subprocess
import sys
import time
from concurrent.futures import ThreadPoolExecutor
from contextlib import contextmanager

from make_world import make_world
from mirror_fixture import bmr_exe, mirror
from paths import DEFAULT, ROOT
from setup import resolve, setup

TIMES: list[tuple[str, float]] = []


@contextmanager
def stage(name: str):
    print(f"== {name}", flush=True)
    t = time.monotonic()
    yield
    TIMES.append((name, time.monotonic() - t))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--mc", required=True)
    ap.add_argument("--bluemap", default=DEFAULT.bluemap)
    ap.add_argument("--force", action="store_true", help="regenerate worlds and re-render")
    args = ap.parse_args()
    t0 = time.monotonic()

    with stage("toolchain"):
        tc = resolve(args.mc, args.bluemap)
        print(f"Minecraft {tc.mc}, BlueMap {tc.bluemap}, Java {tc.java_major} -> {tc.work}")
        setup(tc)

    with stage("worlds"), ThreadPoolExecutor() as pool:
        worlds = [pool.submit(make_world, name, args.force, tc, "2G", port)
                  for name, port in [("debug", 25601), ("template-void", 25602)]]
        for w in worlds:
            w.result()

    with stage("render + mirror"):
        mirrored = tc.cache / "debug" / "settings.json"
        if args.force or not mirrored.exists():
            if mirror("debug", tc, force_render=args.force):
                sys.exit("mirroring the debug world failed")
        else:
            print(f"ok      {mirrored.parent}")

    with stage("pack"):
        code = subprocess.call(
            [
                bmr_exe(), "pack", "build",
                "--mc-version", tc.mc,
                "--library-mirror", tc.cache / "debug",
                "--library-world", tc.worlds / "debug" / "world",
                "--template", tc.worlds / "template-void" / "world",
                "--blocks", tc.blocks_json,
                "--out", tc.pack,
            ],
            cwd=ROOT,
        )
        if code:
            sys.exit(code)
    print("  ".join(f"{n} {s:.1f}s" for n, s in TIMES))
    print(f"done in {time.monotonic() - t0:.0f}s -> {tc.pack}")


if __name__ == "__main__":
    main()
