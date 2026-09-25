"""Independent Sponge Schematic v3 validator (stdlib NBT parser, shares nothing with bmr's fastnbt writer).
Checks tag types and structure against the spec and that Blocks/Biomes data decode to W*H*L varints.

Usage: py -3 tools/check_schem.py <file.schem> [--show N]   (prints the N most common block states)
"""
import argparse
import gzip
import struct
import sys
from collections import Counter

TAGS = {1: "Byte", 2: "Short", 3: "Int", 4: "Long", 5: "Float", 6: "Double", 7: "ByteArray", 8: "String",
        9: "List", 10: "Compound", 11: "IntArray", 12: "LongArray"}


class Reader:
    def __init__(self, data: bytes):
        self.d, self.i = data, 0

    def take(self, n: int) -> bytes:
        b = self.d[self.i:self.i + n]
        self.i += n
        return b

    def unpack(self, fmt: str):
        return struct.unpack(">" + fmt, self.take(struct.calcsize(">" + fmt)))[0]

    def string(self) -> str:
        return self.take(self.unpack("H")).decode("utf-8")

    def payload(self, tag: int):
        """(type name, value)"""
        name = TAGS[tag]
        if tag in (1, 2, 3, 4, 5, 6):
            return name, self.unpack("bhiqfd"[tag - 1])
        if tag == 7:
            return name, self.take(self.unpack("i"))
        if tag == 8:
            return name, self.string()
        if tag == 9:
            elem, n = self.unpack("b"), self.unpack("i")
            return name, [self.payload(elem) for _ in range(n)]
        if tag == 10:
            out = {}
            while (t := self.unpack("b")) != 0:
                key = self.string()
                out[key] = self.payload(t)
            return name, out
        if tag in (11, 12):
            n = self.unpack("i")
            return name, [self.unpack("i" if tag == 11 else "q") for _ in range(n)]
        raise ValueError(f"unknown tag {tag}")


def varints(data: bytes) -> list[int]:
    out, v, shift = [], 0, 0
    for b in data:
        v |= (b & 0x7F) << shift
        if b & 0x80:
            shift += 7
        else:
            out.append(v)
            v, shift = 0, 0
    return out


def expect(cond: bool, msg: str) -> None:
    if not cond:
        sys.exit(f"INVALID: {msg}")


def field(compound: dict, key: str, kind: str):
    expect(key in compound, f"missing {key}")
    t, v = compound[key]
    expect(t == kind, f"{key} is {t}, spec says {kind}")
    return v


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("file")
    ap.add_argument("--show", type=int, default=0)
    args = ap.parse_args()
    raw = open(args.file, "rb").read()
    expect(raw[:2] == b"\x1f\x8b", "not gzip")
    r = Reader(gzip.decompress(raw))
    expect(r.unpack("b") == 10, "root is not a compound")
    expect(r.string() == "", "root compound must be unnamed")
    _, root = r.payload(10)
    s = field(root, "Schematic", "Compound")
    expect(field(s, "Version", "Int") == 3, "Version must be 3")
    field(s, "DataVersion", "Int")
    w, h, l = (field(s, k, "Short") & 0xFFFF for k in ("Width", "Height", "Length"))
    if "Offset" in s:
        expect(len(field(s, "Offset", "IntArray")) == 3, "Offset must have 3 ints")
    n = w * h * l
    for container in ("Blocks", "Biomes"):
        if container not in s:
            continue
        c = field(s, container, "Compound")
        palette = field(c, "Palette", "Compound")
        expect(all(t == "Int" for t, _ in palette.values()), f"{container}.Palette values must be Int")
        ids = sorted(v for _, v in palette.values())
        expect(ids == list(range(len(ids))), f"{container}.Palette indices must be 0..n-1")
        cells = varints(field(c, "Data", "ByteArray"))
        expect(len(cells) == n, f"{container}.Data decodes to {len(cells)} entries, expected {n}")
        expect(max(cells, default=0) < len(ids), f"{container}.Data index out of palette range")
        if container == "Blocks":
            for t, be in field(c, "BlockEntities", "List") if "BlockEntities" in c else []:
                expect(t == "Compound", "BlockEntity must be a compound")
                pos = field(be, "Pos", "IntArray")
                expect(len(pos) == 3 and all(0 <= p < d for p, d in zip(pos, (w, h, l))), f"BlockEntity Pos {pos} outside")
                field(be, "Id", "String")
            names = {v: k for k, (_, v) in palette.items()}
            counts = Counter(names[i] for i in cells)
            print(f"valid v3: {w}x{h}x{l}, {len(palette)} block states, {len(field(c, 'BlockEntities', 'List')) if 'BlockEntities' in c else 0} block entities")
            for state, k in counts.most_common(args.show):
                print(f"  {k:>9}  {state}")
        else:
            print(f"  biomes: {sorted(palette)}")


if __name__ == "__main__":
    main()
