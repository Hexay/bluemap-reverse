"""End-to-end check of a multi-dimension site: the `dimensions` fixture (one world, overworld + nether + end)
served as BlueMap's three default maps, `bmr pull` without --map must reconstruct all three into one
world. Scores each dimension against the original and loads the output in the server.

Usage: py -3 tools/check_dimensions.py
"""
import json
import shutil
import subprocess
import sys
import zipfile

from make_world import load_fixture, make_world
from mirror_fixture import bmr_exe, wait_until_serving
from paths import DEFAULT, ROOT, WEB_HOST, WEB_PORT
from render_serve import bluemap, configure
from resave_world import resave

FIXTURE = "dimensions"
MAPS = {"overworld": "minecraft:overworld", "nether": "minecraft:the_nether", "end": "minecraft:the_end"}


def main() -> None:
    tc = DEFAULT
    make_world(FIXTURE, False, tc)
    original = tc.worlds / FIXTURE / "world"
    out_dir = tc.work / "out" / "check-dimensions"
    shutil.rmtree(out_dir, ignore_errors=True)
    out_dir.mkdir(parents=True)
    zip_path = out_dir / "pulled.zip"
    base = configure(FIXTURE, tc)
    if bluemap(base, "-r", tc=tc).wait():
        sys.exit("render failed")
    server = bluemap(base, "-w", tc=tc)
    try:
        wait_until_serving()
        pull = subprocess.run(
            [bmr_exe(), "pull", f"http://{WEB_HOST}:{WEB_PORT}/", "-o", zip_path, "--cache", out_dir / "cache", "--offline"],
            cwd=ROOT, capture_output=True, text=True, encoding="utf-8", errors="replace",
        )
    finally:
        server.terminate()
        server.wait(30)
    print(pull.stdout[-3000:])
    if pull.returncode:
        sys.exit(f"pull failed: {pull.stderr[-2000:]}")

    with zipfile.ZipFile(zip_path) as z:
        z.extractall(out_dir / "unzipped")
    pulled = out_dir / "unzipped" / "world"
    ok = True
    for map_id, dim in MAPS.items():
        report = out_dir / f"score-{map_id}.json"
        subprocess.run(
            [bmr_exe(), "score", original, pulled, "--mirror", out_dir / "cache", "--map", map_id,
             "--dimension", dim, "--json", report],
            cwd=ROOT, check=True, stdout=subprocess.DEVNULL,
        )
        r = json.loads(report.read_text())
        occ = 100 * r["occupied"]["exact"] / max(r["occupied"]["total"], 1)
        rend = 100 * r["rendered"]["exact"] / max(r["rendered"]["total"], 1)
        good = r["rendered"]["total"] > 0 and rend > 95
        ok &= good
        print(f"{dim:22} occupied {occ:6.2f}%  rendered {rend:6.2f}%  {'OK' if good else 'FAILED'}")

    spec, _ = load_fixture(FIXTURE)
    a = spec["area"]
    loads = resave(pulled, [a[0] - 32, a[1] - 32, a[2] + 32, a[3] + 32], tc) == 0
    print(f"server loads the output: {'OK' if loads else 'FAILED'}")
    sys.exit(0 if ok and loads else 1)


if __name__ == "__main__":
    main()
