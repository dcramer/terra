#!/usr/bin/env bash
# Runs a Python script inside Blender, headless. See README.md.
#   tools/blender/run.sh <script.py> [--open file.blend] [args for the script...]
#   tools/blender/run.sh --find    prints the Blender it would use, or fails if there is none
# It uses the Blender named in tools/blender/version; from WSL, that Windows install, which
# renders on the real GPU; on Linux, an official tarball of it unpacked in the account's ~/opt
# (the render worker's, D445), else blender on the PATH. Set BLENDER to use another; it must
# still be that version.
set -euo pipefail
here=$(dirname "$(realpath "$0")")
pinned=$(tr -d '[:space:]' < "$here/version")

if [[ -z "${BLENDER:-}" ]]; then
  BLENDER="/mnt/c/Program Files/Blender Foundation/Blender $pinned/blender.exe"
  [[ -e "$BLENDER" ]] || { [[ -x /Applications/Blender.app/Contents/MacOS/Blender ]] && BLENDER=/Applications/Blender.app/Contents/MacOS/Blender; } || BLENDER=blender
  # The account's home, not $HOME: a render worker job's HOME is its slot's (tools/render-worker).
  if [[ "$BLENDER" == blender ]] && command -v getent >/dev/null; then
    tarball=$(ls -d "$(getent passwd "$(id -u)" | cut -d: -f6)"/opt/blender-"$pinned".*-linux-x64/blender 2>/dev/null | sort -V | tail -1 || true)
    [[ -n "$tarball" && -x "$tarball" ]] && BLENDER=$tarball
  fi
fi
if [[ "${1:-}" == --find ]]; then
  [[ -e "$BLENDER" ]] || command -v "$BLENDER" >/dev/null || exit 1
  echo "$BLENDER"; exit 0
fi

[[ $# -ge 1 ]] || { echo "usage: tools/blender/run.sh <script.py> [--open file.blend] [args...]" >&2; exit 2; }
script=$(realpath "$1")
shift
open=(--factory-startup)
if [[ "${1:-}" == --open ]]; then open=("$(realpath "$2")"); shift 2; fi

# A Windows Blender takes Windows paths. Relative ones need nothing: it starts in this folder.
path() { if [[ "$BLENDER" == *.exe ]]; then wslpath -w "$1"; else echo "$1"; fi; }
args=()
for a in "$@"; do
  if [[ "$BLENDER" == *.exe && "$a" == /* && -e "$a" ]]; then args+=("$(wslpath -w "$a")"); else args+=("$a"); fi
done
[[ "${open[0]}" == --factory-startup ]] || open=("$(path "${open[0]}")")

# Refuses a Blender other than the pinned one, since another version can make different
# files. Then runs the script as __main__ with its own folder on sys.path, which Blender does
# not do for a script on a \\wsl.localhost path, so a script can import the modules beside it.
# A script's error reaches the caller last, after what it printed: Python's output into a pipe
# is held back in blocks, so Blender's traceback came out first, above the run's own lines,
# where a caller reading the tail never saw it (job 5a's prep), and a crash lost the held lines
# outright. So both streams go line by line, a traceback starts at the script's own frame,
# and a native crash prints the Python stack it died in (faulthandler; off again once the
# script ends, since Blender's teardown on Windows raises access violations it handles itself,
# which faulthandler would print by the dozen).
script_w=$(path "$script")
boot="import sys, runpy, bpy, faulthandler, traceback
sys.stdout.reconfigure(line_buffering=True); sys.stderr.reconfigure(line_buffering=True)
faulthandler.enable()
v = '%d.%d' % bpy.app.version[:2]
if v != '$pinned':
    print('Blender %s is not the pinned %s (tools/blender/version): install it, or set BLENDER to it' % (bpy.app.version_string, '$pinned'))
    sys.exit(3)
sys.path.insert(0, r'$(path "$(dirname "$script")")')
try:
    runpy.run_path(r'$script_w', run_name='__main__')
except SystemExit:
    raise
except BaseException as e:
    tb = e.__traceback__
    while tb is not None and tb.tb_frame.f_code.co_filename != r'$script_w':
        tb = tb.tb_next
    traceback.print_exception(type(e), e, tb or e.__traceback__)
    sys.exit(1)
finally:
    faulthandler.disable()"
set +e
# TERRA_BLENDER_GPU_BACKEND (vulkan, opengl) picks the GPU backend Eevee draws with: the render
# worker sets vulkan, since over its Xvfb Blender's OpenGL is software (llvmpipe, ~20 s a frame).
backend=()
[[ -n "${TERRA_BLENDER_GPU_BACKEND:-}" ]] && backend=(--gpu-backend "$TERRA_BLENDER_GPU_BACKEND")
"$BLENDER" --background ${backend[@]+"${backend[@]}"} "${open[@]}" --python-exit-code 1 --python-expr "$boot" -- ${args[@]+"${args[@]}"} </dev/null 2>&1 \
  | grep -v -e '^$' -e 'DeprecationWarning' -e 'use_nodes'
status=${PIPESTATUS[0]}
# Said on stderr, past a caller's grep of the output: a script can fail printing nothing a
# caller looks for (sys.exit, SystemExit's message), and build_bodies.sh's verdict reads this.
[[ $status == 0 ]] || echo "run.sh: $(basename "$script") exited $status" >&2
exit "$status"
