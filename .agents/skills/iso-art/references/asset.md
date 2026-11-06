# Making a good asset

A hero, creature, NPC, prop or piece of scenery, from a description to a checked GLB in `assets/models/iso/`, by [tools/iso-art](../../../../tools/iso-art/README.md) (D628). One command a stage; each prints a line, logs Tripo's progress to `target/iso-art/NAME/<stage>.log` and writes its credits and minutes to the ledger `art/iso/NAME/asset.json`. `node tools/iso-art/iso-art.ts status` shows every asset at a glance: start there.

## 0. Before spending

- `node tools/tripo/tripo.ts balance`. A spending stage refuses if the balance is short; it reads the price from tools/tripo/README.md's table, the one place prices are written.
- Look for a project already made: `node tools/tripo/tripo.ts list`, then `show PROJECT` (rigged? animated?). An adopted project costs nothing: `node tools/iso-art/iso-art.ts adopt NAME PROJECT --kind K --height M`. The coordinator's lead handoff lists the free rigged ones (knight 18615a10, ogre a661c0b1, tree-monster f76557e2, tree creatures 120f7f53 and 0b21883d, moose db7311a1 walk only, chicken 24b756b4).
- The repository's animated animals (`assets/models/creatures/`) may fill a scene, said to be not from this pipeline.

## 1. Concept

`concept NAME --kind hero|creature|npc|prop|scenery --theme "the biome" [--body biped|other] [--height M] "DESCRIPTION"`. The description says what it is and what it is made of, nothing of style: the tool adds the iso part (whole subject from the front; big masses that read from above; gear fixed to the one figure, nothing loose; a T-pose for a biped, square on its feet for another body) and docs/art-style.md's concept paragraph word for word. `--height` is its real height (D110); a hero defaults to 1.85 m, an NPC to 1.75.

Read `art/iso/NAME/concept.png` against [art-style.md](../../../../docs/art-style.md)'s checklist before `model`: one figure, gear on it, no weapon floating, values that separate the parts. A bad concept costs a model's 55 credits later.

Concepts now cost 10 to 20 credits: the month's 100 free images were used by 2026-11-05 (a 403 "Insufficient credits"); try no other `--model` without the credits for it.

## 2. Model, rig, presets

- `model NAME` 55 credits, `--standard` 25 (standard geometry and texture, no PBR). The default is for heroes and creatures, which are judged close; standard may do for props.
- `rig NAME` 20: humanoid for hero and npc, `--body other` reads the shape for a creature (quadruped, hexapod, octopod, serpentine, aquatic). An ogre or a tree monster on two legs rigs as biped.
- `animate NAME`, free: the kind's presets from `tools/iso-art/kinds.ts`. A biped gets idle, walk, run, attack (`slash`) and death (`fall`, the nearest). No `hit`: Tripo's `hurt` is 13.8 s lying wounded from frame 0. Every other body gets one walk (`march` for serpentine and aquatic): no idle, attack or death, and `--prompt` motions (20 credits) are humanoid only.

## 3. Export

`export NAME`, free, to `assets/models/iso/<name>.glb` at the kind's texture size. In Node, no Blender (`normalize.ts`):
- **names** the clips ours (Tripo's are plain `idle walk run slash fall`);
- **in place**: Tripo's `--in-place` keeps only its `root` joint still while the pelvis walks 2.8 m and runs 5.5 m, so each chain joint's ground drift (end minus start, in its parent's frame) is taken out, as Mixamo's and Unreal's in-place cuts;
- **cuts** a clip to its `window` in the preset's seconds (the attack to 0.75 to 2.75 s of the 6.6 s slash; the game blends back to idle);
- **loops**: a clip whose ends meet within 1% is only closed; any other is cut to the window whose ends meet best (searched at 120 fps within the spec's bounds, every joint against the body over the ground) and the rest spread over it, rotations by the quaternion between (Unity's loop match and Loop Pose). Tripo's run is 1.25 s of nearly two strides 15 to 23% of the height apart; one stride inside meets within 4 to 6%. Over 10% is left uncut for `check` to fail. After closing, `check`'s loop reads 0: the honest number is the ledger's export note ("ends X% apart before closing"). Pose is matched at the seam, not velocity;
- **root node** named for the asset: life size, facing +Z by its walk, standing on y=0 with the body over the origin.

## 4. Check

`check NAME` exits 1 on a failure; it reads the GLB in Node, no GPU. Clips by name and length, loops close, walk and run in place, walk and run speed from the planted foot (recorded, the game moves the body at it; run over 1.3x walk), facing +Z within 25 degrees, death ends under 60% of its start height, triangles and textures within budget, height within 5%, the idle's first frame on y=0. Then look at it in the scene at the game camera (scene.md) and judge it (judging.md, the asset rubric).

## The budgets (D628; D630 is Open on whether to keep main Terra's)

One triangle a pixel of the asset's box at the closest camera (the hero at 0.20 of a 1080p frame, 117 px a metre), the box the larger of its front and its top, at least 1,000 and at most the kind's cap: hero 24k, creature 24k, NPC 16k, prop 8k, scenery 20k (each kind's share of about 1M triangles a frame; an estimate a bench of the scene is to confirm, D185). Textures: hero, creature and scenery 2K, NPC and prop 1K. Over budget: decimate in Blender, free (the models skill, on a machine that may run it), or `retopo` at 10 credits. Over on 2026-11-05: ogre 28.6k, tree-monster 29.0k, bone-pile 9.8k over 8k; torch 4.6k and skull 7.8k over their 1,000 (the skull has 22 triangles a pixel).

## The ledger

`art/iso/NAME/asset.json`: kind, height, body, project, the concept and its prompt, the description, each stage's credits (the Tripo CLI's own charge) and minutes, export notes (loop gaps, speeds). Commit each GLB (Git LFS) with its ledger as its own commit naming iso-art.

## Cost per asset by kind (measured 2026-11-05)

| Asset | Credits | Minutes |
|---|---|---|
| Adopted rigged character | 0 | animate 0.7 to 1.7, export 0.5 to 0.9, check under 0.01 |
| Adopted prop or scenery | 0 | export 0.1 to 0.2 |
| New hero, NPC or biped creature | concept 10 to 20 + model 55 (25 standard) + rig 20 = 85 to 95 (55 to 65) | model and rig not yet timed |
| New other-bodied creature | the same; one walk only | as above |
| New prop or scenery | concept 10 to 20 + model 55 (25) | as above |
| Over budget | + retopo 10, or a free decimation | |

A re-export Tripo has cached reads 0.0 min; use the first export's minutes for cost.
