# Worlds

Status: the developers set out on 2026-10-10 what the planets are to be. What they said is below, as they said it, then what follows from it and what is still open. Nothing in this doc is built except a single stand-in planet in the sandbox.

## What the developers want

| | Said | Status |
|---|---|---|
| **Terrain** | Procedurally generated. | Decided (D8) |
| **More than one planet** | Each one different. Not thousands; "probably a dozen", and maybe more over time. | Decided that there are several (D17); the number is Leaning |
| **Variety** | Planets are thematic and varied: caverns, water features and the like. | Decided (D59) |
| **Terraforming** | Making a planet livable and breathable exists on *some* planets, with all that implies. | Leaning (D60) |
| **Foliage** | Thematically varied from planet to planet. | Decided (D59) |
| **Digging** | You can dig into the ground, much as in No Man's Sky. | Decided (D9, D20) |
| **Survival** | Attributes coupled to survival: health, stamina, oxygen, hunger, thirst, possibly more later. They may work differently on each world. See [survival.md](survival.md). | Decided that they exist (D61); the list may grow |
| **The starter planet** | Normal atmosphere and the like: the easiest place to be, for now. | Leaning (D62) |
| **Gravity** | Planets have gravity. | Decided (D63) |
| **Sea level** | Every planet with water has one. The sea's surface is level, the same height all round the planet, and fills what lies below it wherever the sea can reach. | Decided (D75) |
| **Weather** | Differs by planet, and on planets that can be terraformed, by how far along they are. | Decided (D78) |
| **Water you can enter** | The character swims, using stamina for now. Under water you use up oxygen, as on a planet with thin or no air. No set limit on depth: water should be flexible. | Decided (D74) |
| **Size** | Fully procedural and somewhat large: big enough that walking all the way round is a ridiculous task, but not infinite, and efficient. Always round, so you wrap around. | Decided; the numbers are Leaning (D13) |
| **Getting between planets** | By flight, for sure; maybe other ways later. Leaving a planet, being in space and arriving at another has to perform well. | Decided (D72) |
| **How planets differ** | Mostly in the obvious ways. | Answers the old open question 2 |
| **More than one breathable planet** | Some planets besides the starter one might be breathable. | Leaning (D86) |
| **A setting per planet** | Planets might differ in kind of theme, not only in climate: one fantasy, one primitive, one futuristic. | Leaning (D86) |

## What a planet is

A planet is a seed plus a **theme**. The theme is the small set of numbers and lists that make one planet unlike another; the seed makes each place on it unlike every other place. Everything else is computed from those two (D8, D47).

What a theme sets, as a starting list:

- **Size, and the strength of gravity at the surface.**
- **Atmosphere**: whether it can be breathed, its pressure, and how hot or cold it runs. This is what the survival attributes respond to.
- **Ground**: the shape of the land (how mountainous, how flat), what the rock and soil are made of, and how hollow it is: whether there are caverns, how big and how deep.
- **Water**: seas, lakes, underground water, ice, or none.
- **Life**: which kinds of plant grow where, from a palette that belongs to the planet, and how dense.
- **Palette**: the colours of a place, as the art direction asks (D14).
- **Whether it can be terraformed**, and if so where it starts on that path.

A new planet should be a new theme, not new code. That is the same rule the construction system follows (D56): the things that vary are data.

## Size

Every planet is a sphere: walk far enough in a straight line and you come back to where you started. The developers want that walk to be ridiculous, and the planet still finite and cheap to run.

How the candidates feel, for a player jogging at the sandbox's 4.3 m/s and a craft at 100 m/s:

| Radius | Once round | On foot | By air | Horizon from eye height | Surface |
|---|---|---|---|---|---|
| 2 km (the sandbox today) | 13 km | under an hour | 2 minutes | 82 m | 50 km² |
| 10 km | 63 km | 4 hours | 10 minutes | 184 m | 1,260 km² |
| 20 km | 126 km | 8 hours | 21 minutes | 261 m | 5,030 km² |
| **30 km** | **189 km** | **12 hours** | **31 minutes** | **319 m** | **11,300 km²** |
| 60 km (Space Engineers' Earth-like planet, as commonly cited) | 377 km | 24 hours | an hour | 452 m | 45,200 km² |

**Proposal: about 30 km for the starter planet, and 10 to 50 km for the rest, set by each planet's theme** (D13, Leaning until it is walked). At 30 km, walking round takes half a day; flying round takes half an hour; the horizon from the ground is a few hundred metres, so the planet feels like a world and not a ball, and from a hilltop or the air the curve shows. A dozen such planets are around a hundred thousand square kilometres, which a thousand players will not fill (D52).

What makes a planet that size affordable:

- **Nothing is stored that can be computed** (D47). The ground, the rocks, the trees are functions of the seed and the place. Only what players change is kept. A planet nobody has visited costs nothing.
- **Only what is near a player is generated in full.** The terrain is level-of-detail voxels (D9): fine where you stand, coarse further off, a few numbers from orbit. Objects follow the same rule.
- **Precision.** A 32-bit float at 30 km from a planet's centre has steps of about 2 mm, which is fine for drawing but not for physics far from where it was measured. So positions are kept relative to a nearby origin that moves with the player, and the server holds planet-scale positions in 64-bit (D36).
- **The sandbox cannot test this yet.** Its stand-in planet scales its hills and its fine ground patch with the radius, so asking it for 30 km gives hills kilometres across and ground drawn every 15 m. The voxel terrain (D9, [terrain.md](terrain.md)) is what will make a large planet look right.

## The starter planet

Breathable, temperate, ordinary gravity, water and plants. The survival attributes all work, but nothing about the place is trying to kill you. It is where the player learns the game, and it is the planet the sandbox's stand-in is closest to.

## Breathable planets, and a setting per planet

From the developers, 2026-10-10, with "might" (D86, Leaning): more than one planet may be breathable, and planets may have different kinds of theme, "like fantasy or primitive or futuristic".

Breathable beyond the starter planet fits what is here already: whether the air can be breathed is one number in a planet's theme, and the survival attributes read it (D61).

A setting is a bigger thing than a climate. Proposed, to react to: a planet's theme gains a **setting** that chooses its palette, the shapes its plants, rocks and ruins are drawn from, and what can be found there. It stays data (D59): a fantasy planet is a different list of models and colours, not different code. It also fits the two inspirations, Enshrouded's fantasy and Subnautica's science fiction (D85).

Open:

Said later the same day, with "maybe" and "idk": a planet's setting shows in the **cultures, or tribes, found there**. Finding a different culture might unlock different things, and you might recruit people from it. The art style stays the same across the whole game; what changes from culture to culture is the exact models and details, made by the procedural generation and the theme. Your gear is your gear: what you carry goes with you from planet to planet.

So a setting proposed as data would name its cultures, and a culture its models, its colours within the one art style, what it can teach you, and who in it might join you.

Open:

9. Answered in part: **what does a setting change?** The look of a culture's things within the one art style, what you can unlock by finding it, and perhaps who you can recruit. How unlocking from a culture works is open in [progression.md](progression.md), and recruiting in [gameplay.md](gameplay.md#npcs).
10. Set aside: **how do settings sit together in one solar system?** The developers are considering worlds as alternate dimensions instead (D87), which needs no solar system to explain them.
11. Answered: **one art style across the game** (D14). Cultures differ in their models and details, not in their style.
12. Answered: **your gear is your gear.** What you carry works the same on every planet.

## Planets or dimensions?

From the developers, 2026-10-10, with "maybe" and "might" (D87, Open): the worlds may be more like alternate dimensions, in the spirit of *He Who Fights With Monsters*, than planets in one solar system. Terraforming might not happen, and nor might flying into space.

Nothing here is changed yet. If it is adopted:

- **What stays**: procedural worlds, each with its own theme, setting and cultures (D59, D86); voxel ground you dig; survival; building; items.
- **What goes or changes**: a real solar system and seamless flight to space (D4, D72); terraforming and the planet simulation (D6, D60); and in the engine, the ground-to-orbit transition, the sky seen from space, and solar-system coordinates (D39, D36).
- **What it asks**: how a player crosses between worlds (a portal, a ritual, a machine?), and whether a world is still a round planet you can walk around, or something else.

## Terraforming, on some planets

The simulation of a planet becoming livable (D6, [planet-sim.md](planet-sim.md)) runs on the planets where terraforming is possible. Elsewhere a planet's climate is part of its theme and stays as it is, which costs nothing to compute. The dead-to-alive arc in [vision.md](vision.md) is the story of those planets, not of every planet.

## Weather

<a id="weather"></a>There are weather systems (D78), different on each planet and, where a planet can be terraformed, changing as it goes: the phases in [gameplay.md](gameplay.md#terraforming-phases) already have storms while thawing and rain once wet. A terraform rating (D79, Leaning) might be what weather and those phases follow.

Open: which kinds of weather there are, whether they hurt, help or only look, whether they are the same for everyone on a planet at once (with a thousand players, D52, weather is shared state the server sends), and how far the planet simulation (D6) drives them.

## What it asks of the engine

- **Caverns** are no extra work for the shape of the ground: smooth voxels describe the inside of a planet as easily as its surface (D9). They are extra work for generation (shaping them so they are worth exploring), for light (it is dark underground), and for streaming (what is below you matters as much as what is beside you).
- <a id="water"></a>**The sea** (D75) is a sphere round the planet's centre, which is what a level surface is on a round planet with gravity toward its centre. Water fills the ground below it that the sea can reach, so depth is whatever the ground makes it. A hollow below sea level that the sea cannot reach stays dry; working out what the sea reaches is a fill over the terrain, done once when a planet is generated and again only when digging opens a way in. The sandbox's stand-in is not yet this: it draws the sea everywhere below sea level, reached or not.
- **Water features** go beyond one sea level (D24). Lakes at different heights and water underground need water to be asked for by place, which the engine's contract already allows (D45), and the decision on pools (D44) is the same decision.
- **Gravity** pulls toward a planet's centre with a strength the planet sets. The player's movement already treats "down" as toward the centre. Between planets, and in orbit, it is open.
- **Several planets, reached by flight** (D72). Leaving a planet, crossing space and arriving at another, without a loading screen (D4) and without the frame rate falling, is a system of its own:
  - **Coordinates.** A position far from the origin loses precision. Each planet needs its own frame, and space needs one that holds them all (D36).
  - **Detail by distance.** From orbit a planet is one simple shape; on the ground it is terrain, plants and bases. The terrain's levels of detail (D9) carry most of this; plants, objects and bases need the same.
  - **What is loaded.** Only the planet you are near is generated in full; the rest are a few numbers until you approach.
  - **The server.** A thousand players across a dozen planets (D52) means a player flying from one to another moves between parts of the server.
  The Godot spike flew from the ground to orbit on one planet; nothing has crossed between planets yet.
- **Digging** is the terrain's job (D9); what you get from it is harvesting (D43).

## Open questions

1. **How many planets**, and are they all in one solar system? Answered in part: you fly between them (D72), perhaps by other means later.
2. Answered: planets mostly differ in the obvious ways (gravity, air, temperature, terrain, water, life). Whether any differ in less obvious ways (day length, weather, resources found nowhere else) can be settled planet by planet.
8. **What do you fly in?** A ship you board, a suit that keeps climbing, or something else? It is the largest open piece of the player's design ([player.md](player.md), question 6).
3. **Which planets can be terraformed**, and is a terraformed planet a reward, a goal, or the endgame?
4. **How is the next planet reached and unlocked?**
5. **Caverns**: a feature of some planets or all? Do they hold anything the surface does not?
6. **Water**: does it flow, or only sit at a level (D24, D44)? The sea sits at its level (D75); whether it pours into a hole dug below that level is the open part.
7. Answered in part: planets differ in size, set by their theme. The numbers above are a proposal to walk and judge.
