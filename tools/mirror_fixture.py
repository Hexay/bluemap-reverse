"""Render a fixture with BlueMap, serve it, mirror it with `bmr fetch` into work/cache/<fixture>, stop serving.
The mirror is taken over HTTP on purpose: it is exactly what a public site would give us.

Usage: py -3 tools/mirror_fixture.py <fixture> [--force-render]
"""
import argparse
import shutil
import subprocess
import sys
import time
import urllib.request

from paths import ROOT, WEB_HOST, WEB_PORT, WORK
from render_serve import bluemap, configure

BMR = ROOT / "target" / "debug" / "bmr.exe"
URL = f"http://{WEB_HOST}:{WEB_PORT}/"


def wait_until_serving(timeout: float = 120) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            urllib.request.urlopen(URL + "settings.json", timeout=2)
            return
        except OSError:
            time.sleep(0.5)
    sys.exit(f"BlueMap webserver did not come up on {URL}")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("fixture")
    ap.add_argument("--force-render", action="store_true")
    args = ap.parse_args()

    base = configure(args.fixture)
    render = ["-r", "-f"] if args.force_render else ["-r"]
    if bluemap(base, *render).wait():
        sys.exit("render failed")

    out = WORK / "cache" / args.fixture
    shutil.rmtree(out, ignore_errors=True)
    server = bluemap(base, "-w")
    try:
        wait_until_serving()
        code = subprocess.call([BMR, "fetch", URL, "--out", out, "--concurrency", "8"], cwd=ROOT)
    finally:
        server.terminate()
        server.wait(30)
    sys.exit(code)


if __name__ == "__main__":
    main()
