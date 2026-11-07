#!/bin/sh
# The creator's eye colours (D111): the pack's one eye map (T_Eye_Brown, brown) with its iris
# alone turned to each colour, the white and the lid's skin left as painted. Written to
# assets/models/character/eyes/<colour>.png, which assets/models/character/creator.json names.
# Run from the repository's root; needs ImageMagick's convert.
#
#   tools/characters/eyes.sh
set -eu
from=art/characters/quaternius/universal-base-characters/T_Eye_Brown.png
to=assets/models/character/eyes
mkdir -p "$to"
# The iris: a disc about the pupil, soft at its rim.
# name brightness,saturation,hue (ImageMagick's -modulate: 100 is unchanged; hue turns the wheel)
turn() {
  convert "$from" \( +clone -modulate "$2" \) \
    \( -size 256x256 xc:black -fill white -draw 'circle 127,128 127,157' -blur 0x1.5 \) \
    -composite -strip "$to/$1.png"
}
cp "$from" "$to/brown.png"
turn hazel 105,85,112
turn green 105,75,147
turn blue 115,70,3
turn grey 110,15,100
