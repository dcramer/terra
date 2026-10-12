#!/usr/bin/env bash
# A template's own skin, painted by Tripo on the base's mesh in its shape (D112):
#
#   tools/characters/paint_skin.sh NAME FRONT.png
#
# NAME is a morph made by make_morph.sh (tools/characters/out/NAME-game.glb); FRONT is the
# design's front view (art/characters/<body>/front.png). About 20 credits. The painting is
# kept as art/characters/<base>/morphs/NAME/skin.glb; rerunning bakes it again without paying.
set -euo pipefail
root="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$root"
name=$1 front=$2 base=${BASE:-base-male}
blender=tools/blender/run.sh here=tools/characters out=tools/characters/out
kept="art/characters/$base/morphs/$name/skin.glb"
if [[ ! -f "$kept" ]]; then
  $blender $here/paint_skin.py out "$name" | grep -E "BARE|Error" || true
  project=$(node tools/tripo/tripo.ts import "$out/$name-bare.glb" -o "$out/$name-imported.glb" | tail -1 | node -p 'JSON.parse(require("fs").readFileSync(0, "utf8")).project')
  mkdir -p "$(dirname "$kept")"
  node tools/tripo/tripo.ts texture "$project" --image "$front" --quality detailed -o "$kept" >/dev/null
fi
cp "$kept" "$out/$name-painted.glb"
$blender $here/paint_skin.py in "$name" | grep -E "PAINTED|Error" || true
