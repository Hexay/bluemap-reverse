"""Extract the structure templates that seed detection matches against (village town centres, igloo top, mansion
entrance) from the server jar into crates/bmr-seed/src/detect/templates.json: block name → flat [x, y, z, ...].

Usage: python3 tools/gen_structure_templates.py [--mc 26.3]
"""
import argparse
import gzip
import io
import json
import struct
import zipfile

from paths import DEFAULT, ROOT, toolchain

OUT = ROOT / "crates" / "bmr-seed" / "src" / "detect" / "templates.json"
PREFIXES = ("village/", "igloo/top", "woodland_mansion/entrance")
SKIP = {"air", "structure_void", "jigsaw"}


def read_tag(buf: bytes, pos: int, tag: int):
    fixed = {1: ">b", 2: ">h", 3: ">i", 4: ">q", 5: ">f", 6: ">d"}
    if tag in fixed:
        return struct.unpack_from(fixed[tag], buf, pos)[0], pos + struct.calcsize(fixed[tag])
    if tag in (7, 11, 12):
        n = struct.unpack_from(">i", buf, pos)[0]
        width = {7: 1, 11: 4, 12: 8}[tag]
        return None, pos + 4 + n * width
    if tag == 8:
        n = struct.unpack_from(">H", buf, pos)[0]
        return buf[pos + 2:pos + 2 + n].decode("utf-8"), pos + 2 + n
    if tag == 9:
        kind, n = buf[pos], struct.unpack_from(">i", buf, pos + 1)[0]
        pos += 5
        out = []
        for _ in range(n):
            v, pos = read_tag(buf, pos, kind)
            out.append(v)
        return out, pos
    if tag == 10:
        out = {}
        while (kind := buf[pos]) != 0:
            n = struct.unpack_from(">H", buf, pos + 1)[0]
            name = buf[pos + 3:pos + 3 + n].decode("utf-8")
            out[name], pos = read_tag(buf, pos + 3 + n, kind)
        return out, pos + 1
    raise ValueError(f"NBT tag {tag}")


def template(data: bytes) -> dict:
    raw = gzip.decompress(data)
    root, _ = read_tag(raw, 3 + struct.unpack_from(">H", raw, 1)[0], 10)
    palette = root.get("palette") or root["palettes"][0]
    names = [(p.get("Name") or p["id"]).removeprefix("minecraft:") for p in palette]
    blocks: dict[str, list[int]] = {}
    for b in root["blocks"]:
        name = names[b["state"]]
        if name not in SKIP:
            blocks.setdefault(name, []).extend(b["pos"])
    return {"size": root["size"], "blocks": dict(sorted(blocks.items()))}


def wanted(path: str) -> bool:
    rel = path.removeprefix("data/minecraft/structure/")
    return path.endswith(".nbt") and rel.startswith(PREFIXES) and ("town_centers/" in rel or "village/" not in rel)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--mc", default=DEFAULT.mc)
    mc = ap.parse_args().mc
    jar = (DEFAULT if mc == DEFAULT.mc else toolchain(mc, DEFAULT.bluemap, 25, 25)).server_jar
    with zipfile.ZipFile(jar) as bundler:
        inner = bundler.read(f"META-INF/versions/{mc}/server-{mc}.jar")
    templates = {}
    with zipfile.ZipFile(io.BytesIO(inner)) as z:
        for path in sorted(filter(wanted, z.namelist())):
            name = path.removeprefix("data/minecraft/structure/").removesuffix(".nbt")
            templates[name] = template(z.read(path))
    OUT.write_text(json.dumps({"mc": mc, "templates": templates}, separators=(",", ":")) + "\n")
    print(f"wrote {len(templates)} templates to {OUT.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
