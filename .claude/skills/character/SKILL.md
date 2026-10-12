---
name: character
description: Make a game character for Terra (a base body, an NPC, a body template or morph) from a design sheet to a rigged, animated, checked model in the game: concept image, Tripo model and auto-rig on the Studio subscription, conversion onto our skeleton, the rig checker, export, and a look in the sandbox's workbench. Use when asked to make, generate, rig or fix a character or humanoid model.
---

# Making a character

From a design to a model that plays every animation in the game, with Tripo doing the hard parts and scripts doing the rest. Worked out on 2026-10-11 ([research](../../../research/ai-models-for-art.md#characters-made-and-rigged), D115). The male base body was made this way.

Everything runs from the repository root under WSL, with the Windows Blender (`tools/blender/run.sh`, which finds it) for the steps that must run inside Blender, and the Tripo CLI (`tools/tripo/tripo.ts`, the `tripo` skill) for everything Tripo does, logged in with `TRIPO_STUDIO_COOKIE` in `.env`. Check it first with `node tools/tripo/tripo.ts balance`.

Costs per character, on the Studio subscription: design sheets are free (100 images a month); the model and the rig cost what [tools/tripo/README.md](../../../tools/tripo/README.md) lists.

## 1. Design sheet (a person approves it)

A turnaround sheet: the same character three times, front, side and back, **in a T-pose** (arms straight out at shoulder height, palms down, legs straight and a little apart), plain light grey background. A base body wears plain underclothes and has a shaved head: hair and clothes are separate parts (D111, D112).

Draw it in the art style, [docs/art-style.md](../../../docs/art-style.md) (D14, direction D), which every character, creature and piece of gear follows: big simple shapes, flat areas of colour, true proportions, no painted shading. End the prompt with the guide's concept paragraph, keeping the sheet's own framing (three views, T-pose, light grey background):

```bash
node tools/tripo/tripo.ts image "Character turnaround sheet for a stylised game character: <who they are, build, clothing>. The same person three times side by side, front view, side view and back view, standing in a T-pose with both arms straight out to the sides at shoulder height, palms down, legs straight and slightly apart. Stylised 3D render with a graphic, illustrative look: bold simplified shapes, large flat areas of colour from a limited palette, no texture, no outlines, no painted shading; strong, clear silhouette; true real-world proportions. Lit evenly and softly from the upper left. Plain light grey background, no text, no scenery." --t-pose --aspect 16:9 -o tools/characters/out/NAME-sheet.png
```

Once an approved D image is at `art/style/reference.png`, give it as `--ref` too. Don't use `art/style/mood-medieval-town.png`: it is the earlier painted direction (C), kept as research. The bodies made so far were drawn after it and still carry Tripo's painted textures; moving them to flat palette roles is listed in the guide as work to do.

For a second body of a cast, such as the female base to match the male, give the first body's sheet as a second reference and say so ("the female base body to match the man in the second reference image ... wearing the same underclothes"), so the two share a style and their underclothes.

Show it to the developers before spending anything else. To change a detail, redraw from the approved sheet (`--ref tools/characters/out/NAME-sheet.png`, or `art/characters/NAME/sheet.png` once it is made) and say only what changes; it keeps the rest. **Always end in a T-pose**: a body generated with its arms down and moved into our T-pose crushes its shoulders in every animation.

## 2. Model, rig, convert, check

One command, on the Studio subscription:

```bash
tools/characters/make_character.sh NAME tools/characters/out/NAME-sheet.png 1.80
```

The last number is the character's height in metres (life size, D110). It:

1. crops the front view (the left 44% of the sheet) and makes a textured model (`tripo.ts model`);
2. rigs it with Tripo's current auto-rig in its Unreal skeleton (`tripo.ts rig --skeleton unreal`, the default): the only one of Tripo's skeletons whose twist bones carry skin, with the bone names our clips use;
3. runs the rest, which is free and is `finish_character.sh NAME HEIGHT` on its own:
   - converts Tripo's skeleton onto ours (`convert_rig.py`): our joints move to Tripo's, our bones keep their directions, Tripo's four twist bones a side are added, Tripo's weights are kept by name, smoothed lightly, and every vertex kept to four bones;
   - checks it (`rig_check.py`) and renders it moving (`rig_render.py`, to `tools/characters/out/NAME-sheet.png`);
   - exports it as the game reads it (`export_character.py`, to `tools/characters/out/NAME-game.glb`): body, skeleton, sockets and all our animation clips in one file, every clip re-sampled on this character's skeleton, with the twist step baked in (`twist.py`, below). That matters: Bevy sets each joint's whole position from the clip, so the mannequin's own clips would pull our moved joints back to the mannequin's.

**When the conversion, checker or export improves, run `finish_character.sh` again for every character** (`art/characters/` lists them, each with its `rigged.glb`), refit their gear (step 5), and check them in the workbench. It costs nothing; only `make_character.sh` spends credits.

Read the line starting `RIGCHECK`. It must say `"pass": true`. Then look at the render sheet.

## 3. When the checker fails

The checker compares the rig with our skeleton and with the hand-rigged mannequin on the same animations. Its baseline, `art/characters/rig-baseline.json`, comes from running it on the mannequin (`assets/models/character/mannequin.glb`) first. It skins as the game does, with dual quaternions.

| Check | What failing means | What to do |
|---|---|---|
| `skeleton` | Bones missing or with the wrong parent | Tripo's bone names no longer match ours (`RENAME` in `convert_rig.py`); Tripo's skeleton may have changed, or the rig was made with another `--skeleton` |
| `rest_pose` | A bone points a different way from ours | Never turn bones when fitting; move joints only. Our animations assume our directions |
| `weights` | Unweighted vertices, or more than four bones on one | Re-run the conversion; Bevy reads only four |
| `seams` | Copies of a vertex split at a texture seam have different weights, so the seam will open as the body moves | The weld in `convert_rig.py` was skipped or the threshold missed them |
| `reach` | Vertices driven by a bone far from them (an arm stuck to the hip) | Usually the body was generated with limbs touching: redraw the sheet with more space between arms and body, legs apart |
| `symmetry` | A vertex and its mirror driven by different weights (more than a quarter apart) | Usually an asymmetric model; check the sheet, regenerate |
| `motion` | Too many edges crush or stretch in some animation, compared with the mannequin, or a ring of skin round an elbow, knee or wrist shrinks by more than a fifth (`pinched`, `pinched_at`) | Edges: see `worst_places`; try `SMOOTH=4` on the conversion. A pinched elbow or wrist means a limb's roll has nowhere to go: check the body has its twist bones and the clips were baked (`twist.py`), and run `clip_check.py` on the clips. Look at `rig_render`'s sheet before deciding: the limits were set by eye |

## Clips and twist

The clips come from Quaternius's Universal Animation Library 1 and 2 (current versions, bought; `art/README.md`), on a skeleton with Unreal's bone names that Tripo's rig shares. `art/animation/clips.txt` lists the ones the game plays; `clips.py` cuts them into `assets/models/character/mannequin.glb`. To add a clip, add its name there, rerun `clips.py`, then `clip_check.py`, then `build_bodies.sh`.

Those clips never roll the upper arm or thigh: an arm's whole roll is in the forearm (up to 164 degrees) and a leg's in the shin, and plain skinning wrings the skin shut where that roll happens. Engines fix it with twist bones, and so do we (`twist.py`, research/skin-deformation.md): as each clip is sampled for a body, the forearm's and shin's roll moves to the hand and foot, which end up exactly where they were, and the twist bone halfway along takes half the roll. The game skins weight-blended: dual quaternions, which keep a limb's volume across what is left and a knee's through a deep bend, but the linear blend round the hips, where dual quaternions bulge (`crates/terra_game/src/skinning.rs`; the rule is data, `assets/models/character/skinning.json`). Every tool that poses a body skins it the same way (`look.skin_as_game`). `clip_check.py` fails a clip that rolls the upper arm or thigh, or leaves too big a step for the skin.

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

The export also gives the body its sockets, the points what is held and worn hangs from: `socket.hand.R` and `.L`, `socket.head`, `socket.back`, `socket.belt.R` and `.L`, placed from this body's own bones and skin. To check or adjust one, drive a running workbench live ([tools/sandbox](../../../tools/sandbox/README.md)): `look socket.hand.R`, `place socket.hand.R --rotation …`, `shot`. When a value is right, put it in `export_character.py`, which places sockets for every body.

## 5. Gear

The game has many items and many bodies (D127), so gear is data: every worn item has one look, `assets/models/gear/<item>.glb`, made once on the base body and carrying the base's morphs, so it fits every body (D128). The item is named as in the catalogue in lower case with `_` for spaces (`leather_coat`). The game finds it by that name and puts it on as the file says: a skinned look is bound to the body's skeleton and follows its morphs; a rigid one names its socket in its glTF extras. No game code names an item, so adding gear is adding files.

A piece is made on the base (below), into `tools/characters/out/gear/base-male/`, then added to the lists in `build_bodies.sh`, which checks it on every morph and writes the game's file (section 7).

**Clothing and armour that bend with the body** (chest, legs, gloves, feet) are made on the base body, so nothing is set per character:

1. Draw that base's own sheet wearing it (free): `--ref` the base's approved sheet, and say "the same person, in exactly the same T-pose ... now also wearing <the piece>". Check it kept the person and the pose.
2. Make the dressed model (a `model`'s credits): crop the front view as `make_character.sh` does, then `tripo.ts model`. Keep the sheet, front view and model as source art in `art/gear/dressed-<body>-<item>/` (`sheet.png`, `front.png`, `model.glb`) and commit them.
3. Build the piece from the base body:

```bash
tools/blender/run.sh tools/characters/shell_garment.py tools/characters/out/base-male-game.glb art/gear/dressed-male-coat/model.glb leather_coat Leather chest_sleeveless
```

The last word is its coverage, one of `COVERAGE` in the script (`chest_sleeveless`, `chest_long_sleeves`, `legs_long`, `gloves`, `feet`...): which parts of the body it covers and where its hems fall. The piece is those faces of the base body, pushed out (and as far as the dressed model stands off, for bulk), keeping the body's own weights, so it moves as the skin under it does; its colour is baked from the dressed model. Do not try to cut the piece out of the dressed model by colour or distance: Tripo paints its dressed body a shade off ours and makes it a little thinner, and every such cut came out full of holes.

**Rigid worn things** (a pack, a cap, a quiver) are made as models on their own and fitted at their socket:

```bash
tools/blender/run.sh tools/characters/fit_gear.py tools/characters/out/base-male-game.glb small_pack large_pack quiver
```

Each piece's turn, size (a share of the body's height) and fit are in `PIECES`. A pack keeps only its bag from the model: its straps are made from the body's own skin (`bands.py`), along a route of rays cast from the body's joints, so they bend as the shoulders do and fit any body. Keep to the heights off the skin in `docs/characters.md`: a chest piece within 1.8 cm above the waist, straps solid from the skin to 2.2 cm, so a pack and a jerkin from different makers still stack. Tripo turns each model its own way and adds parts that stand out, such as a pack's stiff strap loops, so render a new model from four sides first and set the turn that puts its front to the body's front. Read each `FIT` line: it says how much of the piece rests on the body, and `FLOATING` when too little does.

**Check every piece on every body**, by numbers first:

```bash
tools/blender/run.sh tools/characters/gear_check.py tools/characters/out/base-male-game.glb leather_coat leather_cap small_pack quiver
```

It plays our clips and reports, per clip, the share of skin near each piece that shows through it (the piece just beneath the skin, and nothing of it covering the skin from outside), and `where` on the body. Then look, live, with the body hidden if need be ([tools/sandbox](../../../tools/sandbox/README.md)): a picture through the body hid a coat full of holes. Each piece also lists the patches of skin it covers, which the game does not draw under it (`patches.py`, `body_mask.wgsl`).

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
2. makes Tripo's model (a `model`'s credits);
3. registers the base onto it (`morph.py`);
4. exports the body (`export_character.py`);
5. carries every worn look onto it (`carry_gear.py`);
6. checks them (`gear_check.py`).

Each step is skipped when its source art is already kept, so rerunning costs nothing.

**Make the female (or any other rigged body) a morph** of the base, its own skeleton giving its proportions:

```bash
tools/characters/finish_character.sh base-female 1.70
tools/characters/make_morph.sh female --design tools/characters/out/base-female-game.glb
tools/characters/paint_skin.sh female art/characters/base-female/front.png
```

`paint_skin.sh` has Tripo paint the template's own skin on the base's mesh in its shape, from the design's front view (a `texture`'s credits). It keeps the painting as `morphs/female/skin.glb` and bakes it onto the base's layout. A build morph keeps the base's skin and needs no painting.

**Read the output:**

- `MORPH ... fitted: mean 2.2 mm, 95% 4.5 mm, unmatched 5.2%`: the base's skin against the design's.
  - Expect a few millimetres and under about 6% unmatched.
  - Much worse means the design is not in the base's pose, or the model carries something besides the body.
- `GEARCHECK` lines, one per worn look: as in section 5, each under 2%.
- Then look: `tools/blender/run.sh tools/characters/compare.py out.png "tools/characters/out/base-male-game.glb" "tools/characters/out/heavy-game.glb+tools/characters/out/heavy-leather_coat.glb"`. Use `+` to dress a body, `--clay` for shape alone.

**What each step does, and why:**

- **The pose comes first.** The base's own limbs are turned into the design's, its skin following by its weights. Without this, an arm a few degrees off becomes a change of shape.
- **The shape is a pull smoothed over distance** (from 6 cm down to 1.5 cm), never over mesh steps, so no crease of the design's clothes comes through. The hands stay the base's: generated hands are worse.
- **A build morph keeps the base's skeleton.** Placing joints from the skin round them let belly fat drag the hips and spine 9 to 12 cm. Height and limb length are bone proportions, made separately; a rigged design, such as the female, brings its own joints.
- **The body keeps the base's patches**, so a garment's list of hidden patches means the same skin on every body.
- **Gear is carried, not remade.** A part on the skin follows it vertex by vertex. A part that stands off (a pack's bag) moves as one piece, clearing the skin that moved out most beneath it.

## 7. Rebuilding everything, and into the game

```bash
tools/characters/build_bodies.sh
```

One command rebuilds every character asset the game loads from the source art in `art/`, for free: the base, its gear, every morph (each checked with every look on it), each template's painted skin, and the game's files:

| File | What it is |
|---|---|
| `assets/models/character/base.glb` | The base body with every morph as a morph target, and each morph's joint and socket offsets in the skeleton node's glTF extras (`{"morphs": {M: {node: [x, y, z]}}}`) |
| `assets/models/character/skins/<template>.png` | A template's painted skin, on the base's layout |
| `assets/models/character/templates.json` | The templates (D111): each a mix of morphs and a skin. Edited by hand |
| `assets/models/gear/<item>.glb` | Each worn look, with the same morph targets in the same order |

Run it after changing a script it calls, or after adding a morph, a skin or a piece of gear to the lists at its top. Every `RIGCHECK`, `MORPH` and `GEARCHECK` line it prints must pass. A long run: about an hour, most of it writing the items' files. Don't edit the scripts while it runs: bash reads its script as it goes, and each step loads the Python afresh, so a change mid-run mixes two versions.

Rewriting the items' files changes their bytes, so `cargo xtask check` then finds their painted icons stale. If an item's look did not change, compare its render with its icon (`cargo xtask icons --renders DIR`) and keep the painting, `cargo xtask icons --keep <item>`. If it did change, paint it again (the `item-icons` playbook).

**In the game** (`crates/terra_game/src/appearance.rs`):
- A body's `Appearance` (morph weights and skin) sets its morph weights by name.
- Each of its joints and sockets moves by the mix of its morphs' offsets, kept there under animation.
- Its skin, and everything worn on it, gets bind poses for the moved joints, shared between bodies that look the same.

**Check it in the game's own renderer**: each template side by side, dressed, through the clips, with where its sockets stand:

```bash
cargo xtask win -p terra_sandbox --copy wb -- --experiment workbench --model models/character/base.glb --template male --model models/character/base.glb --template female --model models/character/base.glb --template heavy --wear leather_coat --wear leather_cap --wear small_pack --anim Idle_Loop --anim Jog_Fwd_Loop --anim Sword_Attack --sides 30,200 --moments 0.3,0.7 --joints --sheet shots/wb/templates --clean --settle 60
```

- **The `JOINTS` lines**, in Blender's axes, must match the `SOCKETS` lines the export printed for that morph (`tools/characters/out/M-game.glb`) to a millimetre or so: that is the game shaping a body as the tools do.
- **Live, with `--remote`**: `node tools/sandbox/remote.ts shape --model 2 heavy=0.5 strong=0.3` reshapes a body in the running workbench, to look at any mix.

### Gear on every body

A worn look is made once, on the base, and every morph carries it (`carry_gear.py`):
- **Skinned parts close to the skin** follow it vertex by vertex: garments, straps.
- **Parts that stand off** move as one, far enough to keep their clearance over the skin that grew most beneath them: a pack's bag.
- **A look hung on a socket** moves with its socket, which each morph moves, and then by the same rule as a part that stands off. A bigger seat does not swallow a quiver. Its shift per morph is in its extras (`morphs`), and the game mixes it by the body's morph weights (`character::Shifts`).
- **The clipping fixer.** A morph can still bring skin through a look, as a heavy flank through a strap in a swing.
  - `make_morph.sh` checks each look on the morph's body with `gear_check.py --fix`. That records how far each part of the look must move out to clear the skin in every frame, plus 4 mm.
  - It then carries the look again with that push baked into the morph, away from the skin and spread smoothly round each place, and checks again, for up to three rounds.
  - A round is kept only while the look gets better. One that makes it worse is undone, and no more are tried.
  - That is BodySlide's clipping fixer per extreme, through our clips as well as at rest. The game's files carry the same pushes (`out/M-ITEM-push.npz`).
  - What a push cannot fix: clipping that happens only in pose, where it would have to go one way in one frame and another in the next. A thigh lifting into a coat's skirt is that kind. Pushing the skirt out sinks it into the belly at rest, and it measured worse each round, so its rounds are undone. That wants the look's weights fixed (the skirt partly following the thigh), not more push.

## Traps already found

- Blender's glTF importer leaves the skeleton posed in the first frame of some animation. Clear every pose bone's transform before using its rest pose, or that pose becomes the rest pose.
- The importer turns the skeleton to make Z up, so its own axes are not the world's. Compute directions in the skeleton's space and check them by measuring, never by assuming a sign.
- Scale the skeleton as an object; never apply the scale. Applying it changes the units of the animations' hip movements and the hips sink.
- Blender's automatic weights fail on generated meshes unless they are given a watertight copy (a voxel remesh), and the weights copied back.
- Copying the mannequin's weights onto a generated body did twice as badly as Tripo's rig. Don't.
- Tripo models face +X; ours face -Y. Every script turns them.
- glTF splits a vertex wherever the texture has a seam. Weld the copies before touching weights (`convert_rig.py` does), or each copy gets different weights and the seams open into light lines as the body moves. The checker's `seams` check catches it.
- Bevy needs its `jpeg` feature for Tripo's textures; without it the model fails to load.
- Tripo's own (V1) rig and its `actorcore` skeleton have twist and share bones with no skin on them; the `unreal` skeleton's carry skin. Check with a weight count before trusting a rig's bones.
- A third-party clip library may be a stale copy: our first was a 2025 mirror of Quaternius's, which missed his fix for the sword swing's elbow twist. Take it from the author, at the current version, and read the changelog.
- glTF does not keep Blender's constraints, and the game plays clips: the twist step is baked into every clip by `export_character.py`, and any script that plays the mannequin's clips on a body bakes them too (`twist.bake`). A body file's clips are baked already.
- Two files loaded in one Blender scene each bring their own copy of a clip, the second named `Name.001`: give each body its own (`compare.py` does), or one plays the other's.
- A roll past half a turn is ambiguous in any one frame: unwrap it over the whole clip (`twist.unwrapped`), never from the first frame.
- The edge check cannot see a pinch: a ring of skin shrinking changes no edge's length much. The `pinched` measure watches the rings at elbows, knees and wrists.
- Scripts that `cd` lose relative paths; resolve them first (`realpath`).
- Current Blender keeps vertex group names on the mesh: a piece built on a new mesh loses them, and the exporter writes it without a skin. Build on a copy of the body's mesh.
- A body file splits vertices at texture seams and patch borders; weld before working on its surface as one, or every patch looks like a separate speck.
- A sheet drawn at 1,024 pixels and padded to 2,048 without scaling leaves a small figure in a pale box, and Tripo models the box. Both `make_character.sh` and `make_morph.sh` scale the front view and pad it with the sheet's own background.
- A generated model can carry stray pieces (a slab of backdrop); `morph.py` keeps only its largest connected part, after joining points split at texture seams.
- The clipping check once counted skin that had passed into a closed piece (a heavy back inside a pack's bag), which no one can see. It now counts only skin with nothing of the piece covering it from outside. Packs also hide the patches wholly under them, as garments do.
- In zsh, `r pose $c` passes "Idle_Loop 0.3" as one word; use `${=c}`.
