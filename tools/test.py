"""Run the tests of the crates a change can affect: crates with changed files plus every workspace crate that
depends on them, directly or not. The full `cargo test --workspace` is for acceptance; most of its time is
bmr-seed's cracking tests, which a bmr-fill/bmr-invert edit cannot break.

Usage: python3 tools/test.py [--base REF] [--all] [cargo test args...] [-- test-binary args...]
  changes = working tree vs HEAD (plus untracked files), or vs REF with --base (e.g. --base origin/main)
  a change to the workspace Cargo.toml / Cargo.lock / rust-toolchain tests everything
"""
import json
import subprocess
import sys

from paths import ROOT

GLOBAL = {"Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "rustfmt.toml"}


def git(*args: str) -> list[str]:
    return subprocess.run(["git", *args], capture_output=True, text=True, check=True, cwd=ROOT).stdout.split()


def changed_files(base: str) -> list[str]:
    return git("diff", "--name-only", base) + git("ls-files", "--others", "--exclude-standard")


def workspace() -> dict[str, set[str]]:
    """Crate dir name → names of the workspace crates it depends on (any kind of dependency)."""
    meta = json.loads(subprocess.run(["cargo", "metadata", "--format-version", "1", "--no-deps"], capture_output=True,
                                     text=True, check=True, cwd=ROOT).stdout)
    names = {p["name"] for p in meta["packages"]}
    return {p["name"]: {d["name"] for d in p["dependencies"]} & names for p in meta["packages"]}


def affected(files: list[str], deps: dict[str, set[str]]) -> set[str] | None:
    """Affected crates, or None for everything."""
    if any(f in GLOBAL for f in files):
        return None
    hit = {f.split("/")[1] for f in files if f.startswith("crates/") and f.count("/") >= 2} & deps.keys()
    while True:
        more = {c for c, ds in deps.items() if ds & hit} - hit
        if not more:
            return hit
        hit |= more


def main() -> None:
    args, base, everything = sys.argv[1:], "HEAD", False
    if "--base" in args:
        i = args.index("--base")
        base = args[i + 1]
        del args[i:i + 2]
    if "--all" in args:
        args.remove("--all")
        everything = True
    crates = None if everything else affected(changed_files(base), workspace())
    if crates is not None and not crates:
        print(f"no crate changed vs {base}; nothing to test (--all runs the workspace)")
        return
    select = ["--workspace"] if crates is None else [a for c in sorted(crates) for a in ("-p", c)]
    print("testing:", "workspace" if crates is None else " ".join(sorted(crates)), flush=True)
    sys.exit(subprocess.run(["cargo", "test", "-j", "4", *select, *args], cwd=ROOT).returncode)


if __name__ == "__main__":
    main()
