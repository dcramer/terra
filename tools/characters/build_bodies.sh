#!/usr/bin/env bash
# Rebuilds every character asset the game loads from the source art in art/, for free (D112,
# D128; the character playbook, section 7). Run it after changing any of the scripts it calls,
# or after adding a morph, a skin or a piece of gear to the lists below.
#
#   tools/characters/build_bodies.sh
#
#   1. the base body (base-male) from Tripo's rig: converted, checked, exported
#   2. every worn look made on it: garments from the dressed models (shell_garment.py),
#      rigid pieces at their sockets (fit_gear.py); into tools/characters/out/gear/base-male/
#   3. every morph kept in art/characters/base-male/morphs/ (make_morph.sh), each body checked
#      with every look on it; the female from her own rigged body
#   4. each template's own painted skin (paint_skin.sh, from the painting kept in art/)
#   5. the game's files:
#        assets/models/character/base.glb      the base with every morph and their offsets
#        assets/models/character/skins/M.png   each template's skin
#        assets/models/gear/<item>.glb         each look, with the same morphs
# Read the RIGCHECK, MORPH and GEARCHECK lines it prints: every one must pass.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
blender=tools/blender/run.sh here=tools/characters out=tools/characters/out
base=base-male

# Worn looks made on the base: item, how it is made, and what it is made from.
garments=(
  "leather_coat Leather chest_sleeveless art/gear/dressed-male-coat/model.glb"
  "leather_cap Leather head_cap art/gear/dressed-male-cap/model.glb"
)
rigid=(small_pack large_pack quiver)
# Morphs: a design kept in art/characters/base-male/morphs/NAME/model.glb, or a rigged body.
designed=(heavy thin strong)
rigged=("female base-female 1.70")
# Templates with their own painted skin: morph and the design's front view.
skins=("female art/characters/base-female/front.png")

echo "== 1. the base"
$here/finish_character.sh $base 1.80

echo "== 2. its gear"
for g in "${garments[@]}"; do
  read -r item role coverage dressed <<<"$g"
  $blender $here/shell_garment.py "$out/$base-game.glb" "$dressed" "$item" "$role" "$coverage" | grep -E "^SHELL .*hides|EXPORTED|Error" || true
done
$blender $here/fit_gear.py "$out/$base-game.glb" "${rigid[@]}" | grep -E "^FIT|FITTED|Error" || true
echo "== 3. the morphs"
for m in "${designed[@]}"; do
  $here/make_morph.sh "$m" "(kept in art)"
done
for r in "${rigged[@]}"; do
  read -r m body height <<<"$r"
  $here/finish_character.sh "$body" "$height" >/dev/null
  $here/make_morph.sh "$m" --design "$out/$body-game.glb"
done
echo "== 4. the skins"
mkdir -p assets/models/character/skins
for k in "${skins[@]}"; do
  read -r m front <<<"$k"
  $here/paint_skin.sh "$m" "$front"
  cp "$out/$m-skin.png" "assets/models/character/skins/$m.png"
done
echo "== 5. the game's files"
mkdir -p assets/models/gear
morphs=$(printf "%s\n" "${designed[@]}" $(for r in "${rigged[@]}"; do echo "${r%% *}"; done) | sort | paste -sd, -)
$blender $here/export_character.py "$out/$base.glb" assets/models/character/base.glb "--morphs=$morphs" | grep -E "MORPH|SOCKETS|EXPORTED|Error" || true
for look in $out/gear/$base/*.glb; do
  $blender $here/carry_gear.py "$out/$base-game.glb" "$look" "assets/models/gear/$(basename "$look")" "--morphs=$morphs" | grep -E "CARRIED|Error" || true
done
echo "BUILT base.glb with morphs $morphs, $(ls $out/gear/$base/*.glb | wc -l) looks"
