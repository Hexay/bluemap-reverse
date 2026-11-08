# BlueMap v5.27 → v5.29 → master: what matters to bmr

Checked 2026-11-07 against `github.com/BlueMap-Minecraft/BlueMap`: v5.27, v5.28 (`0f3a9fb`, 2026-09-25), v5.29
(`a61e143`, 2026-10-10), master `96ae320` (2026-10-30, 20 commits past v5.29). Partial clone in scratchpad
`bluemap-upstream`. Baseline: docs/research/01-bluemap-web-format.md, 02-model-inversion.md.

## Verdict

- **v5.28 and v5.29 change no bytes bmr reads.** PRBM layout, textures.json, lowres PNG encoding, map/root
  settings.json, URL layout, compression, resourceExtensions, culling/AO/light/tint and model code are identical
  to v5.27. These files have zero diff v5.27..master: `PRBMWriter`, `ArrayTileModel`, `storage/compression/*`,
  `WebFilesManager`, `MapSettingsSerializer`, `util/Grid`, `resourcepack/texture/*`,
  `core/src/main/resourceExtensions/**`, `webapp/src/js/map/hires/**`, `webapp/src/js/util/**`.
- **A 5.27 pack works on a 5.28/5.29 site unchanged.** The only effect is the informational
  "site runs BlueMap 5.29, pack was built with 5.27" line (`crates/bmr-pack/src/compat.rs`) and losing the
  `bluemap_match` tie-break in `select.rs` ranking, which only matters between otherwise equal packs.
- **Unreleased master changes liquid geometry** (`fcef695`, see below). bmr's inversion tolerates it, but
  render round-trip diffs don't.

## Changes that touch render or web output

| Commit / PR | In | What changed | Alters tile bytes / meshes / textures / URLs | bmr needs |
|---|---|---|---|---|
| `fcef695` #892 "Inset liquid faces" | master only (site `version` = `5.29-10` or later) | `LiquidModelRenderer.createElementFace` translates every water/lava face by `-faceDir * 0.016/16` = **0.001 block inward** (top down, bottom up, sides inward). Normals unchanged (still recomputed from positions). | **Yes, liquid positions only.** Lone source top is now at y+0.874 instead of y+0.875, bottom at +0.001, sides at 0.001/0.999. | **Nothing for inversion.** `FaceKey` quantum is 1/64 (`bmr-invert/src/face.rs` `Q=64`), so 0.064 units rounds away; `owner()` (centroid − 1e-3·normal) stays in the same cell; `liquid_dir()` y==0 / boundary tests still hold. Liquid faces never reach library signatures (`library.rs`: liquid split off, matched by direction). Edge case: averaged flowing-corner heights with a fractional part in [0.5, 0.564) of a 1/64 step round one quantum lower (e.g. a (6.4+0+0)/3 corner gives 8.53→8.47). Harmless because bmr doesn't solve levels from heights. **Breaks:** `bmr-prbm/src/diff` (QUANT 1/4096 → offset 4.1 units) won't pair liquid faces when the site and the re-render straddle this commit. Round-trip against such a site needs a matching BlueMap build, or a liquid tolerance in diff. |
| `de72057` #881, `2c8a18f`, `5c842fa`, `96ae320` | 5.29 / master | Allocation-only: reused variant consumer, `textureVariableProvider` field instead of a per-call method ref (block and entity `ResourceModelRenderer`, `LiquidModelRenderer`), DoubleSupplier, removed iterators. | No. Same call order, same values. | Nothing |
| `703df39` | 5.28 | `BlockState.fromString` without regex; `BlockStateMapping.fitsTo` iterates the sorted property array. | No for valid strings. Edge: a string with `[` but no trailing `]` is now taken whole as the id instead of regex-splitting. Never occurs in vanilla blockColors/defaultBlockstates. | Nothing |
| `691a525`, `3a50026`, `cce3759`, `d734bfb` (storage transactions / streams) | 5.29 | All map-storage writes go through `FilepartTransaction` (`<name>.filepart`, `<name>.N.filepart` temp, then atomic move with retry). PRBM/PNG/JSON writers are wrapped in `storage.write(x, z, action)`. Writer charset is still UTF-8. | No. Same content, same final paths. A static host could briefly expose a `*.filepart` during a write, but the webapp never requests it. | Nothing |
| `dcf959e` #883 lowres race | 5.29 | `LowresTile` gains a `modified` flag; failed saves are re-marked; unmodified tiles are skipped. | No format change. Fewer stale/missing lowres tiles on live servers, so lowres agrees with hires more often. | Nothing |
| `d3f2a79` debounced map save | 5.28+ | `BmMap.saveDebounced` (15 s) replaces the time-gated save. textures.json is still written only if absent. | No | Nothing |
| `793f346` #882 Accept-Encoding q-values | 5.29 | Built-in server parses `;q=`, and `*` now counts as accepting. | No for bmr: it sends `Accept-Encoding: gzip` (`bmr-fetch/src/http.rs:64`). The re-encode-to-gzip behaviour is unchanged. Sending `*` or `zstd` would now get native storage bytes; bmr already sniffs magic bytes. | Nothing |
| `308d8b2`, `HttpResponse.setBody`, `BlueMapResponseModifier` | 5.28 | Body-stream lifetime fixes. The error-body condition was inverted (`!= null` → `== null`), so 4xx responses now carry a text body. | No for tiles: missing tiles are still 204 (unchanged code path). 404 responses now have a body. | Nothing (`http.rs` treats 204/404 as empty) |
| `7c1813b` #870 headers | 5.28 | Default `Cache-Control: public, max-age=86400, stale-if-error=604800`, `CDN-Cache-Control: max-age=60`. | No | Nothing |
| `088123a` webapp forced reload | 5.28+ | `TileLoader`/`LowresTileLoader` always revalidate forced (SSE) loads. | No (URL unchanged) | Nothing |
| `60e733f` SQL `table-prefix`, `5eef58a` init SQL, commandset rewrites | 5.28 | Table names `<prefix>map` etc. (default `bluemap_`). Shipped `sql.php` **not** updated for prefixes. | No for HTTP. Only matters if bmr ever reads a BlueMap SQL DB directly (it doesn't). | Nothing |
| `2b4c766`, `4d9debf`, `b50a4a4`, `MCARegion`, `MapUpdateService` | 5.28/5.29 | Render-state read cache; `region-file-check-interval` (core.conf, default 5). | No. `rstate/` contents unchanged. | Nothing. Optional key; tools' generated configs keep working. |
| `0eddf85` marker colour space, `e0fb6ec`/`79e1387`/`e121004` path validation, CLI `.build()`→`.get()` | 5.28 | Markers / security / commons-cli API. CLI flags (`-r -f -w -c -v`) unchanged. | No | Nothing |

Everything else in the range is a dependency bump, CI, translation, or gradle wrapper.

## Adding BlueMap 5.29 to bmr

- Download: the asset name is unchanged (`bluemap-5.29-cli.jar` at
  `releases/download/v5.29/`), so `Toolchain.bluemap_url` in `tools/paths.py` works as is. Java target is
  still 25; `setup.py:jar_java_major` reads it from the jar anyway.
- Build: `uv run tools/build_pack.py --mc <mc> --bluemap 5.29` produces `packs/bmr-mc<mc>-bluemap5.29.pack`.
  Run it in the background. To make 5.29 the default, change `DEFAULT` in `tools/paths.py`. Note that
  `DEFAULT` keeps its `work/` layout and `toolchain()` only keys `work/v/<mc>` on mc, so two BlueMap versions
  of the same MC share worlds/bluemap dirs. Use `--force` or separate them if both are needed side by side.
- Necessity: low. A 5.29 pack would be byte-identical in content to the 5.27 one apart from `meta.bluemap_version`.
  Rebuilding is only worth it for the exact-version tie-break, or (later) for a release containing
  `fcef695`. Even then the inversion doesn't care. Only round-trip/diff tooling needs the matching BlueMap.
- Watch for: the first release after 5.29 (v5.30?) will include the liquid inset. When it lands, either
  render round-trips with that version or give `bmr-prbm` diff a ~0.002-block tolerance on liquid faces.
