#!/usr/bin/env bash
# The character pipeline, end to end, on the Studio subscription's credits:
#
#   make_character.sh NAME SHEET.png HEIGHT
#
# SHEET is an approved design sheet in a T-pose (front view on the left third). Steps:
#   1. crop the front view, make a textured model on Studio (about 25 credits)
#   2. rig it with Tripo's auto-rig on Studio (about 20 credits)
#   3. the rest, free, with finish_character.sh: convert the rig onto our skeleton, check
#      it, render it moving, and export it as the game loads it
# Writes out/NAME.glb (mesh and skin on our skeleton), out/rig-check.json, out/NAME-sheet.png
# and out/NAME-game.glb (the file the game reads).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
bake="$here/../model_bakeoff"
# The sheet may be given relative to anywhere; the steps below run from the bake-off folder.
name=$1 sheet=$(realpath "$2") height=${3:-1.80}
cd "$bake"
convert "$sheet" -gravity West -crop 44%x100%+0+0 +repage -background '#b0b0b4' -gravity center -extent 2048x2048 "out/tripo/$name-front.png"
tripo="$here/../../tools/tripo/tripo.ts"
made=$(node "$tripo" model "out/tripo/$name-front.png" --faces 30000 -o "out/tripo/$name.glb")
project=$(node -p 'JSON.parse(process.argv[1]).project' "$made")
node "$tripo" rig "$project" --skeleton tripo -o "out/tripo/$name-rigged.glb"
"$here/finish_character.sh" "$name" "$height" "out/tripo/$name-rigged.glb"
