#!/usr/bin/env bash
# Runs a Python script inside Blender, headless. See README.md.
#   tools/blender/run.sh <script.py> [--open file.blend] [args for the script...]
# From WSL it uses the newest Windows Blender, which renders on the real GPU; set BLENDER to
# use another.
set -euo pipefail
[[ $# -ge 1 ]] || { echo "usage: tools/blender/run.sh <script.py> [--open file.blend] [args...]" >&2; exit 2; }
script=$(realpath "$1")
shift
open=(--factory-startup)
if [[ "${1:-}" == --open ]]; then open=("$(realpath "$2")"); shift 2; fi

if [[ -z "${BLENDER:-}" ]]; then
  BLENDER=$(ls -d "/mnt/c/Program Files/Blender Foundation/Blender "*/blender.exe 2>/dev/null | sort -V | tail -1 || true)
  [[ -n "$BLENDER" ]] || { [[ -x /Applications/Blender.app/Contents/MacOS/Blender ]] && BLENDER=/Applications/Blender.app/Contents/MacOS/Blender; } || BLENDER=blender
fi
# A Windows Blender takes Windows paths. Relative ones need nothing: it starts in this folder.
path() { if [[ "$BLENDER" == *.exe ]]; then wslpath -w "$1"; else echo "$1"; fi; }
args=()
for a in "$@"; do
  if [[ "$BLENDER" == *.exe && "$a" == /* && -e "$a" ]]; then args+=("$(wslpath -w "$a")"); else args+=("$a"); fi
done
[[ "${open[0]}" == --factory-startup ]] || open=("$(path "${open[0]}")")

# Runs the script as __main__ with its own folder on sys.path, which Blender does not do for
# a script on a \\wsl.localhost path, so a script can import the modules beside it.
boot="import sys, runpy; sys.path.insert(0, r'$(path "$(dirname "$script")")'); runpy.run_path(r'$(path "$script")', run_name='__main__')"
set +e
"$BLENDER" --background "${open[@]}" --python-exit-code 1 --python-expr "$boot" -- ${args[@]+"${args[@]}"} </dev/null 2>&1 \
  | grep -v -e '^$' -e 'DeprecationWarning' -e 'use_nodes'
exit "${PIPESTATUS[0]}"
