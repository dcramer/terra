#!/usr/bin/env bash
# The character pipeline from Tripo's rig on, free: convert, check, render, export.
#
#   finish_character.sh NAME HEIGHT [RIGGED.glb]
#
# RIGGED defaults to spike/model_bakeoff/out/tripo/NAME-rigged.glb, as make_character.sh
# leaves it. Run it again for every character whenever the conversion improves.
#   1. convert the rig onto our skeleton, with forearm twist bones
#   2. check it against our skeleton and the mannequin, and render it moving
#   3. export it as the game loads it, with our clips re-sampled on its skeleton, and sockets
# Writes out/NAME.glb, out/rig-check.json, out/NAME-sheet.png and out/NAME-game.glb.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
name=$1 height=$2
rigged=$(realpath "${3:-$here/../model_bakeoff/out/tripo/$name-rigged.glb}")
cd "$here/../model_bakeoff"
./run.sh ../character_gear/convert_rig.py "$rigged" "$name" "$height" 2 | grep -E "WEIGHTS|EXPORTED|Error" || true
./run.sh ../character_gear/rig_check.py "../character_gear/out/$name.glb" | grep RIGCHECK || true
./run.sh ../character_gear/rig_render.py "../character_gear/out/$name.glb" "$name-sheet" | grep -E "SHEET|Error" || true
./run.sh ../character_gear/export_character.py "../character_gear/out/$name.glb" "../character_gear/out/$name-game.glb" | grep -E "SOCKETS|EXPORTED|Error" || true
