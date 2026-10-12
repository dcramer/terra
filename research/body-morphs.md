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

## Limits

- Morphs were checked one at a time, at full weight. Mixes (heavy and strong together, beyond 1) are untested; sliders would be clamped to what passes.
- The game side is untested: morph weights shared by body and gear in Bevy, and per-body skeletons for proportions.
- The female's painted skin shows a faint line across the thighs, at the hem of the base's shorts. Skin paints are worth a look per template.
- Faces were not changed: heavy, thin and strong keep the base's face. Head shapes as their own morphs are untried.
