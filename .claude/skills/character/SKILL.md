---
name: character
description: Make a game character for Terra (a base body, an NPC, a body template) from a design sheet to a rigged, animated, checked model in the game: concept image, Tripo model and auto-rig on the Studio subscription, conversion onto our skeleton, the rig checker, export, and a look in the sandbox's workbench. Use when asked to make, generate, rig or fix a character or humanoid model.
---

# Making a character

From a design to a model that plays every animation in the game, with Tripo doing the hard parts and scripts doing the rest. Worked out on 2026-10-11 ([research](../../../research/ai-models-for-art.md#characters-made-and-rigged), D115). The male base body was made this way.

Everything runs from the repository root under WSL, with the Windows Blender (`spike/model_bakeoff/run.sh` finds it) for the steps that must run inside Blender, and the Tripo CLI (`tools/tripo/tripo.ts`, the `tripo` skill) for everything Tripo does, logged in with `TRIPO_STUDIO_COOKIE` in `.env`. Check it first with `node tools/tripo/tripo.ts balance`.

Costs per character, on the Studio subscription: design sheets are free (100 images a month), the model about 25 credits, the rig 20.

## 1. Design sheet (a person approves it)

A turnaround sheet: the same character three times, front, side and back, **in a T-pose** (arms straight out at shoulder height, palms down, legs straight and a little apart), plain light grey background. A base body wears plain underclothes and has a shaved head: hair and clothes are separate parts (D111, D112).

Draw it in the mood board's style by giving the image model a mood-board image as a reference:

```bash
node tools/tripo/tripo.ts image "Character turnaround sheet for a stylised game character: <who they are, build, clothing>. The same person three times side by side, front view, side view and back view, standing in a T-pose with both arms straight out to the sides at shoulder height, palms down, legs straight and slightly apart. Draw it in the art style of the townsfolk in the reference image: hand-painted, slightly simplified forms, warm soft light, not realistic. Plain light grey background, no text, no scenery." --t-pose --aspect 16:9 --ref spike/model_bakeoff/out/tripo/mood-medieval-town-generated_image_url.png -o spike/model_bakeoff/out/tripo/NAME-sheet.png
```

The reference is the medieval-town mood board (docs/art.md); any approved image in our style works.

Show it to the developers before spending anything else. To change a detail, redraw from the approved sheet (`--ref spike/model_bakeoff/out/tripo/NAME-sheet.png`) and say only what changes; it keeps the rest. **Always end in a T-pose**: a body generated with its arms down and moved into our T-pose crushes its shoulders in every animation.

## 2. Model, rig, convert, check

One command, on the Studio subscription:

```bash
spike/character_gear/make_character.sh NAME spike/model_bakeoff/out/tripo/NAME-sheet.png 1.80
```

The last number is the character's height in metres (life size, D110). It:

1. crops the front view (the left 44% of the sheet) and makes a textured model (`tripo.ts model`);
2. rigs it with Tripo's auto-rig in Tripo's own skeleton (`tripo.ts rig --skeleton tripo`), the one the converter reads;
3. converts Tripo's skeleton onto ours (`convert_rig.py`): our joints move to Tripo's, our bones keep their directions, Tripo's weights are renamed onto our bones with its twist and helper bones folded in, then smoothed lightly and kept to four bones a vertex;
4. checks it (`rig_check.py`) and renders it moving (`rig_render.py`, to `spike/character_gear/out/NAME-sheet.png`);
5. exports it as the game reads it (`export_character.py`, to `spike/character_gear/out/NAME-game.glb`): body, skeleton and all our animation clips in one file, every clip re-sampled on this character's skeleton. That matters: Bevy sets each joint's whole position from the clip, so the mannequin's own clips would pull our moved joints back to the mannequin's.

Read the line starting `RIGCHECK`. It must say `"pass": true`. Then look at the render sheet.

## 3. When the checker fails

The checker compares the rig with our skeleton and with the hand-rigged mannequin on the same animations. Its baseline, `spike/character_gear/out/rig-baseline.json`, comes from running it on `figure.gltf` first.

| Check | What failing means | What to do |
|---|---|---|
| `skeleton` | Bones missing or with the wrong parent | The conversion's bone map (`MAP`, `JOINT` in `convert_rig.py`) missed a Tripo bone; Tripo's skeleton may have changed |
| `rest_pose` | A bone points a different way from ours | Never turn bones when fitting; move joints only. Our animations assume our directions |
| `weights` | Unweighted vertices, or more than four bones on one | Re-run the conversion; Bevy reads only four |
| `seams` | Copies of a vertex split at a texture seam have different weights, so the seam will open as the body moves | The weld in `convert_rig.py` was skipped or the threshold missed them |
| `reach` | Vertices driven by a bone far from them (an arm stuck to the hip) | Usually the body was generated with limbs touching: redraw the sheet with more space between arms and body, legs apart |
| `symmetry` | Left and right driven differently | Usually an asymmetric model; check the sheet, regenerate |
| `motion` | Too many edges crush or stretch in some animation, compared with the mannequin | See `worst_places`. Try `SMOOTH=4` on the conversion; if still bad, look at `rig_render`'s sheet before deciding: the limit (3.5x the mannequin) was set by eye |

If Tripo's rig cannot be made to pass, `rig_base.py` rigs the same body with Blender's own weights instead (`run.sh ../character_gear/rig_base.py <refined body> heat <name>`); it did a little worse but passes most checks.

## 4. Into the game

```bash
cp spike/character_gear/out/NAME-game.glb crates/terra_sandbox/assets/models/character/NAME.glb
```

Add a row for it to `THIRD_PARTY.md` under "Made with AI". Then look at it in the game's own renderer with the workbench, on the Windows GPU (about a minute; WSL's CPU renderer takes far too long). Beside the mannequin, through the same clips, from two sides at four moments:

```bash
cargo xtask win -p terra_sandbox --copy wb -- --experiment workbench --model models/character/figure.gltf --model models/character/NAME.glb --anim Idle_Loop --anim Walk_Loop --anim Jog_Fwd_Loop --anim Sword_Attack --sides 30,100 --moments 0.1,0.35,0.6,0.85 --sheet shots/wb/NAME --clean --settle 60
```

It prints each model's clips, joints and sockets, saves one contact sheet per clip, `shots/wb/NAME-<clip>.png` (rows are sides, columns moments), and exits; `xtask win` runs it in a copy under the Windows temp folder (`%TEMP%\terra\terra_sandbox-wb`), so the pictures are there. A model that fails to load says why and exits with an error. Look for limbs bending at the joints, no light lines or tears, nothing crushed at the shoulders and groin, feet on the floor, the face and hands intact, and how it moves beside the mannequin. Then a tool in hand, close up: `--model models/character/NAME.glb --hold stone_axe --focus socket.hand.R`. Then show the developers.

The export also gives the body its sockets, the points what is held and worn hangs from: `socket.hand.R` and `.L`, `socket.head`, `socket.back`, `socket.belt.R` and `.L`, placed from this body's own bones and skin. To check or adjust one, drive a running workbench live ([tools/sandbox](../../../tools/sandbox/README.md)): `look socket.hand.R`, `place socket.hand.R --rotation …`, `shot`. When a value is right, put it in `export_character.py`, which places sockets for every body.

## Traps already found

- Blender's glTF importer leaves the skeleton posed in the first frame of some animation. Clear every pose bone's transform before using its rest pose, or that pose becomes the rest pose.
- The importer turns the skeleton to make Z up, so its own axes are not the world's. Compute directions in the skeleton's space and check them by measuring, never by assuming a sign.
- Scale the skeleton as an object; never apply the scale. Applying it changes the units of the animations' hip movements and the hips sink.
- Blender's automatic weights fail on generated meshes unless they are given a watertight copy (a voxel remesh), and the weights copied back.
- Copying the mannequin's weights onto a generated body did twice as badly as Tripo's rig. Don't.
- Tripo models face +X; ours face -Y. Every script turns them.
- glTF splits a vertex wherever the texture has a seam. Weld the copies before touching weights (`convert_rig.py` does), or each copy gets different weights and the seams open into light lines as the body moves. The checker's `seams` check catches it.
- Bevy needs its `jpeg` feature for Tripo's textures; without it the model fails to load.
