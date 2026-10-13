# Art style guide

How to make anything the game draws: characters, creatures, props, machines, building pieces, plants, rocks, items and their icons. It holds for every realm, whatever its setting: medieval, cyber, tech, alien or something nobody has thought of yet. Read it before making or judging a model, and check what you made against [the checklist](#checklist).

Status: Decided (D14), 2026-10-12. The style is **textured, lightly stylised realism**: real materials in detailed textures, a little shaded, neither photoreal nor cartoon. It replaced direction D's flat colour the same day, after the developers saw a log cabin both ways ([research/realistic-building-textures.md](../research/realistic-building-textures.md)). Like the rest of pre-production it can change. How the renderer draws it is [below](#how-the-renderer-makes-it). The explorations that led here are research, not guidance: see [research/art-explorations.md](../research/art-explorations.md).

## The look in one line

Real materials, wood that looks like wood and stone like stone, in detailed textures that are a little shaded and a little simplified, with strong silhouettes and true proportions: as a skilled game artist would make a real place, not a photograph and not a cartoon.

## What every realm shares, and what each brings

The rules on this page never change between realms. A realm (or a maker, or a people, D182) changes what it is made of: its materials, models and motifs.

| Constant: this page | From the theme: each realm's data |
|---|---|
| Shape language: big clear forms, strong silhouette | Motifs and parts: timber and thatch, panels and cables, chitin and crystal |
| Realistic materials, a little shaded and simplified | Its materials: the woods, stones, metals and cloths it is made of |
| Detail scaled to the world, life size | The light: sun colour and height, shadow tint, haze, night |
| No outlines, no cartoon colour | What glows, and in what colour |

A model carries its own materials. A realm with other materials gets other models, or the same shapes textured again, rather than a palette swap.

The starting realm reads more fantasy than medieval, given the lore ([vision.md](vision.md)), so its things need not be historically true: hand-made, with a touch of carved ornament and organic curves.

## Life-sized

The world is at real-world scale (D110). A broadleaf tree stands 15 to 25 metres, a pine taller; boulders range from knee-high to bigger than a house; hills rise tens of metres and mountains hundreds. The player should feel small against the land, which is also what makes haze and layered distance work. Simple shapes do not mean small things: a model is simplified, never shrunk ([Shape](#shape), rule 3).

Trees should be mighty, towering over the player (the developers, 2026-10-12). So a tree is drawn as an old-growth specimen at the top of its species' real range, with the massive trunk and full crown that go with it: an ancient oak 35 m across on a trunk 3 m thick, a redwood 100 m tall. That is still life size; it is the biggest real trees, not bigger than real.

## Shape

1. **Silhouette first.** Filled in solid black, a thing must still say what it is. Make the outline distinct and the proportions clear, and give it one or two features that tell it apart from its neighbours.
2. **Big, medium, small.** A thing is a few big shapes, each broken by a few medium ones. Small detail is rare and goes only where it tells what the thing is: a vice's handle, a hinge, an eye. Aim for about 60% of the area in big shapes, 30% in medium, 10% in small.
3. **Simplify, don't shrink.** The world is life size (D110), so a tree stands 20 metres tall and a boulder can outgrow a house. Detail scales with the world, not the object. A big thing gets more medium shapes, not bigger ones: a 20 m tree is many car-sized leaf clumps, not five house-sized ones.
4. **Straight where made, curved where grown.** Built things have clean planes and straight runs, with only a little wear. Living things are bold, sweeping curves: lobes, clumps and tapers, never noise.
5. **True proportions, simplified.** Things are as tall, wide and thick as real ones. Simplify the forms, but don't caricature them. No oversized heads, hands or tools.
6. **Not chunky.** No cubes standing in for shapes, no visible grid and no blocky stepping, even though structures sit on the voxel grid (D181). A shape that reads as "blocks" is wrong.

## Colour and materials

1. **Real materials.** Each part is what it is made of, and looks it: weathered pine, split shakes, fieldstone, wrought iron. Its texture carries the material's colour, grain and wear.
2. **A little shaded, a little simplified.** Textures are detailed but not photographic: gentle painted shading in the hollows and on the edges, grain and cracks clear but not noisy, colour a touch richer and cleaner than a photo. Never cartoon colour or flat fills.
3. **Value before hue.** Parts that touch differ in lightness, not only in hue, so the form reads in greyscale: a pale cut log end against its darker side, a dark roof over lighter walls.
4. **One kit, one grade.** Every piece of a kit, a building's logs, roof, boards and stones, comes from one concept and shares its colour grade, so they belong together.
5. **Glow is still sparing.** Fire, runes, crystals and screens glow, as an accent.

## Light and surface

1. **Textures carry the surface.** A model has a colour texture, a normal map for its relief and a roughness map, as Tripo makes them. Fine detail is in the normal map, not in triangles, as games like The Forest do it.
2. **The light does the rest.** The renderer's sun, sky and shadows shade the form; the textures' own shading is gentle, so they hold up in any light.
3. **No outlines.**
4. **Edges: hard at creases, smooth across curves.** Normals are smooth across a gentle curve and split where faces meet at more than about 50 degrees. Never flat-shade every triangle.
5. **No shimmer.** Textures have mipmaps and are filtered, so grain does not sparkle at a distance. Textures are 2K unless a piece is seen very close.

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
| A building kit's log or beam, generated (D200), a metre of it | up to 700 |
| A building kit's roof, generated (D209), a square metre of it, covering, battens, boards and caps | up to 500 |

## Made to vary

Most things come in many variants (D182), so make them for it:

- **In parts, where variants are planned.** Make a frame, a top and a trim as separate pieces, so a theme can swap one piece and keep the rest.
- **Neutral, not themed, where it can be.** A crate's shape should not depend on one realm's lore. The theme comes from the parts it adds and its materials.
- **Several of a piece placed many times.** A building kit's logs and stones come as three or four models each, turned and flipped where they are placed, so no two neighbours match.

## The concept prompt

Most models start as a concept image ([props](../tools/props/README.md), [characters](../.agents/skills/character/SKILL.md)). End the subject's description with this paragraph, word for word, so that every concept is drawn alike. `tools/props/props.ts` reads it from here, so keep it as the one quoted line under this heading:

> Stylised realistic 3D game asset render: real materials in detailed textures with natural grain, wear and colour, gently shaded and slightly simplified, not photorealistic and not cartoon; strong, clear silhouette; true real-world proportions. Single subject, whole subject in frame and centred, lit evenly and softly from the upper left, on a plain light grey background, no ground, no text.

Keep the subject's description to what the thing is, what it is made of and its setting's motifs. Say nothing about style there. A concept for a scene or a mood picture, rather than a model, swaps the second sentence for the realm's light: "a few crisp shadow shapes; warm low sunlight and atmospheric depth", or whatever that realm's light is.

Even light matters for a model: Tripo textures the model from the concept, so shadows in the concept would be painted into its texture.

## Checklist

Check every model in the game's own light, beside a figure for scale (the workbench's `--prop`, `--model`, or `props.ts` sheets):

- [ ] **Silhouette:** reads as what it is when filled in black, and at icon size.
- [ ] **Size:** life size; within its catalogue height (D147, D110).
- [ ] **Shapes:** big, medium and small, mostly big; nothing chunky.
- [ ] **Material:** reads as what it is made of, close and at a distance, without shimmer.
- [ ] **Shading:** a little shaded, not photographic, not cartoon.
- [ ] **One grade:** matches the rest of its kit or setting.
- [ ] **Values:** touching parts differ in lightness.
- [ ] **Normals:** split at creases, smooth across curves.
- [ ] **Budget:** inside its range above.
- [ ] **Contracts:** faces +Z, level, foot at the origin, and parts as [docs/characters.md](characters.md#the-contracts) says.

## What it is not

- **Not chunky or Minecraft-like.** That rule stands from the start (D14).
- **Not photoreal.** Detail serves the shape; textures are a little shaded and simplified.
- **Not cartoon.** No flat fills, no ink outlines and no exaggerated proportions.
- **Not faceted.** Never flat-shade every triangle.

## Where the pipeline stands

Not everything we have made yet follows this guide. Each gap is work to do, not an exception to the rules:

- **Most props are flat colour by role,** from direction D and before. They will be made again, textured, as each comes up.
- **Item icons are painted in direction C** (D130), and will be repainted to match.

## How the renderer makes it

Models carry their own materials; the light, the sky and the air are shared shader code and each realm's data.

| Ingredient | Where it comes from | State |
|---|---|---|
| Models | Concept, Tripo, clean-up keeping its textures (`tools/props`) | Working for doors and windows; most props still flat colour |
| Long members | Logs and beams generated from their kit's numbers and each one's seed, a real bark or wood material displacing and texturing them (`terra_build::timber`, D200) | Working: round logs (`logs.ron`) and hewn timber (`hewn.ron`) |
| Block kinds | Stone and the like drawn as their cubes' surface, rounded and displaced by a real material (`terra_build::surface`, D197) | Working: fieldstone |
| Tileable materials | CC0 libraries first, generated where they lack one (`tools/textures`, D191) | Working for building kits and the ground |
| Crease-aware normals | Normals smooth across curves, split at creases over about 50 degrees | Working: the clean-up shades them so |
| No shimmer | Mipmaps and anisotropic filtering for every model's and material's textures | Working |
| Ground | Soil, grass, rock and sand textures, triplanar, blended by what grows and tinted by the planet (D190) | Working on the voxel terrain |
| Sky light | The shade lit by the sky above and the ground's bounce below, from a cube map painted from the sky (D190) | Working |
| Haze and layered distance | Bevy's physical atmosphere's aerial perspective, its distances scaled 18 times so a few hundred metres read as many kilometres (D212) | Working |
| Sky | Bevy's physical atmosphere (Hillaire's model, as Unreal's), with our clouds, stars and moon drawn over it, and the dusk and night in chosen colours, since it goes dark at sunset (D212) | Working |
| Exposure, bloom and grading | A fixed exposure for sunlit ground, opening a little through the dusk; AgX tone mapping, a film's response; soft bloom (D212) | Working |
| Dense foliage and wind | Instanced plants, merged grass blades, one wind shader | Working near the start; its models are still direction D |
| Ground cover | Grass as clumped blades round the camera, fading into ground of its own colour; leaf litter and moss painted onto the ground under trees (D202) | Working in the lush forest; litter is still the soil texture tinted |
| Water | A see-through surface mirroring the sky at a low angle | Working; no waves |
| Shade in corners | Screen-space ambient occlusion over about a metre (D212), and contact shadows where things meet (D204) | Working |
| Trees, bushes and rocks | Their parts drawn in real materials, bark, leaves and lichen-covered rock, projected triplanar and tinted by their kind, normals smoothed across curves (D212) | Working on the free models, which are still crude in shape; see below |

- **Effort goes into light and atmosphere**, which is shader code and shared by everything.
- **Fullness comes from quantity.** Ground cover, undergrowth and canopy in layers, as in Sons of the Forest (D148).

## Open questions

- **Trees, plants and rocks** are drawn in real materials now, but their shapes are still direction D's: pines as stacked cones, crowns as faceted blobs, rocks of 70 to 130 triangles, small plants in flat colour. They are to be made again to this guide. Trees could be grown in code instead, with real bark and leaf cards: the lush forest spike runs 2,700 of them at about 5 ms a frame ([research/lush-forest-spike.md](../research/lush-forest-spike.md)). Which way is open.
- **How materials change through seasons and phases** (D149).
- **Night.** Moonlight and lamps on textured surfaces.
- **Blur and framing**, which matter more in third person.
