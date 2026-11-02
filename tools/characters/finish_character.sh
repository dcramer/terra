#!/usr/bin/env bash
# The character pipeline from Tripo's rig on, free: convert, check, render, export.
#
#   tools/characters/finish_character.sh NAME HEIGHT [RIGGED.glb]
#
# RIGGED defaults to art/characters/NAME/rigged.glb, as make_character.sh leaves it, or is one
# of Quaternius's bodies under art/characters/quaternius/, which skips step 1 (D599). Run it
# again for every character whenever the conversion improves.
#   1. convert the rig onto our skeleton
#   2. check it against our skeleton and the mannequin, and render it moving
#   3. export it as the game loads it, with our clips re-sampled on its skeleton, and sockets
#   4. check its proportions against a real man's (proportions.py)
# Writes tools/characters/out/NAME.glb, rig-check-NAME.json, NAME-sheet.png and NAME-game.glb.
# The conversion reads SMOOTH (weight smoothing passes, default 2), TRIANGLES (a triangle budget,
# default Tripo's as made) and CREATURE=1 (keep the design's hands and feet, not a person's).
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
name=$1 height=$2
rigged=$(realpath "${3:-$root/art/characters/$name/rigged.glb}")
cd "$root"
blender=tools/blender/run.sh here=tools/characters
# Quaternius's bodies (art/characters/quaternius/) are on our skeleton as made and stay so
# (D599): no conversion, no stretch, no twist bones; the rig is their file itself, and every
# output is proved theirs vertex for vertex (intact.ts). HEIGHT is then unused: theirs as made.
theirs=""
case "$rigged" in "$root/art/characters/quaternius/"*) theirs=1 ;; esac
if [ -n "$theirs" ]; then
  rig=$rigged
else
  $blender $here/convert_rig.py "$rigged" "$name" "$height" 2 | grep -E "WEIGHTS|EXPORTED|Error" || true
  rig=$here/out/$name.glb
fi
# The mannequin's floor contact, which the rig check and the export lift each body to (lift.py).
$blender $here/lift.py | grep -E "^LIFT|Error" || true
$blender $here/rig_check.py "$rig" | grep RIGCHECK || true
$blender $here/rig_render.py "$rig" "$name-sheet" | grep -E "SHEET|Error" || true
$blender $here/export_character.py "$rig" "$here/out/$name-game.glb" | grep -E "SOCKETS|RETARGET|EXPORTED|Error" || true
if [ -n "$theirs" ]; then node $here/intact.ts "$here/out/$name-game.glb"; fi
# Its reach, fingertips and crotch against a real man's (D593).
$blender $here/proportions.py "$here/out/$name-game.glb" | grep -E "^PROPORTIONS|Error" || true
