"""Sample-profile a command with samply (ETW; needs the Windows Performance Toolkit and admin → one UAC
prompt per run), then print the hottest functions by self and inclusive time.

Usage: py -3 tools/profile.py <name> [--clean DIR] [--rate HZ] -- <command...>
       py -3 tools/profile.py <name> --analyze-only       (re-analyze work/prof/<name>.json.gz)
Output: work/prof/<name>.json.gz (+ .syms.json); open in https://profiler.firefox.com for flame graphs.
"""
import argparse
import bisect
import ctypes
import gzip
import json
import re
import shutil
import subprocess
import sys
from collections import Counter
from ctypes import wintypes
from pathlib import Path

from paths import ROOT, WORK

SAMPLY = Path.home() / ".cargo" / "bin" / "samply.exe"
PROF = WORK / "prof"


class ShellExecuteInfo(ctypes.Structure):
    _fields_ = [
        ("cbSize", wintypes.DWORD), ("fMask", ctypes.c_ulong), ("hwnd", wintypes.HWND),
        ("lpVerb", wintypes.LPCWSTR), ("lpFile", wintypes.LPCWSTR), ("lpParameters", wintypes.LPCWSTR),
        ("lpDirectory", wintypes.LPCWSTR), ("nShow", ctypes.c_int), ("hInstApp", wintypes.HINSTANCE),
        ("lpIDList", ctypes.c_void_p), ("lpClass", wintypes.LPCWSTR), ("hkeyClass", wintypes.HKEY),
        ("dwHotKey", wintypes.DWORD), ("hIconOrMonitor", wintypes.HANDLE), ("hProcess", wintypes.HANDLE),
    ]


def run_elevated(exe: Path, args: list[str], cwd: Path) -> int:
    ctypes.windll.shell32.ShellExecuteExW.argtypes = [ctypes.POINTER(ShellExecuteInfo)]
    ctypes.windll.shell32.ShellExecuteExW.restype = wintypes.BOOL
    info = ShellExecuteInfo()
    info.cbSize = ctypes.sizeof(info)
    info.fMask = 0x40  # SEE_MASK_NOCLOSEPROCESS
    info.lpVerb, info.lpFile = "runas", str(exe)
    info.lpParameters = subprocess.list2cmdline(args)
    info.lpDirectory, info.nShow = str(cwd), 0
    if not ctypes.windll.shell32.ShellExecuteExW(ctypes.byref(info)):
        err = ctypes.GetLastError()
        raise SystemExit("elevation declined (UAC)" if err == 1223 else f"ShellExecuteExW failed, error {err}")
    ctypes.windll.kernel32.WaitForSingleObject(info.hProcess, 0xFFFFFFFF)
    code = wintypes.DWORD()
    ctypes.windll.kernel32.GetExitCodeProcess(info.hProcess, ctypes.byref(code))
    return code.value


def build_profiling() -> None:
    """Profiling build of bmr with an MSVC linker map for symbolication (flag on the final crate only:
    RUSTFLAGS would hit every build script and they'd all fight over one map file)."""
    cargo = Path.home() / ".cargo" / "bin" / "cargo.exe"
    map_arg = f"link-arg=/MAP:{ROOT / 'target' / 'profiling' / 'bmr.map'}"
    subprocess.run([str(cargo), "rustc", "-q", "--profile", "profiling", "-p", "bmr-cli", "--bin", "bmr", "--", "-C", map_arg],
                   check=True, cwd=ROOT)


def record(name: str, cmd: list[str], rate: int) -> Path:
    PROF.mkdir(parents=True, exist_ok=True)
    out = PROF / f"{name}.json.gz"
    exe = ROOT / cmd[0] if (ROOT / cmd[0]).exists() else Path(cmd[0])
    args = ["record", "--save-only", "--unstable-presymbolicate", "--rate", str(rate), "-o", str(out), "--", str(exe), *cmd[1:]]
    code = run_elevated(SAMPLY, args, ROOT)
    if code or not out.exists():
        raise SystemExit(f"samply failed (exit {code})")
    return out


class Symbols:
    """RVA → symbol per module, from samply's --unstable-presymbolicate sidecar."""

    def __init__(self, syms_path: Path):
        s = json.loads(syms_path.read_text())
        strings = s["string_table"]
        self.by_lib = {}
        for d in s["data"]:
            table = sorted((e["rva"], e["size"], strings[e["symbol"]]) for e in d["symbol_table"])
            self.by_lib[d["debug_name"].lower()] = table

    def resolve(self, debug_name: str, rva: int) -> str | None:
        table = self.by_lib.get(debug_name.lower())
        if not table:
            return None
        lo, hi = 0, len(table)
        while lo < hi:
            mid = (lo + hi) // 2
            if table[mid][0] <= rva:
                lo = mid + 1
            else:
                hi = mid
        if lo and table[lo - 1][0] <= rva < table[lo - 1][0] + table[lo - 1][1]:
            return table[lo - 1][2]
        return None


class MapSymbols:
    """RVA → demangled function from an MSVC linker map (`-C link-arg=/MAP:...`). samply cannot read
    our Rust PDB symbols, so the profiled exe is built with a map (see build_profiling in main)."""

    LINE = re.compile(r"^\s*[0-9a-f]{4}:[0-9a-f]{8}\s+(\S+)\s+([0-9a-f]{16})", re.I)

    def __init__(self, map_path: Path):
        text = map_path.read_text(errors="replace")
        base = int(re.search(r"Preferred load address is ([0-9a-f]+)", text, re.I).group(1), 16)
        rows = []
        for line in text.splitlines():
            m = self.LINE.match(line)
            if m and int(m.group(2), 16) >= base:
                rows.append((int(m.group(2), 16) - base, m.group(1)))
        rows.sort()
        self.rvas = [r for r, _ in rows]
        self.names = [n for _, n in rows]

    def resolve(self, rva: int) -> str | None:
        i = bisect.bisect_right(self.rvas, rva) - 1
        return demangle(self.names[i]) if i >= 0 else None


RUST_ESCAPES = {"$LT$": "<", "$GT$": ">", "$RF$": "&", "$BP$": "*", "$C$": ",", "$SP$": "@", "$u20$": " ",
                "$u27$": "'", "$u5b$": "[", "$u5d$": "]", "$u7b$": "{", "$u7d$": "}", "$u3b$": ";",
                "$u2b$": "+", "$u22$": '"', "$u7e$": "~", "..": "::"}


def demangle(sym: str) -> str:
    """Rust legacy (`_ZN…E`) exactly; v0 (`_R…`) approximately: the path identifiers only, crate
    disambiguators and generic structure dropped (enough to read a profile)."""
    s = sym.lstrip("_")
    if s.startswith("R"):
        return demangle_v0(s)
    if not s.startswith("ZN"):
        return sym
    i, parts = 2, []
    while i < len(s) and s[i].isdigit():
        j = i
        while s[j].isdigit():
            j += 1
        n = int(s[i:j])
        parts.append(s[j:j + n])
        i = j + n
    if parts and re.fullmatch(r"h[0-9a-f]{16}", parts[-1]):
        parts.pop()
    out = "::".join(parts)
    for k, v in RUST_ESCAPES.items():
        out = out.replace(k, v)
    return out


def demangle_v0(s: str) -> str:
    idents, i = [], 1
    while i < len(s):
        m = re.match(r"(\d+)_?", s[i:])
        if not m:
            i += 1
            continue
        n, start = int(m.group(1)), i + m.end()
        ident = s[start:start + n]
        if n and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", ident):
            idents.append(ident)
            i = start + n
        else:
            i += 1
    # drop noise identifiers from std generics plumbing, keep order
    keep = [x for x in idents if x not in ("core", "alloc", "std")]
    return "::".join(keep[-6:]) or s


def analyze(profile: Path, top: int, process: str, map_path: Path | None) -> None:
    p = json.load(gzip.open(profile))
    syms = Symbols(profile.with_suffix("").with_suffix(".json.syms.json"))
    exe_map = MapSymbols(map_path) if map_path and map_path.exists() else None
    libs = p["libs"]
    self_w, incl_w, thread_w = Counter(), Counter(), Counter()
    total = 0
    for t in p["threads"]:
        if t.get("processName") != process:
            continue
        ft, st, rt, strings = t["frameTable"], t["stackTable"], t["resourceTable"], t["stringArray"]
        func_res = t["funcTable"]["resource"]

        def frame_name(f: int) -> str:
            res = func_res[ft["func"][f]]
            if res < 0:
                return "?"
            lib = libs[rt["lib"][res]]
            rva = ft["address"][f]
            name = None
            if exe_map and lib["name"].lower() == process.lower():
                name = exe_map.resolve(rva)
            name = name or syms.resolve(lib["debugName"], rva)
            return f"{lib['name']}!{name}" if name else f"{lib['name']}!0x{rva:x}"

        names = [frame_name(f) for f in range(ft["length"])]
        stacks: dict[int, list[str]] = {}

        def stack_names(s: int) -> list[str]:
            if s not in stacks:
                out, cur = [], s
                while cur is not None:
                    out.append(names[st["frame"][cur]])
                    cur = st["prefix"][cur]
                stacks[s] = out
            return stacks[s]

        # CPU used since the previous sample, so idle/waiting threads weigh nothing
        weights = t["samples"].get("threadCPUDelta") or t["samples"].get("weight") or [1] * t["samples"]["length"]
        for s, w in zip(t["samples"]["stack"], weights):
            if s is None or not w:
                continue
            frames = stack_names(s)
            total += w
            thread_w[t["name"]] += w
            self_w[frames[0]] += w
            for n in set(frames):
                incl_w[n] += w

    print(f"CPU-weighted samples: {total} in {process} ({len(thread_w)} threads; main thread {100 * thread_w.get(process, 0) / max(total, 1):.0f}% of CPU)")
    for title, counter in (("self", self_w), ("inclusive", incl_w)):
        print(f"\ntop {top} by {title} time:")
        for name, w in counter.most_common(top):
            print(f"  {100 * w / total:5.1f}%  {name[:150]}")


def main() -> None:
    argv = sys.argv[1:]
    split = argv.index("--") if "--" in argv else len(argv)
    ap = argparse.ArgumentParser()
    ap.add_argument("name")
    ap.add_argument("--clean")
    ap.add_argument("--rate", type=int, default=1000)
    ap.add_argument("--top", type=int, default=30)
    ap.add_argument("--process", default="bmr.exe")
    ap.add_argument("--analyze-only", action="store_true")
    ap.add_argument("--build", action="store_true", help="rebuild target/profiling/bmr.exe (+ map) first")
    ap.add_argument("--map", type=Path, default=ROOT / "target" / "profiling" / "bmr.map",
                    help="linker map of the profiled exe (build: RUSTFLAGS='-C link-arg=/MAP:<path>' cargo build --profile profiling)")
    args = ap.parse_args(argv[:split])
    out = PROF / f"{args.name}.json.gz"
    if args.build:
        build_profiling()
    if not args.analyze_only:
        if args.clean:
            shutil.rmtree(ROOT / args.clean, ignore_errors=True)
        out = record(args.name, argv[split + 1:], args.rate)
    analyze(out, args.top, args.process, args.map)


if __name__ == "__main__":
    main()
