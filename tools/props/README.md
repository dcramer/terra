# Props

Makes the props things are built as (stations, markers, furniture, anything that stands in the world) from a one-line description to the file the game loads. It is the bake-off's route ([research/ai-models-for-art.md](../../research/ai-models-for-art.md)), kept as a tool: a concept image drawn to the [style guide](../../docs/art-style.md), a low-poly Tripo model of it, then a free clean-up in Blender that welds it, cuts it to a triangle budget and gives every face one of our palette colours, so the game can colour it as it colours every prop (D14, D54).

Each prop is a folder in [art/props](../../art/props), named as you like, holding `prop.json` and what it was made from:

| File | What it is |
|---|---|
| `prop.json` | What it is and how it is made ours, below |
| `concept.png`, `concept.prompt.txt` | The concept, and the prompt that drew it |
| `model.glb` | Tripo's textured model of the concept, as Studio keeps it |

```json
{
  "subject": "village blacksmith's forge: a waist-high hearth of stacked stone …",
  "file": "Forge",
  "size": 2.6,
  "budget": 2500,
  "roles": ["Rock", "RockDark", "Ore", "MetalDark", "Wood", "WoodDark", "Fire", "Black"]
}
```

`subject` finishes "A single …": what the thing is, what it is made of and its setting's motifs, and nothing about style. The concept's prompt is the subject followed by the style guide's concept paragraph, which `props.ts` reads from [docs/art-style.md](../../docs/art-style.md#the-concept-prompt) so that every concept is drawn alike; an approved image in the style at `art/style/reference.png` is given as a reference too, once there is one. `file` is its name in `assets/models/props`, as `assets/data/builds.ron` names it, and `size` its longest side in metres, as there. `budget` is the triangles it may have. `roles` are the palette colours its faces may take, named in [tools/characters/look.py](../characters/look.py)'s `PALETTE`; give only the ones the concept shows, since every face takes the nearest of them. A face coloured `Fire` glows in the game. Optional: `flat: true` for a thing that lies flat, such as a rug, which generators model tilted or curled: it is laid on the plane it best fits and pressed to 2 cm; `turn`, degrees about the up axis if its front comes out facing the wrong way; `faces`, Tripo's face limit (5,000); `textured: true` to keep the textures Tripo made, scaled to 2K, as the style guide now has it (D14), rather than painting each face a role; `generator`, `h3.1` for Tripo's most detailed model, textured in detail, rather than the low-poly `p1`, for a piece seen close and often such as a building kit's (about 55 credits rather than 45); `into`, another folder of `assets/models` to write it to; `block: true` for a block, such as a section of stone wall, squared to its box however far it leans, so it sits flush against its neighbours, and its sides' colour evened out, since Tripo lights a model's back darker than its front; `reference`, another prop whose concept this one is drawn after, so the pieces of a kit share one material; `leaf`, for a door in its frame, where its leaf's edges are across the refined model and how high its top is, in metres, so [split.py](split.py) cuts it into a `Frame` that stays put and a `Leaf` that the game swings on its hinges; and `grade`, another prop whose colour this one's texture is matched to when refined (its mean and spread on the surface), so they share one grade too. Refine that one first. A tool's model goes `into` `tools`, standing on the end of its handle, with its parts as `Wood` and `Metal`: [fit_held.py](../characters/fit_held.py) makes the look held in the hand from it, colouring the metal for a stone or an iron head.

## Making one

Run from the repository root. The `models` skill (`.agents/skills/models`) gives the whole order of work around these steps, from choosing a model's size, budget and roles to putting it in the game's data. Tripo needs its setup ([tools/tripo](../tripo/README.md)); Blender runs through [tools/blender](../blender/README.md).

```bash
node tools/props/props.ts concept forge   # free: draws art/props/forge/concept.png; look at it
node tools/props/props.ts model forge     # about 45 Studio credits: views, then a P1 model textured at 2K
node tools/props/props.ts refine forge    # free: assets/models/props/Forge.glb, and a sheet
```

Every model the game loads stands the same way by default: facing glTF's front (+Z), level, its foot at the origin. `refine` makes a generated model so: Tripo's front, which is the concept's, is turned to +Z; a model resting on its corners a few degrees askew is levelled on them (`leaned_degrees` in the checks says by how much, and a lean over 8 degrees is left as its shape); and it is squared up, turned by the smallest angle that gives it the tightest footprint, so a model set askew, such as a pickaxe with its head at 45 degrees, lies along the axes. `turn` in `prop.json` corrects one whose front still comes out wrong. To check every model at once:

```bash
node tools/props/props.ts fronts   # tools/props/out/fronts.png: every prop, tool and village model from the front
```

Each should face you, stand level and rest on the ground line. Where a colour comes out wrong because the texture paints a part in another shade than its role's, such as a forge's dull red coals that should be `Fire`, `match` gives the texture's sRGB colour for that role: `"match": {"Fire": [0.68, 0.24, 0.17]}`. To see what the texture really paints, sample it in Blender before guessing. Colours are matched in Lab with lightness counting half, so a part's hue decides its role more than how the texture shaded it.

`all` runs the three in turn, skipping a concept or model `art/props` already has (`--again` remakes it). With no props named, a step does every prop. Look at the concept before spending on a model, and at the sheet, `tools/props/out/<prop>.png`, before calling it done: four views in the game's light, with the figure beside it for scale. `out/<prop>.json` beside it has the checks: triangles, size, loose parts, and how much of it each colour covers.

The concept, model and `prop.json` are source art and are committed (`art/README.md`); the views and sheets in `out/` are not. `refine` is free, so when it improves, run it on every prop:

```bash
node tools/props/props.ts refine
```

## Checking it in the game

The sheet shows a prop in Blender. To see it as the game draws it (our colours, the glow, the size `builds.ron` gives it) beside a body, without playing, line it up in the sandbox's workbench; each prop's size is printed:

```bash
cargo run -p terra_sandbox -- --experiment workbench --model models/character/base.glb --prop Forge --prop Workbench --sides 0 --sheet shots/props
```

`--prop all` stands every item in the catalogue. A tool is checked in the hand with `--hold stone_pickaxe`, after [fit_held.py](../characters/fit_held.py) has made its held look.

Every item in `builds.ron` says how tall a real one stands (`height`, lowest and highest in metres), and a test reads every item's model at its catalogue size and fails if it stands outside that (`play::props`, `cargo xtask check`). A squat model, such as a coffee table sold as a dining table, is caught there rather than in play. A held tool's real length is set in `fit_held.py`'s `TOOLS`.

| File | What it does |
|---|---|
| `props.ts` | The steps above |
| `refine.py` | The clean-up, inside Blender |
| `split.py` | A door cut into its frame and its leaf, inside Blender |
| `fronts.py` | Each model from the front, for the `fronts` sheet |
