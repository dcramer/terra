#!/usr/bin/env bash
# Rebuilds every character asset the game loads from the source art in art/, for free (D112,
# D128; the character playbook, section 7), incrementally: a step whose inputs have not changed
# since it last ran is skipped, so a rerun redoes only what a change reaches, and a run killed
# part-way resumes where it stopped (build.ts; its steps, build_steps.ts). One worn item alone:
# build_item.sh.
#
#   tools/characters/build_bodies.sh [--jobs N] [--plan]
#
#   0. the clips the game plays (art/animation/clips.txt) cut from the animation libraries
#      into the mannequin file (clips.py), if the libraries are here; the clips we made
#      (art/animation/made.json) put in place of theirs (made_clips.py); all checked
#      (clip_check.py)
#   1. the base body (bodies.json) from Tripo's rig: converted, checked, exported
#   2. every worn look made on it from its data, art/gear/<item>/fit.json (gear_list.ts):
#      garments from the dressed models (shell_garment.py, hung_garment.py), rigid pieces at
#      their sockets (fit_gear.py), into tools/characters/out/gear/base-male/
#   3. every morph (bodies.json): the base registered onto its design (morph.py) and exported;
#      the female from her own rigged body. Every look carried onto the base and each morph,
#      pushed clear where its skin comes through in our clips and checked, a step each
#   4. each template's own painted skin (paint_skin.sh, from the painting kept in art/)
#   5. the game's files:
#        assets/models/character/base.glb      the base with every morph and their offsets
#        assets/models/character/skins/M.png   each template's skin
#        assets/models/gear/<item>.glb         each look, with the same morphs
# A line a step: built, skipped (unchanged) or failed, and why. Each step's output is kept in
# tools/characters/out/steps/, all of it in build order in tools/characters/out/build.log. It
# ends with BUILD pass, or BUILD fail and every failure a step printed or exited with
# (build_verdict.ts), and exits 1 on a failure. --plan says what would run, running nothing.
set -euo pipefail
exec node "$(dirname "$0")/build.ts" "$@"
