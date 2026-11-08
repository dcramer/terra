# iso-art

The iso experiment's art pipeline (iso-art): a hero, creature, NPC or prop from a description to a checked GLB with its clips, through [Tripo](../tripo/README.md), one command a stage. One mesh per character with its gear, rigged by Tripo; a creature plays Tripo's preset clips as they are, a person the game's own, converted onto our skeleton as main Terra's people are ([character skill](../../.agents/skills/character/SKILL.md)) and kept by `convert` (D628). TypeScript on Node 22.18+, no dependencies, no build step; `node --test tools/iso-art/iso-art.test.ts`. How an agent uses it, with the scenes and judging: the [iso-art skill](../../.agents/skills/iso-art/SKILL.md).

```bash
iso=tools/iso-art/iso-art.ts
node $iso concept ranger --kind hero --theme "a dark-fantasy ruin at night" "a hooded ranger in worn leather and a mail shirt"
node $iso model ranger          # 55 credits (--standard 25); refuses if the balance is short
node $iso rig ranger            # 20; humanoid for hero and npc, as its shape reads for a creature (--body)
node $iso animate ranger        # free: the kind's presets (a creature)
node $iso convert ranger FILE   # free: a person, FILE finish_character.sh's NAME-game.glb (TEXTURES=1024) off art/iso/NAME/rigged.glb, kept to the game's clips (--unarmed: a punch)
node $iso export ranger         # free: assets/models/iso/ranger.glb
node $iso decimate ranger [CUT] # free: over its budget, the mesh cut in Blender (or CUT, decimate.py's) and spliced in
node $iso check ranger          # exits 1 if a check fails
node $iso adopt ogre a661c0b1 --kind creature --height 2.8   # an existing project, at its step
node $iso status                # every asset's stages, credits and minutes
```

Each stage prints a line; Tripo's progress goes to `target/iso-art/NAME/<stage>.log`, with its drafts. Each records its credits (the Tripo CLI's own charge) and minutes in the ledger `art/iso/NAME/asset.json`, with the concept, its prompt and the description. Credits are read from tools/tripo/README.md's table, the one place they are written; a spending stage checks the balance against them first.

- **concept**: the description and theme, an iso part (the whole subject from the front; big shapes that read from above; a person's gear part of the one figure or a creature all its own body, by its kind; a T-pose for a biped, a square stance for another body; a neutral studio backdrop, the theme never drawn as a scene), then docs/art-style.md's concept paragraph word for word. 10 to 20 credits: the month's 100 free images were used by 2026-11-05 (Tripo answers a 403 "Insufficient credits"). Look at `art/iso/NAME/concept.png` before `model`.
- **clips** by the rig's type, Tripo's V3 presets: a biped gets `idle`, `walk`, `run`, `attack` (preset `slash`) and `death` (`fall`, the nearest: there is no death preset). No hit: `hurt` is 13.8 s lying wounded from its first frame, so a hit is the game's flash, flinch and hit-stop, or a `--prompt` motion (20 credits). Every other body Tripo rigs (quadruped, hexapod, octopod, serpentine, aquatic) has one preset, a walk (`march` for the last two): no idle, attack or death, which would need `--prompt` motions, humanoid only, so not for them.
- **export** asks for the clips in place at the kind's texture size, names them ours, and adds a root node named for the asset that makes it its height (`--height`, life size, D110), turns it to face +Z by the way its walk goes, and stands it on y=0 with the body over the origin (the model contract's base anchor). The game moves the body at the speed `check` records. On Tripo's file it does three things more (`normalize.ts`):
  - **in place**: Tripo's `--in-place` keeps only its `root` joint still while the pelvis walks 2.8 m and runs 5.5 m, so each chain joint's ground drift (end minus start, in its parent's frame) is taken out, as Mixamo's and Unreal's in-place cuts.
  - **cut**: a clip with a `window` in `kinds.ts` is cut to it, in the preset's own seconds: the attack to 0.75 to 2.75 s of the 6.6 s `slash`, whose last 3 s return slowly to guard (the game blends back to idle).
  - **loop**: a looping clip whose ends meet within 1% is only closed; any other is cut to the window whose ends meet best (searched at 120 fps within the spec's length bounds, each joint against the body over the ground) and what is left spread over it, rotations by the quaternion between (Unity's loop match and Loop Pose). Tripo's run is nearly two strides 15 to 23% of the height apart; one stride inside meets within 4 to 6%. A best window over 10% is left uncut, for `check` to fail. After closing `check` reads 0, so the ledger's export note ("ends X% apart before closing") is the honest number; `check`'s seam reads the rest (below).
  - **centre**: a walk or run's body is moved, by its mean place over the clip, back over its rest place: the run's loop is cut from 0.41 s on, where Tripo's pelvis is 0.8 to 1.5 m ahead (41 to 58% of the height, all five bipeds, 2026-11-06), so it played ahead of the body and leapt there from the walk.
- **decimate**, after every export (export writes the mesh anew): Blender's collapse decimation of the mesh in its rest pose, Decimate first in the stack ahead of the Armature, so the vertex groups (the weights) follow the cut (`tools/blender/decimate.py`, as Simplygon's and Unreal's LOD reduction keep the skin). Only the mesh is spliced back into export's GLB (`decimate.ts`), so its skeleton, binds, clips and the hands' frames a held prop is measured in stay byte for byte; its joints are remapped by name and its points mapped through both files' binds (Blender writes a skinned mesh in the world at rest, Tripo's carries export's scale under it). Blender runs on the render worker only: `cargo xtask remote-render --collect target/job/dec -- sh -c "mkdir -p target/job/dec; tools/blender/run.sh tools/blender/decimate.py assets/models/iso/NAME.glb target/job/dec/NAME.glb BUDGET > target/job/dec/NAME.txt 2>&1; true"` (BUDGET `check`'s triangle "at most"), then `decimate NAME ~/.cache/terra-review/remote/<job>/target/job/dec/NAME.glb` here. The ogre, tree-monster and both treants, 27.5k to 29.0k, went to 23.7k so (2026-11-06), their posed bounds within 2% of the uncut in every clip.

## Cost per asset by kind

Measured 2026-11-05; the credits are tools/tripo/README.md's.

| Asset | Credits | Minutes |
|---|---|---|
| An adopted rigged character | 0 | animate 0.7 to 1.7, export 0.5 to 0.9, check under 0.01 |
| An adopted prop or scenery | 0 | export 0.1 to 0.2 |
| A new hero, NPC or creature | concept 10 to 20, model 55 (25 `--standard`), rig 20: 85 to 95 (55 to 65) | model and rig not timed yet |
| A new prop or scenery | concept 10 to 20, model 55 (25): 65 to 75 (35 to 45) | as above |
| Over its triangle budget | retopo 10, or `decimate` free | the splice under 0.01; Blender's cut on the worker not timed yet |

A re-export Tripo has cached takes 0.0 minutes: cost from the first.

## The checks

| Check | For |
|---|---|
| each clip by name, its length | the game plays it by name; a length far off is the wrong preset or a broken export |
| loops close (idle, walk, run) | the last frame meets the first, or the body jumps each time round |
| walk, run in place | the game moves the body; root motion would carry it twice |
| seam (idle, walk, run) | played round, the step into and out of the seam under 3x the clip's median, its change of velocity under 1.5x the clip's largest: a snap or a velocity that does not meet |
| walk, run centred | the body's mean place within 3% of the height of its rest place, or the gait plays ahead of the body |
| walk, run speed (recorded) | the planted foot's speed against the body, so the game moves it without the feet sliding; run over 1.3x walk |
| facing | the walk goes toward +Z, glTF's front, within 25 degrees |
| death ends down | the body ends under 60% of its start height; the nearest preset may not |
| triangles, textures | the budget (`budget()` in `kinds.ts`): one triangle a pixel of the asset's box at the closest camera (117 px a metre), at least 1,000, at most the kind's cap (hero and creature 24k, NPC 16k, prop 8k, scenery 20k for trees and set pieces) |
| height, on the ground | life size within 5%; the lowest point of the idle's first frame on y=0 |

## Measuring a still

`node tools/iso-art/stats.ts <out.json> <still>...` adds each still's measures (luma p5, p50, p95 and mean, fine detail, saturation, the dark, mid and bright shares, all at 960x540) to the JSON and prints every row; `name=path` copies the still beside the JSON first. A PNG is decoded with node:zlib, any other image through ffmpeg; the downscale is Pillow's Lanczos, its fixed point and passes, so the numbers are the judges' old `stats.py`'s: on the ruin's step1-fight.png both give p5 0.0015, p50 0.0156, p95 0.121, mean 0.0342, fine 0.0676, saturation 0.5163, shares 0.8161, 0.1779, 0.006; on le-river-beam.jpg they differ by at most 0.0005 (ffmpeg's JPEG decoder against libjpeg's). What the measures mean: the skill's judging page. `node --test tools/iso-art/stats.test.ts`.

## Reading a GLB

Read from the GLB in Node (`gltf.ts`: accessors, the node hierarchy posed by a clip, the skin's points), so a check needs no GPU. The checks are `check.ts`; what export does to Tripo's file (names, in place, cuts, loops, the root node) is `normalize.ts`; the ledgers and the credits read from Tripo's README are `ledger.ts`; the cut to budget in Blender, splicing the mesh back under the skeleton, is `decimate.ts`; the asset and clip sheets from the iso crate's asset view are `sheet.ts`; a biome roster's walk speeds are held to their models' measured ones by `roster.ts` (`node --test tools/iso-art/roster.test.ts`). The pipeline's choices are D628. Looking at it in the iso experiment is that crate's own command (`cargo iso --asset NAME`, crates/terra_iso): this tool depends on no crate (D133).
