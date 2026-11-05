# Character: body templates

## 6. Body templates: morphs of the base

Every body is the base body (`base-male`) with a few morphs, as modern games do it (D112, [research](../../../../research/modern-character-variety.md)): build (heavy, thin, strong), sex (female), and later heads. A morph is a change of shape of the base's own mesh, so the base's skeleton, weights, texture layout and skin patches serve every body, and gear made on the base fits them all. Worked out on 2026-10-12 ([research/body-morphs.md](../../../../research/body-morphs.md)).

**Make a build morph** from a design of the base person:

```bash
tools/characters/make_morph.sh heavy "the man is much heavier, overweight with a big round belly, thick arms, thighs and neck, a fuller face"
```

The first run only draws the sheet (free) into `art/characters/base-male/morphs/heavy/` and stops. Look at it: it must be the same person in the same T-pose, front view on the left; redraw (delete `sheet.png`) until it is. Run the same command again. It:
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

