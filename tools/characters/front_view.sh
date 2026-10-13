#!/usr/bin/env bash
# The front view of a turnaround sheet, as Tripo is given it to model:
#
#   tools/characters/front_view.sh SHEET.png FRONT.png
#
# The sheet's front view is on its left; it ends at the first clear gap after it, columns
# with nothing on them at least GAP pixels wide (a figure's fingertips can reach past a fixed
# share of the sheet). Written at 2048 square on the sheet's own background, so the
# generator sees one figure on a plain ground and models nothing else.
set -euo pipefail
sheet=$1 front=$2 gap=${GAP:-6}
# The sheet's columns, marked where anything is on them; read whole, then searched (awk
# stopping early would cut the pipe under convert).
columns=$(convert "$sheet" -edge 1 -colorspace gray -threshold 8% -scale x1! -threshold 0 txt:-)
width=$(awk -F'[,:]' -v gap="$gap" 'NR > 1 { x = $1; on = ($0 ~ /#FFFFFF|white/) }
      NR > 1 && on { if (seen && x - last > gap) { print int((last + x) / 2); exit } seen = 1; last = x }' <<<"$columns")
[[ -n "$width" ]] || { echo "front_view: no gap after the front view on $sheet" >&2; exit 1; }
ground=$(convert "$sheet" -format '%[pixel:p{4,4}]' info:)
convert "$sheet" -crop "${width}x+0+0" +repage -resize 2048x2048 -background "$ground" -gravity center -extent 2048x2048 "$front"
