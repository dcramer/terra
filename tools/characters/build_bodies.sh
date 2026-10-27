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
# Everything it prints is kept in tools/characters/out/build.log, the morphs' logs with it. It
# ends with BUILD pass, or BUILD fail and every failure a step printed or exited with
# (build_verdict.ts), and exits 1 on a failure.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
blender=tools/blender/run.sh here=tools/characters out=tools/characters/out
base=base-male

# Worn looks made on the base: item, its main material's role, how it is made (a coverage of
# shell_garment.py, or a kind of hung_garment.py for what hangs: a skirt, an apron), what it is
# made from, and its options, in order, since a piece worn over another is made after it.
# `--shape=female`: made from a dressed model of the female (her morph's offsets, from step 2).
garments=(
  "leather_coat Leather chest_long_sleeves art/gear/dressed-male-outfit/model.glb"
  "trousers Cloth legs_long art/gear/dressed-male-outfit/model.glb"
  "smith_apron Leather smith_apron art/gear/dressed-male-smith/model.glb --over=trousers"
  "shirt Linen chest_long_sleeves art/gear/dressed-male-smith/model.glb --over=trousers --under=smith_apron --plain"
  "cap Wool head_cap art/gear/dressed-male-smith/model.glb --plain"
  "boots Leather feet_ankle art/gear/dressed-male-smith/model.glb --over=trousers"
  "shift Linen chest_long_sleeves art/gear/dressed-female-kirtle/model.glb --shape=female --plain"
  "bodice Wool bodice art/gear/dressed-female-kirtle/model.glb --shape=female --over=shift --plain"
  "coif Linen head_coif art/gear/dressed-female-kirtle/model.glb --shape=female --plain"
  "shoes Leather feet art/gear/dressed-female-kirtle/model.glb --shape=female"
  "skirt Wool skirt art/gear/dressed-female-kirtle/model.glb --shape=female --over=shift,bodice --plain"
  "apron Linen apron art/gear/dressed-female-kirtle/model.glb --shape=female --over=skirt,shift,bodice"
)
hung=(skirt apron smith_apron)
rigid=(small_pack large_pack quiver)
# Morphs: a design kept in art/characters/base-male/morphs/NAME/model.glb, or a rigged body.
designed=(heavy thin strong)
rigged=("female base-female 1.70")
# Templates with their own painted skin: morph and the design's front view.
skins=("female art/characters/base-female/front.png")

build() {
echo "== 0. the clips"
if ls art/animation/library/*.glb >/dev/null 2>&1; then
  $blender $here/clips.py | grep -E "CLIPS|Error|not in" || true
fi
$blender $here/made_clips.py | grep -E "^MADE|Error|no clean stride" || true
$blender $here/clip_check.py | grep -E "^CLIPCHECK (pass|fail)|\"pass\": false|Error" | tee $out/build-clips.log || true

echo "== 1. the base"
$here/finish_character.sh $base 1.80

echo "== 2. its gear"
# A piece drawn on the female needs her morph's offsets first: her body registered onto the
# base (make_morph.sh does it again in step 3, with every look carried onto her).
if printf "%s\n" "${garments[@]}" | grep -q -- "--shape=female"; then
  for r in "${rigged[@]}"; do
    read -r m body height <<<"$r"
    [[ $m == female ]] || continue
    $here/finish_character.sh "$body" "$height" > "$out/build-$m-shape.log" 2>&1
    $blender $here/morph.py "$out/$base-game.glb" "$out/$body-game.glb" "$m" | grep -E "^MORPH|Error" || true
  done
fi
for g in "${garments[@]}"; do
  read -r item role coverage dressed flags <<<"$g"
  # shellcheck disable=SC2086
  if printf "%s\n" "${hung[@]}" | grep -qx "$coverage"; then
    $blender $here/hung_garment.py "$out/$base-game.glb" "$dressed" "$item" "$role" "$coverage" ${flags:-} | grep -E "^HUNG .*(hides|vertices)|EXPORTED|Error" || true
  else
    $blender $here/shell_garment.py "$out/$base-game.glb" "$dressed" "$item" "$role" "$coverage" ${flags:-} | grep -E "^SHELL .*hides|EXPORTED|Error" || true
  fi
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
node $here/mirror.ts assets/models/character/base.glb    # the mirrored clips (art/animation/mirrors.json)
node $here/markers.ts assets/models/character/base.glb   # each clip's foot-down markers (D455)
for look in $out/gear/$base/*.glb; do
  $blender $here/carry_gear.py "$out/$base-game.glb" "$look" "assets/models/gear/$(basename "$look")" "--morphs=$morphs" --textures=1024 | grep -E "CARRIED|Error" &
done
wait
echo "BUILT base.glb with morphs $morphs, $(ls $out/gear/$base/*.glb | wc -l) looks"
}

# Every check's verdict, from the whole build's output, the morphs' logs in it, and whether it
# ran to its end. The base's rig check, not a rigged design's (the female's own body is only
# the shape her morph is fitted to; what ships is the base's skeleton and weights in her
# shape), removed first so a check that never ran is not read from the last build.
mkdir -p "$out"
rm -f "$out/rig-check-$base.json"
set +e
( set -e; build ) 2>&1 | tee "$out/build.log"
status=${PIPESTATUS[0]}
set -e
node $here/build_verdict.ts "$out/build.log" "$out/rig-check-$base.json" "$status"
