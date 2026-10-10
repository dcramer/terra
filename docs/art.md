# Art direction

Status: Leaning (D14). The developers named [Forever Ago](https://store.steampowered.com/app/1215940/Forever_Ago/) as the visual inspiration on 2026-10-10, with one firm constraint: it must not look chunky or Minecraft-like. Like everything in pre-production, this is current thinking.

## The essence

Simple shapes, flat colour, and light doing nearly all the work. The world looks like a carefully lit model: clean enough to read at a glance, soft enough to feel warm.

1. **Simple shapes, finely cut.** Things are built from flat planes you can see, but many small ones, with graceful outlines. A boulder is dozens of facets, not six.
2. **Flat colour, no texture.** A surface is one colour. Detail comes from its shape and from how light falls across it, never from a painted-on pattern.
3. **Warm light, cool shadow.** A warm sun against shadows lit by a cool sky. This contrast is most of the mood.
4. **Distance becomes layers.** Haze fades far things toward the colour of the sky, so hills turn into soft pastel bands instead of sharp detail.
5. **A small palette per place.** A handful of colours that belong together, saturated but soft. A place is recognisable by its colours alone.
6. **Small, dense life.** Where things grow, they grow as many small flat-coloured pieces: ferns, grass, flowers. Lushness comes from quantity, not detail.
7. **A lens.** A little bloom, gentle colour grading, and sometimes blur in the distance. It should feel photographed, not rendered.

## What it is not

- **Not chunky.** No cubes, no visible grid, no oversized facets on small things. If a shape reads as "blocks", it is wrong.
- **Not photoreal.** No surface textures, no noise, no grit.
- **Not outlined.** No cartoon ink lines; edges come from light.
- **Not pixelated.**

## The reference: Forever Ago

Four of its store screenshots, and what to look for in each:

| Screenshot | What it shows |
|---|---|
| [Van in the desert](https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1215940/d3b6a8ba3fa8163acca5cb980fd304c1a7a5cf1e/ss_d3b6a8ba3fa8163acca5cb980fd304c1a7a5cf1e.1920x1080.jpg) | Distance as layers: salmon ground, then lilac and blue mountain bands, then a soft sky. A palette of about five colours. |
| [Forest stream](https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1215940/aff7bf62c47284177232e7b0a381b3f162ee2b12/ss_aff7bf62c47284177232e7b0a381b3f162ee2b12.1920x1080.jpg) | Faceted rocks beside dense small foliage. Warm light shafts, cool shadow, blur front and back. |
| [House and dog](https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1215940/f252144f5d2c4d22e08fa7e889961246e8ddbd92/ss_f252144f5d2c4d22e08fa7e889961246e8ddbd92.1920x1080.jpg) | Flat-coloured grass, ferns and flowers in quantity. Low sun and long shadows. |
| [Shop interior](https://shared.akamai.steamstatic.com/store_item_assets/steam/apps/1215940/215e8d117a2fafa80d78a3d3c3996223e68f47df/ss_215e8d117a2fafa80d78a3d3c3996223e68f47df.1920x1080.jpg) | Built things in the same language: flat colour, simple forms, coloured light. Useful for bases and machines. |

These are linked, not copied into the repository, because they are someone else's copyrighted images.

## Other references

Games with a related look, and the one thing worth taking from each:

| Game | Take from it |
|---|---|
| [Firewatch](https://store.steampowered.com/app/383870/) | Landscapes as flat colour layers; a palette that changes with the time of day |
| [art of rally](https://store.steampowered.com/app/550320/) | Low-polygon scenery made elegant by restraint and by lens blur |
| [Alba: A Wildlife Adventure](https://store.steampowered.com/app/1337010/) | A soft, friendly low-polygon world full of plants and animals |
| [The Witness](https://store.steampowered.com/app/210970/) | Crisp flat colour with no texture, and strong separation between areas by palette |
| [Road 96](https://store.steampowered.com/app/1466640/) | Stylised people and vehicles in the same simple-shape language |
| [Jusant](https://store.steampowered.com/app/1977170/) | Soft stylised rock and a climb through changing biomes |
| [No Man's Sky](https://store.steampowered.com/app/275850/) | Not the look, but the method: a palette generated per planet, and variety from rules |

## Where we are

First tests of the look, made in the sandbox on 2026-10-10. Run them with `cargo run -p terra_sandbox -- --experiment look --experiment forest`.

![Look test: a faceted desert with boulders, shrubs, haze and a soft sky](images/look-test-2026-10-10.png)

![Forest test: low-polygon pines over the same desert](images/forest-test-2026-10-10.png)

The second image adds the `forest` experiment: generated pines, each built from a few flat-faced cones.

What they show: flat-coloured faceted ground, generated boulders and shrubs with flat faces, a warm sun with cool shadows, haze toward the horizon, a gradient sky with a soft cloud, and bloom.

What it does not have, and the reference does:

- **Authored shapes.** Every model in the reference was designed by an artist. Ours are lumpy spheres.
- **Foliage.** One kind of tree, and no grass, ferns or flowers. This is the biggest gap wherever the planet is alive.
- **Composition.** The reference frames each scene. We have a free camera on a generated world.
- **Layered distance.** Our planet is small, so the horizon is a few hundred metres away and there are no far mountains to turn into bands.
- **Water, interiors, people.**

## How the look is made

| Ingredient | Where it comes from | State |
|---|---|---|
| Faceted, flat-shaded surfaces | A shader lights each triangle as one plane | Working on the ground and on generated shapes |
| Warm sun, cool shadow | Light colours and a sky-coloured fill light | Working |
| Haze and layered distance | Distance fog tinted toward the horizon | Working |
| Sky | A gradient shader with soft clouds | Working; no day and night |
| Bloom and grading | Camera effects built into Bevy | Working |
| Blur in the distance | A camera effect built into Bevy | Not tried; needs care in first person |
| Elegant shapes | Bought low-polygon packs, and generators for rocks and plants | Started: generated boulders, shrubs and one pine |
| Dense foliage | Instancing many small pieces | Not started; the load test says a discrete GPU can draw hundreds of thousands |
| A palette per place | Generated from the simulation's values | Not started |

## Why this direction suits us

- **It is the most available style.** Flat-coloured low-polygon packs are plentiful and consistent with each other, which matters with no artists (D1).
- **It is the easiest style to generate.** A believable low-polygon rock or fern is far simpler to produce from rules than a realistic one.
- **It hides what we are weak at.** No textures means no mismatched textures.
- **Voxel terrain can wear it.** Smooth voxel meshes lit one triangle at a time give faceted ground for free. The risk is facet size: coarse terrain reads as chunky, so close-up terrain has to be fine.

## Principles that still hold

- **The art is the simulation rendered.** Sky colour from air pressure, ground tint from moisture and life, plant density from the simulation.
- **One material pipeline for everything.** Every asset goes through the same palette and shader, so the game reads as one thing.
- **Effort goes into light and atmosphere**, which is shader code.

## Open questions

- **What does a dead planet look like in this style?** The references are lush or warm. Ours starts airless and lifeless, and has to be beautiful then too.
- **How does the look change through the phases?** A palette per biome and phase is the idea; nobody has drawn one.
- **Night, and the view from orbit.**
- **First person or third?** Blur and framing matter much more to this look in third person. First person removes a large animation cost.
- **Machines and bases.** The shop interior hints at an answer; nothing is designed.
