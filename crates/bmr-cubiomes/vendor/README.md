# Vendored cubiomes

- Upstream: <https://github.com/xpple/cubiomes> (fork of Cubitect/cubiomes), commit `18edd56`. MIT, see `LICENSE`.
- Vendored: the library sources (`*.c`, `*.h`, `features/`, `tables/`). Dropped: `tests.c`, `loot/`, build files.
- Built by `../build.rs` (every `.c` here and in `features/`, plus `../shim/shim.c`).

## Local changes

MSVC has no VLAs and rejects some GCC-isms. Each edit is tagged `bmr patch` in the source
(`rg "bmr patch" vendor`): VLAs replaced by max-size arrays, constants or `alloca` (`carver.c`, `finders.c`,
`terrainnoise.c`, `features/ore.c`, `features/abandoned_camp.c`), and one `static` table made non-static
(`finders.c`, shipwreck info). `../shim/msvc_compat.h` is force-included under MSVC only, mapping
`__builtin_popcountll` and `alloca`.

## Updating

1. Copy the new upstream sources over this directory, keeping the same subset.
2. Re-apply the `bmr patch` edits (diff against the old commit to find them) and fix any new VLAs MSVC rejects.
3. Update the commit here and in `../build.rs`; add any new MC version to `VERSIONS` in `../src/lib.rs` and
   new structure sets to `../shim/shim.c`.
4. `cargo test -p bmr-cubiomes` and `cargo test -p bmr-seed` (fixture truth tests).
