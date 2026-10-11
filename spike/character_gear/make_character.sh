#!/usr/bin/env bash
# The character pipeline, end to end, on the Studio subscription's credits:
#
#   make_character.sh NAME SHEET.png HEIGHT
#
# SHEET is an approved design sheet in a T-pose (front view on the left third). Steps:
#   1. crop the front view, make a textured model on Studio (about 25 credits)
#   2. rig it with Tripo's auto-rig on Studio (about 20 credits)
#   3. refine nothing (the texture is kept), convert the rig onto our skeleton
#   4. check it against our skeleton and the mannequin, and render it moving
# Writes out/NAME.glb (mesh and skin on our skeleton), out/rig-check.json and out/NAME-sheet.png.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
bake="$here/../model_bakeoff"
name=$1 sheet=$2 height=${3:-1.80}
cd "$bake"
convert "$sheet" -gravity West -crop 44%x100%+0+0 +repage -background '#b0b0b4' -gravity center -extent 2048x2048 "out/tripo/$name-front.png"
tripo="$here/../../tools/tripo/tripo.ts"
made=$(node "$tripo" model "out/tripo/$name-front.png" --faces 30000 -o "out/tripo/$name.glb")
project=$(node -p 'JSON.parse(process.argv[1]).project' "$made")
node "$tripo" rig "$project" --skeleton tripo -o "out/tripo/$name-rigged.glb"
./run.sh ../character_gear/convert_rig.py "out/tripo/$name-rigged.glb" "$name" "$height" 2 | grep -E "WEIGHTS|EXPORTED|Error" || true
./run.sh ../character_gear/rig_check.py "../character_gear/out/$name.glb" | grep RIGCHECK || true
./run.sh ../character_gear/rig_render.py "../character_gear/out/$name.glb" "$name-sheet" | grep -E "SHEET|Error" || true
