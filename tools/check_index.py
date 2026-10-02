"""End-to-end check of the pack index: publish packs/ + index.json on a local HTTP server, serve the
superflat fixture with BlueMap, and `bmr pull` it from an empty folder: the right pack must be chosen from
the index, downloaded, verified and used.

Usage: py -3 tools/check_index.py   (needs the superflat fixture rendered: tools/up.py superflat)
"""
import functools
import http.server
import shutil
import subprocess
import sys
import tempfile
import threading
from pathlib import Path

from mirror_fixture import URL, bmr_exe, serving
from paths import DEFAULT, ROOT, WEB_HOST
from render_serve import configure

INDEX_PORT = 8201


def main() -> None:
    packs = ROOT / "packs"
    if subprocess.call([bmr_exe(), "pack", "index", packs], cwd=ROOT):
        sys.exit("pack index failed")
    handler = functools.partial(http.server.SimpleHTTPRequestHandler, directory=str(packs))
    index_server = http.server.ThreadingHTTPServer((WEB_HOST, INDEX_PORT), handler)
    threading.Thread(target=index_server.serve_forever, daemon=True).start()
    empty = Path(tempfile.mkdtemp(prefix="bmr-index-"))
    try:
        with serving(configure("superflat", DEFAULT)):
            pull = subprocess.run(
                [bmr_exe(), "pull", URL, "-o", "out.zip", "--data-dir", empty,
                 "--pack-index", f"http://{WEB_HOST}:{INDEX_PORT}/index.json"],
                cwd=empty, capture_output=True, text=True, encoding="utf-8", errors="replace",
            )
    finally:
        index_server.shutdown()
    print(pull.stdout[-2500:])
    expected = f"bmr-mc{DEFAULT.mc}-bluemap{DEFAULT.bluemap}.pack"
    ok = pull.returncode == 0 and (empty / "packs" / expected).exists() and (empty / "out.zip").exists()
    shutil.rmtree(empty, ignore_errors=True)
    print(f"index pick + download + pull: {'OK' if ok else 'FAILED'} (expected {expected})")
    if not ok:
        sys.exit(pull.stderr[-2000:] or 1)


if __name__ == "__main__":
    main()
