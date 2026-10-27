# The bodies build_bodies.sh makes and build_item.sh carries a look onto, sourced by both.
# The worn looks are not listed here: each is its data, art/gear/<item>/fit.json (gear_list.ts).
base=base-male
# Morphs: a design kept in art/characters/base-male/morphs/NAME/model.glb, or a rigged body.
designed=(heavy thin strong)
rigged=("female base-female 1.70")
# Templates with their own painted skin: morph and the design's front view.
skins=("female art/characters/base-female/front.png")
# Every morph's name, in the order the game's files carry them.
morph_names() { printf "%s\n" "${designed[@]}" $(for r in "${rigged[@]}"; do echo "${r%% *}"; done) | sort; }
