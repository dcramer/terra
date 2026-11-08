# Ambient life

Status: **Proposal, 2026-11-08** (D647, the developers'): for the developers to read before more of it is built (D49). What is built is marked.

**The aim**: a world that feels as alive as Crimson Desert's: at any place, by day, dozens of small living things in view at once, in the air, on the ground and in the water; never overwhelming and never rare. They are for the eye and ear: nothing is gained from them, none is fought, none crosses the network.

## Rules

- **The art is uniform** (D14, D647): whatever is a bird's size or more is a model in the game's style, made by the creature and model pipelines. Only what is a fly's size is a speck or card drawn by a shader (`terra_world::drift`).
- **By place, season, warmth and hour**, as data (D136): a kind is about where it would be.
- **As many at any quality preset**: how much life is about is the world's to say, not the machine's. What a preset changes is how far off a kind is drawn.
- **Seen is measured**: `wildlife.state` counts the creatures in the camera's picture at 6 pixels tall or more. At the meadow start, standing: 5 or 6 creatures, **no bird** of 57 near. A finch at 1.6 times life size is 6 pixels tall only within 15 m, and takes flight at 7 m; a pigeon within 19 m. Dozens of birds seen needs birds that are larger (crow, pheasant, gull: models not made) or nearer in flocks (a flush staged ahead of a walking player, as the meetings stage animals, D384), not more small birds.
- **Counted, then looked at**: `wildlife.state` and `drift.state` say what is about (`tools/sandbox/ambience.ts`); a still or film of each place shows whether it is seen.

## What there is (measured at the first realm's meadow start, a July day)

| Kind | Made as | About | Seen in a view |
|---|---|---|---|
| Butterflies, white and brown | Shader card, built | 290 round the eye | About 15 |
| Bees, midges, dust | Shader speck, built | Hundreds | Hardly: too small to read past a few metres |
| Dragonflies by water | Shader card (the far bird's shape), built | 24 | A few darts over the lake |
| Fireflies on a warm night | Shader speck with a light, built | 300 | About seven, before their number was raised |
| Chaffinches | Model, built; 24 a hectare in woods and open ground, a quarter of them on the wing at a time | 56 within 110 m | **None**: those ahead are 30 to 50 m off, 3 to 5 pixels each at 12 cm; the near ones are in the grass |
| Sparrows, pigeons, mallards, herons, ptarmigan | Models, built | By their haunts | Not looked at |
| Wood pigeons | Model (the pigeon's), built; 5 a hectare in woods and open ground | 13 within 150 m | One on the ground by the player in six frames |
| Birds far overhead | Shader cards (D379), kept as the far form until a model stands in for them | A few flocks | A few, small. The swallows and pipits, cards flying near, are taken out: a bird is a model |
| Fish | Nothing | None | None |

## What to build, in order

1. **Birds seen.** Built so far: small birds fly a quarter of the time (a tenth before) and 8 to 30 m at a flight, and `wildlife.state` counts them and those on the wing. It is not enough: a 12 cm bird is not seen past some 15 m, and one feeding on the ground of a meadow is under the grass, which stands higher than it: stood 9 m from a finch and 15 m from a wood pigeon and aimed at each, neither is seen. So numbers alone do not show: what shows is a bird in the air, or on a perch clear of the grass (a fence, a ridge, a rock, a bare branch, short turf). A small bird is seen when it flies near. Flocks that cross the view between perches every few seconds, rise from the grass as a player comes near and settle again further off; perches chosen in sight (fence, roof ridge, bare branch, rock) over ones hidden in a crown. The models and the fly, hop and peck clips exist; the minds have roost, perch and fly. Needs: the flock's rule, and a measure of the cost of 100 birds (120 is the most allowed now, apart from the 48 other animals).
2. **Far birds as models.** The swallows, pipits and far birds drawn as cards become the same models drawn small, or go. A low-detail model of each is needed; no swallow, crow or buzzard model exists.
3. **Fish.** Models, in schools in the shallows and singly in deep water, seen from the bank and when swimming; moved by a bend along the body in the shader, as shipped games move schools, with no skeleton. Needs: two or three fish models (a small schooling fish, a trout, a pike), each about 50 Tripo credits, and the school's rule. The mallards and herons are the water's birds already.
4. **Small ground life**: frogs at the water's edge, lizards on warm rock, mice: models, not yet made.
5. **Their sound**: birdsong by hour and place, insects' hum, a splash: with the ambient beds of [audio.md](audio.md).
6. **Each realm its own**: the industrial realm's small life is not chosen (D642).

## Taken as working assumptions (the agent's, 2026-11-08; the developers overrule)

- A small bird is drawn 1.6 times life size (finch, sparrow), as a butterfly is drawn at 10 cm: at life size neither is seen past a few metres.
- Half of a bird's flights go past a player who is about; larger kinds (the wood pigeon) carry the middle distance.
- The far birds overhead stay cards until a model stands in for them; no card bird flies near.
- Fish, when made: a small schooling fish, a trout and a pike, for the eye only in v0. Not made: each model is some 50 credits, the developers' to spend.

## Open, for the developers

- Whether those assumptions stand.
- Whether a butterfly, the largest thing drawn by a shader, may stay a card or should be a model too.
- Which fish, and whether they may be caught: this proposal has them for the eye only.
