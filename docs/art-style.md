# Art style guide

How to make anything the game draws: characters, creatures, props, machines, building pieces, plants, rocks, items and their icons. It holds for every realm, whatever its setting: medieval, cyber, tech, alien or something nobody has thought of yet. Read it before making or judging a model, and check what you made against [the checklist](#checklist).

Status: Decided (D14), 2026-10-12. The style is direction D, **graphic layers**. It is being tried, and like the rest of pre-production it can change. The world-level look (light, atmosphere, how the renderer draws it) is in [art.md](art.md). The explorations that led here are research, not guidance: see [research/art-explorations.md](../research/art-explorations.md).

## The look in one line

Bold, simple shapes in big flat areas of colour, with crisp shadow shapes and strong silhouettes, as if a good illustrator had drawn the world in a few layers of colour.

## What every realm shares, and what each brings

The rules on this page never change between realms. A realm (or a maker, or a people, D182) changes only what it is made of and what colour it is.

| Constant: this page | From the theme: each realm's data |
|---|---|
| Shape language: big simple forms, strong silhouette | Motifs and parts: timber and thatch, panels and cables, chitin and crystal |
| Colour as flat areas, one palette role per part | The palette: what colour each role is |
| Shading made by the light, never painted on | The light: sun colour and height, shadow tint, haze, night |
| Detail scaled to the world, life size | What glows, and in what colour |
| No textures, no outlines, no noise | |

So a model never carries its realm's colours or light baked in. It carries shapes and roles, and the realm supplies the rest. The same chair can stand in a starter village and an ash realm, and look like it belongs in both.

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

1. **The light does the shading.** A model carries no painted shading: no baked shadows, highlights, ambient occlusion, gradients or dirt. The renderer breaks light into a few crisp bands with a cool shadow tint, and that makes the shadow shapes ([art.md](art.md#how-the-look-is-made)).
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
- **The game has no banded light yet.** It shades with Bevy's standard lighting, so models look softer in the game than in this guide.
- **Characters and gear keep Tripo's painted textures**, and template skins are painted. They have not yet been moved to flat roles.
- **Item icons are painted in direction C** (D130). They will be repainted once a D reference exists.
- **The props drawn before this guide** were drawn after the C mood board. They are coloured by role already, so they mostly carry over, but each should be checked against the list above.
