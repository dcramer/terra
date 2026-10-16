# Art style guide

How to make anything the game draws: characters, creatures, props, machines, building pieces, plants, rocks, items and their icons, and how a place looks. It holds for every realm, whatever its setting: medieval, cyber, tech, alien or something nobody has thought of yet. Read it before making or judging a model or a place; check a model against [the checklist](#checklist) and a place against [its rules](#places) (`cargo xtask looks`).

Status: Decided (D14), 2026-10-12. The style is **textured, lightly stylised realism**: real materials in detailed textures, a little shaded, neither photoreal nor cartoon. It replaced direction D's flat colour the same day, after the developers saw a log cabin both ways ([research/realistic-building-textures.md](../research/realistic-building-textures.md)). Its environment half, [how a place looks](#places), is D356, written 2026-10-15 from the first four places' reviews. Like the rest of pre-production it can change. How the renderer draws it is [below](#how-the-renderer-makes-it). The explorations that led here are research, not guidance: see [research/art-explorations.md](../research/art-explorations.md).

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
2. **A little shaded, a little simplified.** Textures are detailed but not photographic: gentle painted shading in the hollows and on the edges, grain and cracks clear but not noisy, colour a touch richer and cleaner than a photo. Never cartoon colour or flat fills. **Leaves are the exception** (D291): drawn true to life, each leaf with its own veins, relief, tilt and sheen, in real leaves' varied greens, as Sons of the Forest's are; measured card by card and in the picture as [Places](#places)' rule 12 says.
3. **Value before hue.** Parts that touch differ in lightness, not only in hue, so the form reads in greyscale: a pale cut log end against its darker side, a dark roof over lighter walls.
4. **One kit, one grade.** Every piece of a kit, a building's logs, roof, boards and stones, comes from one concept and shares its colour grade, so they belong together.
5. **Glow is still sparing.** Fire, runes, crystals and screens glow, as an accent.

## Light and surface

1. **Textures carry the surface.** A model has a colour texture, a normal map for its relief and a roughness map, as Tripo makes them. Fine detail is in the normal map, not in triangles, as games like The Forest do it.
2. **The light does the rest.** The renderer's sun, sky and shadows shade the form; the textures' own shading is gentle, so they hold up in any light.
3. **No outlines.**
4. **Edges: hard at creases, smooth across curves.** Normals are smooth across a gentle curve and split where faces meet at more than about 50 degrees. Never flat-shade every triangle.
5. **No shimmer.** Textures have mipmaps and are filtered, so grain does not sparkle at a distance. Textures are 2K unless a piece is seen very close.

## Places

How a place looks, the environment half of the style (D356). A place is aimed at a reference: frames of the look wanted (another game's, photographs) measured into numbers ([the environment skill](../.agents/skills/environment/SKILL.md), D311). Its fixed views are held to those numbers, and the rules below hold in every place.

| Place | Aimed at | Reference | Held to, among others (tolerance) | Review, 2026-10-15 |
|---|---|---|---|---|
| The lush forest (D301) | Sons of the Forest's forest by day: dark shade broken by flecks, tall trunks in rows, crowns far overhead | `sotf-forest-day`, 20 frames | brightness 0.148 (±0.05), near black 46% (±15), hard shade edges at most 34%, trunks across the middle 49% (±20) | 6/10 |
| The mountain lake | Far Cry 5's Montana for the look; Morskie Oko's photographs for where trees, scree and snow lie | `mountain-lake-day`, 11 frames | brightness 0.386 (±0.08), sky 0.73 (±0.10), far land under the sky 0.45 (±0.08), far contrast at least 0.13 | 4/10 |
| The tundra | Photographs of tundra in early autumn: red and rust dwarf shrubs, lichen, sedge, pools, snow in the lees | `tundra-autumn-day`, 29 photographs | brightness 0.487 (±0.08), near black 3% (±3), facet edges at most 0.05 widths, debug tint at most 0.5% | 3/10 |
| The market town | Kingdom Come: Deliverance I and II and The Witcher 3's towns by day | `town-market-day`, 8 frames | brightness 0.34 (±0.06), sky 0.69 (±0.12), lit orange ground at most 16%, lilac at most 0.3% | 4/10 |

Every number, its tolerance and its rule is in [tools/sandbox/style.json](../tools/sandbox/style.json); a tolerance is about one standard deviation of the reference's own frames. Values are sRGB, as a person sees them.

**The rules every place keeps.** Each says what breaks it, as the reviews found it, and how it is measured: by **the check**, `cargo xtask looks [--place <name>]`, which shoots the place's views and prints each number against its target (D356), or by **review**, the adversarial review against the reference frames at each milestone. A number only finds a symptom; a review still judges the whole.

1. **Lit and exposed as the reference.** The shade is as deep as the reference's and the sky as bright: not a forest exposed back up to mid-grey, not a noon sky half the photographs' brightness. *Check:* brightness, near black, the sky's brightness.
2. **Coloured as the reference.** Its warmth and blue are the reference's: olive under the trees, no cyan cast, no orange grade. *Check:* red and blue against green.
3. **Shade is soft and takes the sky's colour.** Cool and blue in the open, olive under leaves; never a hard-edged disc, never black, never lilac. A sun's penumbra metres from what casts it is wide. *Check:* the share of shading edges that are hard (brightness halving within four pixels, its colour kept), the shade's red and blue against green.
4. **Plants are communities with height and self-shadow.** Kinds grow mixed in patches, tall over short, each shading itself and what is under it: never flat paint on the ground, never a lawn of one green. *Check:* local contrast, of the near ground where the reference has bands. *Review:* the mix and the height.
5. **Ground layers blend by height and noise.** Where one layer gives way to another (litter, moss, lichen, heath, snow), it follows the ground's height and a noise, never the triangles of the mesh; litter lies dark and umber, not lit orange. *Check:* long straight slanting colour edges in the lower half, in picture widths; lit orange below.
6. **Nothing reads as a debug tint.** No saturated flat fill a real material never is: orange rocks, a lime log, lilac skin. Real saturated things, autumn leaves, flowers, cloth, are textured. *Check:* saturated smooth squares below the skyline; lilac.
7. **Far things keep the near things' light.** Far forest and far slopes are lit by the same sun and sky as near ones, paler and bluer in the air but never darker than the trees they stand for. *Check:* the far land's brightness, and how far under the sky it falls.
8. **The horizon is never empty.** Something stands on it: ridges, far trees, peaks, a roofline, each with its own shape against the sky, never a bare line of haze. *Check:* the far land's local contrast, at least the reference's less its tolerance.
9. **Every surface is textured in the style.** Every prop, the ground and the people are textured as [Colour and materials](#colour-and-materials) asks, never a flat fill or a placeholder colour. *Check:* flat colour, squares below the skyline whose brightness hardly varies. *Review:* the style.
10. **What the eye meets is the reference's.** The forest's view is trunks across its middle and crowns far overhead, the sky in slits. *Check:* trunks across the middle, lime crowns above (the forest only).
11. **Water mirrors what stands round it and takes the colour of what lies under it.** A lake reflects its shore, forest and peaks, dark green over rock, brown over peat, milky turquoise under snow; never a flat sheet of the sky's colour. *Review* (flat colour finds a flat sheet).
12. **Leaves read as leaves** (D291, D368). Each leaf its own shape, veins, relief, tilt and sheen, in varied greens, lit through; never a flat fill of one green lit as one plane, however small the plant. What breaks it: a card drawn from a picture, its normal map flat (the world's pine, holly, birch, fern); a card grown so big its leaves are magnified near (the world's oaks, leaves 40 cm); a pale mint grade; and in the picture, a bush or crown whose leaves differ only in brightness. A leaf lit by the sun and the sky alone keeps its saturation however bright; real ones glint pale where they mirror the sky, glow yellower lit through and go bluer in shade, so their saturation varies. *Check, the cards:* `cargo xtask foliage`, part of `cargo xtask check` ([tools/textures/foliage.ts](../tools/textures/foliage.ts)), measures every card the plants wear from its atlas and normal map and holds it to what the cards that read as leaves (the lush oaks' and the Scots pine's) have: 8 leaves or needles a card or more, a leaf 90 texels long or more, a texel for each pixel it covers at 2 m at 1080p, its normal map tilting at least 30% of it past 8 degrees and its leaves 12 degrees apart, a thickness map, brightness in a leaf spread 0.09 of itself and fine detail 0.015, leaf to leaf 0.08 in brightness and 2.5 degrees in hue, three quarters of its green a living leaf's (hue 50 to 160 degrees, saturation 0.25 to 0.92, luma 0.45 at most). Failures already known are listed with why in [foliage.json](../tools/textures/foliage.json); a new one fails the check, and so does a known one fixed and still listed. *Check, the picture:* flat foliage (the share of 64-pixel squares of leaves whose saturation spreads under 0.06), the leaf colour spread and the leaves catching the light (half as bright again as their square's middle leaf), each against the reference.

`cargo xtask looks` fails on any number out of tolerance, naming the view furthest out; run it after every change to a place, and before the review. Its measures are tested on made-up pictures whose answers are known (`tools/sandbox/look.test.ts`).

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
| A tree, near | 2,000 to 8,000, its leaf cards included (D222) |
| A building kit's log or beam, generated (D200), a metre of it | up to 700 |
| A building kit's roof, generated (D209), a square metre of it, covering, battens, boards and caps | up to 500 |

A tree's leaves are cards, each cut to an octagon round its leaves, six triangles where a square would be two but a third fewer empty pixels shaded, so a full crown near the camera costs more triangles than a solid one would; its farther levels of detail fall to a few hundred (D203, D222).

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

## The model contract

Every model the game places obeys one contract (D303), as a shipped game's asset pipeline holds its meshes to a pivot and socket convention (Unreal's pivots and sockets, glTF's axes), so the game places, fits, swings and holds it from its catalogue entry's size, its anchor and its sockets alone. No model gets its own offset, turn or scale from a number in code or in the catalogue; a model that needs one is made again. The one way a model is drawn at other than its size is fitting, below: stretched along the axes its kind fits along, by what it is fitted to, never by a number of its own.

| | The rule |
|---|---|
| **Real size** | Its catalogue entry states how wide, high and deep it is, in metres (`dims`); its file is that size, within 2% or a centimetre, whichever is more, and is drawn at it, unscaled. |
| **Axes** | glTF's: +Y up, its front facing +Z, metres. |
| **Anchor** | Its origin, by kind: the middle of its base for anything stood on a floor or the ground; for a door's or a shutter's leaf, the foot of its hinge, on the edge it turns on, in the middle of its thickness, the leaf running along +X (so every leaf hinges on its left seen from its front; the second of a pair is drawn mirrored); for something hung on a wall, the middle of its back face; for a frame or a set of shutters, the bottom middle of the opening; for a building kit's piece, its place on the grid in ticks (D208), as `tools/kits` fits it into its box. |
| **Sockets** | The points code attaches to are empties in the file named `socket.<name>`: a leaf's `hinge` (at its origin) and `handle`; a seat's `seat`, where someone sits. Code reads them: a door swings about its hinge socket and a villager sits on the seat socket. |
| **Fit** | A fitted model is stretched only along the axes its kind fits along, from its anchor, so its sockets meet what the layout gives, as Unreal's spline meshes run between their start and end. A door's or shutters' leaf fits its hole across (+X from its hinge) and up: it is made for the first hole its entry fits, at which it is the model's size within the tolerance, and a larger hole stretches it. A building kit's module (a ridge, a batten, a log) fits its line along +X, from its `start` socket to its `end`, in whole copies each stretched the same to meet the line's ends. Nothing else is stretched; across and through it every fitted model keeps its size. |
| **Kit pieces** | Every modelled piece a kit lays states its real size, a module its `dims` and its `start` and `end` sockets, and the kit checker (`cargo xtask kits`) holds it to it. Corner stones (quoins, jamb stones) are not modelled: they are made from each block material's own wall, at the sizes its pattern's numbers give, and checked against them. |

`tools/props` makes a model so from its `prop.json` (`dims`, `anchor`, `hinge`, `sockets`), `cargo xtask contract` checks every model against its entry, and `cargo xtask check` runs the same test. The workbench draws each prop's box (green when it keeps the contract, red when not), its origin as three short lines (X red, Y green, Z blue) and its sockets as yellow stars, and logs what is wrong: `--prop all` shows every one.

## Checklist

Check every model in the game's own light, beside a figure for scale (the workbench's `--prop`, `--model`, or `props.ts` sheets):

- [ ] **Silhouette:** reads as what it is when filled in black, and at icon size.
- [ ] **Size:** life size; its catalogue `dims` (D110).
- [ ] **Shapes:** big, medium and small, mostly big; nothing chunky.
- [ ] **Material:** reads as what it is made of, close and at a distance, without shimmer.
- [ ] **Shading:** a little shaded, not photographic, not cartoon.
- [ ] **One grade:** matches the rest of its kit or setting.
- [ ] **Values:** touching parts differ in lightness.
- [ ] **Normals:** split at creases, smooth across curves.
- [ ] **Budget:** inside its range above.
- [ ] **Contracts:** [the model contract](#the-model-contract) (`cargo xtask contract`), and a character's parts as [docs/characters.md](characters.md#the-contracts) says.

## What it is not

- **Not chunky or Minecraft-like.** That rule stands from the start (D14).
- **Not photoreal.** Detail serves the shape; textures are a little shaded and simplified.
- **Not cartoon.** No flat fills, no ink outlines and no exaggerated proportions.
- **Not faceted.** Never flat-shade every triangle.

## Where the pipeline stands

Not everything we have made yet follows this guide. Each gap is work to do, not an exception to the rules:

- **Tools and the modelled trees are flat colour by role,** from direction D and before: a tool because its held look is coloured by its Wood and Metal parts (`tools/characters/fit_held.py`). Every prop the game places is textured (D304).
- **Textures are not compressed yet.** A prop's maps are JPEG in its file and plain RGBA on the GPU, with mipmaps made when they load; GPU-compressed textures (KTX2, BC7) come with the asset pipeline step D188 names.
- **Item icons are painted in direction C** (D130), and will be repainted to match.

## How the renderer makes it

Models carry their own materials; the light, the sky and the air are shared shader code and each realm's data.

| Ingredient | Where it comes from | State |
|---|---|---|
| Models | Concept, Tripo with its normal, roughness and metal maps, clean-up keeping them at a budget by size: 2K colour and normal maps for a prop 1.5 m or more, 1K below, roughness and metal at half (`tools/props`, D304); drawn in their own textures, a part named `Fire` glowing in its own colours (`play::props`); made to [the model contract](#the-model-contract) (D303) | Working for every prop the game places, doors and shutters among them; tools and trees still flat colour |
| Long members | Logs and beams generated from their kit's numbers and each one's seed, a real bark or wood material displacing and texturing them (`terra_build::timber`, D200), bark in patches over weathered wood and shade baked into the grooves (D262) | Working: round logs (`logs.ron`) and hewn timber (`hewn.ron`) |
| Block kinds | Stone and the like drawn as their cubes' surface, rounded and displaced by a real material (`terra_build::surface`, D197) | Working: fieldstone |
| Tileable materials | CC0 libraries first, generated where they lack one (`tools/textures`, D191) | Working for building kits and the ground |
| Crease-aware normals | Normals smooth across curves, split at creases over about 50 degrees | Working: the clean-up shades them so |
| No shimmer | Mipmaps and anisotropic filtering for every model's and material's textures | Working |
| Ground | Soil, grass, rock and sand textures, triplanar, blended by what grows and tinted by the planet (D190) | Working on the voxel terrain |
| Sky light | The shade lit by the sky above and the ground's bounce below, from a cube map painted from the sky (D190) | Working; buildings take it as a grey card does, held so on the material bench (D386: in shade, an 18% grey wall as blue as the card) |
| Haze and layered distance | Bevy's physical atmosphere's aerial perspective, its distances scaled 18 times so a few hundred metres read as many kilometres (D212) | Working |
| Sky | Bevy's physical atmosphere (Hillaire's model, as Unreal's), with our clouds, stars and moon drawn over it, and the dusk and night in chosen colours, since it goes dark at sunset (D212) | Working |
| Distance past the world | Far trees as billboards merged a square at a time, drawn and lit by the shader; past them, ridges of far land in the sky below the horizon, each paler in the air's own horizon colour; out of the trees, a thinner haze in the horizon's colour (D273) | Working in the lush forest |
| Exposure, bloom and grading | A fixed exposure for sunlit ground, opening a little through the dusk; AgX tone mapping, a film's response; soft bloom (D212) | Working |
| Dense foliage and wind | Plants grown from cards, merged grass blades, one wind shader | Working near the start |
| Ground cover | Grass as clumped blades round the camera, by the light the canopy lets through, in tussocks where it is thin (D268), under the canopy most of the floor, shade grass and sedge (D293), fading into ground of its own colour; leaf litter, needles, moss, and duff with roots painted onto the ground under trees and round their feet, umber, micro-shadowed in the sun (D202, D246, D294), litter heaped against trunks, logs and rocks (D294) | Working in the lush forest; litter, needles and roots are Poly Haven's Forest Leaves 02, Forest Ground 03 and Roots, layers of the ground |
| Water | The sea a see-through surface mirroring the sky; lakes by depth, with shore foam lapping and waves by the wind, each length running at its own speed (`lake.wesl`, ported from an MIT Godot shader, D322), mirroring what stands round them on screen, the air's sky and the skyline past the screen's edge | Working |
| Shade in corners | Screen-space ambient occlusion over about a metre (D212), and contact shadows where things meet (D204) | Working |
| Trees, bushes and small plants | Grown by rules, as SpeedTree makes them: trunks and branches of real bark, leaves as cards of real sprays, broadleaves composed from scanned leaves with a normal map (D269) and conifers drawn needle by needle, every leaf at its real size (D276), a few variants a kind, levels of detail and shadow stand-ins by size (D203, D222) | Working: pine, birch and a broadleaf, two bushes, fern, herb and three flowers; in the lush forest, a tall conifer stand (D301): Scots pines 24 to 34 m, pointed, flat-topped, forked and broken, each its own girth, lean and bark, in knots (D276), their live crowns their top quarter or third, pads of needle tufts on crooked limbs with sky between (D318), Norway spruces as tall in its hollows, and as the exception oaks in three forms and veteran oaks whose trunks are models in the grown bark with grown crowns (D235, D264), leaves lit through from behind and dark seen from below (D247, D264), each leaf at its own tilt and crowns and shrubs dark inside under a bright skin (D269), and under them bracken, ferns, herbs, hazel, bilberry, oak scrub, saplings, dead branches and flowers in drifts (D246), with a middle layer of hazel stools as vases, fans, rods and overstood stems (D276), holly, young spruces, saplings, bramble and snags one to five metres tall (D268) |
| Rocks | Shapes from CC0 scans cut to 350 to 550 triangles, drawn in real rock materials projected triplanar and tinted by their kind (D212, D222); in the lush forest sunk into the ground, moss over what faces up, a scanned moss in cushions (D246, D294) | Working |

- **Effort goes into light and atmosphere**, which is shader code and shared by everything.
- **Fullness comes from quantity.** Ground cover, undergrowth and canopy in layers, as in Sons of the Forest (D148).

## Open questions

- **Leaves as cards or clumps** (D148) are the developers' to choose by eye. Trees, bushes and small plants are grown with cards for now (D222), as shipped games draw them; trees are still the small end of their species' range, below the mighty ones this guide asks for, since how big a tree is decides which axe fells it (D96).
- **How materials change through seasons and phases** (D149).
- **Night.** Moonlight and lamps on textured surfaces.
- **Blur and framing**, which matter more in third person.
