"""Regenerate the data of the `cargo test` accuracy gate (crates/bmr-cli/tests/accuracy_gate.rs): a crop of a
fixture's mirror and of its truth region, gzipped, under crates/bmr-cli/tests/accuracy-gate. Needs the fixture
mirrored (tools/up.py, tools/mirror_fixture.py). Re-measure the floors in accuracy_gate.rs after regenerating.

textures.json keeps only names: reverse and score never read the images, which are ~95% of its size.

Usage: python3 tools/gen_accuracy_gate.py [--fixture context] [--tiles X0,Z0,X1,Z1]   (hires tiles, inclusive)
"""
import argparse
import gzip
import json
import shutil
import struct
from pathlib import Path

from paths import DEFAULT, ROOT

OUT = ROOT / "crates" / "bmr-cli" / "tests" / "accuracy-gate"
SECTOR = 4096
EXTERNAL_CHUNK = 0x80


def write_gz(path: Path, data: bytes) -> None:
    path = path.with_name(path.name + ".gz")
    path.parent.mkdir(parents=True, exist_ok=True)
    # mtime=0: regenerating from the same inputs gives identical files
    path.write_bytes(gzip.compress(data, 9, mtime=0))


def tile_rel(tile: tuple[int, int]) -> str:
    def split(n: int) -> str:
        return ("-" if n < 0 else "") + "".join(f"{c}/" for c in str(abs(n)))
    return f"tiles/0/x{split(tile[0])}z{split(tile[1])[:-1]}.prbm"


def crop_mirror(source: Path, map_id: str, tiles: list[tuple[int, int]], out: Path) -> dict:
    write_gz(out / "settings.json", (source / "settings.json").read_bytes())
    map_dir = source / "maps" / map_id
    settings = (map_dir / "settings.json").read_bytes()
    write_gz(out / "maps" / map_id / "settings.json", settings)
    names = [{"resourcePath": t["resourcePath"]} for t in json.loads((map_dir / "textures.json").read_bytes())]
    write_gz(out / "maps" / map_id / "textures.json", json.dumps(names).encode())
    for t in tiles:
        write_gz(out / "maps" / map_id / tile_rel(t), (map_dir / tile_rel(t)).read_bytes())
    manifest = {"layers": {"0": {"present": [list(t) for t in sorted(tiles)], "empty": []}}}
    write_gz(out / "bmr-manifest" / f"{map_id}.json", json.dumps(manifest).encode())
    return json.loads(settings)["hires"]


def tile_chunks(tiles: list[tuple[int, int]], hires: dict) -> set[tuple[int, int]]:
    (w, h), (ox, oz) = hires["tileSize"], hires["translate"]
    chunks = set()
    for tx, tz in tiles:
        x0, z0 = tx * w + ox, tz * h + oz
        chunks |= {(cx, cz) for cx in range(x0 >> 4, ((x0 + w - 1) >> 4) + 1)
                   for cz in range(z0 >> 4, ((z0 + h - 1) >> 4) + 1)}
    return chunks


def crop_region(source: Path, keep: set[tuple[int, int]]) -> bytes | None:
    """Region file holding only the `keep` chunks (world chunk coordinates) of `source`, or None if it has none."""
    data = source.read_bytes()
    rx, rz = (int(p) for p in source.name.split(".")[1:3])
    offsets = bytearray(SECTOR)
    timestamps = bytearray(SECTOR)
    body = bytearray()
    for i in range(1024):
        if (rx * 32 + i % 32, rz * 32 + i // 32) not in keep:
            continue
        entry = struct.unpack_from(">I", data, i * 4)[0]
        if not entry:
            continue
        start = (entry >> 8) * SECTOR
        length, compression = struct.unpack_from(">IB", data, start)
        if compression & EXTERNAL_CHUNK:
            raise SystemExit(f"{source}: chunk {i} is stored externally; pick other tiles")
        chunk = data[start:start + 4 + length]
        sectors = (len(chunk) + SECTOR - 1) // SECTOR
        struct.pack_into(">I", offsets, i * 4, (2 + len(body) // SECTOR) << 8 | sectors)
        timestamps[i * 4:i * 4 + 4] = data[SECTOR + i * 4:SECTOR + i * 4 + 4]
        body += chunk + bytes(sectors * SECTOR - len(chunk))
    return bytes(offsets + timestamps + body) if body else None


def crop_world(world: Path, keep: set[tuple[int, int]], out: Path) -> None:
    rel = Path("dimensions/minecraft/overworld/region")
    for mca in sorted((world / rel).glob("r.*.mca")):
        cropped = crop_region(mca, keep)
        if cropped:
            write_gz(out / rel / mca.name, cropped)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--fixture", default="context")
    ap.add_argument("--tiles", default="-1,-2,0,-1")
    a = ap.parse_args()
    x0, z0, x1, z1 = (int(v) for v in a.tiles.split(","))
    tiles = [(x, z) for x in range(x0, x1 + 1) for z in range(z0, z1 + 1)]
    shutil.rmtree(OUT, ignore_errors=True)
    hires = crop_mirror(DEFAULT.cache / a.fixture, a.fixture, tiles, OUT / "mirror")
    crop_world(DEFAULT.worlds / a.fixture / "world", tile_chunks(tiles, hires), OUT / "world")
    files = [p for p in OUT.rglob("*") if p.is_file()]
    print(f"{len(files)} files, {sum(p.stat().st_size for p in files) / 1024:.0f} KB -> {OUT}")


if __name__ == "__main__":
    main()
