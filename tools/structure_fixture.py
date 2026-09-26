"""Write a structure-detection fixture from a truth file (tools/structure_truth.py): a 512x512 core around spawn
(negatives) plus a 7x7-chunk patch around every known structure within --radius chunks. Generated with
make_world.py's extra_areas, so a wide spread of structures costs a few thousand chunks instead of millions.

Usage: py -3 tools/structure_fixture.py <truth_name> <fixture_name> [--radius 64]
Then:  py -3 tools/make_world.py <fixture> && py -3 tools/mirror_fixture.py <fixture> && py -3 tools/reverse_fixture.py <fixture>
"""
import argparse
import json

from paths import ROOT

CORE = [-256, -256, 255, 255]


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("truth")
    ap.add_argument("fixture")
    ap.add_argument("--radius", type=int, default=64, help="chunks from spawn")
    ap.add_argument("--sets", nargs="+", help="patches only around these structure sets")
    args = ap.parse_args()

    truth = json.loads((ROOT / "fixtures" / "seed" / f"{args.truth}.json").read_text())
    near = [s for s in truth["structures"]
            if all(abs(c) < args.radius for c in s["chunk"]) and (not args.sets or s["set"] in args.sets)]
    patches = []
    for s in near:
        cx, cz = s["chunk"]
        a = [(cx - 3) * 16, (cz - 3) * 16, (cx + 3) * 16 + 15, (cz + 3) * 16 + 15]
        if not (a[0] >= CORE[0] and a[1] >= CORE[1] and a[2] <= CORE[2] and a[3] <= CORE[3]):
            patches.append(a)
    spec = {
        "description": f"Structure detection: 512x512 core plus 7x7-chunk patches around the /locate-d structures "
                       f"within {args.radius * 16} blocks (truth: fixtures/seed/{args.truth}.json)",
        "properties": {"level-type": "minecraft:normal", "generate-structures": "true", "level-seed": truth["text_seed"] or truth["seed"]},
        "area": CORE,
        "extra_areas": patches,
    }
    out = ROOT / "fixtures" / args.fixture / "fixture.json"
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(spec, indent=1) + "\n")
    print(f"wrote   {out}: {len(near)} structures, {len(patches)} patches")


if __name__ == "__main__":
    main()
