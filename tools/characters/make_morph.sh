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
#   2. its front view (front_view.sh)
#   3. Tripo's model of it (credits: tools/tripo/README.md)
#   4. the base registered onto the model (morph.py): the morph and the base at full weight
#   5. that body exported as the game loads it (export_character.py), keeping the base's patches
#   6. every worn look made for the base carried onto it (carry_gear.py), pushed clear wherever
#      its skin comes through in our clips (gear_check.py --fix, the clipping fixer), and checked
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
    prompt="Redraw this exact character turnaround sheet with one change: $change. Keep everything else exactly the same: the same person, the same face and head, the same height and proportions of limbs, the same T-pose with both arms straight out to the sides at shoulder height, palms down, fingers together, legs straight and slightly apart, the same style, skin, lighting and clothes, front view, side view and back view in the same places, plain light grey background, no text."
    node tools/tripo/tripo.ts image "$prompt" --ref "art/characters/$base/sheet.png" --model "${IMAGE_MODEL:-gpt-image-2.5}" --aspect 16:9 -o "$art/sheet.png" >/dev/null
    echo "$prompt" > "$art/sheet.prompt.txt"
    echo "SHEET $art/sheet.png: look at it before the model is made; rerun to go on"
    exit 0
  fi
  if [[ ! -f "$art/front.png" ]]; then
    $here/front_view.sh "$art/sheet.png" "$art/front.png"
  fi
  [[ -f "$art/model.glb" ]] || node tools/tripo/tripo.ts model "$art/front.png" --faces 30000 -o "$art/model.glb" >/dev/null
  design="$art/model.glb"
fi
# Steps 4 to 6 are the character build's own (build.ts --morph, D563): the morph and each look
# carried onto it, pushed clear in three rounds and checked, a step each, skipped while unchanged.
exec node tools/characters/build.ts --morph "$name" --design "$design"
