#!/usr/bin/env bash
# One worn item built alone, on the bodies a full build (build_bodies.sh) has made: the steps of
# that build its game file needs, each skipped when unchanged (build.ts; docs/attachment.md,
# "Fitting a new item"):
#
#   tools/characters/build_item.sh ITEM
#
#   1. made on the base from its data, art/gear/ITEM/fit.json (gear_list.ts): a garment by
#      shell_garment.py or hung_garment.py, a pack or belt piece by fit_gear.py
#   2. carried onto the base and every morph (bodies.json), each pushed clear where its skin
#      comes through in our clips (gear_check.py --fix, three rounds) and checked
#   3. the game's file, assets/models/gear/ITEM.glb, with every morph
#
# Needs a full build in tools/characters/out (its stamps). The item's steps' output is kept in
# tools/characters/out/build-ITEM.log; it ends with ITEM pass, or ITEM fail and each check that
# failed, and exits 1 on a failure.
set -euo pipefail
exec node "$(dirname "$0")/build.ts" --item "${1:?usage: build_item.sh ITEM}"
