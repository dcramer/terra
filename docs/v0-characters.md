# v0 characters: where it stands and next steps

The people of v0: the player's body, what it wears, how it stands and moves, as [v0-goals.md](v0-goals.md) asks of them (the creator of a few choices so two players tell each other apart, D111; equipping gear; the boss's mark shown on the arm, D590; locomotion and fight sets earning the developers' yes). How characters are made is [characters.md](characters.md); this page is where the work stands and what comes next, kept short, newest last. Pick-up point for whoever continues it.

## Decided

- **D599**: Terra's people are built on Quaternius's base bodies and modular outfits, in our own paint. The developers, 2026-11-02: "the quarternius stuff is clearly good, and its way better than our stuff. so you're going to stick with it. but we might have an issue w/ clothing/etc".
- **Their parts stay as made**: meshes, UVs, normals, weights, the split into pieces and the bones are never changed by our tools (no clipping pushes, stretch, scale, retopology or decimation). We add paint (new maps on their UVs) and new pieces they lack; a new piece that clips is fixed, never theirs. A check proves every imported piece matches its source vertex for vertex. The developers: "just dont fuck up their parts/etc".
- **D588**: moves no source does well are keyed from a generated pose sheet ([keyed-moves.md](keyed-moves.md)). The idle in the game today is one, on the old body.
- **No twist bones** (D599): their 65 joints and weights stay as made; a forearm wringing in a sword swing goes back to the developers with the film.
- **The swap** (D600): an empty slot shows a bare piece cut exactly from a copy of their body.
- **The starting kit's clothes are the Ranger outfit** (D602): coat = its body and arms, its leggings, its boots.
- **Composed per slot from a wardrobe catalogue** (D611, agent): bodies, pieces and looks as data (`assets/models/character/wardrobe.json`), the game showing a piece a slot and the bare piece where one is empty.
- **Their elbows and wrists hold without twist bones**: shown their man through our clips, the developer: "looks good".
- **One game copy a machine** (D319): the game refuses a second copy, naming the one running and what to do; `--multirun` allows exactly one more for a two-player test; the filming tools film in turn, the render box keeps its three slots. Built (`terra_world::single_instance`, one-copy j2).
- **The paint direction holds**: shown the repainted Ranger beside theirs, the developer: "looks the same tbqh so i guess thats good?" At play distance our paint reads as theirs does, so job 4 refines, it does not push.

## Licences

The developers bought the Source editions, 2026-11-02, from quaternius.itch.io. Each zip's own licence file governs:

| Pack | Licence | In the public repo |
|---|---|---|
| Universal Base Characters [Source] (2025-08) | CC0 1.0 | Yes, with its License_Source.txt verbatim beside it |
| Modular Character Outfits - Fantasy [Source] (v2.1) | CC0 1.0 | Yes, likewise |
| Bestiary - Dungeon Monsters Kit [Source] | Quaternius Asset License v1.0 (2026-08-28): use in our game, no redistribution of the assets, modified or not | **No.** Kept out of git; a public repo is redistribution |

The animation libraries we already use (UAL1, UAL2) are CC0.

## Where it stands

The numbers now: `cargo xtask characters` (D625), from recorded results. What each job did is in git's log (`git log --grep 'char-'`) and [decisions-log.md](decisions-log.md); this page keeps only the state (D627).

- **The man is in the game** on their body, composed a piece a slot from the wardrobe catalogue (`assets/models/character/wardrobe.json`, `terra_walker::wardrobe`, D611): the player's follows his equipment, taking the coat off shows their bare pieces (D600); the town and the sparring partner wear a look whole. `build_bodies.sh --step wardrobe:male` builds `male.glb` (skeleton, sockets, the fight's clips) and each piece. The woman, and the folk's women, stay on the old base.
- **The wardrobe**: their Ranger, Peasant, Knight, Knight Cloth, Noble and Wizard (colourways 2 and 3) and the men's hairstyles, vendored byte for byte (`vendor.ts`); 40 pieces, 6 looks, optional headwear, hair and beard. `node tools/characters/wardrobe.ts` guards their files (checksums, the private repository's bytes, no reshaping script on them) and checks the looks in every state of undress.
- **Pairings** (`pairings.ts`, `--new` for a new piece alone): 496 measured, 70 refused, 3,357 dressed figures (128,073 in their colourways). Hair and beards are judged against the bare figure as shipped (D611, char-models j1): the hair on what the other piece replaces of it, the other on the bare head; every look wears hair. Left: compose drops a person's hair beside a piece that refuses it, where it should choose the slot's next piece (criteria.json's task 7).
- **The figure's transitions** are scored by `cargo xtask anim` (D631, the `anim` skill) on the headless animation course: inertialization in place of the linear crossfade, at acts' starts and ends and at every change but a stride loop into a stride loop; the foot lock's steps land headed as over their landing. The distance-matched stop is built (branch `anim-stop-cap`) and waits on the developer's call on the mover's brake (on the desk): its pop grows with its play rate squared.
- **Known faults**: his Sprint_Loop plant skates 2.7/7.8 cm (`markers.ts`, over 6; item 6); the town's men are all the same Ranger until the starting kit is outfit data (item 5). His packs and quiver are fitted over his default look (the build's `wardrobe-gear:male` step, into `assets/models/character/male/gear/`, bodies.json's `gear`) and drawn where no piece of his is the item's look; the large pack pokes 18 mm through the Ranger tunic at the right shoulder in Death01 (`layers.ts`; the small pack 1.3 mm), and over another look than the Ranger they are unmeasured. `cargo xtask combat` and `controller` passed with no regression when he went in.
- **Cost**: at the market vantage (146 people, integrated Radeon 8060S, sandbox `--bench`) triangles a frame 9.86 to 8.05 million and GPU 8.47 to 7.38 ms against the old base.

## Next, in v0's order

The switch as bounded jobs (the research note's "The switch in jobs" has the detail). One game run at a time.

**The man goes into the game first** (2026-11-02): item 7 for the man alone, before items 3 to 6; the woman stays on the old base until item 3. The developer, asked whether to bring the dressed man in ahead of the woman and the paint: "Yes, man in first".

1. **Their files in, and the clothing answer**: bodies and outfits under `art/` with their licences; the intact-parts check; how their outfits split, how they map to our slots and v0's starting kit (small_pack, leather_coat, trousers, boots, sword and shield), what they lack, whether pieces of different outfits mix, what the separate woman costs. Files in, the intact-parts check landed, mixing measured; the pack fitted over the dressed Ranger (plain leather straps, lying over his tunic and belts through the clips; a crease where each turns in its own plane and their ends below the bag are left). The rest picks up from `~/.cache/terra-handoff/char-switch-j8.md`: one piece repainted, the woman's pieces, and the recommendation with its cost; the straps' crease and ends if the developers want them.
2. **The man through our pipeline**: done, out of `finish_character.sh` (`tools/characters/out/quaternius-male-game.glb`, not yet in the game). Left over: his skin patches (a second UV set, his mesh untouched) are marked on his full body and matter only if D600 goes the hidden-body way. The dressed man's game file is done (`out/quaternius-male-ranger-game.glb`).
3. **The woman**, with a woman's proportion bands in `proportions.py` (it has only a man's). Her Ranger body and legs weight a vertex to up to eight bones (1,351 of the body's 2,785 vertices past four, up to 7.5% of a vertex's weight); the game now reads and skins all eight (D601: `JOINTS_1` and `WEIGHTS_1` in our `bevy_gltf`, `bevy_mesh` and `bevy_pbr` patches and our skinning shader), held on the game side by `terra_walker::skinning`'s glTF test and the shader test, as `intact.ts` holds the files. `export_character.py` exports all influences already; `rig_check.py` still limits a vertex to four (`max_influences`), to raise to eight before her file goes through it.
4. **The paint as a tool**: our materials on their UVs, the Ranger's first pass refined.
5. **The starting kit as outfit data**: their pieces where they have them, new pieces made on their body where not.
6. **The fight on the new bodies**: `cargo xtask combat` and the carry poses against their baselines.
7. **Into the game**, one body a sex, the arm left clear for the mark; the creator's few choices (D111: sex, skin tone, eye colour, hair; no height or build). The man is in as the player and the sparring partner (char-switch j8): `assets/models/character/bodies.json` names each sex's model, clips and gear folder, a template its sex. His file is a `build_bodies.sh` step, and the folk stand on their sex's body, so the town's men are all the same Ranger (char-switch j9). Left, from `~/.cache/terra-handoff/char-switch-j10.md`: the town's men need variety on him (outfits of theirs or ours, dyes, hair); players of more than one sex need `figure::Moves` per body; the crouch (no key in play); where the mark goes on the dressed man.
8. **Delete the Tripo base** and the scripts it alone needed (`stretch.py`, the clipping fixer's use on bought pieces).

## Open with the developers

- Their man's arms (fingertips 2.2 cm above the 99th percentile) stay as made under the rule; D599 withdrew the stretch.
- Where the mark (D590) shows while he wears the Ranger, whose sleeves and gloves cover all his arm skin: drawn over the sleeve (recommended), only on the bare arm, or a new arm piece of ours with one forearm bare. On the desk.

## Later, past v0

Body build morphs, the other outfits, levels of detail for crowds, the Bestiary's monsters (licence above), the spear's own attack (it goes into the spine in Sword_Attack today), skin-aware hip keeping (D535).
