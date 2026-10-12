#!/usr/bin/env bash
# The character pipeline from Tripo's rig on, free: convert, check, render, export.
#
#   tools/characters/finish_character.sh NAME HEIGHT [RIGGED.glb]
#
# RIGGED defaults to art/characters/NAME/rigged.glb, as make_character.sh leaves it. Run it
# again for every character whenever the conversion improves.
#   1. convert the rig onto our skeleton, with forearm twist bones
#   2. check it against our skeleton and the mannequin, and render it moving
#   3. export it as the game loads it, with our clips re-sampled on its skeleton, and sockets
# Writes tools/characters/out/NAME.glb, rig-check.json, NAME-sheet.png and NAME-game.glb.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
name=$1 height=$2
rigged=$(realpath "${3:-$root/art/characters/$name/rigged.glb}")
cd "$root"
blender=tools/blender/run.sh here=tools/characters
$blender $here/convert_rig.py "$rigged" "$name" "$height" 2 | grep -E "WEIGHTS|EXPORTED|Error" || true
$blender $here/rig_check.py "$here/out/$name.glb" | grep RIGCHECK || true
$blender $here/rig_render.py "$here/out/$name.glb" "$name-sheet" | grep -E "SHEET|Error" || true
$blender $here/export_character.py "$here/out/$name.glb" "$here/out/$name-game.glb" | grep -E "SOCKETS|EXPORTED|Error" || true
