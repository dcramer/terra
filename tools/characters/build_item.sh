#!/usr/bin/env bash
# One worn item built alone, on the bodies a full build (build_bodies.sh) has made: what that
# build does for every look, done for this one (docs/attachment.md, "Fitting a new item"):
#
#   tools/characters/build_item.sh ITEM
#
#   1. made on the base from its data, art/gear/ITEM/fit.json (gear_list.ts): a garment by
#      shell_garment.py or hung_garment.py, a pack or belt piece by fit_gear.py
#   2. carried onto the base and every morph (bodies.sh), side by side, each pushed clear where
#      its skin comes through in our clips (gear_check.py --fix, three rounds) and checked
#   3. the game's file, assets/models/gear/ITEM.glb, with every morph
#
# Needs a full build's bodies in tools/characters/out (the base's and each morph's -game.glb
# and .npz). Its output is kept in tools/characters/out/build-ITEM.log; it ends with ITEM pass,
# or ITEM fail and each check that failed, and exits 1 on a failure.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
blender=tools/blender/run.sh here=tools/characters out=tools/characters/out
source tools/characters/bodies.sh
item=${1:?usage: build_item.sh ITEM}
line=$(node $here/gear_list.ts one "$item")
mapfile -t morphs < <(morph_names)
for b in "$base" "${morphs[@]}"; do
  [[ -f $out/$b-game.glb ]] || { echo "✗ $item: no $out/$b-game.glb; run build_bodies.sh first"; exit 1; }
done
log=$out/build-$item.log

build() {
  echo "== 1. $item on the base"
  read -r maker rest <<<"$line"
  if [[ $maker == fit_gear ]]; then
    $blender $here/fit_gear.py "$out/$base-game.glb" "$item" | grep -E "^FIT|FITTED|Error" || true
  else
    read -r item maker role coverage dressed flags <<<"$line"
    script=$([[ $maker == hung ]] && echo hung_garment.py || echo shell_garment.py)
    # shellcheck disable=SC2086
    $blender $here/$script "$out/$base-game.glb" "$dressed" "$item" "$role" "$coverage" ${flags:-} | grep -E "^(SHELL|HUNG) .*(hides|vertices)|EXPORTED|Error" || true
  fi
  [[ -f $out/gear/$base/$item.glb ]] || { echo "✗ $item: its maker wrote no $out/gear/$base/$item.glb"; return 1; }
  echo "== 2. carried onto every body, side by side"
  for b in "$base" "${morphs[@]}"; do
    (
      rm -f "$out/$b-$item-push.npz" "$out/$b-$item-uncover.json"
      carry() { $blender $here/carry_gear.py "$out/$base-game.glb" "$out/gear/$base/$item.glb" "$out/$b-$item.glb" "$b" | grep -E "PUSHED|Error" || true; }
      carry
      for round in 1 2 3; do
        $blender $here/gear_check.py "$out/$b-game.glb" "$out/$b-$item.glb" --fix | grep -E "^FIX|Error" || true
        carry
      done
      $blender $here/gear_check.py "$out/$b-game.glb" "$out/$b-$item.glb" | grep -E "^GEARCHECK|Error" || true
    ) > "$out/build-$item-$b.log" 2>&1 &
  done
  wait
  for b in "$base" "${morphs[@]}"; do cat "$out/build-$item-$b.log"; done
  echo "== 3. the game's file"
  mkdir -p assets/models/gear
  $blender $here/carry_gear.py "$out/$base-game.glb" "$out/gear/$base/$item.glb" "assets/models/gear/$item.glb" "--morphs=$(printf "%s\n" "${morphs[@]}" | paste -sd, -)" --textures=1024 | grep -E "CARRIED|Error" || true
}

set +e
( set -e; build ) > "$log" 2>&1
status=$?
set -e
checks=$(grep -c '^GEARCHECK {' "$log" || true)
failed=$(grep '^GEARCHECK {' "$log" | grep -c '"pass": false' || true)
if [[ $status -ne 0 ]] || grep -qE "Error|^✗" "$log" || [[ $checks -ne $((${#morphs[@]} + 1)) ]] || [[ $failed -ne 0 ]] || ! grep -q "^CARRIED" "$log"; then
  echo "✗ $item fail: $checks of $((${#morphs[@]} + 1)) bodies checked, $failed over the bar; see $log"
  grep -E "Error|^✗|\"pass\": false" "$log" | cut -c1-200 | head -10
  exit 1
fi
echo "✓ $item pass: made, carried onto $checks bodies and checked, assets/models/gear/$item.glb written ($log)"
