---
name: character
description: Make a game character for Terra (a base body, an NPC, a body template or morph) from a design sheet to a rigged, animated, checked model in the game: concept image, Tripo model and auto-rig on the Studio subscription, conversion onto our skeleton, the rig checker, export, and a look in the sandbox's workbench. Use when asked to make, generate, rig or fix a character or humanoid model.
---

# Making a character

From a design to a model that plays every animation in the game, with Tripo doing the hard parts and scripts doing the rest. Worked out on 2026-10-11 ([research](../../../research/ai-models-for-art.md#characters-made-and-rigged), D115). The male base body was made this way.

Everything runs from the repository root under WSL, with the Windows Blender (`tools/blender/run.sh`, which finds it) for the steps that must run inside Blender, and the Tripo CLI (`tools/tripo/tripo.ts`, the `tripo` skill) for everything Tripo does, logged in with `TRIPO_STUDIO_COOKIE` in `.env`. Check it first with `node tools/tripo/tripo.ts balance`.

Costs per character, on the Studio subscription: design sheets are free (100 images a month), the model about 25 credits, the rig 20.

## 1. Design sheet (a person approves it)

A turnaround sheet: the same character three times, front, side and back, **in a T-pose** (arms straight out at shoulder height, palms down, legs straight and a little apart), plain light grey background. A base body wears plain underclothes and has a shaved head: hair and clothes are separate parts (D111, D112).

Draw it in the mood board's style by giving the image model a mood-board image as a reference:

```bash
node tools/tripo/tripo.ts image "Character turnaround sheet for a stylised game character: <who they are, build, clothing>. The same person three times side by side, front view, side view and back view, standing in a T-pose with both arms straight out to the sides at shoulder height, palms down, legs straight and slightly apart. Draw it in the art style of the townsfolk in the reference image: hand-painted, slightly simplified forms, warm soft light, not realistic. Plain light grey background, no text, no scenery." --t-pose --aspect 16:9 --ref art/style/mood-medieval-town.png -o tools/characters/out/NAME-sheet.png
```

The reference is the medieval-town mood board (docs/art.md); any approved image in our style works.

For a second body of a cast, such as the female base to match the male, give the first body's sheet as a second reference and say so ("the female base body to match the man in the second reference image ... wearing the same underclothes"), so the two share a style and their underclothes.

Show it to the developers before spending anything else. To change a detail, redraw from the approved sheet (`--ref tools/characters/out/NAME-sheet.png`, or `art/characters/NAME/sheet.png` once it is made) and say only what changes; it keeps the rest. **Always end in a T-pose**: a body generated with its arms down and moved into our T-pose crushes its shoulders in every animation.

## 2. Model, rig, convert, check

One command, on the Studio subscription:

```bash
tools/characters/make_character.sh NAME tools/characters/out/NAME-sheet.png 1.80
```

The last number is the character's height in metres (life size, D110). It:

1. crops the front view (the left 44% of the sheet) and makes a textured model (`tripo.ts model`);
2. rigs it with Tripo's auto-rig in Tripo's own skeleton (`tripo.ts rig --skeleton tripo`), the one the converter reads;
3. runs the rest, which is free and is `finish_character.sh NAME HEIGHT` on its own:
   - converts Tripo's skeleton onto ours (`convert_rig.py`): our joints move to Tripo's, our bones keep their directions, Tripo's weights are renamed onto our bones with its twist and helper bones folded in, then smoothed lightly; forearm twist bones are added (`twist.py`), and every vertex kept to four bones;
   - checks it (`rig_check.py`) and renders it moving (`rig_render.py`, to `tools/characters/out/NAME-sheet.png`);
   - exports it as the game reads it (`export_character.py`, to `tools/characters/out/NAME-game.glb`): body, skeleton, sockets and all our animation clips in one file, every clip re-sampled on this character's skeleton with the twist bones baked in. That matters: Bevy sets each joint's whole position from the clip, so the mannequin's own clips would pull our moved joints back to the mannequin's.

**When the conversion, checker or export improves, run `finish_character.sh` again for every character** (`art/characters/` lists them, each with its `rigged.glb`), refit their gear (step 5), and check them in the workbench. It costs nothing; only `make_character.sh` spends credits.

Read the line starting `RIGCHECK`. It must say `"pass": true`. Then look at the render sheet.

## 3. When the checker fails

The checker compares the rig with our skeleton and with the hand-rigged mannequin on the same animations. Its baseline, `art/characters/rig-baseline.json`, comes from running it on `figure.gltf` first.

| Check | What failing means | What to do |
|---|---|---|
| `skeleton` | Bones missing or with the wrong parent | The conversion's bone map (`MAP`, `JOINT` in `convert_rig.py`) missed a Tripo bone; Tripo's skeleton may have changed |
| `rest_pose` | A bone points a different way from ours | Never turn bones when fitting; move joints only. Our animations assume our directions |
| `weights` | Unweighted vertices, or more than four bones on one | Re-run the conversion; Bevy reads only four |
| `seams` | Copies of a vertex split at a texture seam have different weights, so the seam will open as the body moves | The weld in `convert_rig.py` was skipped or the threshold missed them |
| `reach` | Vertices driven by a bone far from them (an arm stuck to the hip) | Usually the body was generated with limbs touching: redraw the sheet with more space between arms and body, legs apart |
| `symmetry` | Left and right driven differently | Usually an asymmetric model; check the sheet, regenerate |
| `motion` | Too many edges crush or stretch in some animation, compared with the mannequin, or a ring of skin round an elbow, knee or wrist shrinks by more than a fifth (`pinched`, `pinched_at`) | Edges: see `worst_places`; try `SMOOTH=4` on the conversion. A pinched elbow means a forearm's turn has nowhere to go: check the twist bones (`twist.py`) were added and are driven. Look at `rig_render`'s sheet before deciding: the limits were set by eye |

If Tripo's rig cannot be made to pass, `rig_base.py` rigs the same body with Blender's own weights instead (`tools/blender/run.sh tools/characters/rig_base.py art/characters/NAME/model.glb heat <name>`); it did a little worse but passes most checks.

## 4. Into the game

```bash
cp tools/characters/out/NAME-game.glb assets/models/character/NAME.glb
```

Add a row for it to `THIRD_PARTY.md` under "Made with AI". Then look at it in the game's own renderer with the workbench, on the Windows GPU (about a minute; WSL's CPU renderer takes far too long). Beside the mannequin, through the same clips, from two sides at four moments:

```bash
cargo xtask win -p terra_sandbox --copy wb -- --experiment workbench --model models/character/figure.gltf --model models/character/NAME.glb --anim Idle_Loop --anim Walk_Loop --anim Jog_Fwd_Loop --anim Sword_Attack --sides 30,100 --moments 0.1,0.35,0.6,0.85 --sheet shots/wb/NAME --clean --settle 60
```

It prints each model's clips, joints and sockets, saves one contact sheet per clip, `shots/wb/NAME-<clip>.png` (rows are sides, columns moments), and exits; `xtask win` runs it in a copy under the Windows temp folder (`%TEMP%\terra\terra_sandbox-wb`), so the pictures are there. A model that fails to load says why and exits with an error. Look for limbs bending at the joints, no light lines or tears, nothing crushed at the shoulders and groin, feet on the floor, the face and hands intact, and how it moves beside the mannequin. Then a tool in hand, close up: `--model models/character/NAME.glb --hold stone_axe --focus socket.hand.R`. Then show the developers.

The export also gives the body its sockets, the points what is held and worn hangs from: `socket.hand.R` and `.L`, `socket.head`, `socket.back`, `socket.belt.R` and `.L`, placed from this body's own bones and skin. To check or adjust one, drive a running workbench live ([tools/sandbox](../../../tools/sandbox/README.md)): `look socket.hand.R`, `place socket.hand.R --rotation …`, `shot`. When a value is right, put it in `export_character.py`, which places sockets for every body.

## 5. Gear

The game has many items and several bodies (D127), so gear is data: every worn item has one look per base body, `assets/models/gear/<body>/<item>.glb`, the item named as in the catalogue in lower case with `_` for spaces (`leather_coat`). The game finds it by that name and puts it on as the file says: a skinned look is bound to the body's skeleton; a rigid one names its socket in its glTF extras. No game code names an item, so adding gear is adding files.

**Clothing and armour that bend with the body** (chest, legs, gloves, feet) are made on each base body, so nothing is set per character (D128):

1. Draw that base's own sheet wearing it (free): `--ref` the base's approved sheet, and say "the same person, in exactly the same T-pose ... now also wearing <the piece>". Check it kept the person and the pose.
2. Make the dressed model (25 credits): crop the front view as `make_character.sh` does, then `tripo.ts model`. Keep the sheet, front view and model as source art in `art/gear/dressed-<body>-<item>/` (`sheet.png`, `front.png`, `model.glb`) and commit them.
3. Build the piece from the base body:

```bash
tools/blender/run.sh tools/characters/shell_garment.py assets/models/character/base-male.glb art/gear/dressed-male-coat/model.glb leather_coat Leather chest_sleeveless
```

The last word is its coverage, one of `COVERAGE` in the script (`chest_sleeveless`, `chest_long_sleeves`, `legs_long`, `gloves`, `feet`...): which parts of the body it covers and where its hems fall. The piece is those faces of the base body, pushed out (and as far as the dressed model stands off, for bulk), keeping the body's own weights, so it moves as the skin under it does; its colour is baked from the dressed model. Do not try to cut the piece out of the dressed model by colour or distance: Tripo paints its dressed body a shade off ours and makes it a little thinner, and every such cut came out full of holes.

**Rigid worn things** (a pack, a cap, a quiver) are made as models on their own and fitted at their socket:

```bash
tools/blender/run.sh tools/characters/fit_gear.py assets/models/character/base-male.glb small_pack large_pack leather_cap quiver
```

Each piece's turn, size (a share of the body's height) and fit are in `PIECES`. A pack keeps only its bag from the model: its straps are made from the body's own skin (`bands.py`), along a route of rays cast from the body's joints, so they bend as the shoulders do and fit any body. Keep to the heights off the skin in `docs/characters.md`: a chest piece within 1.8 cm above the waist, straps solid from the skin to 2.2 cm, so a pack and a jerkin from different makers still stack. Tripo turns each model its own way and adds parts that stand out, such as a pack's stiff strap loops, so render a new model from four sides first and set the turn that puts its front to the body's front. Read each `FIT` line: it says how much of the piece rests on the body, and `FLOATING` when too little does.

**Check every piece on every body**, by numbers first:

```bash
tools/blender/run.sh tools/characters/gear_check.py assets/models/character/base-male.glb leather_coat leather_cap small_pack quiver
```

It plays our clips and reports, per clip, the share of skin near each piece that comes through it (or that it sinks into), and `where` on the body. Then look, live, with the body hidden if need be ([tools/sandbox](../../../tools/sandbox/README.md)): a picture through the body hid a coat full of holes. Each piece also lists the patches of skin it covers, which the game does not draw under it (`patches.py`, `body_mask.wgsl`).

Then add a row for it to `THIRD_PARTY.md`.

What is held is the same idea, without a body: one look per item, `models/held/<item>.glb`, with the grip at the origin, the handle along +Y toward the working end, and how far it tips when carried in its glTF extras (`carry_tilt`). `tools/characters/fit_held.py` makes them; each item's length, grip and tilt are in its `TOOLS`. Check a held look in the workbench with `--hold <item>` through a carried clip and a swing.

## 6. Body templates: morphs of the base

Every body is the base body (`base-male`) with a few morphs, as modern games do it (D112, [research](../../../research/modern-character-variety.md)): build (heavy, thin, strong), sex (female), and later heads. A morph is a change of shape of the base's own mesh, so the base's skeleton, weights, texture layout and skin patches serve every body, and gear made on the base fits them all. Worked out on 2026-10-12 ([research/body-morphs.md](../../../research/body-morphs.md)).

**Make a build morph** from a design of the base person:

```bash
tools/characters/make_morph.sh heavy "the man is much heavier, overweight with a big round belly, thick arms, thighs and neck, a fuller face"
```

The first run only draws the sheet (free) into `art/characters/base-male/morphs/heavy/` and stops. Look at it: it must be the same person in the same T-pose, front view on the left third; redraw (delete `sheet.png`) until it is. Run the same command again. It:
1. crops the front view;
2. makes Tripo's model (25 credits);
3. registers the base onto it (`morph.py`);
4. exports the body (`export_character.py`);
5. carries every worn look onto it (`carry_gear.py`);
6. checks them (`gear_check.py`).

Each step is skipped when its source art is already kept, so rerunning costs nothing.

**Make the female (or any other rigged body) a morph** of the base, its own skeleton giving its proportions:

```bash
tools/characters/make_morph.sh female --design assets/models/character/base-female.glb
tools/characters/paint_skin.sh female art/characters/base-female/front.png
```

`paint_skin.sh` has Tripo paint the template's own skin on the base's mesh in its shape, from the design's front view (20 credits). It keeps the painting as `morphs/female/skin.glb` and bakes it onto the base's layout. A build morph keeps the base's skin and needs no painting.

**Read the output:**

- `MORPH ... fitted: mean 2.2 mm, 95% 4.5 mm, unmatched 5.2%`: the base's skin against the design's.
  - Expect a few millimetres and under about 6% unmatched.
  - Much worse means the design is not in the base's pose, or the model carries something besides the body.
- `GEARCHECK` lines, one per worn look: as in section 5, each under 2%.
- Then look: `tools/blender/run.sh tools/characters/compare.py out.png "assets/models/character/base-male.glb" "tools/characters/out/heavy-game.glb+tools/characters/out/heavy-leather_coat.glb"`. Use `+` to dress a body, `--clay` for shape alone.

**What each step does, and why:**

- **The pose comes first.** The base's own limbs are turned into the design's, its skin following by its weights. Without this, an arm a few degrees off becomes a change of shape.
- **The shape is a pull smoothed over distance** (from 6 cm down to 1.5 cm), never over mesh steps, so no crease of the design's clothes comes through. The hands stay the base's: generated hands are worse.
- **A build morph keeps the base's skeleton.** Placing joints from the skin round them let belly fat drag the hips and spine 9 to 12 cm. Height and limb length are bone proportions, made separately; a rigged design, such as the female, brings its own joints.
- **The body keeps the base's patches**, so a garment's list of hidden patches means the same skin on every body.
- **Gear is carried, not remade.** A part on the skin follows it vertex by vertex. A part that stands off (a pack's bag) moves as one piece, clearing the skin that moved out most beneath it.

## Traps already found

- Blender's glTF importer leaves the skeleton posed in the first frame of some animation. Clear every pose bone's transform before using its rest pose, or that pose becomes the rest pose.
- The importer turns the skeleton to make Z up, so its own axes are not the world's. Compute directions in the skeleton's space and check them by measuring, never by assuming a sign.
- Scale the skeleton as an object; never apply the scale. Applying it changes the units of the animations' hip movements and the hips sink.
- Blender's automatic weights fail on generated meshes unless they are given a watertight copy (a voxel remesh), and the weights copied back.
- Copying the mannequin's weights onto a generated body did twice as badly as Tripo's rig. Don't.
- Tripo models face +X; ours face -Y. Every script turns them.
- glTF splits a vertex wherever the texture has a seam. Weld the copies before touching weights (`convert_rig.py` does), or each copy gets different weights and the seams open into light lines as the body moves. The checker's `seams` check catches it.
- Bevy needs its `jpeg` feature for Tripo's textures; without it the model fails to load.
- Our skeleton has no twist bones, so without `twist.py` a forearm's turn all happens at the elbow and the skin there pinches to a thin neck in swings and reaches. glTF does not keep Blender's constraints, so every script that loads a character and plays it calls `twist.drive`.
- The edge check cannot see a pinch: a ring of skin shrinking changes no edge's length much. The `pinched` measure watches the rings at elbows, knees and wrists.
- Scripts that `cd` lose relative paths; resolve them first (`realpath`).
- Current Blender keeps vertex group names on the mesh: a piece built on a new mesh loses them, and the exporter writes it without a skin. Build on a copy of the body's mesh.
- A body file splits vertices at texture seams and patch borders; weld before working on its surface as one, or every patch looks like a separate speck.
- A sheet drawn at 1,024 pixels and padded to 2,048 without scaling leaves a small figure in a pale box, and Tripo models the box. Both `make_character.sh` and `make_morph.sh` scale the front view and pad it with the sheet's own background.
- A generated model can carry stray pieces (a slab of backdrop); `morph.py` keeps only its largest connected part, after joining points split at texture seams.
- The clipping check counts skin inside a rigid piece's volume (a heavy back inside a pack's bag) though it cannot be seen; a rigid piece should hide the patches wholly under it.
- In zsh, `r pose $c` passes "Idle_Loop 0.3" as one word; use `${=c}`.
