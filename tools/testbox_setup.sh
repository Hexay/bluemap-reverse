#!/usr/bin/env bash
# One-time testbox setup (tools/testbox.py): Rust, the release bmr, downloads, and the library fixtures that
# `bmr reverse` needs (debug world + mirror, template-void). Idempotent: every step skips what exists.
set -euo pipefail
command -v cargo >/dev/null || { curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal; . "$HOME/.cargo/env"; }
cargo build --release -p bmr-cli
python3 tools/setup.py
python3 tools/make_world.py template-void
python3 tools/make_world.py debug
python3 tools/mirror_fixture.py debug
