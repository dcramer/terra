#!/bin/sh
# Builds the spike's fire.wesl from main's (130d62b3): breath_of grows the fireball, the rod is
# the whole stream, breath_gas and breath() are gas.wesl and march.wesl, the sprites are hidden
# and the hull writes the film mask. Usage: sh assemble.sh <main fire.wesl> <out>
set -e
D=$(dirname "$0")
IN=$1
OUT=$2
G0=$(grep -n "^// The gas at a point in the jet's frame" "$IN" | cut -d: -f1)
G1=$(grep -n "^// The breath's gas as the screen shows it" "$IN" | cut -d: -f1)
M0=$(grep -n "^// The breath along the ray through a fragment" "$IN" | cut -d: -f1)
M1=$(grep -n "^// The hull round a breath, in its frame" "$IN" | cut -d: -f1)
{
  sed -n "1,$((G0 - 1))p" "$IN"
  cat "$D/gas.wesl"
  sed -n "${G1},$((M0 - 1))p" "$IN"
  cat "$D/march.wesl"
  sed -n "${M1},\$p" "$IN"
} > "$OUT.tmp"
node "$D/edits.ts" "$OUT.tmp" "$OUT"
rm "$OUT.tmp"
