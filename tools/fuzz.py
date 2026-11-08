"""cargo-fuzz targets under fuzz/ (docs/fuzzing.md): regenerate the committed seeds, run targets one at a time.

Usage: python3 tools/fuzz.py seeds [--from CHECKOUT]        (needs local fixtures: work/cache, work/worlds, packs/)
       python3 tools/fuzz.py run [TARGET...] [--seconds N]  (default: every target, 180 s each)
"""
import argparse
import base64
import gzip
import json
import os
import shutil
import struct
import subprocess
import sys
import zlib
from compression import zstd
from pathlib import Path

from paths import ROOT

FUZZ = ROOT / "fuzz"
SEEDS = FUZZ / "seeds"
CORPUS = FUZZ / "corpus"
# cargo-fuzz's default triple is the one it was built for, which may be musl (no sanitizers there)
TRIPLE = "x86_64-unknown-linux-gnu"
# target → libFuzzer -max_len: room to grow the seeds, small enough to keep executions fast
TARGETS = {
    "prbm": 16384,
    "decompress": 4096,
    "lowres": 16384,
    "region": 32768,
    "chunk_nbt": 16384,
    "pack": 8192,
    "pack_index": 8192,
    "textures_json": 8192,
    "texture_png": 4096,
    "settings_json": 2048,
}
SECTOR = 4096


def smallest(root: Path, pattern: str, n: int, min_size: int = 0) -> list[Path]:
    files = [p for p in root.glob(pattern) if p.stat().st_size >= min_size]
    return sorted(files, key=lambda p: (p.stat().st_size, str(p)))[:n]


def region_chunks(path: Path) -> list[tuple[int, bytes]]:
    """(compression type, payload) of every chunk in a region file."""
    data = path.read_bytes()
    out = []
    for slot in range(1024):
        loc = struct.unpack_from(">I", data, slot * 4)[0]
        if loc:
            off = (loc >> 8) * SECTOR
            length = struct.unpack_from(">I", data, off)[0]
            out.append((data[off + 4], data[off + 5:off + 4 + length]))
    return out


def one_chunk_region(kind: int, payload: bytes) -> bytes:
    header = bytearray(2 * SECTOR)
    struct.pack_into(">I", header, 0, (2 << 8) | ((len(payload) + 5 + SECTOR - 1) // SECTOR))
    body = struct.pack(">IB", len(payload) + 1, kind) + payload
    return bytes(header) + body + bytes(-len(body) % SECTOR)


def gather(checkout: Path) -> dict[str, list[bytes]]:
    work = checkout / "work"
    cache = work / "cache"
    seeds: dict[str, list[bytes]] = {t: [] for t in TARGETS}
    seeds["prbm"] = [p.read_bytes() for p in smallest(cache, "*/maps/*/tiles/0/**/*.prbm", 3, min_size=1024)]
    seeds["lowres"] = [p.read_bytes() for p in smallest(cache, "*/maps/*/tiles/[1-9]/**/*.png", 3)]
    settings = [cache / "vanilla" / "settings.json", cache / "vanilla" / "maps" / "vanilla" / "settings.json"]
    seeds["settings_json"] = [p.read_bytes() for p in settings]

    textures = json.loads((cache / "vanilla" / "maps" / "vanilla" / "textures.json").read_bytes())
    small = sorted(textures, key=lambda t: len(t["texture"]))
    animated = [t for t in small if t.get("animation")][:1]
    picked = small[:2] + animated
    seeds["textures_json"] = [json.dumps(picked).encode()]
    seeds["texture_png"] = [base64.b64decode(t["texture"].split(",", 1)[1]) for t in picked]

    sample = settings[1].read_bytes()
    seeds["decompress"] = [bytes([i]) + c for i, c in
                           enumerate([gzip.compress(sample), zlib.compress(sample), zstd.compress(sample)])]

    region = work / "worlds" / "end" / "world" / "dimensions" / "minecraft" / "the_end" / "region" / "r.0.0.mca"
    kind, payload = min(region_chunks(region), key=lambda c: len(c[1]))
    seeds["region"] = [one_chunk_region(kind, payload)]
    seeds["chunk_nbt"] = [zlib.decompress(payload) if kind == 2 else payload]

    index = json.loads((checkout / "packs" / "index.json").read_bytes())
    index["textures"] = index["textures"][:16]
    for e in index["packs"]:
        e["textures"] = base64.b64encode(base64.b64decode(e["textures"])[:2]).decode()
    index["packs"] = index["packs"][:2]
    seeds["pack_index"] = [json.dumps(index).encode()]
    # pack target: input without the pack magic is a raw postcard body (see fuzz_targets/pack.rs)
    seeds["pack"] = [bytes(8)]
    return seeds


def write_seeds(checkout: Path) -> None:
    for target, files in gather(checkout).items():
        d = SEEDS / target
        shutil.rmtree(d, ignore_errors=True)
        d.mkdir(parents=True)
        for i, b in enumerate(files):
            (d / f"seed{i}").write_bytes(b)
        print(f"{target}: {len(files)} seeds, {sum(map(len, files))} bytes")


def run(targets: list[str], seconds: int) -> int:
    env = {**os.environ, "CARGO_BUILD_JOBS": os.environ.get("CARGO_BUILD_JOBS", "2")}
    fuzz = ["cargo", "+nightly", "fuzz"]
    subprocess.run([*fuzz, "build", "--target", TRIPLE], cwd=FUZZ, env=env, check=True)
    failed = []
    for t in targets:
        corpus = CORPUS / t
        corpus.mkdir(parents=True, exist_ok=True)
        args = [f"-max_total_time={seconds}", "-rss_limit_mb=1024", f"-max_len={TARGETS[t]}", "-timeout=10",
                "-print_final_stats=1"]
        print(f"== {t} ({seconds} s)", flush=True)
        r = subprocess.run([*fuzz, "run", "--target", TRIPLE, t, str(corpus), str(SEEDS / t), "--", *args],
                           cwd=FUZZ, env=env, capture_output=True, text=True)
        lines = (r.stdout + r.stderr).splitlines()
        print("\n".join(lines[-12 if r.returncode == 0 else -60:]), flush=True)
        if r.returncode != 0:
            failed.append(t)
    print("failed:", " ".join(failed) if failed else "none")
    return 1 if failed else 0


def main() -> None:
    ap = argparse.ArgumentParser()
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("seeds")
    s.add_argument("--from", dest="checkout", type=Path, default=ROOT,
                   help="checkout whose work/ and packs/ hold the fixtures (a worktree has none)")
    r = sub.add_parser("run")
    r.add_argument("targets", nargs="*", metavar="TARGET", help=" ".join(TARGETS))
    r.add_argument("--seconds", type=int, default=180)
    a = ap.parse_args()
    if unknown := set(getattr(a, "targets", [])) - TARGETS.keys():
        ap.error(f"unknown targets {sorted(unknown)}")
    if a.cmd == "seeds":
        write_seeds(a.checkout)
    else:
        sys.exit(run(a.targets or list(TARGETS), a.seconds))


if __name__ == "__main__":
    main()
