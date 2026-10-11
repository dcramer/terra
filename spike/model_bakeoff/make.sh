#!/usr/bin/env bash
# The repeatable pipeline, in two steps with a look in between.
#
#   make.sh concept NAME "subject"           # 10 credits: a concept image to check by eye
#   make.sh model NAME FACES HEIGHT BUDGETS COLOURS...
#                                            # 60 credits: four views, a low-poly model, then
#                                            # refined free in Blender to our palette
#   make.sh refine NAME HEIGHT BUDGETS COLOURS...   # free: redo only the Blender step
#
# e.g. make.sh model wolf 8000 1.0 0,3000,1500 FurGrey Fur FurDark FurLight Black
# Everything lands in out/: the concept, views, Tripo's model and its record of what it
# cost, the finished .glb files, a sheet of renders and the checks.
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
cd "$here"
step=$1 name=$2
shift 2
case $step in
  concept)
    python3 -I tripo.py concept "$name-concept" "$1"
    echo "Look at out/tripo/$name-concept-generated_image_url.png before making a model."
    ;;
  model)
    faces=$1
    shift
    python3 -I tripo.py views "$name-views" "out/tripo/$name-concept-generated_image_url.png"
    v=out/tripo/$name-views
    python3 -I tripo.py multiview "$name-p1" "$v-front_view_url.jpeg" "$v-left_view_url.jpeg" \
      "$v-back_view_url.jpeg" "$v-right_view_url.jpeg" --model P1-20260311 --faces "$faces" --texture
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
