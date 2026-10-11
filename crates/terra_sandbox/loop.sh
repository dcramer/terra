#!/usr/bin/env bash
# Drives the play experiment's loop end to end and checks each step happened (D69):
# talk to the elder, eat, kill the spiders, free the smith and bring them home, claim
# land, build a smelter and a forge, have the smith take up the forge, smelt iron, and
# have the smith make an iron axe. Each run reads
# what happened from the log, where every on-screen notice is written.
#
# Runs on the Windows GPU from WSL by default, in its own copy of the game so it does not
# disturb one being played. Set RUN to run elsewhere, such as
#   RUN="cargo run --release -p terra_sandbox --" crates/terra_sandbox/loop.sh
# Screenshots of each step are left where the game runs, named loop-*.png.
set -uo pipefail

RUN=${RUN:-cargo xtask win -p terra_sandbox --copy test --}
failed=0
log=

# step <name> <what the log must say>... -- <game options>...
step() {
    local name=$1
    shift
    local wanted=()
    while [[ $1 != -- ]]; do
        wanted+=("$1")
        shift
    done
    shift
    log=$($RUN --experiment play --clean --shot "loop-$name.png" "$@" 2>&1)
    for line in "${wanted[@]}"; do
        if grep -qF -- "$line" <<<"$log"; then
            echo "ok    $name: $line"
        else
            echo "FAIL  $name: never saw \"$line\""
            failed=1
        fi
    done
}

swings=()
for i in $(seq 0 60); do swings+=(--press "e@$((100 + i * 30))"); done

step talk 'Notice: Maren: "Spiders took Bren' 'Notice: Ate berries' -- \
    --fresh --give berries:2 --give fed:50 \
    --at -68.1,50.6 --face -68.1,52.9 --press f@150 --press h@170 --settle 200

# In leather with a spear, facing the spiders as they come, then over to Bren.
step rescue 'Killed a Spider' 'Freed Bren.' -- \
    --fresh --save loop.ron \
    --give spear:1 --give leather_coat:1 --give leather_cap:1 \
    --at 58,-58 --face 70,-60 --press 5@60 "${swings[@]}" \
    --warp 63.8,-61.9:66,-64.4@1950 --press down@1955-1975 --press left@1976-1981 \
    --press f@1990 --warp 8,6:0,0@2020 --settle 3500

# Where the last step placed something, as "east south" of the outpost, from its log.
placed() {
    grep -o "Placed a $1 at \[[^]]*\]" <<<"$log" | tail -1 | tr -d '[],' | awk '{print $5, $7}'
}

# Up to a station, a little short of it, looking at it: "--warp ... --press ...".
toward() {
    local east south
    read -r east south <<<"${1:-0 0}"
    local before
    before=$(awk "BEGIN {print $south - 2.8}")
    echo "--warp $east,$before:$east,$south@30 --press down@40-65"
}

# Bren followed home. A claim stone, and a smelter in the claim.
step build 'Placed a Claim stone' 'Placed a Smelter' -- \
    --save loop.ron --give stone:12 --give wood:2 \
    --warp 14,14:22,22@30 --press 1@50 \
    --press b@70 --press 1@80 --press enter@90 --press e@120-200 \
    --press b@230 --press 4@240 --press right@250 --press enter@260 \
    --press right@280-320 --press e@340-540 --settle 600
smelter=$(placed Smelter)

# A forge beside it, which Bren takes up.
step forge 'Placed a Forge' 'Bren took up the forge.' -- \
    --save loop.ron --give stone:8 --give wood:4 \
    --warp 12,12:6,18@30 --press 1@50 \
    --press b@130 --press 4@140 --press right@150 --press right@155 --press enter@165 \
    --press e@200-400 --settle 1000
forge=$(placed Forge)

# Three iron queued at the smelter, which works on its own; it hands them over when
# opened again after they are done.
# shellcheck disable=SC2046
step smelt 'Queued Iron at the Smelter.' 'Made Iron.' -- \
    --save loop.ron --give ore:6 --give wood:3 \
    $(toward "$smelter") --press f@70 --press enter@90 --press enter@95 --press enter@100 \
    --press f@120 --press f@2100 --settle 2160

# An iron axe at the forge, made while you stay: the better tool the loop is for.
# shellcheck disable=SC2046
step axe 'Queued Iron axe at the Forge.' 'Made Iron axe.' -- \
    --save loop.ron --give wood:2 \
    $(toward "$forge") --press f@70 --press enter@90 --settle 600

exit $failed
