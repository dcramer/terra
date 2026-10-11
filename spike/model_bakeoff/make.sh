#!/usr/bin/env bash
# The repeatable pipeline, in two steps with a look in between.
#
#   make.sh concept NAME "subject"           # free: a concept image to check by eye
#   make.sh model NAME FACES HEIGHT BUDGETS COLOURS...
#                                            # about 65 credits: four views, a low-poly model,
#                                            # then refined free in Blender to our palette
#   make.sh refine NAME HEIGHT BUDGETS COLOURS...   # free: redo only the Blender step
#
# e.g. make.sh model wolf 8000 1.0 0,3000,1500 FurGrey Fur FurDark FurLight Black
# Everything lands in out/: the concept, views, Tripo's model, the finished .glb files, a
# sheet of renders and the checks. Tripo runs on the Studio subscription (tools/tripo).
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
cd "$here"
tripo="$here/../../tools/tripo/tripo.ts"
# The concept's brief: one subject, readable, in our look.
brief="Concept art of a single %s, for a stylised low-poly game. Three-quarter view, the \
whole object in frame and centred, on a plain light grey background. Simple, finely cut \
faceted shapes made of flat planes, like a carefully lit papercraft model; flat colours, \
one colour per part, from a small soft palette; no textures, no outlines, no text; soft \
even light, no ground, no cast shadow."
step=$1 name=$2
shift 2
case $step in
  concept)
    mkdir -p out/tripo
    # shellcheck disable=SC2059
    node "$tripo" image "$(printf "$brief" "$1")" -o "out/tripo/$name-concept.png"
    echo "Look at out/tripo/$name-concept.png before making a model."
    ;;
  model)
    faces=$1
    shift
    v=out/tripo/$name-views
    node "$tripo" views "out/tripo/$name-concept.png" -o "$v"
    node "$tripo" model --views "$v-front.jpeg" "$v-left.jpeg" "$v-back.jpeg" "$v-right.jpeg" \
      --model p1 --faces "$faces" -o "out/tripo/$name-p1.glb"
    ./run.sh refine.py "out/tripo/$name-p1.glb" "$name" "$@" | grep -E "REPORT|Error" || true
    echo "Sheet: out/refined-$name.png"
    ;;
  refine)
    ./run.sh refine.py "out/tripo/$name-p1.glb" "$name" "$@" | grep -E "REPORT|Error" || true
    echo "Sheet: out/refined-$name.png"
    ;;
  *)
    echo "unknown step $step" >&2
    exit 1
    ;;
esac
