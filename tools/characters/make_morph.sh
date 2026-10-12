#!/usr/bin/env bash
# A body morph of the base, end to end (D112, research/body-morphs.md):
#
#   tools/characters/make_morph.sh NAME "CHANGE"            a design of the base person, redrawn
#   tools/characters/make_morph.sh NAME --design BODY.glb   a body already rigged on our skeleton
#
# CHANGE says how the base person differs, as the image model will read it, e.g. "the man is
# much heavier, overweight with a big round belly, thick arms, thighs and neck, a fuller face".
# Steps, each skipped when what it makes is already kept in art/characters/BASE/morphs/NAME/:
#   1. the base's approved sheet redrawn with the change, same person and T-pose (free)
#   2. its front view, scaled to 2048 on the sheet's own background
#   3. Tripo's model of it (credits: tools/tripo/README.md)
#   4. the base registered onto the model (morph.py): the morph and the base at full weight
#   5. that body exported as the game loads it (export_character.py), keeping the base's patches
#   6. every worn look made for the base carried onto it (carry_gear.py), and checked (gear_check.py)
# Look at the sheet before step 3 spends credits, and at the result with compare.py after.
# BASE defaults to base-male; set BASE= to use another. The base is its exported body,
# tools/characters/out/BASE-game.glb (build_bodies.sh or finish_character.sh makes it).
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
name=$1 base=${BASE:-base-male}
blender=tools/blender/run.sh here=tools/characters out=tools/characters/out
mkdir -p "$out"
if [[ "${2:-}" == --design ]]; then
  design=$3
else
  change=$2
  art="art/characters/$base/morphs/$name"
  mkdir -p "$art"
  if [[ ! -f "$art/sheet.png" ]]; then
    prompt="Redraw this exact character turnaround sheet with one change: $change. Keep everything else exactly the same: the same person, the same face and head, the same height and proportions of limbs, the same T-pose with both arms straight out to the sides at shoulder height, palms down, fingers together, legs straight and slightly apart, the same hand-painted style and clothes, front view, side view and back view in the same places, plain light grey background, no text."
    node tools/tripo/tripo.ts image "$prompt" --ref "art/characters/$base/sheet.png" -o "$art/sheet.png" >/dev/null
    echo "$prompt" > "$art/sheet.prompt.txt"
    echo "SHEET $art/sheet.png: look at it before the model is made; rerun to go on"
    exit 0
  fi
  if [[ ! -f "$art/front.png" ]]; then
    ground=$(convert "$art/sheet.png" -format '%[pixel:p{4,4}]' info:)
    convert "$art/sheet.png" -gravity West -crop 44%x100%+0+0 +repage -resize 2048x2048 -background "$ground" -gravity center -extent 2048x2048 "$art/front.png"
  fi
  [[ -f "$art/model.glb" ]] || node tools/tripo/tripo.ts model "$art/front.png" --faces 30000 -o "$art/model.glb" >/dev/null
  design="$art/model.glb"
fi
$blender $here/morph.py "$out/$base-game.glb" "$design" "$name" | grep -E "MORPH|EXPORTED|Error" || true
$blender $here/export_character.py "$out/$name.glb" "$out/$name-game.glb" | grep -E "PATCHES|EXPORTED|Error" || true
looks=()
for look in tools/characters/out/gear/$base/*.glb; do
  item=$(basename "$look" .glb)
  $blender $here/carry_gear.py "$out/$base-game.glb" "$look" "$out/$name-$item.glb" "$name" | grep -E "Error" || true
  looks+=("$out/$name-$item.glb")
done
$blender $here/gear_check.py "$out/$name-game.glb" "${looks[@]}" | grep -E "^GEARCHECK|Error" || true
