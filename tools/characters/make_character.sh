#!/usr/bin/env bash
# The character pipeline, end to end, on the Studio subscription's credits:
#
#   tools/characters/make_character.sh NAME SHEET.png HEIGHT
#
# SHEET is an approved design sheet in a T-pose, front view on the left. Steps:
#   1. crop the front view (up to the first clear gap), make a textured model on Studio
#   2. rig it with Tripo's auto-rig on Studio, its Unreal skeleton
# (credits for each: tools/tripo/README.md)
#   3. the rest, free, with finish_character.sh: convert the rig onto our skeleton, check
#      it, render it moving, and export it as the game loads it
# Keeps what cost credits as source art, in art/characters/NAME/: sheet.png, front.png,
# model.glb and rigged.glb. Commit them. The rest goes to tools/characters/out/.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
name=$1 sheet=$(realpath "$2") height=${3:-1.80}
art="$root/art/characters/$name"
mkdir -p "$art"
[[ "$sheet" == "$art/sheet.png" ]] || cp "$sheet" "$art/sheet.png"
"$root/tools/characters/front_view.sh" "$art/sheet.png" "$art/front.png"
tripo="$root/tools/tripo/tripo.ts"
made=$(node "$tripo" model "$art/front.png" --faces 30000 -o "$art/model.glb")
project=$(node -p 'JSON.parse(process.argv[1]).project' "$made")
node "$tripo" rig "$project" --skeleton unreal -o "$art/rigged.glb"
"$root/tools/characters/finish_character.sh" "$name" "$height" "$art/rigged.glb"
