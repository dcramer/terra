#!/usr/bin/env bash
# Rebuilds every character asset the game loads from the source art in art/, for free (D112,
# D128; the character playbook, section 7). Run it after changing any of the scripts it calls,
# or after adding a morph, a skin or a piece of gear to the lists below.
#
#   tools/characters/build_bodies.sh
#
#   0. the clips the game plays (art/animation/clips.txt) cut from the animation libraries
#      into the mannequin file (clips.py), if the libraries are here; the clips we made
#      (art/animation/made.json) put in place of theirs (made_clips.py); all checked
#      (clip_check.py)
#   1. the base body (base-male) from Tripo's rig: converted, checked, exported
#   2. every worn look made on it: garments from the dressed models (shell_garment.py),
#      rigid pieces at their sockets (fit_gear.py); into tools/characters/out/gear/base-male/
#   3. every morph kept in art/characters/base-male/morphs/ (make_morph.sh), each body checked
#      with every look on it; the female from her own rigged body. The morphs are built at
#      once, side by side, each to its own log, tools/characters/out/build-MORPH.log
#   4. each template's own painted skin (paint_skin.sh, from the painting kept in art/)
#   5. the game's files:
#        assets/models/character/base.glb      the base with every morph and their offsets
#        assets/models/character/skins/M.png   each template's skin
#        assets/models/gear/<item>.glb         each look, with the same morphs
# It ends with BUILD pass, or BUILD fail and the checks that failed (RIGCHECK, CLIPCHECK,
# GEARCHECK); the full lines are in its output and the morphs' logs.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
blender=tools/blender/run.sh here=tools/characters out=tools/characters/out
base=base-male

# Worn looks made on the base: item, how it is made, and what it is made from.
garments=(
  "leather_coat Leather chest_long_sleeves art/gear/dressed-male-outfit/model.glb"
  "trousers Cloth legs_long art/gear/dressed-male-outfit/model.glb"
)
rigid=(small_pack large_pack quiver)
# Morphs: a design kept in art/characters/base-male/morphs/NAME/model.glb, or a rigged body.
designed=(heavy thin strong)
rigged=("female base-female 1.70")
# Templates with their own painted skin: morph and the design's front view.
skins=("female art/characters/base-female/front.png")

echo "== 0. the clips"
if ls art/animation/library/*.glb >/dev/null 2>&1; then
  $blender $here/clips.py | grep -E "CLIPS|Error|not in" || true
fi
$blender $here/made_clips.py | grep -E "^MADE|Error|no clean stride" || true
$blender $here/clip_check.py | grep -E "^CLIPCHECK (pass|fail)|\"pass\": false|Error" | tee $out/build-clips.log || true

echo "== 1. the base"
$here/finish_character.sh $base 1.80

echo "== 2. its gear"
for g in "${garments[@]}"; do
  read -r item role coverage dressed <<<"$g"
  $blender $here/shell_garment.py "$out/$base-game.glb" "$dressed" "$item" "$role" "$coverage" | grep -E "^SHELL .*hides|EXPORTED|Error" || true
done
$blender $here/fit_gear.py "$out/$base-game.glb" "${rigid[@]}" | grep -E "^FIT|FITTED|Error" || true
# The base is fixed as a morph is (make_morph.sh): its looks carried onto it, pushed clear
# wherever its skin comes through in our clips, in rounds, and checked; the morphs start from
# what this leaves.
carry_base() {
  looks=()
  for look in $out/gear/$base/*.glb; do
    item=$(basename "$look" .glb)
    $blender $here/carry_gear.py "$out/$base-game.glb" "$look" "$out/$base-$item.glb" "$base" | grep -E "PUSHED|Error" || true
    looks+=("$out/$base-$item.glb")
  done
}
rm -f "$out/$base"-*-push.npz "$out/$base"-*-uncover.json
carry_base
for round in 1 2 3; do
  $blender $here/gear_check.py "$out/$base-game.glb" "${looks[@]}" --fix | grep -E "^FIX|Error" || true
  carry_base
done
$blender $here/gear_check.py "$out/$base-game.glb" "${looks[@]}" | grep -E "^GEARCHECK|Error" | tee "$out/build-gear.log" || true
echo "== 3. the morphs, side by side"
morph_jobs=()
for m in "${designed[@]}"; do
  $here/make_morph.sh "$m" "(kept in art)" > "$out/build-$m.log" 2>&1 &
  morph_jobs+=("$m:$!")
done
for r in "${rigged[@]}"; do
  read -r m body height <<<"$r"
  ( $here/finish_character.sh "$body" "$height" && $here/make_morph.sh "$m" --design "$out/$body-game.glb" ) > "$out/build-$m.log" 2>&1 &
  morph_jobs+=("$m:$!")
done
for job in "${morph_jobs[@]}"; do
  wait "${job#*:}" || echo "MORPH ${job%%:*} failed: see $out/build-${job%%:*}.log"
  cat "$out/build-${job%%:*}.log"
done
echo "== 4. the skins"
mkdir -p assets/models/character/skins
for k in "${skins[@]}"; do
  read -r m front <<<"$k"
  $here/paint_skin.sh "$m" "$front"
  convert "$out/$m-skin.png" -resize "2048x2048>" "assets/models/character/skins/$m.png"   # the body budget (D188)
done
echo "== 5. the game's files"
mkdir -p assets/models/gear
morphs=$(printf "%s\n" "${designed[@]}" $(for r in "${rigged[@]}"; do echo "${r%% *}"; done) | sort | paste -sd, -)
$blender $here/export_character.py "$out/$base.glb" assets/models/character/base.glb "--morphs=$morphs" --textures=2048 | grep -E "MORPH|SOCKETS|EXPORTED|Error" || true
for look in $out/gear/$base/*.glb; do
  $blender $here/carry_gear.py "$out/$base-game.glb" "$look" "assets/models/gear/$(basename "$look")" "--morphs=$morphs" --textures=1024 | grep -E "CARRIED|Error" &
done
wait
echo "BUILT base.glb with morphs $morphs, $(ls $out/gear/$base/*.glb | wc -l) looks"
# Every check's verdict, from this build's output and the morphs' logs.
# The base's rig check, not a rigged design's (the female's own body is only the shape her
# morph is fitted to; what ships is the base's skeleton and weights in her shape).
failed=$( { cat "$out/build-clips.log" "$out/build-gear.log"; node -p '"RIGCHECK " + JSON.stringify(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")))' "$out/rig-check-$base.json"; for m in "${designed[@]}"; do cat "$out/build-$m.log"; done; for r in "${rigged[@]}"; do cat "$out/build-${r%% *}.log"; done; } \
  | grep -oE '^(CLIPCHECK|GEARCHECK) .*"pass": false|^RIGCHECK \{"file":"[^"]*","pass":false|^CLIPCHECK fail|Error.*' | cut -c1-160 || true)
if [[ -z "$failed" ]]; then echo "BUILD pass"; else echo "BUILD fail"; echo "$failed"; fi
