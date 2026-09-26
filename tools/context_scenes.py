"""Scene builders for the `context` fixture: each fills one plot of a `Blocks` map with blocks in realistic
contexts (neighbours, attachments, culling). States that depend on neighbours are set later by
context_rules.finalize. See tools/gen_context.py for the layout.
"""
import random

from context_rules import CW, DIRS, OPP, Blocks, step

Y0 = -60  # first block above the superflat grass
SIZE = 16
FACINGS = list(DIRS)


def cells(x0, z0, size=SIZE):
    return [(x, z) for x in range(x0, x0 + size) for z in range(z0, z0 + size)]


def stairs(w: Blocks, rng: random.Random, x0, z0, material, full):
    for x, z in cells(x0, z0):
        r = rng.random()
        if r < 0.6:
            w[(x, Y0, z)] = [f"{material}_stairs", {"facing": rng.choice(FACINGS), "half": rng.choice(["bottom", "top"]),
                                                    "shape": "straight", "waterlogged": "false"}]
        elif r < 0.7:
            w[(x, Y0, z)] = [full, {}]
        if (x, Y0, z) in w and rng.random() < 0.3:
            w[(x, Y0 + 1, z)] = [f"{material}_stairs", {"facing": rng.choice(FACINGS), "half": "bottom",
                                                        "shape": "straight", "waterlogged": "false"}]


def connectables(w: Blocks, rng: random.Random, x0, z0, kinds, layers=2):
    """`kinds`: names to scatter; `*_fence_gate` gets a random facing, `stone` is the sturdy neighbour."""
    for x, z in cells(x0, z0):
        for y in range(Y0, Y0 + layers):
            if y > Y0 and (x, y - 1, z) not in w:
                break
            if rng.random() < 0.55:
                name = rng.choice(kinds)
                props = {"facing": rng.choice(FACINGS), "open": "false", "powered": "false", "in_wall": "false"} \
                    if name.endswith("_fence_gate") else {} if name in w.full else {"waterlogged": "false"}
                w[(x, y, z)] = [name, props]


def redstone(w: Blocks, rng: random.Random, x0, z0):
    parts = [("redstone_wire", 0.7), ("redstone_block", 0.05), ("lever", 0.05), ("repeater", 0.08),
             ("comparator", 0.04), ("redstone_torch", 0.04), ("target", 0.04)]
    for x, z in cells(x0, z0):
        if rng.random() > 0.6:
            continue
        name = rng.choices([p for p, _ in parts], [wt for _, wt in parts])[0]
        props = {"power": "0", "north": "none", "south": "none", "east": "none", "west": "none"} \
            if name == "redstone_wire" else {}
        if name == "repeater":
            props = {"facing": rng.choice(FACINGS), "delay": str(rng.randint(1, 4)), "locked": "false", "powered": "false"}
        elif name == "comparator":
            props = {"facing": rng.choice(FACINGS), "mode": rng.choice(["compare", "subtract"]), "powered": "false"}
        elif name == "lever":
            props = {"face": "floor", "facing": rng.choice(FACINGS), "powered": rng.choice(["true", "false"])}
        w[(x, Y0, z)] = [name, props]


def slabs(w: Blocks, rng: random.Random, x0, z0):
    for x, z in cells(x0, z0):
        for y in range(Y0, Y0 + rng.randint(0, 3)):
            m = rng.choice(["oak", "stone", "smooth_stone", "cobblestone", "prismarine"])
            w[(x, y, z)] = [f"{m}_slab", {"type": rng.choice(["bottom", "top", "double"]), "waterlogged": "false"}]


def doors(w: Blocks, rng: random.Random, x0, z0):
    """Rows of stone-brick wall with doors cut in; trapdoors hung on the walls and laid on the floor."""
    for z in range(z0 + 1, z0 + SIZE, 4):
        for x in range(x0, x0 + SIZE):
            for y in range(Y0, Y0 + 3):
                w[(x, y, z)] = ["stone_bricks", {}]
        for x in range(x0 + 1, x0 + SIZE - 1, 3):
            m = rng.choice(["oak", "spruce", "iron", "bamboo", "cherry"])
            props = {"facing": rng.choice(["north", "south"]), "hinge": rng.choice(["left", "right"]),
                     "open": rng.choice(["true", "false"]), "powered": "false"}
            w[(x, Y0, z)] = [f"{m}_door", {**props, "half": "lower"}]
            w[(x, Y0 + 1, z)] = [f"{m}_door", {**props, "half": "upper"}]
            t = {"facing": rng.choice(["north", "south"]), "half": rng.choice(["top", "bottom"]),
                 "open": rng.choice(["true", "false"]), "powered": "false", "waterlogged": "false"}
            side = z + (1 if t["facing"] == "south" else -1)
            w[(x + 1, Y0 + 2, side)] = [f"{m}_trapdoor", t]
            w[(x + 1, Y0, z + 2)] = [f"{m}_trapdoor", {**t, "open": "false"}]


def containers(w: Blocks, rng: random.Random, x0, z0):
    """Single and double chests, beds, barrels, shulker boxes."""
    for x, z in cells(x0, z0):
        if (x - x0) % 3 or (z - z0) % 3 or (x, Y0, z) in w:
            continue
        f = rng.choice(FACINGS)
        r = rng.random()
        if r < 0.4:
            name = rng.choice(["chest", "trapped_chest"])
            other = step((x, Y0, z), CW[f])
            if rng.random() < 0.6 and other not in w:
                w[(x, Y0, z)] = [name, {"facing": f, "type": "left", "waterlogged": "false"}]
                w[other] = [name, {"facing": f, "type": "right", "waterlogged": "false"}]
            else:
                w[(x, Y0, z)] = [name, {"facing": f, "type": "single", "waterlogged": "false"}]
        elif r < 0.7:
            head = step((x, Y0, z), f)
            if head not in w:
                color = rng.choice(["red", "white", "blue", "lime"])
                w[(x, Y0, z)] = [f"{color}_bed", {"facing": f, "part": "foot", "occupied": "false"}]
                w[head] = [f"{color}_bed", {"facing": f, "part": "head", "occupied": "false"}]
        else:
            name = rng.choice(["barrel", "ender_chest", "shulker_box", "red_shulker_box"])
            props = {"facing": rng.choice(FACINGS + ["up", "down"])} if name != "ender_chest" else {"facing": f}
            if name == "barrel":
                props["open"] = "false"
            w[(x, Y0, z)] = [name, props]


def attached(w: Blocks, rng: random.Random, x0, z0):
    """A wall of stone bricks with things hung on both sides, and lanterns/chains under a roof."""
    items = [("wall_torch", {}), ("soul_wall_torch", {}), ("ladder", {"waterlogged": "false"}),
             ("oak_wall_sign", {"waterlogged": "false"}), ("stone_button", {"face": "wall", "powered": "false"}),
             ("lever", {"face": "wall", "powered": "false"}), ("redstone_wall_torch", {"lit": "true"}),
             ("oak_wall_hanging_sign", {"waterlogged": "false"}), ("tripwire_hook", {"attached": "false", "powered": "false"}),
             ("cocoa", {"age": "2"})]
    for z in range(z0 + 2, z0 + SIZE, 5):
        for x in range(x0, x0 + SIZE):
            for y in range(Y0, Y0 + 4):
                w[(x, y, z)] = ["jungle_log" if x % 5 == 0 else "stone_bricks", {"axis": "y"} if x % 5 == 0 else {}]
            for side in ("north", "south"):
                y = Y0 + rng.randint(0, 3)
                name, props = rng.choice(items)
                if name == "cocoa" and x % 5:
                    continue
                # cocoa faces its log; wall hanging signs hang from the blocks beside them
                facing = {"cocoa": OPP[side], "oak_wall_hanging_sign": CW[side]}.get(name, side)
                w[step((x, y, z), side)] = [name, {**props, "facing": facing}]
        for x in range(x0 + 1, x0 + SIZE, 3):
            for side in ("north", "south"):
                ledge = step((x, Y0 + 3, z), side)
                w[ledge] = ["stone_bricks", {}]
                w[step(ledge, (0, -1, 0))] = rng.choice([["lantern", {"hanging": "true", "waterlogged": "false"}],
                                           ["iron_chain", {"axis": "y", "waterlogged": "false"}],
                                           ["vine", {"up": "false", "north": "false", "south": "false", "east": "false",
                                                     "west": "false", OPP[side]: "true"}]])


def fire(w: Blocks, rng: random.Random, x0, z0):
    """Floor fire on netherrack / soul sand, and side fire hanging on plank pillars (air below)."""
    for x, z in cells(x0, z0)[::2]:
        if (x - x0) % 4 == 0 and (z - z0) % 4 == 0:
            for y in range(Y0, Y0 + 4):
                w[(x, y, z)] = ["oak_planks", {}]
            for d in rng.sample(FACINGS, rng.randint(1, 3)):
                p = step((x, Y0 + 3, z), d)
                w[p] = ["fire", {"age": "0", "up": "false", "north": "false", "south": "false", "east": "false",
                                 "west": "false", OPP[d]: "true"}]
        elif (x - x0) % 4 == 2 and (z - z0) % 4 == 2:
            base = rng.choice(["netherrack", "soul_sand"])
            w[(x, Y0, z)] = [base, {}]
            w[(x, Y0 + 1, z)] = ["fire" if base == "netherrack" else "soul_fire", {}]


def culling(w: Blocks, rng: random.Random, x0, z0):
    """Transparent full blocks packed together: glass-glass faces cull, glass-leaves faces do not."""
    kinds = ["glass", "white_stained_glass", "tinted_glass", "ice", "slime_block", "honey_block", "stone",
             "oak_leaves", "azalea_leaves", "packed_ice", "sea_lantern", "glowstone"]
    for x, z in cells(x0, z0):
        for y in range(Y0, Y0 + 4):
            if rng.random() < 0.6:
                name = rng.choice(kinds)
                w[(x, y, z)] = [name, {"persistent": "true", "distance": "7", "waterlogged": "false"} if "leaves" in name else {}]


def pool(w: Blocks, rng: random.Random, x0, z0):
    """Stone-walled pool: waterlogged blocks, coral, sea pickles, seagrass, kelp. Water is filled by the caller."""
    wet = [("oak_stairs", {"facing": "north", "half": "bottom", "shape": "straight"}), ("stone_slab", {"type": "bottom"}),
           ("oak_fence", {}), ("glass_pane", {}), ("cobblestone_wall", {}), ("chest", {"facing": "north", "type": "single"}),
           ("lantern", {"hanging": "false"}), ("ladder", {"facing": "north"})]
    plants = ["brain_coral", "tube_coral_fan", "fire_coral", "horn_coral_block", "sea_pickle", "seagrass", "kelp"]
    for x, z in cells(x0, z0):
        edge = x in (x0, x0 + SIZE - 1) or z in (z0, z0 + SIZE - 1)
        for y in range(Y0, Y0 + 3):
            if edge:
                w[(x, y, z)] = ["stone", {}]
        if edge or rng.random() > 0.35:
            continue
        if rng.random() < 0.5:
            name, props = rng.choice(wet)
            props = {**props, "waterlogged": "true"}
            if name.endswith("stairs"):
                props["facing"] = rng.choice(FACINGS)
            w[(x, Y0 + rng.randint(0, 2), z)] = [name, props]
        else:
            name = rng.choice(plants)
            if name == "kelp":
                top = Y0 + rng.randint(0, 2)
                for y in range(Y0, top):
                    w[(x, y, z)] = ["kelp_plant", {}]
                w[(x, top, z)] = ["kelp", {"age": "0"}]
            else:
                w[(x, Y0, z)] = [name, {"waterlogged": "true"} if name not in ("seagrass", "horn_coral_block") else {}]


def jumble(w: Blocks, rng: random.Random, x0, z0, choose):
    """Random blocks packed with stone, 3 high: `choose(rng)` → (name, props)."""
    for x, z in cells(x0, z0):
        for y in range(Y0, Y0 + 3):
            r = rng.random()
            if r < 0.35:
                w[(x, y, z)] = list(choose(rng))
            elif r < 0.5:
                w[(x, y, z)] = ["stone", {}]
