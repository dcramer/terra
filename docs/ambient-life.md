# Ambient life

Status: **Proposal, 2026-11-08** (D647, the developers'): for the developers to read before more of it is built (D49). What is built is marked.

**The aim**: a world that feels as alive as Crimson Desert's: at any place, by day, dozens of small living things in view at once, in the air, on the ground and in the water; never overwhelming and never rare. They are for the eye and ear: nothing is gained from them, none is fought, none crosses the network.

## Rules

- **The art is uniform** (D14, D647): whatever is a bird's size or more is a model in the game's style, made by the creature and model pipelines. Only what is a fly's size is a speck or card drawn by a shader (`terra_world::drift`).
- **A thing in the world, or a speck** (D655): what a player could care about (butterflies, moths, dragonflies, fireflies, bees if caught) is a small model the CPU places and moves, as the fish; dust, pollen, midges, down, rain and snow are the drift. Not built: all of the first list are the drift still.
- **By place, season, warmth and hour**, as data (D136): a kind is about where it would be.
- **As many at any quality preset**: how much life is about is the world's to say, not the machine's. What a preset changes is how far off a kind is drawn.
- **Seen is measured**: `wildlife.state` counts the creatures in the camera's picture at 6 pixels tall or more. At the meadow start, standing: 5 or 6 creatures, **no bird** of 57 near. A finch at 1.6 times life size is 6 pixels tall only within 15 m, and takes flight at 7 m; a pigeon within 19 m. Dozens of birds seen needs birds that are larger (crow, pheasant, gull: models not made) or nearer in flocks (a flush staged ahead of a walking player, as the meetings stage animals, D384), not more small birds.
- **Over a walk** (45 s forward from the meadow start, a look a second): 1.5 creatures in the picture on average and a bird in 2 seconds of 45, before; 2.3 and 3 seconds with small meetings nearer (11 to 17 m ahead) and oftener (every 2 to 4 s), twice as many met at once, and four bird meetings in five a flock crossing. Far short of dozens: the meadow's tall grass leaves the meetings few spots where a small animal shows, and a flying bird is counted by its body, not its wings.
- **Birds rise together** (built): when one takes wing, those of its kind within 8 m go with it. Over the same walk a bird is in the picture in 14 and 24 seconds of 45 (5 before), up to 8 at once, a flying bird counted by its wings. Standing still, none yet. **The count overstates**: a picture taken at a moment it said six birds were in view shows a stag by a boulder, butterflies, and no bird plainly: it does not ask what stands between (trunks, crowns), and six pixels of a dark bird against dark spruce is not seen. The count is a floor to tune by, not proof; the proof is a picture.
- **Counted, then looked at**: `wildlife.state` and `drift.state` say what is about (`tools/sandbox/ambience.ts`); a still or film of each place shows whether it is seen.

## What there is (measured at the first realm's meadow start, a July day)

| Kind | Made as | About | Seen in a view |
|---|---|---|---|
| Butterflies, white and brown | Shader, two hinged wings heading the way each flies (a stopgap: to be things in the world, D655) | 64 round the eye, life size (7 cm): whites in threes close together, browns in fives spread out, for the developers to choose between | None in the one still taken; 290 at 10 cm were about 15, "far too numerous" |
| Bees, midges, dust | Shader speck, built | Hundreds | Hardly: too small to read past a few metres |
| Dragonflies by water | Shader card (the far bird's shape), built | 24 | A few darts over the lake |
| Fireflies on a warm night | Shader speck with a light, built | 300 | About seven, before their number was raised |
| Chaffinches | Model, built; 24 a hectare in woods and open ground, a quarter of them on the wing at a time | 56 within 110 m | **None**: those ahead are 30 to 50 m off, 3 to 5 pixels each at 12 cm; the near ones are in the grass |
| Sparrows, pigeons, mallards, herons, ptarmigan | Models, built | By their haunts | Not looked at |
| Wood pigeons | Model (the pigeon's), built; 5 a hectare in woods and open ground | 13 within 150 m | One on the ground by the player in six frames |
| Birds far overhead | Shader cards (D379), kept as the far form until a model stands in for them | A few flocks | A few, small. The swallows and pipits, cards flying near, are taken out: a bird is a model |
| Fish | Trout, pike and minnows in schools where the water suits each, near whoever looks; they swim a path of their own and dart from a swimmer | Models (`art/props/fish_*`), swum by a bend in the shader | `assets/data/fish.ron`, `terra_creatures::fish` |

## What to build, in order

1. **Birds seen.** Built so far: small birds fly a quarter of the time (a tenth before) and 8 to 30 m at a flight, and `wildlife.state` counts them and those on the wing. It is not enough: a 12 cm bird is not seen past some 15 m, and one feeding on the ground of a meadow is under the grass, which stands higher than it: stood 9 m from a finch and 15 m from a wood pigeon and aimed at each, neither is seen. So numbers alone do not show: what shows is a bird in the air, or on a perch clear of the grass (a fence, a ridge, a rock, a bare branch, short turf). A small bird is seen when it flies near. Flocks that cross the view between perches every few seconds, rise from the grass as a player comes near and settle again further off; perches chosen in sight (fence, roof ridge, bare branch, rock) over ones hidden in a crown. The models and the fly, hop and peck clips exist; the minds have roost, perch and fly. Needs: the flock's rule, and a measure of the cost of 100 birds (120 is the most allowed now, apart from the 48 other animals).
2. **Far birds as models.** The swallows, pipits and far birds drawn as cards become the same models drawn small, or go. A low-detail model of each is needed; no swallow, crow or buzzard model exists.
3. **Fish.** Built for the eye (2026-11-08): each kind a row of `assets/data/fish.ron` (its model, how long, how many swim together, how deep its water, how far under the surface, schools to a hectare, from how far it is drawn, its speed and its tail's beat) put by rule in cells of water round the eye (`terra_creatures::fish`), a school one entity and its fish instances of one mesh, swum by a wave down the body in the vertex shader (`assets/shaders/fish.wesl`), with no skeleton, as shipped games move schools; at most 400 at once. Seen from above and when swimming: trout in a line under the surface, bending as they go, gone in a dart when the swimmer nears. A shoal of minnows a few metres off the bank, seen from it. Not yet: nothing rises or jumps, the sea holds the same fish as a lake, and none may be caught. The mallards and herons are the water's birds already.
4. **Small ground life**: frogs at the water's edge, lizards on warm rock, mice: models, not yet made.
5. **Their sound**: birdsong by hour and place, insects' hum, a splash: with the ambient beds of [audio.md](audio.md).
6. **Each realm its own**: the industrial realm's small life is not chosen (D642).

## Butterflies as things in the world (D655; the developers, 2026-11-09: "you cant use a model generator here or what", "i want like.. diff breeds of butterflies")

- **Breeds**, each a model of its own (`art/props/butterfly_<breed>`, as the fish are props): the first realm's are the small white, the brimstone (lemon yellow), the small tortoiseshell (orange, barred black), the peacock (dark red, four eyespots) and the common blue (small, sky blue). Told apart by colour, pattern and size (D182). Their real spans, 3 to 7 cm, are to be looked up, not taken from here [M]. The far realm keeps its drift moths until its theme is chosen (D642).
- **The model**: wings spread flat as a set specimen's, the body along Z (head +Z), the span along X, a few hundred triangles, its own textures (D304). The shader raises and lowers each side about the body's axis, in bouts with a glide between; no skeleton, as the fish's bend.
- **In the world**: a kind is a row of `assets/data/butterflies.ron` (model, span, how many fly together, how far apart, to a hectare, where: open, woods, water; season, air and hours as the drift's rows had), placed by rule in cells round whoever looks, from the seed, so two players find a group in the same place (`terra_creatures`, beside `fish`). They fly in small groups with stretches between that have none: threes close together or fives spread out, the developers' to choose. For the eye in v0: each game's own, nothing crosses the network.
- **Not chosen**: whether they may be caught (D655), whether they settle on flowers and rise as a player nears.
- When they fly, the drift's two butterfly rows for the first realm go.

## Taken as working assumptions (the agent's, 2026-11-08; the developers overrule)

- A small bird is drawn 1.6 times life size (finch, sparrow), as a butterfly is drawn at 10 cm: at life size neither is seen past a few metres.
- Half of a bird's flights go past a player who is about; larger kinds (the wood pigeon) carry the middle distance.
- The far birds overhead stay cards until a model stands in for them; no card bird flies near.
- Fish: a minnow, a brown trout and a northern pike, for the eye only in v0 (made, 162 credits).

## Open, for the developers

- Whether those assumptions stand.
- Which fish, and whether they may be caught: this proposal has them for the eye only.
