#!/usr/bin/env bash
# Builds the simulation library for Linux and, when the cross toolchain is present, for
# Windows, and copies both where the Godot project expects them.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p godot/bin
cargo build --release -p terra_sim_godot
cp target/release/libterra_sim_godot.so godot/bin/ 2>/dev/null || cp target/release/libterra_sim_godot.dylib godot/bin/
if command -v x86_64-w64-mingw32-gcc >/dev/null; then
  cargo build --release -p terra_sim_godot --target x86_64-pc-windows-gnu
  cp target/x86_64-pc-windows-gnu/release/terra_sim_godot.dll godot/bin/
fi
