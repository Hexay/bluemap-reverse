"""One command from nothing to a live local map: setup → make_world → render + serve.

Usage: py -3 tools/up.py [fixture=superflat] [--force]   (--force regenerates the world and re-renders)
"""
import argparse
import subprocess
import sys
from pathlib import Path

TOOLS = Path(__file__).resolve().parent


def run(script: str, *args: str) -> None:
    code = subprocess.call([sys.executable, str(TOOLS / script), *args])
    if code:
        sys.exit(code)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("fixture", nargs="?", default="superflat")
    ap.add_argument("--force", action="store_true")
    args = ap.parse_args()
    force = ["--force"] if args.force else []
    run("setup.py")
    run("make_world.py", args.fixture, *force)
    run("render_serve.py", args.fixture, *(["--force-render"] if args.force else []))


if __name__ == "__main__":
    main()
