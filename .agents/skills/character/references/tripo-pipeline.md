# Character: the Tripo pipeline, design sheet to the game

Today's base body and its gear are built this way until D599's switch to Quaternius's lands ([the skill](../SKILL.md)).

## 1. Design sheet (a person approves it)

A turnaround sheet: the same character three times, front, side and back, **in a T-pose** (arms straight out at shoulder height, palms down, legs straight and a little apart), plain light grey background. A base body wears plain, close-fitting underwear with no relief (D186) and has a shaved head: hair and clothes are separate parts (D111, D112).

Draw it realistically, as the developers asked for characters on 2026-10-12 (D186): a realistic modern game character, more realistic than a painted or stylised one but not a photograph, with real skin, cloth and leather and a little soft shading. The art style guide ([docs/art-style.md](../../../../docs/art-style.md), D14) is being rewritten to match; until it is, the new base sheet `art/characters/base-male/sheet.png` is the style reference: give it as `--ref`, and draw with `--model gpt-image-2.5` (the default image model drifts back to a painted look). Keep the sheet's framing (three views, T-pose, light grey background):

```bash
node tools/tripo/tripo.ts image "Character turnaround sheet of <who they are, build, clothing>, in the realistic style of the reference image: a realistic modern game character, like a high-end third-person action RPG's character model, not a photograph and not a cartoon or hand-painted illustration. Natural anatomy and true proportions, realistic skin with subtle texture, soft natural shading that shows the form. The same person three times side by side, front view, side view and back view, standing in a T-pose with both arms straight out to the sides at shoulder height, palms down, fingers together, legs straight and slightly apart. Lit evenly and softly from the upper left. Plain light grey background, no text, no scenery." --ref art/characters/base-male/sheet.png --model gpt-image-2.5 --t-pose --aspect 16:9 -o tools/characters/out/NAME-sheet.png
```

Gear drawn for a body (a dressed sheet, a pack, a quiver) takes the same model and reference, and keeps Tripo's maps: the realism is in the colour, normal and roughness maps, which the build scales to their budgets (D188).

For a second body of a cast, such as the female base to match the male, give the first body's sheet as a second reference and say so ("the female base body to match the man in the second reference image ... wearing the same underclothes"), so the two share a style and their underclothes.

Show it to the developers before spending anything else. To change a detail, redraw from the approved sheet (`--ref tools/characters/out/NAME-sheet.png`, or `art/characters/NAME/sheet.png` once it is made) and say only what changes; it keeps the rest. **Always end in a T-pose**: a body generated with its arms down and moved into our T-pose crushes its shoulders in every animation.

## 2. Model, rig, convert, check

One command, on the Studio subscription:

```bash
tools/characters/make_character.sh NAME tools/characters/out/NAME-sheet.png 1.80
```

The last number is the character's height in metres (life size, D110). It:

1. crops the front view, up to the first clear gap after it (`front_view.sh`), and makes a textured model (`tripo.ts model`);
2. rigs it with Tripo's current auto-rig in its Unreal skeleton (`tripo.ts rig --skeleton unreal`, the default): the only one of Tripo's skeletons whose twist bones carry skin, with the bone names our clips use;
3. runs the rest, which is free and is `finish_character.sh NAME HEIGHT` on its own:
   - converts Tripo's skeleton onto ours (`convert_rig.py`): our joints move to Tripo's, each bone turns to point at its moved child (D542), Tripo's four twist bones a side are added, Tripo's weights are kept by name, smoothed lightly, and every vertex kept to four bones;
   - gives a person's body a person's hands, feet, arms and legs: every generated body comes with short arms and a low crotch, so the arms are stretched to a 1.86 m reach and the legs made 10% longer, scaled back to its height (`stretch.py`, D593), its plan kept as `out/NAME-stretch.json` so its designs, dressed models and painted skin are stretched alike;
   - checks it (`rig_check.py`), and its reach, fingertips and crotch against a real man's (`proportions.py`, a `PROPORTIONS fail` line fails the build), and renders it moving (`rig_render.py`, to `tools/characters/out/NAME-sheet.png`);
   - exports it as the game reads it (`export_character.py`, to `tools/characters/out/NAME-game.glb`): body, skeleton, sockets and all our animation clips in one file, every clip carried onto this character's skeleton by its bones' turns from a retarget pose (`retarget.py`, D542) and re-sampled on it, with the twist step baked in (`twist.py`, below). That matters: Bevy sets each joint's whole position from the clip, so the mannequin's own clips would pull our moved joints back to the mannequin's.

**When the conversion, checker or export improves, run `finish_character.sh` again for every character** (`art/characters/` lists them, each with its `rigged.glb`), refit their gear (step 5), and check them in the workbench. It costs nothing; only `make_character.sh` spends credits.

Read the line starting `RIGCHECK`. It must say `"pass": true`. Then look at the render sheet.

## 3. When the checker fails

The checker compares the rig with our skeleton and with the hand-rigged mannequin on the same animations. Its baseline, `art/characters/rig-baseline.json`, comes from running it on the mannequin (`assets/models/character/mannequin.glb`) first. It skins as the game does, with dual quaternions.

| Check | What failing means | What to do |
|---|---|---|
| `skeleton` | Bones missing or with the wrong parent | Tripo's bone names no longer match ours (`RENAME` in `convert_rig.py`); Tripo's skeleton may have changed, or the rig was made with another `--skeleton` |
| `rest_pose` | A bone does not point at its next joint | `convert_rig.py` turns each bone onto its moved child (`retarget.orient`); the clips are carried by the bones' turns (`retarget.py`, D542). Judged only on a rig with twist bones (`twist.py` turns them about +Y); Quaternius's skeleton is reported, not judged, and never re-pointed (D599) |
| `weights` | Unweighted vertices, or more than four bones on one | Re-run the conversion; Bevy reads only four |
| `seams` | Copies of a vertex split at a texture seam have different weights, so the seam will open as the body moves | The weld in `convert_rig.py` was skipped or the threshold missed them |
| `reach` | Vertices driven by a bone far from them (an arm stuck to the hip) | Usually the body was generated with limbs touching: redraw the sheet with more space between arms and body, legs apart |
| `symmetry` | A vertex and its mirror driven by different weights (more than a quarter apart) | Usually an asymmetric model; check the sheet, regenerate |
| `motion` | Too many edges crush or stretch in some animation, compared with the mannequin, or a ring of skin round an elbow, knee or wrist shrinks by more than a fifth (`pinched`, `pinched_at`) | Edges: see `worst_places`; try `SMOOTH=4` on the conversion. A pinched elbow or wrist means a limb's roll has nowhere to go: check the body has its twist bones and the clips were baked (`twist.py`), and run `clip_check.py` on the clips. Look at `rig_render`'s sheet before deciding: the limits were set by eye |

## Clips and twist

The clips come from Quaternius's Universal Animation Library 1 and 2 (current versions, bought; `art/README.md`), on a skeleton with Unreal's bone names that Tripo's rig shares. `art/animation/clips.txt` lists the ones the game plays; `clips.py` cuts them into `assets/models/character/mannequin.glb`. To add a clip (or make one, `made.json`), add its name there, rerun `clips.py`, then `clip_check.py`, then `rig_check.py` on the mannequin with `--baseline` (`tools/blender/run.sh tools/characters/rig_check.py assets/models/character/mannequin.glb --baseline`, which rewrites `art/characters/rig-baseline.json`; say why in the commit. Without it the mannequin is held to its recorded scores, and a clip missing from them is held to the bare limit on a body), then `build_bodies.sh`, and commit the base and gear it writes with the clip.

Pick a new clip's source in [docs/animation.md](../../../../docs/animation.md#where-a-clip-comes-from) first: blows, a way of holding and small loops such as the idle are keyed from a pose sheet (D588, [docs/keyed-moves.md](../../../../docs/keyed-moves.md)).

A combat move's clip is made from its spec in its set's list by one command (D450): `node tools/characters/clip.ts CLIP` (keys, Kimodo takes, aimed export, phases, `moves.ts`, the take kept, into a scratch set; `--into SET.ron` for the game's).

Those clips never roll the upper arm or thigh: an arm's whole roll is in the forearm (up to 164 degrees) and a leg's in the shin, and plain skinning wrings the skin shut where that roll happens. Engines fix it with twist bones, and so do we (`twist.py`, research/skin-deformation.md): as each clip is sampled for a body, the forearm's and shin's roll moves to the hand and foot, which end up exactly where they were, and the twist bone halfway along takes half the roll. The game skins weight-blended: dual quaternions, which keep a limb's volume across what is left and a knee's through a deep bend, but the linear blend round the hips, where dual quaternions bulge (`crates/terra_walker/src/skinning.rs`; the rule is data, `assets/models/character/skinning.json`). Every tool that poses a body skins it the same way (`look.skin_as_game`). `clip_check.py` fails a clip that rolls the upper arm or thigh, leaves too big a step for the skin, or turns a twist bone too far in one frame or flips it (only a loop that winds may, once a turn).

### When a limb twists or squeezes

Measure first and change nothing until the measures say where the fault is. Each step is one command.

1. **Is it the clip?**
   - Run `tools/blender/run.sh tools/characters/clip_check.py [CLIP ...]`. A clip that rolls the upper arm or thigh, or leaves too big a step, fails here.
   - `--trace` prints each frame's limb rolls and twist-bone shares.
   - A new clip library or version goes through this before anything else.
   - A third-party clip that misbehaves: check its author's current version and changelog first.
2. **Where and when?**
   - Run `tools/blender/run.sh tools/characters/deform_check.py tools/characters/out/NAME-game.glb [CLIP ...] --sheet=tools/characters/out/NAME-worst.png`.
   - It gives every ring of skin's worst squeeze, and a picture of each bad one at its worst frame.
   - `--linear` or `--dqs` measure one method throughout instead, to see what the blend does.
3. **Look closer, or compare:** run `tools/blender/run.sh tools/characters/compare.py OUT.png --clay --pose=CLIP,FRACTION --focus=BONE,0.5 a.glb b.glb`. That shows bodies side by side at one moment, each on its own clip; it prints which. `body.glb+look.glb` puts gear on: a skinned look bound to the body, a rigid one on its socket.
4. **In the game:**
   - Use the workbench's `--anim CLIP --moments ... --focus BONE --sheet ...` on the body.
   - With `--linear-skinning`, compare against Bevy's own skinning.
   - The log says which skinning is in use, and how much of each mesh is dual quaternion (`skinning:`).
   - `--joints` prints where the game puts each socket and which way it faces, to compare with the export's `SOCKETS`.
5. **Fix at the cause:**
   - A clip: replace it, or take the library's fixed version.
   - The twist step: `twist.py`, then `clip_check.py`.
   - Weights: the conversion.
   - Never fix by eye from one render: a fix holds when the numbers and the pictures agree on the same frames.

## 4. Into the game

The game has one body, the base (`assets/models/character/base.glb`), and every other body is a morph of it (section 6). A new base, or the base remade, goes in through `build_bodies.sh` (section 7), which exports it with its morphs. To look at a body before that, its export is `tools/characters/out/NAME-game.glb`.

Add a row for it to `THIRD_PARTY.md` under "Made with AI". Then look at it in the game's own renderer with the workbench, on the Windows GPU (about a minute; WSL's CPU renderer takes far too long). Beside the mannequin, through the same clips, from two sides at four moments:

```bash
cargo xtask win -p terra_sandbox --copy wb -- --experiment workbench --model models/character/mannequin.glb --model models/character/base.glb --anim Idle_Loop --anim Walk_Loop --anim Jog_Fwd_Loop --anim Sword_Attack --sides 30,100 --moments 0.1,0.35,0.6,0.85 --sheet shots/wb/NAME --clean --settle 60
```

It prints each model's clips, joints and sockets, saves one contact sheet per clip, `shots/wb/NAME-<clip>.png` (rows are sides, columns moments), and exits; `xtask win` runs it in a copy under the Windows temp folder (`%TEMP%\terra\terra_sandbox-wb`), so the pictures are there. A model that fails to load says why and exits with an error. Look for limbs bending at the joints, no light lines or tears, nothing crushed at the shoulders and groin, feet on the floor, the face and hands intact, and how it moves beside the mannequin. Then a tool in hand, close up: `--model models/character/base.glb --hold stone_axe --focus socket.hand.R`. Then show the developers.

The export also gives the body its sockets, the points what is held and worn hangs from: `socket.hand.R` and `.L`, `socket.head`, `socket.back`, `socket.belt.R` and `.L`, placed from this body's own bones and skin. To check or adjust one, drive a running workbench live ([tools/sandbox](../../../../tools/sandbox/README.md)): `look socket.hand.R`, `place socket.hand.R --rotation …`, `shot`. When a value is right, put it in `export_character.py`, which places sockets for every body.

