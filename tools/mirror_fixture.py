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
import zipfile
from contextlib import contextmanager
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


@contextmanager
def serving(base: Path, tc: Toolchain = DEFAULT):
    """BlueMap's webserver for the BlueMap dir `base`, up for the duration of the `with` block."""
    server = bluemap(base, "-w", tc=tc)
    try:
        wait_until_serving()
        yield
    finally:
        server.terminate()
        server.wait(30)


def render(fixture: str, tc: Toolchain = DEFAULT, force: bool = False, world: Path | None = None,
           name: str | None = None) -> Path:
    """Configure + render `fixture` (or `world` with its map config, see render_serve.configure); returns the
    BlueMap dir. Exits on a failed render."""
    base = configure(fixture, tc, world, name)
    if bluemap(base, *(["-r", "-f"] if force else ["-r"]), tc=tc).wait():
        sys.exit("render failed")
    return base


def mirror(fixture: str, tc: Toolchain = DEFAULT, force_render: bool = False, world: Path | None = None,
           name: str | None = None) -> int:
    """Render + serve + fetch; returns bmr's exit code. Output: tc.cache / (name or fixture)."""
    base = render(fixture, tc, force_render, world, name)
    out = tc.cache / (name or fixture)
    shutil.rmtree(out, ignore_errors=True)
    with serving(base, tc):
        return subprocess.call([bmr_exe(), "fetch", URL, "--out", out, "--concurrency", "8", "--delay-ms", "0"], cwd=ROOT)


def pull_fixture(fixture: str, tc: Toolchain, out_dir: Path) -> tuple[Path, Path, str]:
    """Render + serve `fixture`, `bmr pull --offline` it into a fresh `out_dir` (data dir: mirror in
    out_dir/cache/<site>) and unzip it. Prints the pull's output tail, exits on a failed pull.
    Returns (pulled world, mirror, pull stdout)."""
    shutil.rmtree(out_dir, ignore_errors=True)
    out_dir.mkdir(parents=True)
    zip_path = out_dir / "pulled.zip"
    base = render(fixture, tc)
    with serving(base, tc):
        pull = subprocess.run(
            [bmr_exe(), "pull", URL, "-o", zip_path, "--data-dir", out_dir, "--offline"],
            cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace",
        )
    print(pull.stdout[-3000:])
    if pull.returncode:
        sys.exit(f"pull failed: {pull.stderr[-2000:]}")
    with zipfile.ZipFile(zip_path) as z:
        z.extractall(out_dir / "unzipped")
    mirror_dir = next(p for p in (out_dir / "cache").iterdir() if p.is_dir())
    return out_dir / "unzipped" / "world", mirror_dir, pull.stdout


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
