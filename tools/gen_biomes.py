"""The `biomes` fixture: one 16x16 superflat patch per overworld biome of the toolchain's version (from its
biome_parameters report), each with grass, oak leaves and water at fixed spots, so `bmr pack build` can learn
the grass / foliage / water tint BlueMap draws for every biome. Commands are generated per version (the
biome list differs), so this fixture has no commands.txt.

Usage: py -3 tools/gen_biomes.py [--mc 26.3] [--force]    (makes work/.../worlds/biomes)
"""
import argparse
import json

from make_world import generate, load_fixture
from paths import DEFAULT, Toolchain

PATCH = 16
GROUND = -61  # the superflat grass layer
# offsets inside a patch, >= 4 blocks from its edge so BlueMap's biome blending sees only this biome
LEAVES = (10, 5)
WATER = (5, 10)


def overworld_biomes(tc: Toolchain) -> list[str]:
    path = tc.reports / "reports" / "biome_parameters" / "minecraft" / "overworld.json"
    return sorted({e["biome"] for e in json.loads(path.read_text())["biomes"]})


def patch_origins(n: int, area: list[int]) -> list[tuple[int, int]]:
    x0, z0, x1, _ = area
    per_row = (x1 - x0 + 1) // PATCH
    return [(x0 + (i % per_row) * PATCH, z0 + (i // per_row) * PATCH) for i in range(n)]


def commands(tc: Toolchain, area: list[int]) -> list[str]:
    biomes = overworld_biomes(tc)
    origins = patch_origins(len(biomes), area)
    assert origins[-1][1] + PATCH - 1 <= area[3], f"{len(biomes)} biomes do not fit the fixture area"
    out = []
    for biome, (x, z) in zip(biomes, origins):
        # fillbiome counts blocks against the 32768 limit: 16x16 columns x 128 high is exactly that
        out.append(f"fillbiome {x} -64 {z} {x + PATCH - 1} 63 {z + PATCH - 1} {biome}")
        lx, lz = x + LEAVES[0], z + LEAVES[1]
        out.append(f"fill {lx} {GROUND + 1} {lz} {lx + 1} {GROUND + 1} {lz + 1} minecraft:oak_leaves[persistent=true]")
        wx, wz = x + WATER[0], z + WATER[1]
        out.append(f"fill {wx} {GROUND} {wz} {wx + 1} {GROUND} {wz + 1} minecraft:water")
    return out


def make(tc: Toolchain, force: bool, heap: str = "4G", port: int | None = None) -> None:
    spec, _ = load_fixture("biomes")
    if port is not None:
        spec = {**spec, "properties": {**spec["properties"], "server-port": str(port)}}
    generate(tc.worlds / "biomes", spec, commands(tc, spec["area"]), force, tc, heap)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--mc", default=DEFAULT.mc)
    ap.add_argument("--bluemap", default=DEFAULT.bluemap)
    ap.add_argument("--force", action="store_true")
    args = ap.parse_args()
    from setup import resolve  # lazy: network lookup only for non-default toolchains

    make(resolve(args.mc, args.bluemap), args.force)


if __name__ == "__main__":
    main()
