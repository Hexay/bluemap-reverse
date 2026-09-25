"""Compare a bmr fetch mirror against BlueMap's on-disk webroot for a fixture (byte-exact after gunzip).

Usage: py -3 tools/verify_mirror.py <fixture> [mirror_dir=work/cache/<fixture>]
Exit 1 if any rendered file is missing from the mirror or differs.
"""
import gzip
import sys
from pathlib import Path

from paths import BLUEMAP, WORK

CHECKED = ("settings.json", "textures.json", "tiles/")


def main() -> None:
    fixture = sys.argv[1]
    mirror = Path(sys.argv[2]) if len(sys.argv) > 2 else WORK / "cache" / fixture
    src = BLUEMAP / fixture / "web" / "maps" / fixture
    dst = mirror / "maps" / fixture
    missing, differ, ok = [], [], 0
    for f in sorted(src.rglob("*")):
        rel = f.relative_to(src).as_posix()
        if not f.is_file() or not rel.startswith(CHECKED):
            continue
        data = f.read_bytes()
        if rel.endswith(".gz"):
            data, rel = gzip.decompress(data), rel[:-3]
        target = dst / rel
        if not target.exists():
            missing.append(rel)
        elif target.read_bytes() != data:
            differ.append(rel)
        else:
            ok += 1
    for rel in missing:
        print(f"MISSING {rel}")
    for rel in differ:
        print(f"DIFFERS {rel}")
    print(f"{ok} identical, {len(missing)} missing, {len(differ)} differ")
    sys.exit(1 if missing or differ else 0)


if __name__ == "__main__":
    main()
