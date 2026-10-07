#!/usr/bin/env bash
# Builds the simulation library and copies it where this project expects it.
set -euo pipefail
cd "$(dirname "$0")"
../prototypes/sim_toy/build.sh
mkdir -p bin
cp ../prototypes/sim_toy/godot/bin/* bin/
