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

Someone coming within a kind's flight distance, an animal first walks, paddles or wades off from them, watching; it does what this table says only once they come within 0.4 of that distance (4 m at the least), or it is hurt or frightened (D384).

| Kind | Niche: growing season, places | When startled |
|---|---|---|
| Red squirrel | 7 °C and up, among trees | Scolds, runs up the nearest trunk and waits there until nobody is near |
| Chaffinch | 8 °C and up, among trees; flits from place to place | Calls and flies off 10 to 30 m |
| Brown hare | 9 °C and up, open ground and woods | Sits tight, watching, and bolts at 5 m, silent |
| Red fox | 7 °C and up, woods and open ground | Barks and runs |
| Red deer: a stag alone in the rut, hinds in ones to threes, most with a calf at foot | 5 °C and up, woods and open ground | Snorts and bounds off, the calf with its mother |
| Wild boar | 6 °C and up, among trees, in ones to threes | Grunts and runs |
| Mallard | 6 °C and up, open water | Paddles off; near, quacks and takes wing, coming down on the water 25 to 70 m off; dabbles, tail up |
| Grey heron | 8 °C and up, the shore and the shallows, wading | Stalks off along the shallows; near, croaks and flies 50 to 110 m along the shore |
| Mountain hare, brown in summer and white in winter | up to 7.5 °C, open ground | Sits tight, watching, and bolts at 5 m |
| Ptarmigan | up to 7.5 °C, open ground | In coveys of three to eight; croaks and flushes 25 to 60 m |
| Arctic fox | up to 7 °C, open ground | Barks and runs |
| House sparrow | 5 °C and up, ground trodden by people | Chirps and flies a little way |
| Feral pigeon | 5 °C and up, ground trodden by people, in flocks | Clatters up together and lands a little way off |
| Dog | the town's, at its owner's heels (`terra_town::animals`) | — |

So the lush forest has red deer, wild boar, squirrels, chaffinches, hares and foxes; the mountain lake ducks and a heron, with the forest's kinds in its shore woods; the tundra mountain hares, ptarmigan and arctic foxes; the town sparrows and pigeons on its streets and its dogs. A new land gets its animals where its climate and cover put them, with no list for any map.

- **Variants** (D182): twelve bodies, sixteen kinds. A kind's young (`young`) is a kind of its own, a body of its own proportions, only ever put out at its mother's side and running with her. The sparrow and the pigeon are the chaffinch's body (the pigeon walking with its head nodding and flapping steadily, by clips of its own), the mountain hare and the arctic fox the brown hare's and the red fox's, each textured again for its own coat. Seasons' coats, and hens and drakes, are textures of the same bodies.
- **Found as you go**, as shipped open worlds' ambient wildlife is: the land is cells of 32 m, each looked at once as a player comes within 110 m and given what lives there from its own seed, the same walk meeting the same animals; let go once nobody is within 150 m (`play::wildlife`). Over-hunting a cell is not remembered yet. A player on the move meets something every few seconds besides, staged as Red Dead Redemption 2 and Far Cry stage ambient encounters (D384): a group of a kind whose niche the place is, out to one side of their way where it passes through the open third of the picture, 15° to 30° off the camera's way (a small animal nearer, a big one further), at the spot of 18 tried that shows it best (in view from where the camera will be as it passes, nothing standing or no crown between, in the open and the light, on bare or short ground); every third meeting, the first among them, with a big animal, a deer or a boar, where one lives, with a budget of its own so the cells round a player never leave nothing to meet. Half the meetings, and three in four with a big animal, are a crossing instead: a group already put up, from 28-36 m ahead just inside the trees, running or flying across the way timed to cross 10 m ahead of the player as they come on, seen from small to near and moving all the while. Standing, a player meets a few too, the way they face. A runner that notices someone coming stops and watches them; startled by someone coming on, an animal breaks across their way and on ahead, now and then dashing across it to the other side, a squirrel to a trunk that way and a bird into the air that way, so it crosses the picture near rather than shrinking into cover. A film says how well it worked: what it saw of the wildlife, how much of it, how big, from the run's log (`play::sighted`, summed by `tools/sandbox/trailer.ts`).
- **No two alike:** each animal is a little bigger or smaller than another of its kind and keeps its own beat at rest. A mountain hare's or an arctic fox's coat follows the date and the climate (`play::coats`, `coat` in `creatures.ron`): brown in summer, moulting white from mid-October to the end of November and back from April into May, earlier and further on colder land and only part white on mild, each animal on its own few days and in its own tone.
- **Towns' birds on the roofs:** pigeons and sparrows line the ridges of a town's roofs as well as its streets, and fly up onto them as birds among trees fly into a crown (`Perches`, filled by the town). Swallows hawk low over roofs, meadows and water through the warm months (`drift.ron`). A dog's owner is the one of its household most about the streets and market, and it is at their heels from the start.
- **Up a tree, into a crown:** a squirrel startled runs up the nearest trunk and waits; a bird among trees as often as not flies up into a crown, sits there a while and comes down or crosses to another.
- **The life round the eye** (D379), cheap as shipped games make it: dust motes and midges that show only in the sun's shafts, a few leaves falling under the crowns, down blowing over the heath, flocks crossing the sky, each kind faded in where its place, season and hour are (`terra_world::drift`, `assets/data/drift.ron`); and fish rising on deep water, a ring on the lake's own waves and now and then a leap, most at dawn and dusk (`terra_world::rises`).
- **Made by rule from their models** (`tools/creatures/animal.py`): two body plans, a four-legged runner and a bird, as Spore and Overgrowth animate many bodies with one set of rules ([research](../research/2026-10-14-animation.md)). A kind's skeleton is a few dozen points read off its model; every clip (idle, walk, run or bound, eat, sit, fly, swim, dabble, die) is worked out from a few numbers, its feet placed by inverse kinematics so they stay where they land. Birds are modelled with their wings spread and fold them to stand, the folded wing drawn smaller as a spread fan cannot close by turning alone.
- **Each clip plays near its own rate** (D424): a gait's clip plays from 0.8 to 1.2 times as fast as it was made, its rate the ground speed over the stride it was made with; past that the animal changes gait (walk to run) or goes no faster (`play::creatures::gait`), as Unreal's speed-matched blend spaces and Red Dead Redemption 2's animals keep a big animal's long, slow stride. A kind whose run clip covers less ground than its `run_mps` runs no faster than its clip allows until its clip is made with a longer stride.

## Monsters

- **They form where magic pools.** The place decides the kind: a marsh makes marsh things, a ruin its own. The more it pools, the higher their rank.
- **Left alone they grow**, spread, and surge: they spill into the land around, onto roads, settlements and claims.
- **They fight**: they are what combat is mostly against ([combat.md](combat.md)).
- **What they leave**: materials found nowhere else; cores (D165), which hurry powers, run machines and calm pools; spirit coins, the money that is magic (D167); quintessence, fragments of their kind for workings and items (D168); and sometimes essences ([magic.md](magic.md#essences)). Culling them is a living ([people.md](people.md#trade)).
- **Pooling places can be cleared**, and stay quieter for a while; holding one in a claim keeps it down (D94).

## Kinds, by rule

A small set of body plans (a four-legged runner, a flier, a crawler, a big lumbering thing, a swarm), each varied by size, look, behaviour and rank by the world's theme (D59). A new world brings new kinds without new code. Players learn each kind by meeting it, as with factions (D95): no bestiary is filled in for you (D92).

## Fliers

Creatures that fly are a body plan of their own (D382, `play::fliers`), each kind an entry in `assets/data/fliers.ron`. A flier flies a path, a spline through points, at its cruising speed, banked into its turns by how tight they are and how fast it goes, beating its wings to climb and gliding as it comes down; it eases into a short hover facing a point; it may breathe fire, in a strafing pass or hovering, which sets alight what it meets ([building.md](building.md#fire)). Until monsters think, a script tells it what to do over the remote. The first kind is a dragon, wyvern-built (its wings are its forelimbs), 13 m long and 18 m across its wings; whether dragons are in the game, and where, is for the developers.

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
