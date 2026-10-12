---
name: models
description: Make a model for Terra that is not a character: a prop, station, furniture, tool or weapon, an item, a tree, plant or rock, from a description to a checked model in the game. Covers deciding its size, budget and palette roles; the concept in the art style; the Tripo model on the Studio subscription; the Blender clean-up to palette roles; putting it in the game's data; and checking it against the style guide. Use when asked to make, generate, add or replace a model for the game, or when adding something to the game that needs a model.
---

# Making a model for the game

Any model that is not a character goes from a one-line description to the file the game loads through `tools/props` ([README](../../../tools/props/README.md)): a concept image drawn to the style guide, a Tripo model of it, and a free clean-up in Blender that gives every face one palette role. This skill is the order of work and the decisions around those steps.

**The style guide is [docs/art-style.md](../../../docs/art-style.md) (D14).** It decides how anything looks: shapes, roles, budgets, the concept prompt and the checklist. Read it before deciding anything below, and judge every concept and model by it. Don't restate it or work around it; if it is silent or wrong for a case, say so to the developers.

Not here: characters, bodies and gear (the `character` skill); item icons (`item-icons`); walls, floors and roofs, which are made in code from kits (D177); anything Tripo does outside this pipeline (`tripo`).

## 1. Decide what it is

| It is | Its model goes in | The game finds it in |
|---|---|---|
| Furniture, a prop or a station | `assets/models/props` | `assets/data/builds.ron`, an `Items` variant: file, size, the height a real one stands, cost; a station also in `stations.ron` |
| A tool or weapon | `assets/models/tools` (`"into": "tools"`), standing on the end of its handle, parts `Wood` and `Metal` | `items.ron`; its held look is made from it by `tools/characters/fit_held.py` |
| An item lying on its own, such as a material | `assets/models/items` | `items.ron`; its icon by `item-icons` |
| A tree, bush, plant, flower or rock | `assets/models/nature` | `assets/data/flora.ron`, a kind (below) |
| A village piece | `assets/models/village` | `builds.ron` |

A creature or animal is not on this pipeline yet: it needs a rig (`tripo rig --type other`) and animations. Ask the developers before starting one.

Then, from the style guide:

- **Size:** life size (D110). A real one's height or longest side, in metres.
- **Budget:** the triangles its kind may have, from the guide's table. Things placed by the thousand, such as trees and rocks, stay low in their range.
- **Roles:** 3 to 6 palette roles, named for what each part is made of, from `PALETTE` in [tools/characters/look.py](../../../tools/characters/look.py). If none fits a material, add one named for the material (`Thatch`, `Chitin`), never for its colour, with today's colour for it. Every face of the model will take the nearest role to the concept's colour there, so list only roles the concept shows.
- **Variants** (D182): most things come in many. Plan the parts a variant would swap. A kind of tree or rock wants two to four models, each its own concept and model; plants in quantity read as one kind only if their models differ.

## 2. Write its prop.json

One folder per model in [art/props](../../../art/props), named for it: `art/props/<name>/prop.json`, as the props README describes.

`subject` is what the thing is, what it is made of and its setting's motifs, true to life and simplified, never invented: "English oak tree, mature: a short, thick, gnarled trunk splitting low into heavy crooked limbs …". **Nothing about style or framing**: the tool appends the style guide's concept paragraph word for word.

## 3. Draw the concept (free) and look at it

```bash
node tools/props/props.ts concept <name>
```

Up to 100 images a month are free. Read `art/props/<name>/concept.png` and check it against the guide: silhouette, big-medium-small shapes, few areas of flat colour that become roles, three values when you squint, true proportions, nothing painted on. Each `--again` is a new roll; change the subject rather than rolling many times. A concept for a new kind of thing, or several at once, goes to the developers before anything is spent: show them the images side by side.

## 4. Ask before spending

A model costs about 45 Studio credits (views, then a P1 model textured at 2K; prices in [tools/tripo/README.md](../../../tools/tripo/README.md)). Check the balance (`node tools/tripo/tripo.ts balance`) and say what it will cost. Several models, or more than about 100 credits, needs the developers' go-ahead.

## 5. Model it, then make it ours

```bash
node tools/props/props.ts model <name>    # about 45 credits: art/props/<name>/model.glb
node tools/props/props.ts refine <name>   # free, in Blender: the game's file, and a sheet
```

`refine` needs Blender (`tools/blender`). It welds the model, cuts it to the budget, gives every face a role, stands it facing +Z, level, its foot at the origin, at its size, and draws `tools/props/out/<name>.png` (four views beside a figure, in the game's light) with checks in `out/<name>.json`: triangles, size, loose parts, and how much each role covers. Look at both. Fix what comes out wrong in `prop.json` and refine again, which is free: `match` for a role the texture paints in another shade, `turn` for a front facing the wrong way, `budget`, `flat`.

## 6. Put it in the game

Add it to its data file (the table in step 1). For a kind of tree, plant or rock in `flora.ron`:

- `models`: its files in `assets/models/nature`, two to four for variety.
- `colours`: **every role its models use**, each with our colour and whether it is `Foliage` (moves in the wind) or `Firm`. A role left out takes the shared colours, whose default is foliage green, so bark left out turns green and sways.
- `sway`, `grows` (its layer, and where in it: `below`, `above`, `chance`; the order of kinds decides which is tried first), `size` (life size), and `cut` (what cutting takes and gives, as the other trees do).

## 7. Check it in the game

```bash
cargo run -p terra_sandbox -- --experiment workbench --model models/character/base.glb --prop <File> --sides 0 --sheet shots/<name>
cargo run -p terra_sandbox -- --experiment workbench --model models/character/base.glb --model models/nature/<File>.glb --sides 0 --sheet shots/<name>
```

The first is for anything in the build catalogue, drawn as when placed; the second for any model, such as a tree, beside a body for scale. For growing things, also see them where they grow: a scripted run of the play experiment with `--at` and `--face` near them, and `--shot`. Go through the style guide's checklist, and run `cargo xtask check`: it holds every catalogue item to its real height.

## 8. Commit

In one commit: `art/props/<name>/` (`prop.json`, `concept.png`, `concept.prompt.txt`, `model.glb`, which are source art and cost credits to remake), the file in `assets/models/`, and the data entry. `tools/props/out/` and `shots/` are not committed. Pull, check and push as `AGENTS.md` says.

## Where this pipeline falls short of the guide

The style guide lists what does not yet follow it ("Where the pipeline stands"). The ones that touch this skill:

- **`refine` flat-shades every face.** The guide wants normals split at creases and smooth across curves. Until `refine.py` does that, models come out faceted.
- **No approved image in the style yet.** Concepts are drawn from the guide's paragraph alone. Once one is approved, put it at `art/style/reference.png` and `props.ts` gives it as a reference.
- **Seeing a model in a second realm's palette** has no tool yet; the checklist asks for it.
