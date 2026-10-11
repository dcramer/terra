#!/usr/bin/env bash
# Runs a bake-off script in Blender, headless. From WSL it uses the Windows Blender,
# which renders on the real GPU; set BLENDER to use another.
#   spike/model_bakeoff/run.sh code/rocks.py [args...]
set -euo pipefail
here="$(cd "$(dirname "$0")" && pwd)"
script="$here/$1"
shift
if [[ -z "${BLENDER:-}" ]]; then
  BLENDER=$(ls -d "/mnt/c/Program Files/Blender Foundation/Blender "*/blender.exe 2>/dev/null | sort -V | tail -1 || true)
  [[ -n "$BLENDER" ]] || BLENDER=blender
fi
path() { if [[ "$BLENDER" == *.exe ]]; then wslpath -w "$1"; else echo "$1"; fi; }
"$BLENDER" --background --factory-startup --python-exit-code 1 \
  --python "$(path "$script")" -- "$@" 2>&1 | grep -v -e '^$' -e 'DeprecationWarning' -e 'use_nodes'
