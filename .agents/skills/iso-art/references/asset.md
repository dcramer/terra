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
- **centres** the walk and run over the body's rest place (`centre()`, after the loop): the in-place cut takes out a straight line end to end, and the run's loop, cut from 0.41 s where Tripo's pelvis is still gathering speed from a stand, kept the body 41 to 58% of the height ahead (1.2 m on the tree monster), so the run drew ahead of its entity and leapt there at every switch from walk (iso-art j29; the ledger's "centred run (body N% of height off)");
- **root node** named for the asset: life size, facing +Z by its walk, standing on y=0 with the body over the origin.

Then, over its triangle budget, `decimate NAME CUT` after every export (export writes the mesh anew). The cut is Blender's collapse on the mesh in rest pose with the vertex groups carried (`tools/blender/decimate.py`, on the render worker only: tools/iso-art/README.md has the job's line); only the mesh is spliced into export's GLB, joints by name and points mapped through both files' binds, so the skeleton, clips and the hand frames a held prop uses stay byte for byte. Spliced unmapped, a rigged cut came out 76 to 159% off (j25): Blender writes the skinned points in the world at rest. Then `check`, and re-shoot the asset view: the posed bounds should stay within about 2% of the uncut in every clip.

## 4. Check

`check NAME` exits 1 on a failure; it reads the GLB in Node, no GPU. Clips by name and length, loops close, walk and run in place, walk and run speed from the planted foot (recorded, the game moves the body at it; run over 1.3x walk), facing +Z within 25 degrees, death ends under 60% of its start height, triangles and textures within budget, height within 5%, the idle's first frame on y=0; each loop's seam (every joint at 30 fps: the step across the seam under 3x the clip's median step, the change of velocity under 1.5x the clip's largest; Tripo's come in at 0.8 to 1.7x and 0.3 to 1.1x) and the walk and run centred (body under 3% of height off its rest place). Then the asset view at the game camera: `--asset NAME --shot views/NAME.png` writes the still and each clip's strip for `sheet`, and with `--film N` N frames of the walk three laps then the run three, as the game plays them (repeat, its 220 ms crossfade), to see the seams and the switch hold. Then look at it in the scene at the game camera (scene.md) and judge it (judging.md, the asset rubric).

**A creature stands darker than its ground** (the refs' creatures, `refs/creatures.json`: darker and of another hue than the floor). Tripo's bark and hide come at the ground's value, so in the sun a tree creature reads at the rocks' grey. Measure the step in L*: the body by a magenta mask, against a 10 px ring of the ground beside it, in the brightest scene it plays in; aim at 5 L* darker or more (iso-art j34: forest treant-a +7.8 to -6.3 at albedo x0.2). For now the scene sets it (`ruin/fight.rs` `value`, the fighters' reflectance 0.15 in `rim.rs`); the asset's own grade in its GLB (baseColorFactor, `KHR_materials_specular`, which Bevy's loader reads) is the next step.

## 5. Held props on a Tripo rig

Tripo's characters come empty-handed (the knight), so a weapon is its own prop hung on a hand bone, as Unreal's `hand_r` socket:
- `adopt NAME PROJECT --kind prop --height M` then `export NAME`, both free (the sword: d7c0d997, 0.95 m, 0.2 min). Read its profile along y in Node (gltf.ts `accessor` and `pose`) to find pommel, grip and guard: Tripo's sword stands blade up, edges along x, grip 0.05 to 0.15 m.
- Measure the grip on the body's GLB, not by eye: the hand's finger joints in the hand's own frame (fingers along +y, palm -z, thumb -x on Tripo's V3 rig), then the hand's axes over the attack clip; the blade goes along the axis that sweeps back at the wind-up, forward at the blow and down after it (the fingers' +y tipped 0.35 to the thumb).
- The bone's world scale is the armature's (1.89 on the knight): take it out from the bone's `GlobalTransform` every frame, or the sword is twice its size. Spawn it a child of the bone (`WorldAssetRoot` with `ChildOf(bone)`), and keep body finders to `Without<ChildOf>` so they never take the prop for a body (`spike/iso/src/ruin/held.rs`).

## The budgets (D628; D630 is Open on whether to keep main Terra's)

One triangle a pixel of the asset's box at the closest camera (the hero at 0.20 of a 1080p frame, 117 px a metre), the box the larger of its front and its top, at least 1,000 and at most the kind's cap: hero 24k, creature 24k, NPC 16k, prop 8k, scenery 20k (each kind's share of about 1M triangles a frame; an estimate a bench of the scene is to confirm, D185). Textures: hero, creature and scenery 2K, NPC and prop 1K. Over budget: `decimate`, free (Export above), or `retopo` at 10 credits. Cut 2026-11-06: ogre 28.6k, tree-monster 29.0k, treant-a 28.9k, treant-b 27.6k to 23.7k; the sword 4.8k to 2.4k. Still over: bone-pile 9.8k over 8k; torch 4.6k and skull 7.8k over their 1,000 (the skull has 22 triangles a pixel).

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
| Over budget | + retopo 10, or `decimate`, free | |

A re-export Tripo has cached reads 0.0 min; use the first export's minutes for cost.
