# Creatures: wildlife and monsters

Part of the game's design ([gameplay.md](gameplay.md), adopted as the source of truth on 2026-10-12, D89). Each item is as settled as the decision it cites; the rest is a proposal for the developers to react to.

## What the developers have said

- A living world with monsters and wildlife (D89).
- Factions and settlements, after Kenshi (D95); the world goes on without you.

## Based on

**Kenshi** and **Valheim** for animals that live their own lives and react to you. The lore, after *He Who Fights With Monsters*'s world-building, for monsters that form where magic pools ([lore.md](lore.md#monsters)).

## Wildlife

- **Animals with routines**: graze, drink, rest, flee, hunt, gather in herds, move with the seasons.
- **A food chain**: grazers, the predators that eat them, scavengers. Monsters hunt animals too.
- **Mostly not hostile.** Most flee; some defend themselves or their young; predators hunt what is weak, including a lone player.
- **What they give**: meat, hide, bone, feathers. Hunting is a skill by doing (D88).
- **Some can be tamed**, probably (D100): ridden as mounts, perhaps put to work or kept for what they give.
- **Populations live in regions** and recover when left alone, as the ground heals outside claims (D94). Over-hunting empties a region for a while, not forever.

### Small animals about the land

The developers, 2026-10-15: "we should add some small animals to all biomes, we have already some, others we can generate." Each biome has a few kinds that belong there, found where their niche is, by rule (D380, D378):

| Kind | Niche: growing season, places | When startled |
|---|---|---|
| Red squirrel | 7 °C and up, among trees | Scolds, runs up the nearest trunk and waits there until nobody is near |
| Chaffinch | 8 °C and up, among trees; flits from place to place | Calls and flies off 10 to 30 m |
| Brown hare | 9 °C and up, open ground and woods | Bolts, silent |
| Red fox | 7 °C and up, woods and open ground | Barks and runs |
| Mallard | 6 °C and up, open water | Quacks and swims off; dabbles, tail up |
| Grey heron | 8 °C and up, the shore and the shallows, wading | Croaks and flies 50 to 110 m along the shore |
| Mountain hare, in its autumn moult | up to 7.5 °C, open ground | Bolts |
| Ptarmigan | up to 7.5 °C, open ground | Croaks and flushes 25 to 60 m |
| Arctic fox | up to 7 °C, open ground | Barks and runs |
| House sparrow | 5 °C and up, ground trodden by people | Chirps and flies a little way |
| Feral pigeon | 5 °C and up, ground trodden by people, in flocks | Clatters up together and lands a little way off |
| Dog | the town's, at its owner's heels (`terra_life::animals`) | — |

So the lush forest has squirrels, chaffinches, hares and foxes; the mountain lake ducks and a heron, with the forest's kinds in its shore woods; the tundra mountain hares, ptarmigan and arctic foxes; the town sparrows and pigeons on its streets and its dogs. A new land gets its animals where its climate and cover put them, with no list for any map.

- **Variants** (D182): eight bodies, twelve kinds. The sparrow and the pigeon are the chaffinch's body (the pigeon walking with its head nodding and flapping steadily, by clips of its own), the mountain hare and the arctic fox the brown hare's and the red fox's, each textured again for its own coat. Seasons' coats, and hens and drakes, are textures of the same bodies.
- **Found as you go**, as shipped open worlds' ambient wildlife is: the land is cells of 32 m, each looked at once as a player comes within 110 m and given what lives there from its own seed, the same walk meeting the same animals; let go once nobody is within 150 m (`play::wildlife`). Over-hunting a cell is not remembered yet. A player on the move meets something every few seconds besides: a group of a kind whose niche the place is, put 34 to 52 m ahead beside their way, past where it is drawn, so it comes into view as they near it and breaks from cover as they pass, as Red Dead Redemption 2 and Far Cry stage ambient encounters.
- **Up a tree, into a crown:** a squirrel startled runs up the nearest trunk and waits; a bird among trees as often as not flies up into a crown, sits there a while and comes down or crosses to another.
- **The life round the eye** (D379), cheap as shipped games make it: dust motes and midges that show only in the sun's shafts, a few leaves falling under the crowns, down blowing over the heath, flocks crossing the sky, each kind faded in where its place, season and hour are (`terra_world::drift`, `assets/data/drift.ron`); and fish rising on deep water, a ring on the lake's own waves and now and then a leap, most at dawn and dusk (`terra_world::rises`).
- **Made by rule from their models** (`tools/creatures/animal.py`): two body plans, a four-legged runner and a bird, as Spore and Overgrowth animate many bodies with one set of rules ([research](../research/2026-10-14-animation.md)). A kind's skeleton is a few dozen points read off its model; every clip (idle, walk, run or bound, eat, sit, fly, swim, dabble, die) is worked out from a few numbers, its feet placed by inverse kinematics so they stay where they land. Birds are modelled with their wings spread and fold them to stand, the folded wing drawn smaller as a spread fan cannot close by turning alone.

## Monsters

- **They form where magic pools.** The place decides the kind: a marsh makes marsh things, a ruin its own. The more it pools, the higher their rank.
- **Left alone they grow**, spread, and surge: they spill into the land around, onto roads, settlements and claims.
- **They fight**: they are what combat is mostly against ([combat.md](combat.md)).
- **What they leave**: materials found nowhere else; cores (D165), which hurry powers, run machines and calm pools; spirit coins, the money that is magic (D167); quintessence, fragments of their kind for workings and items (D168); and sometimes essences ([magic.md](magic.md#essences)). Culling them is a living ([people.md](people.md#trade)).
- **Pooling places can be cleared**, and stay quieter for a while; holding one in a claim keeps it down (D94).

## Kinds, by rule

A small set of body plans (a four-legged runner, a flier, a crawler, a big lumbering thing, a swarm), each varied by size, look, behaviour and rank by the world's theme (D59). A new world brings new kinds without new code. Players learn each kind by meeting it, as with factions (D95): no bestiary is filled in for you (D92).

## Designed kinds

Monsters the developers have designed, each its own kind (D250). Their models are in `assets/models/creatures`, made from concepts in `art/props`; none is rigged, animated or in play yet.

| Kind | What it is | Where |
|---|---|---|
| Worman | A giant earthworm standing as tall as a person, disguised as a traveller: a blue cap, round dark glasses, red lips, two thin arms and a knife. It attacks anyone who comes close | Wherever travellers go |
| Drillhog | A groundhog the size of a boar, with drills growing from its back and head, which it digs and attacks with | Dungeons and places underground |
| Seawish | A fish as tall as a person, standing on its tail, with a thick mop of seaweed growing on its head; it fights with a rusty sword and its seaweed. In the water it passes for a floating clump of seaweed | Beaches |

## What it asks of the engine

Technical, mine. Creatures are the most expensive thing in the design: they walk round, diggable ground, find their way, animate and act. So creatures are simulated by how near the nearest player is (D145): every tick close by, thinking less often further out, and waiting where they are beyond; a region's animals and a pooling place's strength are a few numbers when nobody is near; and small behaviours combine, rather than one large brain per creature.

## Questions

1. Answered in part: **taming and mounts**, probably yes (D100). How taming works, and which animals, is open.
2. **Monsters among themselves.** Do they fight each other and the wildlife where they meet?
3. **Bosses.** Are there great monsters at the heart of the worst pooling places? The organisation across realms may be behind some (D172).
4. **Night.** Are monsters more active, or different, after dark ([survival.md](survival.md))?
