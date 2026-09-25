"""Generate a fixture world: fresh server run, force-load the fixture area, apply its commands, save, stop.

Usage: py -3 tools/make_world.py <fixture> [--force]
Output: work/worlds/<fixture>/world
"""
import argparse
import json
import shutil
import sys
import time

from console import Server, ServerTimeout
from paths import FIXTURES, WORLDS

BASE_PROPERTIES = {
    "level-name": "world",
    "online-mode": "false",
    "server-port": "25599",
    "spawn-protection": "0",
    "max-players": "1",
    "view-distance": "4",
    "simulation-distance": "4",
}
FORCELOAD_MAX_CHUNKS = 256


def load_fixture(name: str) -> tuple[dict, list[str]]:
    d = FIXTURES / name
    spec = json.loads((d / "fixture.json").read_text())
    cmd_file = d / "commands.txt"
    commands = []
    if cmd_file.exists():
        for line in cmd_file.read_text().splitlines():
            line = line.strip()
            if line and not line.startswith("#"):
                commands.append(line)
    return spec, commands


def write_server_files(server_dir, properties: dict) -> None:
    server_dir.mkdir(parents=True)
    (server_dir / "eula.txt").write_text("eula=true\n")
    props = {**BASE_PROPERTIES, **properties}
    # java.util.Properties treats ':' as a key/value separator unless escaped
    lines = [f"{k}={v.replace(':', chr(92) + ':')}" for k, v in props.items()]
    (server_dir / "server.properties").write_text("\n".join(lines) + "\n")


def forceload_commands(x0: int, z0: int, x1: int, z1: int) -> list[str]:
    cx0, cz0, cx1, cz1 = x0 >> 4, z0 >> 4, x1 >> 4, z1 >> 4
    rows_per_batch = max(1, FORCELOAD_MAX_CHUNKS // (cx1 - cx0 + 1))
    out = []
    for cz in range(cz0, cz1 + 1, rows_per_batch):
        cz_end = min(cz + rows_per_batch - 1, cz1)
        out.append(f"forceload add {cx0 * 16} {cz * 16} {cx1 * 16 + 15} {cz_end * 16 + 15}")
    return out


def wait_until_loaded(server: Server, x0: int, z0: int, x1: int, z1: int, timeout: float = 600) -> None:
    deadline = time.monotonic() + timeout
    for x, z in [(x0, z0), (x0, z1), (x1, z0), (x1, z1)]:
        while True:
            m = server.query(f"execute if loaded {x} 0 {z}", r"Test (passed|failed)")
            if m.group(1) == "passed":
                break
            if time.monotonic() > deadline:
                raise ServerTimeout(f"chunk at {x},{z} never loaded")
            time.sleep(1)


def make_world(name: str, force: bool) -> None:
    spec, commands = load_fixture(name)
    server_dir = WORLDS / name
    if server_dir.exists():
        if not force:
            print(f"exists  {server_dir / 'world'} (use --force to regenerate)")
            return
        shutil.rmtree(server_dir)
    write_server_files(server_dir, spec.get("properties", {}))

    server = Server(server_dir)
    try:
        server.wait_for(r"Done \(", timeout=600, echo=True)
        area = spec["area"]
        for cmd in forceload_commands(*area):
            server.send(cmd)
        wait_until_loaded(server, *area)
        print(f"loaded  area {area}")
        for cmd in commands:
            server.send(cmd)
        server.query("save-all flush", r"Saved the game", timeout=300)
    finally:
        code = server.stop()
    for err in server.errors:
        print(f"ERROR   {err}")
    print(f"world   {server_dir / 'world'} ({len(commands)} commands, {len(server.errors)} errors, exit {code})")
    if server.errors or code != 0:
        sys.exit(1)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("fixture")
    ap.add_argument("--force", action="store_true")
    args = ap.parse_args()
    make_world(args.fixture, args.force)


if __name__ == "__main__":
    main()
