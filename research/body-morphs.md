# Body morphs on our own base

2026-10-12. Question: can build and sex morphs be made from Tripo designs of the base person, registered onto our base body's mesh, and does gear made once on the base pass its clipping check on every morph? Tried in a spike, now the tools `tools/characters/make_morph.sh` and `paint_skin.sh` (the `character` playbook, section 6), following [docs/character-system-plan.md](../docs/character-system-plan.md).

## Answer

**Yes, for all four morphs tried: heavy, thin, strong and female.**
- **No new art for the gear.** The jerkin, cap and pack made on the base male were carried onto every morph by a tool. Of 12 piece-and-morph checks, 11 pass the clipping limit through our clips.
- **The female is a morph of the male base.** She stands on her own skeleton's proportions and wears her own painted skin, and looks like the original female body. So one base can serve both sexes: gear is made once for both.

## How

- **Designs:** the base man's approved sheet, redrawn by the image model as heavier, thinner and stronger, keeping the same man and the same T-pose (free). Tripo made a model of each (25 credits each). The female is our existing female body.
- **Registration**, each step its own script, nothing borrowed from the design's rig:
  1. The base's own skeleton is turned limb by limb into the design's limbs, its skin following by its own weights. A rigged design on our skeleton instead moves the base's joints onto its own.
  2. The base's skin is drawn onto the design's skin. The pull is smoothed over distance, from 6 cm down to 1.5 cm, so it takes the form and no creases. The hands are kept as the base's.
  3. The pose is undone, giving the change in the T-pose: the morph.
- **Fit:**

  | Morph | Mean distance from the design's skin | 95th percentile |
  |---|---|---|
  | heavy | 2.2 mm | 4.5 mm |
  | thin | 2.7 mm | 2.8 mm |
  | strong | 1.1 mm | 3.8 mm |
  | female | 1.0 mm | 3.6 mm |

  Every morph keeps the base's face, texture layout, weights and skin patches.
- **Joints:** a build morph does not move the skeleton, as Starfield's Heavy does not. Locating joints from the skin round them failed for a heavy body: belly fat dragged the hips and spine 9 to 12 cm. The female's proportions come from her own skeleton.
- **Skin:** Tripo painted the female's skin on the base's mesh in her shape, from her front view (20 credits), baked onto the base's layout.

## Clipping, gear made once on the base

Share of nearby skin through each piece, worst clip; limit 2%.

| Morph | Jerkin | Cap | Pack |
|---|---|---|---|
| base | 1.6% | 0.6% | 1.5% |
| heavy | 1.5% | 0.5% | **2.3%** |
| thin | 1.4% | 0.7% | 0.9% |
| strong | 1.5% | 0.6% | 1.9% |
| female | 1.3% | 0.5% | 1.1% |

The one failure is the pack on the heavy morph, in the sword swing. His larger back presses a few centimetres into the front of the rigid bag. That skin is inside the bag, where it cannot be seen, but the check counts it. The principled fix is the one garments use: a pack hides the skin patches wholly under it.

## Found on the way

- A sheet drawn at 1,024 pixels, padded to 2,048 without scaling, left a small figure in a pale box, and Tripo modelled the box. `make_character.sh` now scales the front view and pads it with the sheet's own background.
- A generated model can carry stray pieces. Registration keeps only the largest connected part, after joining points split at texture seams.
- A body made from the base keeps the base's skin patches (`export_character.py`), so a garment's list of hidden patches means the same skin on every body.

## In the game, 2026-10-12

Built as planned ([docs/character-system-plan.md](../docs/character-system-plan.md)); everything is rebuilt from `art/` by `tools/characters/build_bodies.sh`.

**What ships:**
- `base.glb`: the base body with its four morphs (female, heavy, strong, thin) as morph targets, and their joint and socket offsets.
  - A build morph moves only its six sockets: hands, head, back, belt.
  - The female moves all 60 joints and sockets.
- One file per worn item, with the same morph targets.
- A painted skin for the female.
- `templates.json`, naming five templates.

**In the game:**
- **Morph weights by name.** A body's appearance sets its morph weights by name, and every worn piece takes the same weights.
- **Joints and sockets.** Each joint and socket moves by the weighted offsets. They are put back after each frame's animation, which carries the base's joint positions.
- **Bind poses.** The skin, and every skinned piece worn on it, gets bind poses for the moved joints.
- **Sharing.** Bodies that look the same share one set of bind poses and one skin material.

The player and the five villagers each stand on a template. There is one body file and one file per item, and nothing is per character.

**Checked:**
- **Joints and sockets.** The workbench's `--joints` printed where each template's 10 main joints and sockets stand in the game. All 40 of the shaped templates' matched the morph's exported body, read by `sockets.py`, to the millimetre.
- **Renders.** Sheets of all five templates, dressed, through the idle, the jog and the sword swing, rendered in the game's own renderer on the GPU.
- **Clipping.** Every worn look on every body, with the corrected check below. Worst clip; limit 2%, holes limit 1%:

  | Body | Jerkin | Cap | Small pack | Large pack | Quiver |
  |---|---|---|---|---|---|
  | base | 0.5% | 0.2% | 1.3% | 1.7% | 0% |
  | female | 0.3% | 0% | 1.1% | 1.6% | 0% |
  | thin | 0.4% | 0.2% | 1.1% | 1.3% | 0% |
  | strong | 0.4% | 0% | **2.4%** | **2.3%** | 0.5% |
  | heavy | 0.8% | 0% | **2.9%** | **3.1%** | 0% |

  The packs fail on the two biggest builds in the sword swing. The skin comes through at the right flank and the side of the chest, where the strap runs under the arm to the bag: the strap's 6 mm clearance runs out as a larger flank twists. Turning each strap vertex's offset with the skin's normal made it worse (2.7%), because a solid strap's inner and outer faces turned apart.
- **Holes.** None measured. A hole is skin a piece hides at rest, which the piece then moves more than 5 cm away from.

**The check, corrected.** `gear_check` counted skin that had passed into a closed piece (a heavy back into the bag) as coming through, though nothing can see it. It now counts skin only when the piece is beneath it and the skin is not inside the piece: inside means a ray outward leaves the piece through a face turned the ray's way. A first version, "anything of the piece outward hides the skin", was too lenient: the jerkin's hem pokes read 0.2% instead of 0.5%, the far side of the jerkin hiding them. It also now counts holes. Under the old rule, the same looks on the base read: jerkin 1.6%, packs 1.5%.

**Also built, to check and repeat this:**
- `make_morph.sh`, `paint_skin.sh` and `build_bodies.sh`.
- `sockets.py`.
- The workbench's `--template` and `--joints`.
- The remote's `shape`, which reshapes a body live: `shape --model 2 heavy=0.5`.
- `gear_check --explain`, which now says where skin comes through at each clip's worst moment.
- The icon baker's `--keep`, for a painted icon whose item's file changed but not its look.

**Seen, not yet fixed.** The jerkin is cut from the body's own triangles, so its hem, armholes and neck are sawtoothed. The bake paints that rim in the dressed model's dark edge. On the shorts below the hem it shows as dark and pale triangles.

## Limits

- Morphs were checked one at a time, at full weight. Mixes (heavy and strong together, beyond 1) are untested; sliders would be clamped to what passes.
- The female's painted skin shows a faint line across the thighs, at the hem of the base's shorts. Skin paints are worth a look per template.
- Faces were not changed: heavy, thin and strong keep the base's face. Head shapes as their own morphs are untried.
