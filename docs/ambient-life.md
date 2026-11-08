# Ambient life

Status: **Proposal, 2026-11-08** (D647, the developers'): for the developers to read before more of it is built (D49). What is built is marked.

**The aim**: a world that feels as alive as Crimson Desert's: at any place, by day, dozens of small living things in view at once, in the air, on the ground and in the water; never overwhelming and never rare. They are for the eye and ear: nothing is gained from them, none is fought, none crosses the network.

## Rules

- **The art is uniform** (D14, D647): whatever is a bird's size or more is a model in the game's style, made by the creature and model pipelines. Only what is a fly's size is a speck or card drawn by a shader (`terra_world::drift`).
- **By place, season, warmth and hour**, as data (D136): a kind is about where it would be.
- **As many at any quality preset**: how much life is about is the world's to say, not the machine's. What a preset changes is how far off a kind is drawn.
- **Counted, then looked at**: `wildlife.state` and `drift.state` say what is about (`tools/sandbox/ambience.ts`); a still or film of each place shows whether it is seen.

## What there is (measured at the first realm's meadow start, a July day)

| Kind | Made as | About | Seen in a view |
|---|---|---|---|
| Butterflies, white and brown | Shader card, built | 290 round the eye | About 15 |
| Bees, midges, dust | Shader speck, built | Hundreds | Hardly: too small to read past a few metres |
| Dragonflies by water | Shader card (the far bird's shape), built | 24 | A few darts over the lake |
| Fireflies on a warm night | Shader speck with a light, built | 300 | About seven, before their number was raised |
| Chaffinches | Model, built; 12 a hectare in woods and open ground | 26 within 55 m | **None**: on the ground in the grass or in the crowns |
| Sparrows, pigeons, mallards, herons, ptarmigan | Models, built | By their haunts | Not looked at |
| Swallows, pipits, birds far overhead | Shader cards (D379) | A few flocks | Nearly none: a few pixels each. **In question**: a bird is a model |
| Fish | Nothing | None | None |

## What to build, in order

1. **Birds seen.** A small bird is seen when it flies. Flocks that cross the view between perches every few seconds, rise from the grass as a player comes near and settle again further off; perches chosen in sight (fence, roof ridge, bare branch, rock) over ones hidden in a crown. The models and the fly, hop and peck clips exist; the minds have roost, perch and fly. Needs: the flock's rule, and a measure of the cost of 100 birds (120 is the most allowed now, apart from the 48 other animals).
2. **Far birds as models.** The swallows, pipits and far birds drawn as cards become the same models drawn small, or go. A low-detail model of each is needed; no swallow, crow or buzzard model exists.
3. **Fish.** Models, in schools in the shallows and singly in deep water, seen from the bank and when swimming; moved by a bend along the body in the shader, as shipped games move schools, with no skeleton. Needs: two or three fish models (a small schooling fish, a trout, a pike), each about 50 Tripo credits, and the school's rule. The mallards and herons are the water's birds already.
4. **Small ground life**: frogs at the water's edge, lizards on warm rock, mice: models, not yet made.
5. **Their sound**: birdsong by hour and place, insects' hum, a splash: with the ambient beds of [audio.md](audio.md).
6. **Each realm its own**: the industrial realm's small life is not chosen (D642).

## Open, for the developers

- Whether a butterfly, the largest thing drawn by a shader, may stay a card or should be a model too.
- Which fish, and whether they may be caught: this proposal has them for the eye only.
- Whether the far cards may stay until their models exist, or go now.
