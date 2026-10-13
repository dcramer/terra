# Art style guide

How to make anything the game draws: characters, creatures, props, machines, building pieces, plants, rocks, items and their icons. It holds for every realm, whatever its setting: medieval, cyber, tech, alien or something nobody has thought of yet. Read it before making or judging a model, and check what you made against [the checklist](#checklist).

Status: Decided (D14), 2026-10-12. The style is direction D, **graphic layers**. It is being tried, and like the rest of pre-production it can change. One rule has held from the start: it must not look chunky or Minecraft-like. The explorations that led here (Forever Ago, the four mood boards, the painted four-world board, our early scenes) are research, not guidance: see [research/art-explorations.md](../research/art-explorations.md).

## The look in one line

Bold, simple shapes in big flat areas of colour, with crisp shadow shapes and strong silhouettes and atmospheric depth, as if a good illustrator had drawn the world in a few layers of colour.

## Why direction D

- **It survives many realms.** A model carries shapes and palette roles; a realm brings its palette and its light. The bake-off showed the same files recoloured into starter, cold, gothic, ashen and crystal realms, all convincing (D17, D59, D86).
- **It is mostly a shader.** Our own models, rendered with banded light and a cool shadow, read like the D concepts. The look lives in code we control, not in each model.
- **It forgives generated models.** Big flat colour areas hide the small inconsistencies generated models carry (D1). Painted textures (C) and fine facets (A) both show them.
- **It is ambitious.** The developers chose it as the more ambitious and more attractive of the directions.

## What every realm shares, and what each brings

The rules on this page never change between realms. A realm (or a maker, or a people, D182) changes only what it is made of and what colour it is.

| Constant: this page | From the theme: each realm's data | For example |
|---|---|---|
| Shape language: big simple forms, strong silhouette | Motifs and parts | Timber and thatch; panels and cables; chitin and crystal |
| Colour as flat areas, one palette role per part | The palette: what colour each role is | Warm ochres and greens; cold steel and teal; ash greys with ember accents |
| Shading made by the light, never painted on | The light: sun colour and height, shadow tint, number of bands, night | A low gold sun with violet shadow; a pale white sun with hard blue shadow |
| Depth from atmosphere | Haze and sky | Thick warm haze; a clear black sky with stars |
| Detail scaled to the world, life size | What glows, and in what colour | Lanterns, neon, runes, fungi |
| No textures, no outlines, no noise | | |

A realm is data the renderer and the palette read, never different code or different models (D58). So a model never carries its realm's colours or light baked in. It carries shapes and roles, and the realm supplies the rest. The same chair can stand in a starter village and an ash realm, and look like it belongs in both.

The starting realm reads more fantasy than medieval, given the lore ([vision.md](vision.md)), so its things need not be historically true: hand-made, with a touch of carved ornament and organic curves.

## Life-sized

The world is at real-world scale (D110). A broadleaf tree stands 15 to 25 metres, a pine taller; boulders range from knee-high to bigger than a house; hills rise tens of metres and mountains hundreds. The player should feel small against the land, which is also what makes haze and layered distance work. Simple shapes do not mean small things: a model is simplified, never shrunk ([Shape](#shape), rule 3).

## Shape

1. **Silhouette first.** Filled in solid black, a thing must still say what it is. Make the outline distinct and the proportions clear, and give it one or two features that tell it apart from its neighbours.
2. **Big, medium, small.** A thing is a few big shapes, each broken by a few medium ones. Small detail is rare and goes only where it tells what the thing is: a vice's handle, a hinge, an eye. Aim for about 60% of the area in big shapes, 30% in medium, 10% in small.
3. **Simplify, don't shrink.** The world is life size (D110), so a tree stands 20 metres tall and a boulder can outgrow a house. Detail scales with the world, not the object. A big thing gets more medium shapes, not bigger ones: a 20 m tree is many car-sized leaf clumps, not five house-sized ones.
4. **Straight where made, curved where grown.** Built things have clean planes and straight runs, with only a little wear. Living things are bold, sweeping curves: lobes, clumps and tapers, never noise.
5. **True proportions, simplified.** Things are as tall, wide and thick as real ones. Simplify the forms, but don't caricature them. No oversized heads, hands or tools.
6. **Not chunky.** No cubes standing in for shapes, no visible grid and no blocky stepping, even though structures sit on the voxel grid (D181). A shape that reads as "blocks" is wrong.

## Colour

1. **Flat areas, by role.** Every face takes one palette role (`Wood`, `Metal`, `Cloth`, `Skin`, `Leaves`…) and is that role's colour, flat. The realm's palette says what the colour is ([tools/characters/look.py](../tools/characters/look.py) has today's palette). Name a new role for what the part is made of, never for its colour, so that a realm can recolour it.
2. **Few roles per thing.** Use 3 to 6 roles, 8 at most. Fewer areas of colour, each bigger, is the look.
3. **Value before hue.** Parts that touch differ in lightness, not only in hue, so the form reads in greyscale. Squint at it: you should see three values (light, mid, dark) arranged in big shapes.
4. **One accent.** At most one strong accent (`Accent`, a cloth trim, a gem), on about a tenth of the surface or less, where the eye should go.
5. **Glow is a role.** Fire, neon, runes, crystals, screens, eyes and bioluminescence are all a glowing role (`Fire` today). It is part of the design in every setting, and the realm picks its colour. Glow is used sparingly, as an accent.

## Light and surface

1. **The light does the shading.** A model carries no painted shading: no baked shadows, highlights, ambient occlusion, gradients or dirt. The renderer breaks light into a few crisp bands with a cool shadow tint, and that makes the shadow shapes ([how the renderer makes it](#how-the-renderer-makes-it)).
2. **No texture.** A surface is one colour. There is no pattern, grain, noise or brush stroke, and no photo or painted texture in the game's file. A pattern that matters (stripes on an awning, a banner's device) is modelled as separate faces with their own roles.
3. **No outlines.** Edges come from the light bands and from value contrast between parts.
4. **Edges: hard at creases, smooth across curves.** Normals are smooth across a gentle curve and split where faces meet at more than about 50 degrees. A round trunk then shades in a few clean bands, and a table's edge stays crisp. Never leave a generator's own smoothed normals, and never flat-shade every triangle: that was the old faceted look (direction A).

## Budgets

These are triangles in the game's file, starting from what the bake-off and the props showed reads well. A thing seen up close, or large on screen, may go to the top of its range. A thing placed by the thousand stays at the bottom, and the far tiers go lower still (D148).

| Kind | Triangles |
|---|---|
| A small item: a tool's head, a cup, an ore lump | 300 to 800 |
| A prop: a chair, a barrel, a bench | 600 to 1,500 |
| A large prop or station: a forge, a bed, a machine | 1,500 to 2,500 |
| A creature, wolf-sized | 1,500 to 3,000 |
| A character body | 2,500 to 5,000 |
| A rock | 100 to 600 |
| A tree, near | 2,000 to 5,000, leaf clumps included |

## Made to vary

Most things come in many variants (D182), so make them for it:

- **In parts, where variants are planned.** Make a frame, a top and a trim as separate pieces with roles, so a theme can swap one piece and keep the rest.
- **Neutral, not themed, where it can be.** A crate's shape should not depend on one realm's lore. The theme comes from the parts it adds and from the palette.
- **One model, many palettes.** Before calling a model done, view it in at least two realms' palettes. If it only works in one, its roles are wrong.

## The concept prompt

Most models start as a concept image ([props](../tools/props/README.md), [characters](../.agents/skills/character/SKILL.md)). End the subject's description with this paragraph, word for word, so that every concept is drawn alike. `tools/props/props.ts` reads it from here, so keep it as the one quoted line under this heading:

> Stylised 3D render with a graphic, illustrative look: bold simplified shapes, large flat areas of colour from a limited palette, no texture, no outlines, no painted shading; strong, clear silhouette; true real-world proportions. Single subject, whole subject in frame and centred, lit evenly and softly from the upper left, on a plain light grey background, no ground, no text.

Keep the subject's description to what the thing is, what it is made of and its setting's motifs. Say nothing about style there. A concept for a scene or a mood picture, rather than a model, swaps the second sentence for the realm's light: "a few crisp shadow shapes; warm low sunlight and atmospheric depth", or whatever that realm's light is.

Even light matters for a model. The concept's colours are matched to roles, so painted shadows in the concept would become wrong roles on the model.

## Checklist

Check every model in the game's own light, beside a figure for scale (the workbench's `--prop`, `--model`, or `props.ts` sheets):

- [ ] **Silhouette:** reads as what it is when filled in black, and at icon size.
- [ ] **Size:** life size; within its catalogue height (D147, D110).
- [ ] **Shapes:** big, medium and small, mostly big; nothing chunky or noisy.
- [ ] **Roles:** every face has a role; 3 to 6 roles, 8 at most; named by material.
- [ ] **Values:** touching parts differ in lightness; three values when you squint.
- [ ] **No texture or painted shading** in the game's file.
- [ ] **Normals:** split at creases, smooth across curves.
- [ ] **Budget:** inside its range above.
- [ ] **Two palettes:** looks right in at least two realms' colours.
- [ ] **Contracts:** faces +Z, level, foot at the origin, and parts as [docs/characters.md](characters.md#the-contracts) says.

## What it is not

- **Not chunky or Minecraft-like.** That rule stands from the start (D14).
- **Not faceted papercraft.** That was direction A: every triangle lit flat. Here, facets are not the look.
- **Not painted.** That was direction C: brush strokes and painted textures.
- **Not photoreal.** No grit, no grain and no wear maps.
- **Not cartoon.** No ink outlines and no exaggerated proportions.

## Where the pipeline stands

Not everything we have made yet follows this guide. Each gap is work to do, not an exception to the rules:

- **No D reference image in the repository yet.** The concept prompt above is the reference. The D images are on the private style page; one approved D image should be saved as `art/style/reference.png`, which `props.ts` then gives as `--ref`. Until then, don't give `art/style/mood-medieval-town.png` as a reference: it is direction C.
- **The game has no banded light yet** (see the table below). It shades with Bevy's standard lighting, so models look softer in the game than in this guide.
- **Characters and gear keep Tripo's painted textures**, and template skins are painted. They have not yet been moved to flat roles.
- **Item icons are painted in direction C** (D130). They will be repainted once a D reference exists.
- **The props drawn before this guide** were drawn after the C mood board. They are coloured by role already, so they mostly carry over, but each should be checked against the list above.

## How the renderer makes it

The look is mostly the renderer's: shapes and roles come from the model, and everything else from shared shader code and the realm's data.

| Ingredient | Where it comes from | State |
|---|---|---|
| Shapes and roles | Models made to this guide: concept, Tripo, clean-up into palette roles (D147, D115) | Working for props; characters still carry Tripo's painted textures |
| Banded light | A lighting shader that cuts light into a few crisp bands, the shadow band tinted cool | Not built. Shown in Blender in the bake-off |
| Crease-aware normals | Normals smooth across curves, split at creases over about 50 degrees | Working for props: the clean-up shades them so |
| A palette per realm | Each role's colour, from the realm's data | Roles work; one palette, the starter's |
| Haze and layered distance | Distance fog tinted toward the horizon | Working |
| Sky | A gradient shader with soft clouds, day and night | Working |
| Bloom and grading | Camera effects built into Bevy | Working |
| Dense foliage and wind | Instanced plants, merged grass blades, one wind shader | Working near the start |
| Ground cover | Each ground vertex says what grows there; the shader paints it | Working on the voxel terrain |
| Water | A see-through surface mirroring the sky at a low angle | Working; no waves |
| Shade in corners | Screen-space ambient occlusion | Working; to be checked against the bands |

- **One material pipeline for everything.** Every model goes through the same roles, palette and shader, so the game reads as one thing.
- **Effort goes into light and atmosphere**, which is shader code and shared by everything.
- **Fullness comes from quantity.** Ground cover, undergrowth and canopy in layers, as in Sons of the Forest (D148), drawn in the same simple shapes.

## Open questions

- **How do the bands look on smooth voxel terrain?** Terrain is lit per vertex today; banded light on fine ground may look like contour lines.
- **How does a palette change through seasons and phases** (D149)? Each realm needs a palette made by rule, and nobody has drawn one.
- **Night.** Banded light from a moon and from lamps.
- **Blur and framing**, which matter more in third person.
