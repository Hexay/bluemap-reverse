"""Regenerate untouched terrain for a mirrored map from its seed (same MC version), covering the rendered area.
bmr reverse --regen uses it for everything the tiles do not show (caves, ore blobs, deep floors, biomes).

Usage: py -3 tools/regen_world.py <mirror_name> --seed <seed> [--level-type minecraft:normal] [--force]
Output: work/worlds/regen-<mirror_name>/world
"""
import argparse
import json

from make_world import generate
from paths import WORK, WORLDS


def rendered_area(mirror: str) -> list[int]:
    """Block bbox [x0, z0, x1, z1] of the mirror's hires tiles (single-map mirrors)."""
    root = WORK / "cache" / mirror
    site = json.loads((root / "settings.json").read_text())
    (map_id,) = site["maps"]
    settings = json.loads((root / site.get("mapDataRoot", "maps") / map_id / "settings.json").read_text())
    manifest = json.loads((root / "bmr-manifest" / f"{map_id}.json").read_text())
    tiles = manifest["layers"]["0"]["present"]
    (sx, sz), (tx, tz) = settings["hires"]["tileSize"], settings["hires"]["translate"]
    xs = [t[0] for t in tiles]
    zs = [t[1] for t in tiles]
    return [min(xs) * sx + tx, min(zs) * sz + tz, (max(xs) + 1) * sx + tx - 1, (max(zs) + 1) * sz + tz - 1]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("mirror")
    ap.add_argument("--seed", required=True)
    ap.add_argument("--level-type", default="minecraft:normal")
    ap.add_argument("--force", action="store_true")
    args = ap.parse_args()
    spec = {
        "properties": {"level-type": args.level_type, "level-seed": args.seed, "generate-structures": "true"},
        "area": rendered_area(args.mirror),
    }
    generate(WORLDS / f"regen-{args.mirror}", spec, [], args.force)


if __name__ == "__main__":
    main()
