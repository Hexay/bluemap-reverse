"""Open an existing world in a headless server of the given version, force-load an area, save, stop.
Proves the server accepts our chunks (a regenerated chunk would come back as template void).

Usage: py -3 tools/resave_world.py <world_dir> --area=x0,z0,x1,z1 [--mc 1.21.11]
The server runs in the world's parent dir with level-name = the world folder name.
"""
import argparse
import sys
from pathlib import Path

from console import Server
from make_world import BASE_PROPERTIES, OVERWORLD, forceload_commands, wait_until_loaded
from paths import DEFAULT, Toolchain


def resave(world: Path, area: list[int], tc: Toolchain = DEFAULT, dimensions: list[str] = (OVERWORLD,)) -> int:
    """Force-loads `area` in each of `dimensions`."""
    server_dir = world.parent
    (server_dir / "eula.txt").write_text("eula=true\n")
    props = {**BASE_PROPERTIES, "level-name": world.name}
    (server_dir / "server.properties").write_text("".join(f"{k}={v}\n" for k, v in props.items()))
    server = Server(server_dir, tc=tc)
    try:
        server.wait_for(r"Done \(", timeout=600)
        for dim in dimensions:
            for cmd in forceload_commands(*area, dim):
                server.send(cmd)
            wait_until_loaded(server, *area, dim)
        server.query("save-all flush", r"Saved the game", timeout=300)
    finally:
        code = server.stop()
    for err in server.errors:
        print(f"ERROR   {err}")
    print(f"resaved {world} ({len(server.errors)} errors, exit {code})")
    return 1 if server.errors or code else 0


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("world", type=Path)
    # an option, not positional: argparse reads a leading "-96" as a flag (pass --area=-96,...)
    ap.add_argument("--area", required=True, help="x0,z0,x1,z1 block coords to force-load")
    ap.add_argument("--mc", default=DEFAULT.mc)
    ap.add_argument("--bluemap", default=DEFAULT.bluemap)
    args = ap.parse_args()
    from setup import resolve  # lazy: network lookup only for non-default toolchains

    tc = resolve(args.mc, args.bluemap)
    sys.exit(resave(args.world.resolve(), [int(v) for v in args.area.split(",")], tc))


if __name__ == "__main__":
    main()
