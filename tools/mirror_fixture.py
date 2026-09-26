"""Render a fixture with BlueMap, serve it, mirror it with `bmr fetch` into <toolchain>/cache/<fixture>,
stop serving. The mirror is taken over HTTP on purpose: it is exactly what a public site would give us.

Usage: py -3 tools/mirror_fixture.py <fixture> [--force-render] [--mc 1.21.11 [--bluemap 5.27]]
"""
import argparse
import os
import shutil
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

from paths import DEFAULT, EXE, ROOT, WEB_HOST, WEB_PORT, Toolchain
from render_serve import bluemap, configure

URL = f"http://{WEB_HOST}:{WEB_PORT}/"


def bmr_exe():
    """$BMR_EXE if set (a build in another target dir), else the release build if present, else debug."""
    if os.environ.get("BMR_EXE"):
        return Path(os.environ["BMR_EXE"])
    release = ROOT / "target" / "release" / f"bmr{EXE}"
    return release if release.exists() else ROOT / "target" / "debug" / f"bmr{EXE}"


def wait_until_serving(timeout: float = 120) -> None:
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        try:
            urllib.request.urlopen(URL + "settings.json", timeout=2)
            return
        except OSError:
            time.sleep(0.5)
    sys.exit(f"BlueMap webserver did not come up on {URL}")


def mirror(fixture: str, tc: Toolchain = DEFAULT, force_render: bool = False) -> int:
    """Render + serve + fetch; returns bmr's exit code. Output: tc.cache / fixture."""
    base = configure(fixture, tc)
    render = ["-r", "-f"] if force_render else ["-r"]
    if bluemap(base, *render, tc=tc).wait():
        sys.exit("render failed")
    out = tc.cache / fixture
    shutil.rmtree(out, ignore_errors=True)
    server = bluemap(base, "-w", tc=tc)
    try:
        wait_until_serving()
        return subprocess.call([bmr_exe(), "fetch", URL, "--out", out, "--concurrency", "8"], cwd=ROOT)
    finally:
        server.terminate()
        server.wait(30)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("fixture")
    ap.add_argument("--force-render", action="store_true")
    ap.add_argument("--mc", default=DEFAULT.mc)
    ap.add_argument("--bluemap", default=DEFAULT.bluemap)
    args = ap.parse_args()
    from setup import resolve  # lazy: network lookup only for non-default toolchains

    sys.exit(mirror(args.fixture, resolve(args.mc, args.bluemap), args.force_render))


if __name__ == "__main__":
    main()
