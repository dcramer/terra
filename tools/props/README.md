# Props

Makes the props things are built as (stations, markers, furniture, anything that stands in the world) from a one-line description to the file the game loads. It is the bake-off's route ([research/ai-models-for-art.md](../../research/ai-models-for-art.md)), kept as a tool: a concept image drawn to the [style guide](../../docs/art-style.md), a low-poly Tripo model of it with its colour, normal, roughness and metal maps, then a free clean-up in Blender that welds it, cuts it to a triangle budget, keeps its textures at a budget by its size (D14, D304) and makes it to [the model contract](../../docs/art-style.md#the-model-contract) (D303): its catalogue size, its origin its anchor, its sockets named. The game draws it in its own textures as its file has it, with nothing fixed up for one model.

Each prop is a folder in [art/props](../../art/props), named as you like, holding `prop.json` and what it was made from:

| File | What it is |
|---|---|
| `prop.json` | What it is and how it is made ours, below |
| `concept.png`, `concept.prompt.txt` | The concept, and the prompt that drew it |
| `model.glb` | Tripo's textured model of the concept, as Studio keeps it |

```json
{
  "subject": "rustic wooden chair: four sturdy pegged legs, a seat of woven rush …",
  "file": "Chair",
  "dims": [0.55, 0.95, 0.67],
  "sockets": { "seat": [0, 0.46, 0.05] },
  "budget": 1000
}
```

`subject` finishes "A single …": what the thing is, what it is made of and its setting's motifs, and nothing about style. The concept's prompt is the subject followed by the style guide's concept paragraph, which `props.ts` reads from [docs/art-style.md](../../docs/art-style.md#the-concept-prompt) so that every concept is drawn alike; an approved image in the style at `art/style/reference.png` is given as a reference too, once there is one. `file` is its name in `assets/models/props`, as `assets/data/builds.ron` names it. `dims` is how wide, high and deep it is in metres, along glTF's X, Y (up) and Z (its front), the same as its catalogue entry's `dims`: `refine` makes the model exactly that size, so state a real one's. `budget` is the triangles it may have.

The model contract (D303), which `refine` makes and `cargo xtask contract` checks:

- `anchor`: where its origin goes. `base`, the default, is the middle of its base, for anything stood on a floor or the ground. `hinge` is a door's or a shutter's leaf: the foot of the edge it turns on, in the middle of its thickness, the leaf running along +X, so it hinges on its left seen from its front. A leaf's `dims` are the leaf its first hole takes (its catalogue entry's).
- `hinge`: `right` for a leaf whose model has its hinges on its right seen from its front; it is mirrored, so every leaf hinges at its origin.
- `sockets`: the points code attaches to, by name, in metres in the finished model (glTF's axes, from its origin), written into its file as empties called `socket.<name>`. A leaf needs `hinge` (`[0, 0, 0]`) and `handle`; a seat needs `seat`, where someone sits.

Optional: `roles`, for a textured prop only the palette colours ([tools/characters/look.py](../characters/look.py)'s `PALETTE`) that pick out a part that glows: its faces whose texture is nearest `Fire` among the roles given glow in the game, such as a forge's coals; `palette: true` to paint every face the nearest of its `roles` instead of keeping its textures, for a tool, whose held look [fit_held.py](../characters/fit_held.py) colours by its `Wood` and `Metal`, and the older trees, which give `size`, their longest side, rather than `dims`; `flat: true` for a thing that lies flat, such as a rug, which generators model tilted or curled: it is laid on the plane it best fits and pressed to its height; `turn`, degrees about the up axis if its front comes out facing the wrong way; `faces`, Tripo's face limit (5,000); `generator`, `h3.1` for Tripo's most detailed model, textured in detail, rather than the low-poly `p1`, for a piece seen close and often such as a building kit's (about 55 credits rather than 50); `into`, another folder of `assets/models` to write it to; `block: true` for a block, such as a section of stone wall, squared to its box however far it leans, so it sits flush against its neighbours, and its sides' colour evened out, since Tripo lights a model's back darker than its front; `reference`, another prop whose concept this one is drawn after, so the pieces of a kit share one material; and `grade`, another prop whose colour this one's texture is matched to when refined (its mean and spread on the surface), so they share one grade too. Refine that one first. `grade` may instead be the grade itself, six sRGB numbers (the mean of red, green and blue, then their spread), for a model its generator coloured too pale, such as bone whiter than the floor it lies on; `grime: true` dirties aged bone or stone, by [aged.py](aged.py): its crevices and the side it lies on go earth brown, stain blotches the rest, as a vertex colour the game multiplies into its texture. A tool's model goes `into` `tools`, standing on the end of its handle, with its parts as `Wood` and `Metal`. `dyes` makes copies after refine with its one strongly coloured cloth dyed another colour (sRGB), each its own file, by [dye.py](dye.py): a stall's awning in other dyes, so a market is not one stall over and over; `weather` greys and darkens the rest too, for wood painted fresh. `char: true` chars a fire's logs after refine, by [char.py](char.py): its wood near-black at the fire's foot to dark brown at the tops, cracked, its painted orange kept only as glowing cracks, the stones left as they are (a campfire's logs were clean pale birch, lake film r5); run it once on a freshly refined model. `coals: true` then adds a fire's coal bed, by [coals.ts](coals.ts): a low ash mound inside its logs with a heap of squat coals sunk in it, two dozen big and middling lumps of 80 triangles half buried under a hundred small embers and charcoal of 20, densest and highest under the apex (a few big ones alone read as yellow pebbles, campfire remodel round 5), their cracks narrow glowing seams through dark crust, the mound between them a crackle of glowing seams through dark ash (their glowing undersides read as red-rimmed baubles, fire round 29), grey ash on their tops and between, glowing orange-yellow in the middle, orange, then dull red under more ash at the edge, a grey-white ash skirt round the heap broken into the earth at its rim; its own material, `Coals`, with an emissive map, so it flickers with the fire, and no more than 4,500 triangles (a campfire's coal card was a flat salmon slab, fire round 17; its 20-sided small coals read as boxy dice, fire round 25). `hearth: true` makes the model by [hearth.ts](hearth.ts) rather than from Tripo's: a campfire as a hero prop (D427), split firewood with bark, split faces, end grain and char leaned in an open teepee in a ring of fieldstones, each its own size, outline (broken across two faces), turn and grey-brown tone, sooted toward the fire and smoke-stained on top (fire round 25: they read as identical beige ovals), about 5,600 triangles before its coal bed, its two materials `Wood` (with normal and crack-glow maps) and `Stone` (Tripo's low-poly campfire read as faceted black prisms, fire round 17); refine writes it, then its coals.

Its textures are kept at a budget by its size (D304): colour and normal maps at 2048 for a prop whose longest side is 1.5 m or more, 1024 below, roughness and metal at half that, stored as JPEG with its tangents. Source art keeps Tripo's maps as made.

## Making one

Run from the repository root. The `models` skill (`.agents/skills/models`) gives the whole order of work around these steps, from choosing a model's size, budget and roles to putting it in the game's data. Tripo needs its setup ([tools/tripo](../tripo/README.md)); Blender runs through [tools/blender](../blender/README.md).

```bash
node tools/props/props.ts concept forge   # free: draws art/props/forge/concept.png; look at it
node tools/props/props.ts model forge     # about 50 Studio credits: views, a P1 model textured at 2K, its normal, roughness and metal maps
node tools/props/props.ts refine forge    # free: assets/models/props/Forge.glb, and a sheet
```

Every model the game loads stands the same way: facing glTF's front (+Z), level, its origin its anchor, at its `dims`. `refine` makes a generated model so: Tripo's front, which is the concept's, is turned to +Z; a model resting on its corners a few degrees askew is levelled on them (`leaned_degrees` in the checks says by how much, and a lean over 8 degrees is left as its shape); and it is squared up, turned by the smallest angle that gives it the tightest footprint, so a model set askew, such as a pickaxe with its head at 45 degrees, lies along the axes. `turn` in `prop.json` corrects one whose front still comes out wrong. To check every model at once:

```bash
node tools/props/props.ts fronts   # tools/props/out/fronts.png: every prop, tool and village model from the front
```

Each should face you, stand level and rest on the ground line. Where a role is matched wrongly because the texture paints a part in another shade than its role's, such as a forge's dull red coals that should be `Fire`, `match` gives the texture's sRGB colour for that role: `"match": {"Fire": [0.68, 0.24, 0.17]}`. To see what the texture really paints, sample it in Blender before guessing. Colours are matched in Lab with lightness counting half, so a part's hue decides its role more than how the texture shaded it.

`all` runs the three in turn, skipping a concept or model `art/props` already has (`--again` remakes it). With no props named, a step does every prop. Look at the concept before spending on a model, and at the sheet, `tools/props/out/<prop>.png`, before calling it done: four views in the game's light, with the figure beside it for scale. `out/<prop>.json` beside it has the checks: triangles, size, loose parts, and how much of it each colour covers.

The concept, model and `prop.json` are source art and are committed (`art/README.md`); the views and sheets in `out/` are not. `refine` is free, so when it improves, run it on every prop:

```bash
node tools/props/props.ts refine
```

## Scans

Rocks, and anything else whose real shape a scan has, come from Poly Haven's CC0 scanned models rather than Tripo (D222):

```bash
node tools/props/scans.ts            # every scan in art/scans.json
node tools/props/scans.ts Rock_4     # one
```

[art/scans.json](../../art/scans.json) names each model the game loads and what it is made from: the Poly Haven id, which piece of a set (largest first), the triangle budget and the folder of `assets/models`. [scans.ts](scans.ts) fetches the scan's geometry without its textures into `out/scans`, and [tools/blender/scan.py](../blender/scan.py) cuts it into its loose pieces, largest first, reduces each to the budget, shades it smooth across curves and split at creases (`--loose --crease 50`) and stands it at the origin; the piece wanted is copied into `assets/models`. The game draws it in its kind's material (`flora.ron`).

## Goods

What a market stall shows (a basket of apples, bread on a board, jars, bolts of cloth) is made from Poly Haven's CC0 scanned models with their own textures, composed by a recipe rather than generated (D352):

```bash
node tools/props/goods.ts                  # every recipe in art/goods.json
node tools/props/goods.ts AppleBasket      # one
```

A recipe in [art/goods.json](../../art/goods.json) names its `sources`, each a scan (`from`, cut to a triangle `budget`, its maps kept at `texture` pixels, `tint` and `saturation` dyeing a glaze; `bake`, a voxel size in metres, for a woven basket that decimation tears apart, remeshed as a shell with the scan's colour, relief and roughness baked into it) or cloth made in a CC0 fabric (`make`: a `bolt` wound into a roll or a `fold`); then its `pieces`, each stood `at` a point (glTF's x, y up, z front) turned `turn` degrees, after `rotate` lays it as it rests; and its `heaps`, a `count` of sources dropped into a piece (`into`) or onto an `area`, onto a false floor, by Blender's rigid bodies, so only a heap's top layer is modelled, as games fill a crate. [goods.py](goods.py) does it in Blender, CPU only, and writes `assets/models/props/<file>.glb` to the model contract (its origin the middle of its base, its front +Z) and `out/goods/<file>.json` with its triangles and the `dims` its catalogue entry states (`builds.ron`'s `Goods`). Which stall sells which is `assets/data/market.ron`.

## Checking it in the game

The sheet shows a prop in Blender. To see it as the game draws it (its textures, the glow, its box, origin and sockets) beside a body, without playing, line it up in the sandbox's workbench; each prop's size, sockets and whether it keeps the model contract are printed, and its box is drawn green if it does and red if not:

```bash
cargo run -p terra_sandbox -- --experiment workbench --model models/character/base.glb --prop Forge --prop Workbench --sides 0 --sheet shots/props
```

`--prop all` stands every item, door and shutter in the catalogue. A tool is checked in the hand with `--hold stone_pickaxe`, after [fit_held.py](../characters/fit_held.py) has made its held look.

`cargo xtask contract` checks every model the game places against its catalogue entry, one line each: its size against `dims`, its anchor, its sockets, and for a door or shutters that its leaf fills the first hole it fits and stays within the wall (`play::contract`). `cargo xtask check` runs the same test, so a squat model, a model left where Tripo put its origin, or a door without its hinge never lands. A held tool's real length is set in `fit_held.py`'s `TOOLS`.

| File | What it does |
|---|---|
| `props.ts` | The steps above |
| `refine.py` | The clean-up, inside Blender |
| `fronts.py` | Each model from the front, for the `fronts` sheet |
| `fence.py` | The town's fence modules, wattle and split rail, built in Blender from the kits' own materials rather than generated (D346): `tools/blender/run.sh tools/props/fence.py` |
| `goods.ts`, `goods.py` | A market stall's goods from CC0 scans, by the recipes in `art/goods.json` (D352), above |
