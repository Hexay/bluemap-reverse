"""Ground-truth structure starts for seed-cracker tests: a fresh server /locate's every surface-visible structure
set from a grid of points. No chunks are generated, so a wide area takes minutes, not hours.

Usage: py -3 tools/structure_truth.py <name> [--seed <seed>] [--radius 3072] [--step 512] [--force]
  no --seed: the server picks a random seed (tests the random-seed shortcut)
Output: fixtures/seed/<name>.json  {"mc", "seed", "structures": [{"set", "chunk": [x, z]}]}
"""
import argparse
import json
import shutil

from console import Server
from make_world import write_server_files
from paths import DEFAULT, ROOT

# locate target → structure_set name; locate reports the start chunk's min corner for random_spread sets
TARGETS = {
    "minecraft:desert_pyramid": "desert_pyramids",
    "minecraft:jungle_pyramid": "jungle_temples",
    "minecraft:swamp_hut": "swamp_huts",
    "minecraft:igloo": "igloos",
    "minecraft:pillager_outpost": "pillager_outposts",
    "#minecraft:village": "villages",
    "#minecraft:shipwreck": "shipwrecks",
    "#minecraft:ocean_ruin": "ocean_ruins",
    "#minecraft:ruined_portal": "ruined_portals",
    "minecraft:monument": "ocean_monuments",
    "minecraft:mansion": "woodland_mansions",
    "#minecraft:abandoned_camp": "abandoned_camp",
}
# tags print "The nearest #minecraft:village (minecraft:village_plains) is at ..."
LOCATED = r"The nearest .+? is at \[(-?\d+), [^,]+, (-?\d+)\]|Could not find|Unknown|Incorrect"


def locate_all(server: Server, radius: int, step: int, sets: list[str] | None) -> list[dict]:
    found = set()
    points = range(-radius, radius + 1, step)
    for target, set_name in TARGETS.items():
        if sets and set_name not in sets:
            continue
        for x in points:
            for z in points:
                m = server.query(f"execute positioned {x} 0 {z} run locate structure {target}", LOCATED, timeout=120)
                if m.group(1) is not None:
                    found.add((set_name, int(m.group(1)) >> 4, int(m.group(2)) >> 4))
        print(f"located {target}: {sum(1 for s in found if s[0] == set_name)}", flush=True)
    return [{"set": s, "chunk": [cx, cz]} for s, cx, cz in sorted(found)]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("name")
    ap.add_argument("--seed", default="")
    ap.add_argument("--radius", type=int, default=3072)
    ap.add_argument("--step", type=int, default=512)
    ap.add_argument("--force", action="store_true")
    ap.add_argument("--merge", action="store_true",
                    help="add to the existing file with its seed (e.g. a dense --step 128 pass near spawn: "
                         "a coarse grid misses structures that are never the nearest to a grid point)")
    ap.add_argument("--sets", nargs="+", help="only these structure sets (e.g. igloos swamp_huts)")
    args = ap.parse_args()

    out = ROOT / "fixtures" / "seed" / f"{args.name}.json"
    old = json.loads(out.read_text()) if args.merge else None
    if old:
        args.seed = old["text_seed"] or old["seed"]
    elif out.exists() and not args.force:
        print(f"exists  {out} (use --force to regenerate, --merge to add)")
        return
    server_dir = DEFAULT.worlds / f"truth-{args.name}"
    shutil.rmtree(server_dir, ignore_errors=True)
    write_server_files(server_dir, {"level-type": "minecraft:normal", "level-seed": args.seed, "generate-structures": "true"})

    server = Server(server_dir, "4G", DEFAULT)
    try:
        server.wait_for(r"Done \(", timeout=600)
        seed = server.query("seed", r"Seed: \[(-?\d+)\]").group(1)
        structures = locate_all(server, args.radius, args.step, args.sets)
    finally:
        server.stop()
    shutil.rmtree(server_dir, ignore_errors=True)

    if old:
        known = {(s["set"], *s["chunk"]) for s in structures}
        structures += [s for s in old["structures"] if (s["set"], *s["chunk"]) not in known]
        structures.sort(key=lambda s: (s["set"], s["chunk"]))
    out.parent.mkdir(parents=True, exist_ok=True)
    doc = {"mc": DEFAULT.mc, "seed": seed, "text_seed": args.seed or None, "structures": structures}
    out.write_text(json.dumps(doc, indent=1) + "\n")
    print(f"wrote   {out}: seed {seed}, {len(structures)} structures")


if __name__ == "__main__":
    main()
