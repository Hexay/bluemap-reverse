"""Neighbour-dependent states the game computes when a player places a block, but `setblock` does not:
stair shapes, fence / pane / bars / wall / gate connections, redstone wire sides. Ported from the vanilla
block classes (StairBlock, FenceBlock, IronBarsBlock, WallBlock, FenceGateBlock, RedStoneWireBlock).
"""
DIRS = {"north": (0, 0, -1), "south": (0, 0, 1), "west": (-1, 0, 0), "east": (1, 0, 0)}
CCW = {"north": "west", "west": "south", "south": "east", "east": "north"}
CW = {v: k for k, v in CCW.items()}
OPP = {"north": "south", "south": "north", "west": "east", "east": "west"}
AXIS = {"north": "z", "south": "z", "west": "x", "east": "x"}
UP = (0, 1, 0)
SIGNAL_SOURCES = {"redstone_block", "redstone_torch", "lever", "target", "comparator", "daylight_detector",
                  "stone_button", "oak_button", "stone_pressure_plate", "trapped_chest", "lectern"}


def step(p, d):
    o = DIRS[d] if isinstance(d, str) else d
    return p[0] + o[0], p[1] + o[1], p[2] + o[2]


def kind(name: str | None) -> str | None:
    if name is None:
        return None
    for suffix, k in (("_stairs", "stairs"), ("_fence_gate", "gate"), ("_fence", "fence"), ("_wall", "wall"),
                      ("glass_pane", "pane"), ("_bars", "pane")):
        if name.endswith(suffix):
            return k
    return "wire" if name == "redstone_wire" else None


class Blocks(dict):
    """(x, y, z) → [name, props]; names without the `minecraft:` prefix, props may be partial (the rest is
    the default state). `full` = sturdy full cubes; `defaults` = name → default state's properties."""

    def __init__(self, full: set[str], defaults: dict[str, dict]):
        super().__init__()
        self.full = full
        self.defaults = defaults

    def name(self, p):
        b = self.get(p)
        return b[0] if b else None

    def props(self, p):
        """Full properties; writes go to the placed block's own (possibly partial) dict via `own`."""
        b = self.get(p)
        return {**self.defaults.get(b[0], {}), **b[1]} if b else {}

    def own(self, p) -> dict:
        return self[p][1]

    def sturdy(self, p) -> bool:
        return self.name(p) in self.full


def stair_shape(w: Blocks, p) -> str:
    me = w.props(p)
    f, half = me["facing"], me["half"]

    def same_half_stairs(q):
        return kind(w.name(q)) == "stairs" and w.props(q)["half"] == half

    def can_take(d):
        q = step(p, d)
        return kind(w.name(q)) != "stairs" or w.props(q)["facing"] != f or w.props(q)["half"] != half

    back = step(p, f)
    if same_half_stairs(back):
        d1 = w.props(back)["facing"]
        if AXIS[d1] != AXIS[f] and can_take(OPP[d1]):
            return "outer_left" if d1 == CCW[f] else "outer_right"
    front = step(p, OPP[f])
    if same_half_stairs(front):
        d2 = w.props(front)["facing"]
        if AXIS[d2] != AXIS[f] and can_take(d2):
            return "inner_left" if d2 == CCW[f] else "inner_right"
    return "straight"


def gate_faces_across(w: Blocks, q, d) -> bool:
    return kind(w.name(q)) == "gate" and AXIS[w.props(q)["facing"]] != AXIS[d]


def connects(w: Blocks, p, d) -> bool:
    me, q = kind(w.name(p)), step(p, d)
    other = kind(w.name(q))
    if me == "fence":
        if other == "fence":
            return (w.name(p) == "nether_brick_fence") == (w.name(q) == "nether_brick_fence")
        return gate_faces_across(w, q, d) or w.sturdy(q)
    if me == "pane":
        return w.sturdy(q) or other in ("pane", "wall")
    if me == "wall":
        return w.sturdy(q) or other in ("wall", "pane") or gate_faces_across(w, q, d)
    raise ValueError(me)


def wall_state(w: Blocks, p) -> dict:
    """Needs the block above already final (process walls top-down)."""
    above = step(p, UP)
    above_wall = kind(w.name(above)) == "wall"
    sides = {}
    for d in DIRS:
        if not connects(w, p, d):
            sides[d] = "none"
        elif w.sturdy(above) or (above_wall and w.props(above)[d] != "none"):
            sides[d] = "tall"
        else:
            sides[d] = "low"
    none = {d: s == "none" for d, s in sides.items()}
    corner_or_end = all(none.values()) or none["north"] != none["south"] or none["west"] != none["east"]
    tall_straight = (sides["north"] == sides["south"] == "tall") or (sides["east"] == sides["west"] == "tall")
    if above_wall and w.props(above)["up"] == "true" or corner_or_end:
        up = True
    else:
        up = not tall_straight and w.sturdy(above)
    return {**sides, "up": str(up).lower()}


def wire_state(w: Blocks, p) -> dict:
    """Flat wiring only; power is left for the game to compute (setblock runs onPlace)."""
    sides = {}
    for d in DIRS:
        q = step(p, d)
        n = w.name(q)
        if n == "redstone_wire" or n in SIGNAL_SOURCES:
            sides[d] = "side"
        elif n == "repeater":
            sides[d] = "side" if AXIS[w.props(q)["facing"]] == AXIS[d] else "none"
        elif n == "observer":
            sides[d] = "side" if w.props(q)["facing"] == d else "none"
        else:
            sides[d] = "none"
    ns = sides["north"] == sides["south"] == "none"
    ew = sides["east"] == sides["west"] == "none"
    # a lone wire is a cross; a wire with one axis connected extends straight through
    if ns:
        sides["east"] = sides["west"] = "side"
    if ew:
        sides["north"] = sides["south"] = "side"
    return sides


def finalize(w: Blocks) -> None:
    """Set every neighbour-dependent property in place, the way the game would on placement."""
    for p in sorted(w, key=lambda p: -p[1]):
        k = kind(w.name(p))
        props = w.own(p)
        if k == "stairs":
            props["shape"] = stair_shape(w, p)
        elif k in ("fence", "pane"):
            props.update({d: str(connects(w, p, d)).lower() for d in DIRS})
        elif k == "wall":
            props.update(wall_state(w, p))
        elif k == "gate":
            f = w.props(p)["facing"]
            props["in_wall"] = str(any(kind(w.name(step(p, d))) == "wall" for d in (CW[f], CCW[f]))).lower()
        elif k == "wire":
            props.update(wire_state(w, p))
