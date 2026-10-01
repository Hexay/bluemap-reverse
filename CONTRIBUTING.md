# Contributing

Thanks for helping out. Bug reports, maps that reconstruct badly and pull requests are all welcome.

## Reporting a problem

Open an issue with:

- the `bmr` version or commit, and your OS;
- the command you ran and its full output;
- the Minecraft and BlueMap version of the site, and whether it uses mods or resource packs;
- for a bad reconstruction, coordinates of a spot that's wrong (`bmr explain` output helps a lot).

Only share map URLs you have permission to share.

## Setting up

You need stable Rust (2024 edition), a C compiler and Python 3. The test tooling downloads its own JDK,
Minecraft server and BlueMap.

```sh
cargo build                     # debug build: target/debug/bmr
cargo test --workspace
py -3 tools/up.py superflat     # build, render and serve a fixture world
```

[docs/development.md](docs/development.md) covers the test loop, fixtures, every command and building packs.
Start with [docs/plan.md](docs/plan.md) for the architecture.

## Making changes

- Keep pull requests focused on one change.
- `cargo test --workspace` must pass. CI runs it on Windows, Linux and macOS.
- If you touch reconstruction (matcher, fill, biomes), reverse the affected fixtures with
  `py -3 tools/reverse_fixture.py <fixture>` and include the before/after scores. Each run appends to
  `docs/results/history.jsonl`; commit that alongside the change.
- After matcher changes, reverse both `debug` and `context` and compare `rendered_alike`.
- Performance changes need numbers from `tools/bench.py` (see [docs/performance.md](docs/performance.md)).
- Add a line under `## [Unreleased]` in [CHANGELOG.md](CHANGELOG.md) for anything user-visible.
- Commit messages: a short imperative summary of what changed and why it matters.

## Releasing

1. Bump `version` in the workspace `Cargo.toml`.
2. Rename `## [Unreleased]` in `CHANGELOG.md` to `## [x.y.z] - YYYY-MM-DD`, add a fresh `## [Unreleased]`
   above it and update the links at the bottom.
3. Commit, then `git tag vx.y.z && git push origin vx.y.z`.

The release workflow builds Windows, Linux and macOS binaries with the packs bundled, and publishes them with
that CHANGELOG section as the release notes. Packs themselves are published separately to the `packs` release
(see [docs/development.md](docs/development.md#packs)).

## License

By contributing you agree that your contributions are dual-licensed under MIT and Apache-2.0, as described in
the [README](README.md#license), without additional terms.
