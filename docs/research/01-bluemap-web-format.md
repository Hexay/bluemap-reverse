# 01 — What a BlueMap website exposes (source-level)

Source: `github.com/BlueMap-Minecraft/BlueMap` main @ `60e733f` (2026-09-25). Latest tag **v5.27** (2026-09-16);
`git diff v5.27 HEAD` touches none of the files cited here (only SQL command sets, BlockState, CLI, markers).
Paths below are relative to repo root; `core/…/` = `core/src/main/java/de/bluecolored/bluemap/core/`,
`webapp/` = `common/webapp/src/js/`. Local clone: scratchpad `bm_src_a`.

## 1. URL / file layout

Webroot (`common/…/WebFilesManager.java`, `webapp/BlueMapApp.js`):

| URL | Content |
|---|---|
| `settings.json` | webapp settings: `version, useCookies, defaultToFlatView, startLocation, resolutionDefault, min/maxZoomDistance, hires/lowresSlider*, mapDataRoot ("maps"), liveDataRoot ("maps"), clientDecompression (false), maps[] (ids), scripts[], styles[]` |
| `maps/<id>/settings.json` | per-map (`core/…/map/MapSettingsSerializer.java`), see below |
| `maps/<id>/textures.json` | texture table (index = PRBM material index), see §2.4 |
| `maps/<id>/tiles/0/<path>.prbm` | hires tile (PRBM) |
| `maps/<id>/tiles/<lod>/<path>.png` | lowres tile, lod = 1..lodCount |
| `maps/<id>/live/markers.json`, `live/players.json` | markers / players (JSON). Built-in server also: `live/sse` (EventSource: `tile {x,y,lod}`, `player`, `marker`) |
| `maps/<id>/assets/…` | map assets (e.g. `assets/playerheads/<uuid>.png`) |

Per-map settings.json fields: `name, sorting, hires{tileSize:[32,32], scale:[1,1], translate:[2,2]},
lowres{tileSize:[500,500], lodFactor:5, lodCount:3}, startPos:[x,z], skyColor:[r,g,b,a], voidColor:[…],
ambientLight, skyLight, perspectiveView, flatView, freeFlightView`. Vectors are JSON arrays; webapp reads
`[0]`=x, `[1]`=z (`vecArrToObj(val,true)` in `webapp/util/Utils.js`). Colors are float RGBA arrays.
**Not exposed:** world path, dimension, render-mask, remove-caves-below-y, cave-detection-*, render-edges,
edge-light-strength, ignore-missing-light-data, min-inhabited-time, enable-hires, storage type, MC version,
resource packs. These must be inferred (§4).

### Tile path scheme (digit split)
`FileGridStorage.getItemPath` / `pathFromCoords` (webapp `util/Utils.js`): string `"x"+X+"z"+Z`, split into a
folder after **every digit**; last segment gets the suffix.
`x=-12,z=345` → `x-1/2/z3/4/5.prbm`. `x=0,z=7` → `x0/z7.prbm`.
Built-in webserver regex `tiles/([\d/]+)/x(-?[\d/]+)z(-?[\d/]+).*` strips `/`, so the unsplit form
`tiles/0/x-12z345.prbm` also works there (not on static hosting).
Missing tile → **HTTP 204** (built-in `MapStorageRequestHandler`, `sql.php`, and the documented nginx config).

### Compression / serving
- File storage default `compression: gzip` (`config/storages/file.conf`); options none/gzip/deflate/zstd/lz4
  (`core/…/storage/compression/Compression.java`, suffixes `""/.gz/.deflate/.zst/.lz4`).
- On disk: `tiles/0/**.prbm.gz`, `textures.json.gz`. Lowres PNGs and `settings.json`, `live/*.json` are
  **never** compressed (`FileMapStorage.java`).
- Browser requests `…/x0/z7.prbm` and `textures.json` **without** `.gz` (unless root `clientDecompression:true`,
  then it requests `.prbm.gz` / `textures.json.gz` and inflates client-side — `webapp/map/TileLoader.js`,
  `Map.js`). Server must answer with `Content-Encoding: <compression-id>` (built-in server does this if the
  client's Accept-Encoding allows, else recompresses to gzip / sends raw). Explicit `…​.prbm.gz` request to the
  built-in server always returns gzip bytes. Nginx: `gzip_static always;` + `error_page 404 = @empty` (204)
  — https://bluemap.bluecolored.de/wiki/webserver/ExternalWebserversFile.html
- Scraper rule: send `Accept-Encoding: gzip`, sniff magic bytes and decompress yourself regardless of headers
  (`bmr-compress`). Measured on 5.27 (2026-10-02): with `Accept-Encoding: gzip` the built-in server re-encodes
  every storage compression to gzip; stored bytes are `1f 8b` gzip, `78 9c` zlib (`deflate`), `28 b5 2f fd`
  zstd, `LZ4Block` lz4-java blocks (`lz4`, same framing as chunk compression 4), raw for `none`.
- **SQL storage** (`sql.php`, built-in server): identical URLs; blob returned with `Content-Encoding` from the
  row's compression key. Browser-visible difference: none, except static-host-only files don't exist.
- File-storage extras that a static host may leak (not requested by webapp, not served by built-in server):
  `maps/<id>/rstate/**.tiles.dat|.chunks.dat` (gzip, render state) — unverified whether hosts expose them.

## 2. PRBM hires format (`core/…/map/hires/PRBMWriter.java`, reader `webapp/map/hires/PRBMLoader.js`)

Derived from PRWM (github.com/kchapelier/PRWM) + a trailing group table. All little-endian.

| Off | Size | Value |
|---|---|---|
| 0 | 1 | version = `1` |
| 1 | 1 | flags `0b0000_0111`: bit7 indexed=0, bit6 idx-type=0, bit5 big-endian=0, bits0-4 attr count = 7 |
| 2 | 3 | vertex count V = faces×3 (u24 LE, max 0xFFFFFF; model cap 1,000,000 faces) |
| 5 | 3 | index count = 0 (always non-indexed) |
| 8 | … | 7 attributes, each: ASCII name + `\0`, 1 flag byte, zero-pad to 4-byte boundary, V×card values |
| … | … | pad to 4; groups: repeat `{i32 materialIndex, i32 startVertex, i32 vertexCount}`; terminator `i32 -1` |

Attribute flag byte: bit7 type (0=float for all), bit6 normalized, bits4-5 cardinality-1,
bits0-3 encoding (1=f32, 3=i8, 4=i16, 6=i32, 7=u8, 8=u16, 10=u32). Padding counts from file start.

| # | name | flag | stored | meaning |
|---|---|---|---|---|
| 1 | position | 0x21 | 3×f32 | vertex pos, **relative to tile anchor** (tileMinX, 0, tileMinZ); Y absolute world Y |
| 2 | normal | 0x63 | 3×i8 norm | face normal from cross(p2-p1,p3-p1), `(byte)(n*128-0.5)`; same for 3 verts |
| 3 | color | 0x67 | 3×u8 norm | **tint multiplier only** (`(int)(c*255)`): 255,255,255 if face has no `tintindex`; else biome/fixed tint (§2.3). Per-face, repeated ×3 |
| 4 | uv | 0x11 | 2×f32 | **texture-local** UV (model uv/16, face rotation + uvlock applied), not an atlas |
| 5 | ao | 0x47 | 1×u8 norm | per-vertex AO: 1 − 0.25·occluders (0,0.25..1); 1.0 if model `ambientocclusion:false` or liquid |
| 6 | blocklight | 0x03 | 1×i8 | 0..15 raw, per face (×3) |
| 7 | sunlight | 0x03 | 1×i8 | 0..15 raw skylight, per face (×3) |

Groups: faces are **stable-merge-sorted by materialIndex** before writing (`ArrayTileModel.sort`); one group per
distinct texture, `start`/`count` in vertices. Within a group, faces keep emission order (see §3).

### 2.1 Geometry facts useful for reverse
- Every model face quad = 2 triangles: (c0,c1,c2),(c0,c2,c3); uv (u0,u1,u2),(u0,u2,u3)
  (`ResourceModelRenderer.createElementFace`). Quad corner order per direction is fixed (DOWN c0,c2,c3,c1 …).
- Model element coords are /16 then variant rotation (x/y, uvlock) matrix, then block translate; element
  rotation also applied. Random XZ offset (±0.375) for `randomOffset` blocks (flowers, grass…, list in
  `core/src/main/resourceExtensions/assets/minecraft/blockProperties.json`), hash `hashToFloat(x,z,seed)`.
- Face-level culling uses the model face's `cullface`; faces without cullface are always emitted.
### 2.2 Light
Face sun/block light = max(own block light, light of neighbor in face direction)
(`ResourceModelRenderer`). Liquids: up face uses own; sides use neighbor. `blocklight` also max'd with element
`light_emission`. Light is emitted per tile, not per block → skylight reveals roofs/caves.
### 2.3 Tint / color (`resourceExtensions/assets/minecraft/blockColors.json`, `…/hires/block/color/*`)
`@grass`/`@foliage`/`@water` from biome colormaps, **blended over a 5×3×5 neighborhood**
(`BlendedBlockColorCalculator`, h=2,v=1), `@redstone` by power, fixed ints (birch 0x80a755, spruce 0x619961).
So color gives smoothed biome climate, not an exact per-block biome; redstone power is recoverable from wire tint.
### 2.4 textures.json (`core/…/map/TextureGallery.java`, `resources/pack/resourcepack/texture/Texture.java`)
JSON array; index = materialIndex. Entry: `{resourcePath:"minecraft:block/stone", color:[r,g,b,a] (avg,
straight), halfTransparent:bool, texture:"data:image/png;base64,…", animation?:{…mcmeta}}`.
Index 0 = `bluemap:block/missing`. Then all pack textures sorted: opaque (avg alpha==1) first, then
translucent, each alphabetical. **Includes full PNGs + resource keys** → face→texture name is direct.
Animated textures: full strip PNG; shader samples `frameHeight*(v+frameIndex)` so uv.v ∈ [0,1] per frame.
Order is per-map and depends on loaded packs/MC version; never hardcode ids.

## 3. Hires tiling and coordinates

- `BmMap`: `new Grid(hiresTileSize=32, offset=2)` → tile (tx,tz) covers world X ∈ [32·tx+2, 32·tx+33],
  Z likewise (`core/…/util/Grid.java`). Webapp places mesh at `tx*tileSize.x+translate.x` (`TileLoader.js`).
  World pos = vertex.xz + (32·tx+2, 32·tz+2); Y = vertex.y. Block (x,y,z) full cube spans [x,x+1].
- `hiresTileSize/lowresTileSize/lodCount/lodFactor` are configurable (MapConfig defaults 32/500/3/5) — always
  read settings.json. translate is fixed 2 in code (not from config).
- Y range: `chunk.getMinY..getMaxY` per column (dimension height, e.g. -64..319), clipped by render mask.
- Emission order (`BlockRenderPass`): for x asc, for z asc, for y **desc**, per block: model faces
  (element order, faces D,U,N,S,W,E) then waterlogging water; then entities pass. After stable sort by
  texture, relative order inside each texture group still follows (x,z,y↓) — usable to disambiguate.
- Tile edges: neighbour lookups cross tile borders (full world access), so there are no seam faces.

## 4. What BlueMap does NOT emit / transforms

| Topic | Behaviour (source) |
|---|---|
| Air | `air`, `cave_air`, `void_air` skipped (`BlockState.isAir`, `BlockStateModelRenderer`) |
| Barrier / light / structure_void / sign-type blocks | rendered from vanilla models; blocks whose vanilla model has no `elements` (barrier, light, structure_void, moving_piston, beds, signs, … — **verify against the target MC jar**) produce **no geometry** |
| Unknown blockstate (no resource) | `getBlockState()==null` → nothing emitted (uncertain whether ResourcePool falls back to `bluemap:missing`) |
| Face culling | face dropped if its `cullface` neighbor is `culling` (model has a full-cube element with all 6 faces textured with avg alpha==1, or override), or `cullingIdentical` and same state (glass, stained glass, ice). Leaves/glass → not culling. Mushroom blocks forced culling. (`ResourcePack.loadBlockProperties`, `Model.calculateProperties`) |
| AO occluders | `occluding` = model has a full-cube element (glass overridden false) |
| Caves | face dropped if `y < remove-caves-below-y` (default 55) AND (no ocean-floor heightmap OR `y < oceanFloorY + cave-detection-ocean-floor` (default 10000 ⇒ effectively always)) AND light==0, where light = face sunlight, or max(sun,block) if `cave-detection-uses-block-light`. Per-face, not per-block (`ExtendedBlock.isRemoveIfCave`) |
| Render mask / bounds | blocks outside mask skipped. With `render-edges` (default true) outside-mask blocks are treated as AIR with skylight=`edge-light-strength` (15 default; 8 in generated map.conf) so cut faces are drawn |
| ignore-missing-light-data | default false: chunks without light data are not rendered (only CLI/map-check hints; see `TileHasLightDataCheck`) |
| min-inhabited-time | chunks below threshold not rendered |
| Top-only mode | if hires off or (perspective AND free-flight off): only faces with normal.y>0.01, column stops at first opaque culling block |
| Water / lava | `LiquidModelRenderer`: level-based corner heights (14/16 for source, `14-level*1.9`), top face at 16 if same liquid above; faces between same liquid dropped; side faces culled by `culling` neighbours; top uses `still` texture, or `flow` rotated by flow angle; sides use `flow` (uv scaled 0.5). AO=1. Water tinted `@water` |
| Waterlogging | if `waterlogged=true` or `alwaysWaterlogged` (seagrass, kelp, kelp_plant, tall_seagrass, bubble_column) → extra water model at same block |
| Block entities | BlueMap ships static models (resourceExtensions) for: chest/trapped/ender chest, shulker boxes, banners (base only, no patterns — unverified), skulls/heads, decorated_pot, conduit, bell, cake, copper_golem_statue, bubble_column, legacy `sign` id. **No** sign text, no NBT (chest contents, banner patterns, player-head skins — unverified). Beds/signs: only if vanilla model has elements |
| Entities | `EntityRenderPass` reads entity files but renders only ids with an `entitystates/*.json` in the resource pack; built-in only `bluemap:missing` ⇒ vanilla entities not rendered unless an addon/pack provides states |
| Biome, block id, state props | never stored explicitly; must be inferred from texture keys + geometry + tint |

## 5. Lowres tiles (`core/…/map/lowres/LowresTile.java`, `LowresLayer.java`, `webapp/map/lowres/LowresVertexShader.js`)

- PNG ARGB, size `(tileSize+1) × 2(tileSize+1)` = 501×1002 by default; +1 row/col duplicates the next
  tile's first pixel (seamless edges).
- Top half pixel (x,z): column color, straight ARGB = alpha-composited (top→down) map colors of all
  non-transparent faces' **top faces** (texture avg color × tint × light), with light factor
  `(1-ambient)*max(sun,block)/15+ambient`.
- Bottom half pixel (x, 501+z): `A=255, R=blockLight(top-weighted), G=height>>8, B=height&0xFF`;
  height = signed 16-bit, highest y with non-zero color alpha (0 if none).
- Grid(500, offset 0): lod1 pixel = 1 block, tile (tx,tz) covers X ∈ [500tx, 500tx+499]. lod n: each pixel
  = lodFactor^(n-1) blocks, averaged (color premultiplied mean, height mean, light mean) from lod n-1.
- Path: `tiles/<lod>/<digit-split>.png`, never compressed. Written even when hires disabled.
- Reverse value: a cheap full-map heightmap + top color + top blocklight — good for seeding/validation,
  and covers areas where hires was disabled. Height is of the top **visible (alpha>0)** block, includes water.

## Reconstruction implications (summary)
- Helps: texture keys+PNGs shipped per map; exact float geometry; per-face light; tint = smoothed biome;
  texture groups + stable emission order; lowres heightmap.
- Limits: interior of solid volumes culled (fully hidden blocks unrecoverable); caves below y55 with sky 0
  removed; invisible/no-element blocks absent; no block entity NBT, no entities; biome only blended; block
  state props must be inferred from model geometry (orientation, stairs shape, etc.); tiles 204 where unrendered.
