# 02 — Inverting BlueMap hires meshes to block states

Source: BlueMap `master` @ `60e733f` (shallow clone). Paths below are relative to
`core/src/main/java/de/bluecolored/bluemap/core/` unless they start with `core/src/main/resourceExtensions/`.
"VERIFY" = unverified or from memory, check before relying on it.

## 1. Forward pipeline (what we invert)

### 1.1 Tile iteration and triangle order
- `map/hires/HiresModelManager.render`: one `ArrayTileModel` per tile; runs each configured `RenderPass`
  (`BLOCKS`, optional `ENTITIES`, see `map/hires/RenderPassType`), then `model.sort()`, then PRBM write.
- `map/hires/block/BlockRenderPass.render`: loops `x` ascending, `z` ascending, `y` **descending** (maxY..minY);
  each block is rendered at local origin then `translate(x-anchorX, y, z-anchorZ)`; anchor = tile min x/z, y=0.
- `ArrayTileModel.sort` sorts triangles by `materialIndex` with a **stable** merge sort (`util/MergeSort`, `<= 0`).
  **Consequence: inside one material group, triangles keep emission order**
  `(x asc, z asc, y desc) → variant (multipart part order) → element order → face order D,U,N,S,W,E → tri0,tri1`,
  then the waterlogging water model after the block. This is a free monotonic "which block" signal.
- Each face = 2 triangles `(c0,c1,c2)` and `(c0,c2,c3)` (`ResourceModelRenderer.createElementFace`), non-indexed.

### 1.2 PRBM attributes (`map/hires/PRBMWriter`)
| attr | encoding | notes |
|---|---|---|
| position | f32×3 | tile-local; exact for 1/16 steps, trig noise for rotated elements/variants |
| normal | i8 normalized ×3 | **recomputed** from triangle positions (cross product), `(byte)(v*128-0.5)` |
| color | u8 ×3 | `(int)(v*255)` truncation; tint or white |
| uv | f32 ×2 | per-texture 0..1 (NOT an atlas); face `uv/16`, face rotation, uvlock rotation |
| ao | u8 | per vertex; values 255/191/127/63 (1, .75, .5, .25) |
| blocklight, sunlight | i8 | per face (same for 3 verts), 0..15 exact |
| material groups | (texId, start, count) | texId = index into `textures.json` |

### 1.3 textures.json (`map/TextureGallery`)
- Array of `Texture` (`resources/pack/resourcepack/texture/Texture.java`): `resourcePath` (e.g.
  `minecraft:block/stone`), `color` (avg), `halfTransparent`, `texture` (base64 PNG data URI), `animation`.
- Id 0 = missing texture; rest sorted by (translucent?, key). IDs are **map-specific** → always join by
  `resourcePath`, never by id. We get every texture PNG for free (useful for UV sanity checks).
- VERIFY: for animated textures, whether UV v is per-frame 0..1 (shader handles frames) — looks so, since UVs
  are never scaled by frame count in the renderer.

### 1.4 Block state → variants (`resources/pack/resourcepack/blockstate/*`)
- `BlockStateModelRenderer.render`: air shortcut; render the state's variants; if `waterlogged=true` or
  property `alwaysWaterlogged` → additionally render `BlockState.WATER` (liquid renderer) in the same cell.
- `Variants.forEach`: **first** matching condition wins, else default, else first entry.
  `Multipart.forEach`: every matching part.
- Weighted random pick, `VariantSet.forEach`:
  ```java
  hash = x*73438747L ^ y*9357269L ^ z*4335792L;              // Java long wrap
  f = ((hash*(hash+456149)) & 0xFFFFFF) / (float)0x1000000;
  sel = f * totalWeight; for v in variants { sel -= v.weight; if (sel <= 0) pick v }
  ```
  Fully deterministic from world position → **the variant (and thus random texture rotation/mirror) is
  predictable**, not a free unknown. Multipart parts at one position all share the same `f`.
- `Variant` x/y/z rotation → matrix `translate(-.5)·rotateYXZ(-x,-y,-z)·translate(.5)`; uvlock counter-rotates UVs.

### 1.5 Model → faces (`map/hires/block/ResourceModelRenderer`)
- Per element, per face present: corners from `from/to`; default UVs from `Element.calculateDefaultUV`.
- **Face culling** (per face with `cullface`, rotation-relative neighbor):
  skip if neighbor `properties.isCulling()`, or `cullingIdentical && neighborState == ownState`.
  `culling` defaults (`ResourcePack.loadBlockProperties` → `Model.calculateProperties`): model has a full-cube
  element with all 6 faces textured with **fully opaque** textures. `occluding` = has any full-cube element.
  Overrides: `core/src/main/resourceExtensions/assets/minecraft/blockProperties.json` (glass: non-occluding +
  cullingIdentical; ice: cullingIdentical; mushroom blocks forced culling; seagrass/kelp/bubble_column
  alwaysWaterlogged; ~26 plants `randomOffset`).
- **Cave removal**: if `y < remove-caves-below-y` and (no OCEAN_FLOOR heightmap or `y < oceanFloor +
  cave-detection-ocean-floor(-5)`) and face light (`max(own,facing)` sky, optionally block) == 0 → face dropped
  (`ExtendedBlock.isRemoveIfCave`).
- `renderTopOnly` config drops faces with rotated normal.y < 0.01 (VERIFY target map's config).
- **Tint**: any face with `tintindex >= 0` gets the block's single tint (`BlockColorsConfig` →
  `blockColors.json`), else white. Calculators (`map/hires/block/color/BlockColorCalculatorType`):
  - `@grass`: colormap/grass(biome) ⊕ biome overlay ⊕ grass modifier (dark_forest formula, swamp =
    SimplexNoise(Random(2345)) at `x*0.0225,z*0.0225` → 2 fixed colors), **blended**.
  - `@foliage`, `@dry_foliage`: colormap ⊕ overlay, blended. Birch/spruce leaves, lily pad, stems (by `age`),
    banners/shulker boxes (by color) are fixed hex.
  - `@water` (water, water_cauldron): `biome.waterColor`, blended.
  - `@redstone`: `r=(power+5)/20, g=b=0` → u8 r = 63,76,89,…,255: **power is exactly recoverable**.
  - Blend (`BlendedBlockColorCalculator`): mean of 5×3×5 (dx,dz ∈ ±2, dy ∈ ±1) per-block colors.
  - Colormap (`texture/ColorMap`): `t=clamp(temp), d=clamp(downfall)*t`, pixel `((1-t)*255,(1-d)*255)`.
    Raw biome temperature — no altitude adjustment.
- **Light**: `sun = max(own.sky, facing.sky)`, `block = max(own.block, facing.block)`; `facing` = neighbor in the
  (rotation-relative) element face direction — note: face dir, not cullface. Blocklight additionally
  `max(…, element.light_emission)`.
- **AO** (`testAo`, only if model `ambientocclusion != false`): per vertex, uses **unrotated element-space**
  corner coords; axis sign = ±1 only if coord is exactly 0 or 16, else 0. Tests up to 4 neighbors (two edges
  in the face's outer layer, the third axis pair, corner) with `dot(offset, faceDir) > 0`; each
  `isOccluding()` neighbor −0.25, min 0.25 (clamped at 3). Neighbors are rotation-relative.
- No directional shading is baked (`Element.shade` unused); shading happens in the webapp shader from normals.
- `randomOffset` blocks: whole model translated by `((hash2(x,z,123984)-.5)*.75, 0, (hash2(x,z,345542)-.5)*.75)`,
  `hash2 = x*73428767L ^ z*4382893L ^ seed*457` then same fold. Deterministic.

### 1.6 Liquids (`map/hires/block/LiquidModelRenderer`, `blockstates/water.json` → `"renderer":"bluemap:liquid"`)
- Full-cell box, faces D,U,N,S,W,E; face dropped if neighbor is same liquid (water ≡ water | waterlogged |
  alwaysWaterlogged) or (non-UP) neighbor `culling`. AO always 1. Tint = blended water color (lava: white).
- Top corners: 16 if own `level>=8`, or source with water above, or any of the 4 corner-sharing cells above is
  water; else per corner over the 2×2 cells: any source → 14; else mean of `14-1.9*level` (level≥8 → 16) over
  same-liquid cells, with non-air non-liquid cells excluded and **air counted as 0 height**.
- Top UV: still texture if level 0 (height>0.8·16); else flow texture, UVs scaled 0.5 and rotated by flow
  angle from neighbor height differences → encodes flow direction. Sides: flow texture, scaled 0.5.
- Light for top = own cell; sides = the neighbor cell.

### 1.7 Special blocks (static models only)
- `core/src/main/resourceExtensions/**` ships blockstates+models for chest/trapped/ender/copper chests,
  beds (`beds/`), signs/hanging signs (`signs/`), banners, skulls/heads, shulker boxes, decorated pot, conduit,
  bubble column, copper golem statues, water/lava. Plus `mc1_17`, `mc1_20_3`, `mc1_21_9`, `mc26_1` overlays.
- Chest `type=left` → `entity/chest/left` = `{}` (**renders nothing**); `type=right` draws the whole double chest
  spanning 2 cells. Infer left half from the right half's geometry.
- **Block entities are never read by the hires renderer** (`getBlockEntity()` has no renderer callers): sign
  text, banner patterns, skull owners, pot sherds, container contents, spawner mobs → unrecoverable.
- Entity pass (`map/hires/entity/*`) uses `entitystates`; vanilla ships only `bluemap:missing`. Assume off /
  ignorable unless target config enables it (VERIFY).

## 2. Lookup-table inversion

### 2.1 Build signatures with a forward renderer, validate with a debug world
Recommended: **port the forward model to Rust** (sections 1.4–1.6 are ~600 lines of Java) and load the same
resource pack BlueMap uses (client jar assets + BlueMap `resourceExtensions` from the BlueMap jar). That gives,
for every state × variant × position hash, the exact face list *with cullface metadata* — which a rendered
isolated sample cannot show. Use renders only to validate the port bit-exactly:
- **Debug world** (world preset `minecraft:debug_all_block_states`): every state once at y=70, grid spacing 2,
  barrier floor at y=60 (VERIFY constants in `DebugLevelSource`). Isolated → no culling, full AO=1,
  connection states as stored (debug world does not update shapes). One position per state → one random variant.
- Complement with a generated world from our own MCA writer: each state at several positions (covers variant
  hashes), plus contexts (stone neighbors per side to exercise culling; glass/ice for cullingIdentical).
- Test oracle: forward-render our reconstructed world and diff against the observed PRBM (analysis by
  synthesis). This is also the scorer's best debugging tool.

### 2.2 Face fingerprint index
Key per quad (after assigning its cell, §3): `(texKey, 4 corners in cell-local 1/16 units rounded to 1/256,
4 UVs rounded to 1/1024, tinted?)`, canonicalized over corner cyclic order. Map key → `[(state, variantIdx,
faceIdx)]`. Cell hypothesis = intersect/vote over its quads, then **exact verification**: predicted face set at
`(x,y,z)` (known variant via hash) minus faces whose cullface neighbor is `culling` must equal observed.
Unexplained missing faces → constraint "that neighbor is culling" (feeds hidden-block inference).

### 2.3 Ambiguity catalogue
| class | examples | resolution |
|---|---|---|
| Visually identical states | `infested_*` vs base, `waxed_*` copper vs unwaxed, `petrified_oak_slab` vs `oak_slab`, `*_slab[type=double]` vs full block (same model for most), air/cave_air/void_air | prior: pick the common one; score these as equivalent |
| Non-visual properties | leaves `distance/persistent`, note_block `instrument/note/powered`, sapling `stage`, cactus/sugar_cane/kelp/fire `age`, hopper `enabled`, dispenser `triggered`, daylight_detector/target `power`, bed `occupied`, lectern `powered`, bell `powered`, tripwire `disarmed` | recompute by game rules where possible: leaves distance = BFS to logs (≥7 ⇒ persistent=true), note_block instrument from block below, else default value |
| Encoded in tint | redstone_wire `power` (exact), stems `age` (fixed colors) | read r channel / exact color |
| Encoded in blocklight | `minecraft:light[level]` (invisible), `redstone_ore[lit]`, other lit-only-by-light states | fit light field (§4.2) |
| waterlogged | block + water model in same cell | detect water quads in cell; if fully submerged no water quads exist → use *neighbor* water side faces: water draws a side toward a non-culling non-waterlogged cell, none toward a waterlogged one |
| Culled faces | fully enclosed blocks emit nothing | unrecoverable → priors (stone/deepslate by y, dirt under grass, etc.) |
| Cave-removed | below threshold with light 0 | unrecoverable; same priors |
| Connected models | fences/walls/panes/bars/redstone/stairs shape/chorus | multipart geometry shows connections directly; cross-check with neighbor rules |
| Random variants | stone/grass/sand rotations, mirrored models | not ambiguous: variant = f(x,y,z) (§1.4) — use as a position/consistency check |
| Door/trapdoor open vs other facing | same box, different UV mirroring | UVs disambiguate (verify per state in table build) |
| Double chest left half | renders nothing | pair with adjacent `type=right` |
| Block-entity data | signs, banners, heads | unrecoverable |

## 3. Voxelization (quads → cells)
1. Re-pair triangles into quads: consecutive tris sharing `c0` and `c2` within a material group.
2. Normal from positions (don't trust i8 normals for exactness). Cell guess =
   `floor(centroid - ε·normal)` (ε ≈ 1/64) + anchor offset: faces on a cell boundary belong to the cell *behind*
   them (outward normals), interior faces are inside anyway.
3. Correct overhanging models with the **stable emission order** (§1.1): within a material group the source
   key `(x, z, -y)` must be non-decreasing; a quad whose ε-cell breaks monotonicity belongs to the nearest
   consistent predecessor cell. Known overhangers: double chest (right half spans 2 cells), piston head arm,
   `randomOffset` plants (subtract predicted offset per (x,z) first), bells, hanging signs, tall rotated elements.
4. Partial blocks need no special handling — the fingerprint is cell-local 1/16 geometry, so slabs/stairs/
   carpets/layers match directly. Tolerance ~1e-4 on positions/UVs for rotated variants (trig noise).
5. Liquids: water/lava quads have their own textures (`block/water_still|flow`, `block/lava_*`) → separate
   stream. Solve levels per connected water body from corner heights (linear constraints over 2×2 corner
   neighborhoods; source ⇔ corners 14 & still texture) plus flow-angle UV rotation; falling water = 16 corners.

## 4. Extra information from vertex attributes

### 4.1 Biome from tint
- Per tinted block: observed = mean over 75 cells (5×3×5) of per-cell biome color. Biomes are stored per 4×4×4
  quart, and any 5 consecutive ints span exactly 2 quarts → each observation is a known-weight mix of ≤2×2×2
  quart colors. Solve per quart over the discrete biome palette (greedy/ILP or least squares then snap);
  heavily overdetermined on grassy terrain.
- Three independent channels where present: grass, foliage (oak/jungle/acacia/dark_oak/mangrove leaves,
  vines), water color. Biomes with equal temp/downfall+water color collapse into classes (e.g.
  plains/sunflower_plains) → pick by prior. Swamp and dark_forest grass modifiers are distinctive.
- Needs biome temp/downfall/colors/overlays from the same MC version's datapack (`world/biome/Biome`).
- Error sources: u8 truncation, and y±1 blending into cave biomes. Untinted areas give no biome info.

### 4.2 Light
- Face light = light of the air cell in front (for opaque blocks own light is 0). So visible surfaces sample
  the full sky/block light field of adjacent air.
- Blocklight: forward-simulate vanilla propagation on the reconstructed opacity grid from known emitters; the
  residual localizes **hidden emitters** (`light` blocks, lit redstone ore, emitters behind culled walls).
- Skylight < 15 under open sky reveals light-filtering blocks in the column (leaves, water, ice, cobweb —
  VERIFY per-block lightBlock values for the MC version).
- Also used by the scorer to confirm cave-removal reasoning (why a face is missing).

### 4.3 AO
- Per-vertex occluder *count* over 3–4 neighbors in the layer in front of the face. Summed constraints across
  adjacent faces solve for `occluding` of the ring cells. Mostly redundant with visible geometry, but useful
  at render edges, beside cave-removed regions, and to separate occluding (full-cube element, e.g. leaves) from
  non-occluding (glass) neighbors. Faces of models with `ambientocclusion:false` are always 255.

## 5. Recommended algorithm
1. Parse tile PRBMs + `textures.json`; map material ids → texture keys; reconstruct world positions.
2. Quadify, assign cells (§3), build per-cell quad lists; separate liquid quads.
3. Precomputed table (from Rust forward renderer): `FaceKey → candidates`, and per state×variant full face
   lists with cullface + tintindex + element light emission.
4. Per cell: candidate states by voting, predicted variant via `VariantSet` hash, exact verification with
   culling rule. Multipart: set-cover the quads with parts consistent with one state.
5. Resolve tints: redstone power, stems; biome solve per quart (§4.1).
6. Liquids + waterlogging (§1.6, §2.3).
7. Global constraint pass: connection rules (fence/wall/pane/stairs shape), double chests, doors/beds halves,
   leaves distance, note_block instrument; hidden cells from "neighbor must be culling" + priors.
8. Light-field fit for hidden emitters (§4.2).
9. Forward-render the result and diff against input; iterate on residuals.

Data structures: `HashMap<FaceKey, SmallVec<Cand>>`; per-tile dense `[u16 stateId; 16×16×H]`; state registry
from the MC version's block report (`java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --reports`);
per-quart biome grid.

## 6. Realistic accuracy (surface-visible blocks, VERIFY with the test loop)
| category | expected |
|---|---|
| visible full cubes | ~100% up to identical-model classes (infested/waxed/double slab) |
| partial/rotated blocks (stairs, slabs, doors, trapdoors, rails, fences, walls, panes) | ~99% |
| redstone wire incl. power | ~100% (power via tint) |
| plants incl. randomOffset | ~99% |
| non-visual properties | only via recomputation; else default (~50–90% depending on property) |
| water levels / waterlogged | high for exposed water; submerged waterlogging via neighbor faces |
| biomes | quart-level class ~90%+ where tinted blocks exist; unknown elsewhere |
| fully enclosed / cave-removed interior | prior only (dominant stone/deepslate ⇒ decent raw score, ores lost) |
| block-entity data, entities | 0% |

Key uncertainties: animated-texture UV convention; debug-world grid constants; target map config
(`render-edges`, `remove-caves-below-y`, `renderTopOnly`, entity pass, custom resource packs); floating-point
tolerance for uvlock/rotated variants.
