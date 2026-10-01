"""Run RAM-heavy fixture work on the `testbox` ssh host (Linux, 31 GB) instead of this machine.

Usage: py -3 tools/testbox.py sync                 # working tree (minus target/, work/, .git) → ~/bluemap_reverse
       py -3 tools/testbox.py run "<command>"      # run in the remote checkout, with cargo on PATH
       py -3 tools/testbox.py fetch <remote path> <local path>   # copy a file back (e.g. a truth json)
First time: `run "bash tools/testbox_setup.sh"` (rustup, JDK, server, BlueMap, library fixtures).
"""
import io
import subprocess
import sys
import tarfile

from paths import ROOT

HOST = "testbox"
REMOTE = "bluemap_reverse"
SKIP_DIRS = {".git", "target", "work", "__pycache__", "research"}


def sync() -> None:
    buf = io.BytesIO()
    with tarfile.open(fileobj=buf, mode="w:gz") as tar:
        for path in ROOT.rglob("*"):
            rel = path.relative_to(ROOT)
            if path.is_file() and not SKIP_DIRS.intersection(rel.parts):
                tar.add(path, arcname=rel.as_posix())
    data = buf.getvalue()
    subprocess.run(["ssh", HOST, f"mkdir -p {REMOTE} && tar -xzf - -C {REMOTE}"], input=data, check=True)
    print(f"synced  {len(data) // 1024} KiB -> {HOST}:~/{REMOTE}")


def run(command: str) -> int:
    return subprocess.call(["ssh", HOST, f"cd {REMOTE} && . $HOME/.cargo/env 2>/dev/null; {command}"])


def fetch(remote: str, local: str) -> None:
    data = subprocess.run(["ssh", HOST, f"cat {REMOTE}/{remote}"], capture_output=True, check=True).stdout
    (ROOT / local).write_bytes(data)
    print(f"fetched {remote} -> {local} ({len(data)} bytes)")


def main() -> None:
    cmd = sys.argv[1]
    if cmd == "sync":
        sync()
    elif cmd == "run":
        sys.exit(run(sys.argv[2]))
    elif cmd == "fetch":
        fetch(sys.argv[2], sys.argv[3])
    else:
        sys.exit(__doc__)


if __name__ == "__main__":
    main()
