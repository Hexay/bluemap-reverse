"""Render a fixture world with BlueMap CLI and/or serve it on WEB_HOST:WEB_PORT.

Usage: py -3 tools/render_serve.py <fixture> [--no-render] [--no-serve] [--force-render] [--mc 1.21.11]
Layout: <toolchain>/bluemap/<fixture>/{config,data,web}; map id = fixture name.
Map settings are BlueMap defaults (what public maps run) unless fixture.json has a "bluemap" object.
"""
import argparse
import json
import re
import subprocess
import sys
from pathlib import Path

from paths import DEFAULT, FIXTURES, WEB_HOST, WEB_PORT, Toolchain


def set_conf(path: Path, key: str, value: str) -> None:
    text = path.read_text()
    line = f"{key}: {value}"
    new, n = re.subn(rf"^{re.escape(key)}:.*$", line, text, flags=re.M)
    path.write_text(new if n else text.rstrip() + "\n" + line + "\n")


def conf_value(v) -> str:
    return json.dumps(v) if isinstance(v, str) else str(v).lower() if isinstance(v, bool) else str(v)


def bluemap(cwd: Path, *args: str, tc: Toolchain = DEFAULT) -> subprocess.Popen:
    cmd = [str(tc.bluemap_java), "-jar", str(tc.bluemap_jar), "-c", "config", "-v", tc.mc, *args]
    return subprocess.Popen(cmd, cwd=cwd)


def configure(fixture: str, tc: Toolchain = DEFAULT) -> Path:
    world = tc.worlds / fixture / "world"
    if not world.exists():
        sys.exit(f"no world at {world}; run make_world.py {fixture} first")
    base = tc.bluemap_root / fixture
    cfg = base / "config"
    if not (cfg / "core.conf").exists():
        base.mkdir(parents=True, exist_ok=True)
        bluemap(base, tc=tc).wait()
    set_conf(cfg / "core.conf", "accept-download", "true")
    set_conf(cfg / "core.conf", "metrics", "false")
    set_conf(cfg / "webserver.conf", "ip", json.dumps(WEB_HOST))
    set_conf(cfg / "webserver.conf", "port", str(WEB_PORT))

    maps = cfg / "maps"
    template = maps / "overworld.conf"
    target = maps / f"{fixture}.conf"
    if template.exists():
        template.replace(target)
    for other in maps.glob("*.conf"):
        if other != target:
            other.unlink()
    set_conf(target, "world", json.dumps(world.as_posix()))
    set_conf(target, "name", json.dumps(fixture))
    spec = json.loads((FIXTURES / fixture / "fixture.json").read_text())
    for key, value in spec.get("bluemap", {}).items():
        set_conf(target, key, conf_value(value))
    return base


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("fixture")
    ap.add_argument("--no-render", action="store_true")
    ap.add_argument("--no-serve", action="store_true")
    ap.add_argument("--force-render", action="store_true")
    ap.add_argument("--mc", default=DEFAULT.mc)
    ap.add_argument("--bluemap", default=DEFAULT.bluemap)
    args = ap.parse_args()
    from setup import resolve  # lazy: network lookup only for non-default toolchains

    tc = resolve(args.mc, args.bluemap)
    base = configure(args.fixture, tc)
    flags = []
    if not args.no_render:
        flags.append("-r")
        if args.force_render:
            flags.append("-f")
    if not args.no_serve:
        flags.append("-w")
        print(f"serving http://{WEB_HOST}:{WEB_PORT}/ (Ctrl+C to stop)", flush=True)
    if not flags:
        return
    proc = bluemap(base, *flags, tc=tc)
    try:
        sys.exit(proc.wait())
    except KeyboardInterrupt:
        proc.terminate()


if __name__ == "__main__":
    main()
