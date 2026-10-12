#!/usr/bin/env bash
# Runs a bake-off script in Blender, headless, through tools/blender/run.sh, with the script's
# path relative to this folder:
#   spike/model_bakeoff/run.sh code/rocks.py [args...]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
script="$here/$1"
shift
exec "$here/../../tools/blender/run.sh" "$script" "$@"
