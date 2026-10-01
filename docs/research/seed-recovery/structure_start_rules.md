# Structure start chunk -> block placement rules (Java 1.18+, verified against 26.2 Mojmap)

Source: decompiled Mojmap 26.2, `github.com/Renekovski/26.2-mcp@main`. Abbreviations:
`S/` = `src/net/minecraft/world/level/levelgen/structure/`, `SS/` = `S/structures/`,
`D/` = `src/resources/data/minecraft/`. Template sizes were read from the `.nbt` files in the same
repo (`D/structure/**`), not from memory. "INFERRED" marks anything not read directly in code.

Notation: `X0 = cx*16`, `Z0 = cz*16` (the start chunk's min corner, `ChunkPos.getMinBlockX/Z`).
All offsets below are **inclusive block coords relative to (X0, Z0)**. `sx, sz` = template size.

---

## 0. Shared mechanics (read once)

### 0.1 Per-structure random
`S/Structure.java:248-252` — `GenerationContext.makeRandom`:
```java
WorldgenRandom random = new WorldgenRandom(new LegacyRandomSource(0L));
random.setLargeFeatureSeed(seed, chunkPos.x(), chunkPos.z());
```
`WorldgenRandom.setLargeFeatureSeed` (`levelgen/WorldgenRandom.java:58-64`):
`setSeed(seed); a=nextLong(); b=nextLong(); setSeed((cx*a) ^ (cz*b) ^ seed)`.
Every rotation/template/offset choice below comes from **this** random, in the order listed.
It is keyed by world seed + chunk only (no salt), so it's the same random for every structure type
attempted in that chunk. The placement salt only matters for picking the chunk
(`S/placement/RandomSpreadStructurePlacement.java:67-76`).

### 0.2 Random primitives
| Call | Code | Consumes |
|---|---|---|
| `Rotation.getRandom(r)` | `Util.getRandom(values(), r)` -> `values()[r.nextInt(4)]` (`block/Rotation.java:112`, `util/Util.java:771`) | `nextInt(4)`: 0=NONE, 1=CLOCKWISE_90, 2=CLOCKWISE_180, 3=COUNTERCLOCKWISE_90 |
| `StructurePiece.getRandomHorizontalDirection(r)` | `Direction.Plane.HORIZONTAL.getRandomDirection` (`S/StructurePiece.java:80`, `core/Direction.java:574,585`) | `nextInt(4)` over `[NORTH, EAST, SOUTH, WEST]` |
| `Mth.nextInt(r,a,b)` | `a + nextInt(b-a+1)` (standard) | |
| `StructureTemplatePool.getRandomTemplate` | `templates.get(nextInt(templates.size()))`; `templates` holds each element *weight* times (`pools/StructureTemplatePool.java:55-56,105-108`) | `nextInt(sumWeights)` |

### 0.3 Template transform (all template-based pieces)
`S/templatesystem/StructureTemplate.java:528-556` `transform(pos, mirror, rotation, pivot)`:
mirror first (`LEFT_RIGHT: z=-z`, `FRONT_BACK: x=-x`), then rotate about pivot `(px,pz)`:
| Rotation | local (x,z) -> offset |
|---|---|
| NONE | (x, z) |
| CLOCKWISE_90 | (px+pz - z, pz-px + x) |
| CLOCKWISE_180 | (2px - x, 2pz - z) |
| COUNTERCLOCKWISE_90 | (px-pz + z, px+pz - x) |

Bounding box = `fromCorners(transform(0,0,0), transform(size-1)).move(templatePosition)`
(`StructureTemplate.java:624-629`). The same transform places every block (`calculateRelativePosition`,
`:247`), so the bbox is exact. Default pivot is `BlockPos.ZERO` (`StructurePlaceSettings.java:17`).
With pivot 0: NONE `(x,z)`, CW90 `(-z,x)`, CW180 `(-x,-z)`, CCW90 `(z,-x)`.

### 0.4 Old-style (non-template) pieces
`S/StructurePiece.java:72-78` `makeBoundingBox(x,y,z,dir,w,h,d)`: if `dir.axis==Z` (NORTH/SOUTH)
the box is `[x, x+w-1] x [z, z+d-1]`, otherwise (EAST/WEST) `[x, x+d-1] x [z, z+w-1]`. **The min
corner is always (x, z), whatever the direction.** The direction only sets the rotation and mirror
(`setOrientation`, `:533-557`: SOUTH=mirror LEFT_RIGHT, WEST=LR+CW90, EAST=CW90, NORTH=none).

### 0.5 Universal inversion test (use for every structure)
For each hypothesis `h` (template, rotation, mirror) you know the footprint offsets `[dx0,dx1] x [dz0,dz1]`
relative to `(X0,Z0)`. From the observed block extent `[minX,maxX] x [minZ,maxZ]`:
- **exact footprint visible:** `X0 = minX - dx0` (also needs `maxX - dx1` to be the same). Keep `h`
  only if `X0 % 16 == 0` and `Z0 % 16 == 0`.
- **partial footprint** (integrity-degraded ruins, cropped map): X0 lies in `[maxX - dx1, minX - dx0]`,
  intersected with multiples of 16 (and the same for Z).

Hypotheses usually differ by offsets that are not multiples of 16, so the alignment test almost
always leaves **one chunk**. This works better than "centre of structure".

---

## 1. Single-piece scattered features (desert_pyramid, jungle_pyramid, swamp_hut)

Code:
- Desert/jungle: `S/SinglePieceStructure.java:24-33`. The start is rejected if `getLowestY(ctx, width, depth) < seaLevel`,
  which samples WORLD_SURFACE_WG at `(X0,Z0),(X0,Z0+d),(X0+w,Z0),(X0+w,Z0+d)` (`S/Structure.java:155-182`;
  note `+w`, not `+w-1`). Otherwise `onTopOfChunkCenter` (biome check at `(X0+8, surfaceY, Z0+8)`, `Structure.java:122-130`),
  then `constructor(random, getMinBlockX(), getMinBlockZ())`.
- Swamp hut: `SS/SwampHutStructure.java:18-23`. `onTopOfChunkCenter` only (no sea-level check), then `new SwampHutPiece(random, X0, Z0)`.
- Pieces: `ScatteredFeaturePiece(type, west=X0, floor=64, north=Z0, w, h, d, getRandomHorizontalDirection(random))`
  (`S/ScatteredFeaturePiece.java:17-32`).

| Structure | Piece ctor | w x h x d | Random order | Footprint N/S | Footprint E/W |
|---|---|---|---|---|---|
| desert_pyramid | `DesertPyramidPiece.java:30-31` | 21x15x21 | dir=`nextInt(4)` (first call) | x[0,20] z[0,20] | same |
| jungle_pyramid | `JungleTemplePiece.java:39-40` | 12x10x15 | dir=`nextInt(4)` | x[0,11] z[0,14] | x[0,14] z[0,11] |
| swamp_hut | `SwampHutPiece.java:29-30` | 7x7x9 | dir=`nextInt(4)` | x[0,6] z[0,8] | x[0,8] z[0,6] |

Coverage (checked in postProcess): desert pyramid base `generateBox(0,-4,0,w-1,0,d-1)` sandstone
(`DesertPyramidPiece.java:62-64`); jungle base `generateBox(0,-4,0,w-1,0,d-1)` cobble (`JungleTemplePiece.java:71`);
swamp hut roof stairs span local x 0..6, z 1..8, porch planks x 2..4 at z 0 (`SwampHutPiece.java:57-87`). So the
visible block extent equals the bbox. Only the swamp hut's z=0 row (along the depth axis) is partial.

Y: desert = min MOTION_BLOCKING_NO_LEAVES over bbox, minus `random.nextInt(3)` (`DesertPyramidPiece.java:61`,
`ScatteredFeaturePiece.java:78-101`). Jungle and swamp use the average MOTION_BLOCKING_NO_LEAVES over the bbox
columns inside the current chunk (`ScatteredFeaturePiece.java:50-76`). The postProcess random here is the
decoration random, not the structure random.

**Rule: `X0 = minX`, `Z0 = minZ` of the structure's blocks.** The long axis gives the direction
(N/S vs E/W). Telling N from S (or E from W) needs block asymmetry, and you don't need it to find the chunk.

---

## 2. Igloo

`SS/IglooStructure.java:22-32`: `onTopOfChunkCenter(WORLD_SURFACE_WG)`, `startPos=(X0, 90, Z0)`,
`rotation = Rotation.getRandom(random)` **(first call)**, then `IglooPieces.addPieces`.
`SS/IglooPieces.java:37-60`: `random.nextDouble() < 0.5` -> basement. Depth `nextInt(8)+4` gives a bottom piece plus
`depth-1` ladder pieces. The top is always added.
Templates, pivots and offsets (`IglooPieces.java:40-45`):

| Template | size (x,y,z) | pivot | offset | footprint rel (X0,Z0): NONE / CW90 / CW180 / CCW90 |
|---|---|---|---|---|
| igloo/top | 7,5,8 | (3,5,5) | (0,0,0) | x[0,6]z[0,7] / x[1,8]z[2,8] / x[0,6]z[3,10] / x[-2,5]z[2,8] |
| igloo/middle | 3,3,3 | (1,3,1) | (2,-3,4) | x[2,4]z[4,6] for all rotations |
| igloo/bottom | 7,6,9 | (3,6,7) | (0,-3,-2) | x[0,6]z[-2,6] / x[2,10]z[2,8] / x[0,6]z[4,12] / x[-4,4]z[2,8] |

Y (`IglooPieces.java:137-145`): every piece is shifted by `WORLD_SURFACE_WG(entrance) - 90 - 1`, where
`entrance = templatePos + transform((3-off.x, 0, -off.z))`.

**Rule (top only, full dome 7x8 visible):**
| observed X-span x Z-span | rotation | X0 | Z0 |
|---|---|---|---|
| 7 x 8 | NONE | minX | minZ |
| 7 x 8 | CW180 | minX | minZ-3 |
| 8 x 7 | CW90 | minX-1 | minZ-2 |
| 8 x 7 | CCW90 | minX+2 | minZ-2 |

The two candidates in each row pair differ by 3 in one axis, so the %16 test makes the answer unique.

---

## 3. Ocean monument

`SS/OceanMonumentStructure.java:30-50`: biome pre-check `getBiomesWithin(getBlockX(9), seaLevel, getBlockZ(9), 29)`
(all must be `#required_ocean_monument_surrounding`), then `onTopOfChunkCenter(OCEAN_FLOOR_WG)` (biome check at X0+8,Z0+8).
Piece: `west = X0 - 29`, `north = Z0 - 29`, `dir = HORIZONTAL.getRandomDirection(random)` (**first call**);
`MonumentBuilding` bbox = `makeBoundingBox(west, 39, north, dir, 58, 23, 58)` (`OceanMonumentPieces.java:182-183`).

Footprint: **x[-29, 28], z[-29, 28]** for every direction. Y is fixed at 39..61; the pillars and water clearing
don't change XZ. The room layout comes from the same random (`generateRoomGraph`) after the direction.

**Rule: `X0 = minX + 29 = maxX - 28`, `Z0 = minZ + 29`.** Check: the monument's minX is always `≡ 3 (mod 16)`.

---

## 4. Woodland mansion

`SS/WoodlandMansionStructure.java:30-38`: `rotation = Rotation.getRandom(random)` (**first call**), then
`startPos = getLowestYIn5by5BoxOffset7Blocks(ctx, rotation)` (`S/Structure.java:185-201`):
`O = (X0+7, y, Z0+7)`, with `y` = min WORLD_SURFACE_WG of the 4 corners of `O + (0..±5, 0..±5)`
(sign depends on rotation). Rejected if `y < 60`.
`WoodlandMansionPieces.generateMansion` -> `MansionGrid(random)` (the layout uses the same random), then `createMansion(O, rotation)`.

Fixed anchor, the **entrance** piece (`WoodlandMansionPieces.java:872-876`): template `woodland_mansion/entrance`
size 21x19x16, pivot 0, no mirror (`makeSettings`, `:1241-1243`), placed at `O + 9*rotation.rotate(WEST)`:

| rotation | entrance templatePos rel (X0,Z0) | entrance footprint |
|---|---|---|
| NONE | (-2, 7) | x[-2,18] z[7,22] |
| CW90 | (7, -2) | x[-8,7] z[-2,18] |
| CW180 | (16, 7) | x[-4,16] z[-8,7] |
| CCW90 | (7, 16) | x[7,22] z[-4,16] |

Grid mapping (`WoodlandMansionPieces.java:434-459, 662-675`; startX=8, startY=5, entrance cells gx 7..8, gy 4..5,
`MansionGrid` `:121-139`): the roof/room cell `(gx, gy)` origin is `O + R(8*(gx-8), 8*(gy-4))`, where R is the pivot-0
rotation (grid x = EAST, grid y = SOUTH at NONE). The grid is 11x11. Rows 0,1,9,10 and cells x 9..10 / y 2..7 are
BLOCKED, so the house body lies in `gx 0..8`, `gy 2..8`. At rotation NONE that is local X `[-64, 7]`, Z `[-16, 39]`
from O, and the entrance protrudes on the +X side. INFERRED from the grid code, not rendered: the facade with the door
faces `rotation.rotate(EAST)` and the body extends up to ~72 blocks behind it.

**Rule: find the entrance piece (the front porch/door block, 21 wide x 16 deep) and apply the table above.** For
NONE, `X0 = entrance.minX + 2`, `Z0 = entrance.minZ - 7`. The overall mansion bbox is layout-dependent. Don't use it.

---

## 5. Shipwreck (ocean + beached)

`SS/ShipwreckStructure.java:30-52`: `onTopOfChunkCenter(beached ? WORLD_SURFACE_WG : OCEAN_FLOOR_WG)`.
Random order: `rotation = Rotation.getRandom` **(1st)**, then the template `Util.getRandom(list)` = `nextInt(11)` (beached)
or `nextInt(20)` (ocean) **(2nd)**. templatePos = `(X0, 90, Z0)`, pivot **(4,0,15)**, mirror NONE (`ShipwreckPieces.java:33,73-81,125-131`).

Template lists (`ShipwreckPieces.java:34-67`), with sizes read from the .nbt files:
| Template | size x,y,z | beached list idx | ocean list idx |
|---|---|---|---|
| with_mast | 9,21,28 | 0 | 0 |
| upsidedown_full / _fronthalf / _backhalf | 9,9,28 / 9,9,22 / 9,9,16 | - (never beached) | 1 / 2 / 3 |
| sideways_full / _fronthalf / _backhalf | 9,9,28 / 9,9,24 / 9,9,17 | 1 / 2 / 3 | 4 / 5 / 6 |
| rightsideup_full / _fronthalf / _backhalf | 9,9,28 / 9,9,24 / 9,9,16 | 4 / 5 / 6 | 7 / 8 / 9 |
| with_mast_degraded | 9,21,28 | 7 | 10 |
| upsidedown_*_degraded (full/front/back) | 28/22/16 long | - | 11 / 12 / 13 |
| sideways_*_degraded | 28/24/17 long | - | 14 / 15 / 16 |
| rightsideup_*_degraded | 28/24/16 long | 8 / 9 / 10 | 17 / 18 / 19 |

Every template is 9 wide in X, and the block extent fills the template. The one exception is
`rightsideup_fronthalf_degraded`, whose blocks start at local z=4.

Footprint for length L (pivot trick, computed with the §0.3 formula):
| rotation | footprint | anchored end |
|---|---|---|
| NONE | x[0,8] z[0,L-1] | minZ = Z0 |
| CW90 | x[20-L,19] z[11,19] | maxX = X0+19 |
| CW180 | x[0,8] z[31-L,30] | maxZ = Z0+30 |
| CCW90 | x[-11,L-12] z[11,19] | minX = X0-11 |

**Rule:**
- Ship runs along Z (X-span 9): `X0 = minX`, and `Z0 = minZ` (NONE) or `maxZ - 30` (CW180).
- Ship runs along X (Z-span 9): `Z0 = minZ - 11`, and `X0 = maxX - 19` (CW90) or `minX + 11` (CCW90).

The two options differ by `L-31` (never a multiple of 16), so the alignment test decides, and you don't need L.
For `rightsideup_fronthalf_degraded`, the NONE and CCW90 cases shift by 4 at the bow end.
Y: ocean = mean OCEAN_FLOOR_WG over the **unrotated** `templatePos..templatePos+(sx-1,sz-1)` rectangle (quirk,
`ShipwreckPieces.java:160-176`). Beached = min WORLD_SURFACE_WG there, minus `sizeY/2`, minus `nextInt(3)`.
`isTooBigToFitInWorldGenRegion` (x>32 or y>32) never fires for vanilla templates.
"Beached" = the separate structure `shipwreck_beached` (`Structures.java:102`, `isBeached=true`).

---

## 6. Ocean ruins (warm / cold)

`SS/OceanRuinStructure.java:40-47`: `onTopOfChunkCenter(OCEAN_FLOOR_WG)`, `pos=(X0,90,Z0)`,
`rotation=Rotation.getRandom` **(1st)**. Then `OceanRuinPieces.addPieces` (`OceanRuinPieces.java:152-166`):
`isLarge = nextFloat() <= 0.3` **(2nd)**. Template pick: warm `nextInt(8)` small / `nextInt(4)` big. Cold uses one
`nextInt(8|4)` idx and places brick[idx], cracked[idx] (integrity 0.7), mossy[idx] (0.5), all at the same pos/rotation.
If large: `nextFloat() <= 0.9` -> cluster. Integrity: big 0.9, small 0.8 (`BlockRotProcessor`), so observed extents can be
smaller than the template. Pivot = ZERO (default), mirror NONE (`:280-289`). postProcess changes only Y (`:347-366`).
Config: both types `largeProbability 0.3, clusterProbability 0.9` (`data/worldgen/Structures.java:149-154`).

Sizes: small `warm_1..8`, `brick/cracked/mossy_1..8` = 6x7x7 (x,y,z). Big `big_warm_4..7`, `big_{brick,cracked,mossy}_{1,2,3,8}` = 16x16x16.

**Primary piece (the one on the start chunk): template origin at (X0,Z0), rotated about it.**
| rotation | small 6x7 footprint | big 16x16 footprint |
|---|---|---|
| NONE | x[0,5] z[0,6] | x[0,15] z[0,15] |
| CW90 | x[-6,0] z[0,5] | x[-15,0] z[0,15] |
| CW180 | x[-5,0] z[-6,0] | x[-15,0] z[-15,0] |
| CCW90 | x[0,6] z[-5,0] | x[0,15] z[-15,0] |

**Rule:** the chunk corner is one corner of the primary ruin's bbox: NONE = (min,min), CW90 = (max,min),
CW180 = (max,max), CCW90 = (min,max). For a big ruin the bbox min is `≡0` or `≡1 (mod 16)` per axis. Pick the
bbox edge that is `≡ 0 (mod 16)`.

**Cluster satellites** (`OceanRuinPieces.java:168-210`; only if large):
`BL` = min corner of the big ruin bbox (NONE (0,0), CW90 (-15,0), CW180 (-15,-15), CCW90 (0,-15) rel X0,Z0).
Eight candidate positions are drawn first, in this exact order (16 `Mth.nextInt` calls):
| # | pos rel BL | x range | z range |
|---|---|---|---|
| 1 | (-16+ni(1,8), 16+ni(1,7)) | -15..-8 | 17..23 |
| 2 | (-16+ni(1,8), ni(1,7)) | -15..-8 | 1..7 |
| 3 | (-16+ni(1,8), -16+ni(4,8)) | -15..-8 | -12..-8 |
| 4 | (ni(1,7), 16+ni(1,7)) | 1..7 | 17..23 |
| 5 | (ni(1,7), -16+ni(4,6)) | 1..7 | -12..-10 |
| 6 | (16+ni(1,7), 16+ni(3,8)) | 17..23 | 19..24 |
| 7 | (16+ni(1,7), ni(1,7)) | 17..23 | 1..7 |
| 8 | (16+ni(1,7), -16+ni(4,8)) | 17..23 | -12..-8 |

Then `count = ni(4,8)`. Each iteration: `idx=nextInt(remaining)`, `rot=Rotation.getRandom`, and a small ruin (integrity 0.8) is
placed with its template origin at that pos (footprint per the small column above, relative to pos). It is skipped if its
6x7 box intersects the big ruin's 16x16 box. Satellites never sit on the start chunk's big-ruin footprint. To find the
start, always anchor on the one 16x16 ruin.

---

## 7. Ruined portals (all variants)

`SS/RuinedPortalStructure.java:72-164`. Random order:
1. If the structure has >1 setup: `nextFloat()` weighted pick (only `ruined_portal` [UNDERGROUND 0.5 / ON_LAND_SURFACE 0.5] and
   `ruined_portal_mountain` [IN_MOUNTAIN 0.5 / ON_LAND_SURFACE 0.5]).
2. `airPocket = sample(airPocketProbability)`. This consumes `nextFloat()` only if 0 < p < 1 (`:166-168`).
3. `nextFloat() < 0.05` -> giant: `nextInt(3)` of giant_portal_1..3. Otherwise `nextInt(10)` of portal_1..10.
4. `rotation = Util.getRandom(Rotation.values())` = `nextInt(4)`.
5. `mirror = nextFloat() < 0.5 ? NONE : FRONT_BACK`.
6. Y via `findSuitableY` (more randoms, no XZ effect).

Position: `templatePos = (X0, projectedY, Z0)` (`getWorldPosition()` = `(minBlockX,0,minBlockZ)`, `ChunkPos.java:174`),
**pivot = `(sx/2, 0, sz/2)`** (integer division), mirror FRONT_BACK = `x -> -x` **before** rotation (§0.3). There is no
random XZ offset. The setups are in `data/worldgen/Structures.java:254-305` (placement, airPocketP, mossiness, overgrown, vines,
canBeCold, blackstone, weight): standard UNDERGROUND(1.0)/ON_LAND_SURFACE(0.5), desert PARTLY_BURIED(0), jungle ON_LAND_SURFACE(0.5),
swamp ON_OCEAN_FLOOR(0), mountain IN_MOUNTAIN(1.0)/ON_LAND_SURFACE(0.5), ocean ON_OCEAN_FLOOR(0), nether IN_NETHER(0.5).

Sizes and footprints (x,z ranges rel X0,Z0). Every template's blocks fill its full XZ extent:
| template | size x,y,z | pivot | NONE: NONE / CW90 / CW180 / CCW90 | FRONT_BACK: NONE / CW90 / CW180 / CCW90 |
|---|---|---|---|---|
| portal_1 | 6,10,6 | 3,3 | [0,5][0,5] / [1,6][0,5] / [1,6][1,6] / [0,5][1,6] | [-5,0][0,5] / [1,6][-5,0] / [6,11][1,6] / [0,5][6,11] |
| portal_2 | 9,12,9 | 4,4 | [0,8][0,8] all four | [-8,0][0,8] / [0,8][-8,0] / [8,16][0,8] / [0,8][8,16] |
| portal_3, portal_4 | 8,9,9 | 4,4 | [0,7][0,8] / [0,8][0,7] / [1,8][0,8] / [0,8][1,8] | [-7,0][0,8] / [0,8][-7,0] / [8,15][0,8] / [0,8][8,15] |
| portal_5 | 10,10,7 | 5,3 | [0,9][0,6] / [2,8][-2,7] / [1,10][0,6] / [2,8][-1,8] | [-9,0][0,6] / [2,8][-11,-2] / [10,19][0,6] / [2,8][8,17] |
| portal_6 | 5,7,7 | 2,3 | [0,4][0,6] / [-1,5][1,5] / [0,4][0,6] / [-1,5][1,5] | [-4,0][0,6] / [-1,5][-3,1] / [4,8][0,6] / [-1,5][5,9] |
| portal_7 | 9,7,9 | 4,4 | [0,8][0,8] all four | same as portal_2 |
| portal_8 | 14,9,9 | 7,4 | [0,13][0,8] / [3,11][-3,10] / [1,14][0,8] / [3,11][-2,11] | [-13,0][0,8] / [3,11][-16,-3] / [14,27][0,8] / [3,11][11,24] |
| portal_9 | 10,8,9 | 5,4 | [0,9][0,8] / [1,9][-1,8] / [1,10][0,8] / [1,9][0,9] | [-9,0][0,8] / [1,9][-10,-1] / [10,19][0,8] / [1,9][9,18] |
| portal_10 | 12,8,10 | 6,5 | [0,11][0,9] / [2,11][-1,10] / [1,12][1,10] / [1,10][0,11] | [-11,0][0,9] / [2,11][-12,-1] / [12,23][1,10] / [1,10][11,22] |
| giant_portal_1, _2 | 11,17/16,16 | 5,8 | [0,10][0,15] / [-2,13][3,13] / [0,10][1,16] / [-3,12][3,13] | [-10,0][0,15] / [-2,13][-7,3] / [10,20][1,16] / [-3,12][13,23] |
| giant_portal_3 | 16,16,16 | 8,8 | [0,15][0,15] / [1,16][0,15] / [1,16][1,16] / [0,15][1,16] | [-15,0][0,15] / [1,16][-15,0] / [16,31][1,16] / [0,15][16,31] |

The FRONT_BACK cases really are shifted like this. The pivot isn't adjusted for the mirror, so FB-mirrored portals
land up to 31 blocks from the corner.

Extra blocks outside the template (`RuinedPortalPiece.java:169-273`):
- `spreadNetherrack`: netherrack/magma in a Manhattan diamond of radius < 14 (minus a random `distanceAdjustment`) around
  `bbox.getCenter()` = `(minX + (xspan)/2, ., minZ + (zspan)/2)` (`BoundingBox.java:237-238`). It follows the surface for
  land/ocean-floor placements. **The netherrack patch centre = bbox centre**, which you can use as a secondary anchor.
- Drip columns go below the bbox. Vines and leaves stay within the bbox ±1.
- The piece is only placed by the chunk containing the bbox centre (`chunkBB.isInside(center)`). No XZ effect.

**Rule:** find the template bbox from the obsidian/frame blocks (or the netherrack centre ± half span). Enumerate
13 templates x 4 rotations x 2 mirrors = 104 hypotheses, set `X0 = minX - dx0`, `Z0 = minZ - dz0`, and keep the ones
that are 16-aligned. Using the frame shape to restrict the template (and the frame orientation to restrict the
rotation) usually leaves 1-2 candidates. Gold blocks are 30% removed and some blocks decay (`getBlockReplaceRule`),
so prefer the frame/stone outline over the full block set.

---

## 8. Jigsaw structures (villages, outpost, trail ruins, ancient city, trial chambers)

`SS/JigsawStructure.java:146-163`: `height = startHeight.sample(random)`. ConstantHeight consumes nothing;
trial chambers' `UniformHeight(-40,-20)` consumes one call. `startPos = (X0, height, Z0)`.
`S/pools/JigsawPlacement.java:51-126`, in order:
1. `centerRotation = Rotation.getRandom(random)`. This is the 1st random for constant-height structures.
2. `centerElement = pool.getRandomTemplate(random)` = `nextInt(sumWeights)`.
3. If `start_jigsaw_name` is present: `getShuffledJigsawBlocks` (shuffles, consuming randoms), then find the named jigsaw.
   `anchor = startPos + R(jigsawLocal)`, `adjustedPosition = startPos - (anchor - startPos)`.
   **So the named jigsaw block lands exactly at (X0, ., Z0).** Otherwise `adjustedPosition = startPos` and the
   **template origin lands at (X0, ., Z0)**.
4. Start piece bbox = `template.getBoundingBox(new StructurePlaceSettings().setRotation(rot), adjustedPosition)`
   (`SinglePoolElement.java:132-135`): pivot 0, no mirror, **no random XZ offset**.
5. Y: if `project_start_to_heightmap` is set, `bottomY = startHeight + getFirstFreeHeight(centerX, centerZ, hm)`, where
   `centerX = (bbox.minX+bbox.maxX)/2`. Otherwise `bottomY = startHeight`. The piece is moved vertically only. The
   biome check happens at `(centerX, centerY, centerZ)` of the start piece.

Children (`JigsawPlacement.java:316-409`): child pos = `sourceJigsawPos.relative(front) - targetJigsawLocalPos`, with
child rotations tried in `Rotation.getShuffled(random)` order. Only rotations with `JigsawBlock.canAttach` succeed.

**General start-piece rule (pivot 0).** For start-template local solid extent `[a,b] x [c,d]`:
| rotation | world footprint | X0 | Z0 |
|---|---|---|---|
| NONE | x[X0+a, X0+b] z[Z0+c, Z0+d] | minX - a | minZ - c |
| CW90 | x[X0-d, X0-c] z[Z0+a, Z0+b] | maxX + c | minZ - a |
| CW180 | x[X0-b, X0-a] z[Z0-d, Z0-c] | maxX + a | maxZ + c |
| CCW90 | x[X0+c, X0+d] z[Z0-b, Z0-a] | minX - c | maxZ + a |
For a=c=0, **the chunk corner is a corner of the start piece**: NONE=NW(min,min), CW90=NE(max,min),
CW180=SE(max,max), CCW90=SW(min,max).

### 8.1 Config (from `D/worldgen/structure/*.json`, which match `data/worldgen/Structures.java`)
| structure | start_pool | start_jigsaw_name | start_height | project_to_heightmap | max_dist | size |
|---|---|---|---|---|---|---|
| village_{plains,desert,savanna,snowy,taiga} | village/<type>/town_centers | - | 0 | WORLD_SURFACE_WG | 80 | 6 |
| pillager_outpost | pillager_outpost/base_plates | - | 0 | WORLD_SURFACE_WG | 80 | 7 |
| trail_ruins | trail_ruins/tower | - | -15 | WORLD_SURFACE_WG | 80 | 7 |
| ancient_city | ancient_city/city_center | minecraft:city_anchor | -27 | - | 116 | 7 |
| trial_chambers | trial_chambers/chamber/end | - | uniform -40..-20 | - | 116 | 20 |

### 8.2 Village start pools (weights; size x,y,z; solid local XZ extent `[a,b]x[c,d]`)
Zombie variants have the same size and extent as their normal counterparts.
| pool | template (weight) | size | solid x / z |
|---|---|---|---|
| plains (sum 204) | plains_fountain_01 (50), zombie (1) | 9,4,9 | 0..8 / 0..8 |
| | plains_meeting_point_1 (50), zombie (1) | 10,7,10 | 0..9 / 0..9 |
| | plains_meeting_point_2 (50), zombie (1) | 8,5,15 | 0..7 / 0..14 |
| | plains_meeting_point_3 (50), zombie (1) | 11,9,11 | 0..10 / 0..10 |
| desert (sum 250) | desert_meeting_point_1 (98), zombie (2) | 17,6,9 | **7..16** / 0..8 |
| | desert_meeting_point_2 (98), zombie (2) | 12,6,12 | 0..11 / 0..11 |
| | desert_meeting_point_3 (49), zombie (1) | 15,6,15 | 0..14 / 0..14 |
| savanna (sum 459) | savanna_meeting_point_1 (100), zombie (2) | 14,5,12 | 0..13 / 0..11 |
| | savanna_meeting_point_2 (50), zombie (1) | 11,6,11 | 0..10 / 0..10 |
| | savanna_meeting_point_3 (150), zombie (3) | 9,6,11 | 0..8 / 0..10 |
| | savanna_meeting_point_4 (150), zombie (3) | 9,6,9 | 0..8 / 0..8 |
| snowy (sum 306) | snowy_meeting_point_1 (100), zombie (2) | 12,8,8 | 0..11 / 0..7 |
| | snowy_meeting_point_2 (50), zombie (1) | 11,5,9 | 0..10 / 0..8 |
| | snowy_meeting_point_3 (150), zombie (3) | 7,7,7 | 0..6 / 0..6 |
| taiga (sum 100) | taiga_meeting_point_1 (49), zombie (1) | 22,3,18 | **0..11 / 0..6** |
| | taiga_meeting_point_2 (49), zombie (1) | 9,7,9 | 0..8 / 0..8 |

Pool entries are in list order (`D/worldgen/template_pool/village/*/town_centers.json`). The index is
`nextInt(sum)` over the expanded list: normal templates first, then zombie ones.

**Village rule:** identify the town-centre piece (well, fountain, bell/meeting point) and its solid bbox, then
use the §8 table with that template's `[a,b]x[c,d]`. Mind desert_meeting_point_1 (its blocks start 7 blocks from the
origin in local x) and taiga_meeting_point_1 (its template is larger than its blocks, but a=c=0, so the corner rule is unaffected).

### 8.3 Pillager outpost
The start is `pillager_outpost/base_plate` (legacy element, weight 1), size 16x30x16, with **no solid blocks**: only
air/void and jigsaws. Its jigsaws: `plate_entry` at (0,0,7)W, (7,0,15)S, (15,0,8)E -> feature_plates (outside the
plate). Two `entrance` jigsaws at (7,1,14) and (8,1,14) face north -> pool `pillager_outpost/towers` (watchtower or
watchtower_overgrown, 15x21x15 / 15x23x15, one jigsaw at (7,1,13) facing south).
Attachment: the child rotation must equal the plate rotation (south face meets north face). The tower origin in plate-local is
`(d, 0)` with `d = 0` (via (7,1,14)) or `d = 1` (via (8,1,14)). Which one wins depends on the jigsaw shuffle; the second attempt
collides and fails. The tower fits inside the plate bbox, so it uses the "inside parent" free space.

| rotation | tower footprint d=0 | d=1 |
|---|---|---|
| NONE | x[0,14] z[0,14] | x[1,15] z[0,14] |
| CW90 | x[-14,0] z[0,14] | x[-14,0] z[1,15] |
| CW180 | x[-14,0] z[-14,0] | x[-15,-1] z[-14,0] |
| CCW90 | x[0,14] z[-14,0] | x[0,14] z[-15,-1] |

**Rule:** from the 15x15 watchtower footprint the candidates are `(X0,Z0) ∈ {(minX,minZ), (minX-1,minZ),
(maxX,minZ), (maxX,minZ-1), (maxX,maxZ), (maxX+1,maxZ), (minX,maxZ), (minX,maxZ+1)}`. Keep the 16-aligned ones.
Usually one survives, and at most 2 when the offsets 0/1 straddle a boundary. Tower door orientation picks the rotation.
(INFERRED: feature plates/tents/cages attach outside the plate; they were not traced.)

### 8.4 Trail ruins
Start pool `trail_ruins/tower`: tower_1, tower_2 = 5x13x5; tower_3..5 = 7x13x7 (weight 1 each, `nextInt(5)`).
The origin is at (X0,Z0) and the corner rule of §8 applies. Y = -15 + WORLD_SURFACE_WG at the start centre, so it's
buried (terrain_adaptation `bury`). Usually only the tower top is visible.

### 8.5 Ancient city
Start pool `ancient_city/city_center` (city_center_1..3, 18x31x41, weight 1). `city_anchor` jigsaw at local (13,24,20).
The **anchor jigsaw is at (X0, ≈-27, Z0)**. Exact Y = start_height adjusted by the ground-level-delta move
(`JigsawPlacement.java:110-115`), and it doesn't affect XZ. Start-piece bbox:
| rotation | template origin rel | footprint |
|---|---|---|
| NONE | (-13,-20) | x[-13,4] z[-20,20] |
| CW90 | (20,-13) | x[-20,20] z[-13,4] |
| CW180 | (13,20) | x[-4,13] z[-20,20] |
| CCW90 | (-20,13) | x[-20,20] z[-4,13] |
The chunk corner = the centre of the 41-long axis (`min+20`), and 13 blocks from one end of the 18-wide axis.
Deep underground (y≈-27), so not map-visible.

### 8.6 Trial chambers
Start pool `trial_chambers/chamber/end` -> `trial_chambers/corridor/end_1|end_2` (19x20x19). The origin is at (X0,Z0)
and the corner rule applies: NONE x[0,18]z[0,18], CW90 x[-18,0]z[0,18], CW180 x[-18,0]z[-18,0], CCW90 x[0,18]z[-18,0].
Y uniform -40..-20 (drawn **before** the rotation). Dimension padding is 10. Not surface-visible.

---

## 9. abandoned_camp (26.3)
There is no `AbandonedCamp*` class, no `worldgen/structure/abandoned_camp.json` and no structure .nbt in the 26.2 tree
(`git/trees/main?recursive=1` searched for "abandon": only mineshaft loot tables). Needs the 26.3 source or data. If it
is a data-only jigsaw structure (likely, INFERRED), §8 applies: the start template origin (or `start_jigsaw_name` block)
is at (X0,Z0), with pivot-0 rotation.

---

## 10. Cheat sheet: observed blocks -> (X0,Z0)
| structure | anchor | X0, Z0 |
|---|---|---|
| desert_pyramid / jungle_pyramid / swamp_hut | whole bbox | minX, minZ (exact) |
| monument | whole 58x58 | minX+29, minZ+29 (minX ≡ 3 mod 16) |
| igloo | top dome 7x8 | §2 table (4 hypotheses, one aligned) |
| shipwreck | hull ends | Z-axis: minX, {minZ \| maxZ-30}; X-axis: {maxX-19 \| minX+11}, minZ-11 |
| ocean ruin | primary (big 16x16 if clustered) | the bbox corner that is ≡0 mod 16 |
| ruined portal | frame template bbox | 104 hypotheses (§7), filter by alignment |
| mansion | entrance piece 21x16 | §4 table |
| village / trail ruins / trial chambers | start template | a bbox corner (§8, adjust for a,c) |
| pillager outpost | watchtower 15x15 | 8 candidates (§8.3), filter by alignment |
| ancient city | city_center | §8.5 |

After you have the chunk: `(cx, cz) = (X0>>4, Z0>>4)`. Feed it to
`RandomSpreadStructurePlacement.getPotentialStructureChunk` inversion (see structure_cracking.md §1).
The first `nextInt(4)` of the chunk's `setLargeFeatureSeed` random (rotation or direction) gives 2 extra bits per
structure when the rotation is identifiable (§0.2 mappings). For ruined portals the rotation is the 4th-6th call.
