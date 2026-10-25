# Worlds: the realms as places

**The worlds are realms** (D17, D87, D89): as many as the generator makes, with no fixed number, each a whole round world reached by portals. This page covers them as physical worlds: what a world's theme sets, its ground, caverns, water, size and gravity, and how worlds join. What a realm's people and culture are, and how its theme fits the lore, is in [settings.md](settings.md); how realms work in the lore is in [lore.md](lore.md#realms-and-the-ways-between).

Status: what the developers set out on 2026-10-10 and 2026-10-11 is below, then what follows from it and what is still open. Nothing in this doc is built except a single stand-in world in the sandbox.

## What the developers want

| | Said | Status |
|---|---|---|
| **Terrain** | Procedurally generated. | Decided (D8) |
| **Many worlds** | Each one different, with no fixed limit: realms, as in *He Who Fights With Monsters*. Not a promise to ship unlimited, but nothing constrains it. | Decided (D17) |
| **Realms** | Realms, not a set number of planets, reached probably by portals, with no space between them. | Decided (D87) |
| **Variety** | Each world is thematic and varied: its own terrain, caverns, water features and foliage. | Decided (D59) |
| **Digging** | You can dig into the ground. | Decided (D9, D20) |
| **Survival** | Attributes coupled to survival, which may work differently on each world. See [survival.md](survival.md). | Decided (D61) |
| **The starting world** | Breathable and temperate: the easiest place to be, but with winters to prepare for. | Leaning (D62) |
| **Gravity** | Each world has gravity, toward its centre. | Decided (D63) |
| **Sea level** | Every world with water has one. The sea's surface is level, the same height all round the world, and fills what lies below it wherever the sea can reach. | Decided (D75) |
| **Weather and seasons** | Weather differs by world (D78); seasons change play (D149). See [climate.md](climate.md). | Decided |
| **Water you can enter** | The character swims, using stamina for now. Under water you use up breath. No set limit on depth: water should be flexible. | Decided (D74) |
| **Size** | Large but finite: walking all the way round is a ridiculous task. Always round, so you wrap around. | Decided; the numbers are Leaning (D13) |
| **No space** | No solar system, and no flying to orbit or between worlds. | Decided (D4) |
| **Getting between worlds** | Probably portals, or a similar mechanism. | Leaning (D72) |

## What a world is

A world is a seed plus a **theme**. The theme is the small set of numbers and lists that make one world unlike another; the seed makes each place on it unlike every other place. Everything else is computed from those two (D8, D47). A new world is a new theme, not new code (D59).

What a theme sets, as a starting list:

- **Size, and the strength of gravity at the surface.**
- **Air and climate**: whether it can be breathed, and how hot or cold it runs and how much it swings ([climate.md](climate.md)). This is what the survival attributes respond to.
- **Ground**: the shape of the land (how mountainous, how flat), what the rock and soil are made of, and how hollow it is: whether there are caverns, how big and how deep.
- **Water**: seas, lakes, underground water, ice, or none.
- **Life**: which plants grow where, from a palette that belongs to the world, and how dense ([biomes.md](biomes.md)).
- **Magic**: its leaning and how much there is, which decides what pools and what forms there ([settings.md](settings.md#making-generated-realms-fit-the-lore)).
- **Palette and light**, as the art style asks (D14).

## Size

Every world is a sphere: walk far enough in a straight line and you come back to where you started. The developers want that walk to be ridiculous, and the world still finite and cheap to run.

How the candidates feel, for a player jogging at the sandbox's 4.3 m/s:

| Radius | Once round | On foot | Horizon from eye height | Surface |
|---|---|---|---|---|
| 2 km (the sandbox today) | 13 km | under an hour | 82 m | 50 km² |
| 10 km | 63 km | 4 hours | 184 m | 1,260 km² |
| 20 km | 126 km | 8 hours | 261 m | 5,030 km² |
| **30 km** | **189 km** | **12 hours** | **319 m** | **11,300 km²** |
| 60 km | 377 km | 24 hours | 452 m | 45,200 km² |

**Proposal: about 30 km for the starting world, and 10 to 50 km for the rest, set by each world's theme** (D13, Leaning until it is walked). At 30 km, walking round takes half a day; the horizon from the ground is a few hundred metres, so the world feels like a world and not a ball, and from a hilltop the curve shows. Realms of this size, with no limit on how many, are far more ground than a thousand players will fill (D52).

What makes a world that size affordable:

- **Nothing is stored that can be computed** (D47). The ground, the rocks, the trees are functions of the seed and the place. Only what players change is kept. A world nobody has visited costs nothing.
- **Only what is near a player is generated in full.** The terrain is level-of-detail voxels (D9): fine where you stand, coarse further off. Objects follow the same rule.
- **Precision.** A 32-bit float at 30 km from a world's centre has steps of about 2 mm, which is fine for drawing but not for physics far from where it was measured. So positions are kept relative to a nearby origin that moves with the player, and the server holds world-scale positions in 64-bit (D36).
- **The sandbox cannot test this yet.** Its stand-in world scales its hills with the radius, so asking it for 30 km gives hills kilometres across. The voxel terrain (D9, [terrain.md](terrain.md)) is what will make a large world look right.

## The starting world

Breathable, temperate, ordinary gravity, water and plants. The easiest world, but not a freebie: its winters are a real challenge to prepare for (D62, [climate](climate.md#winter-on-the-starter-world)). It is where the player learns the game, and it is the world the sandbox's stand-in is closest to.

<a id="breathable-planets-and-a-setting-per-planet"></a>
## Settings and cultures

Realms may differ in setting, such as fantasy, primitive or futuristic, shown in the cultures found there, and more than one may be breathable (D116, Leaning). Whether the air can be breathed is one number in a world's theme. How a setting and its cultures work, every one filling the same gameplay roles with rank the only power axis (D116), is in [settings.md](settings.md); one setting is worked through in [a realm of machines](settings.md#a-worked-example-the-techno-realm).

<a id="realms-and-portals"></a>
## Realms and portals

There is no space (D4): worlds are realms joined by portals or something similar (D72, D87).

- **Portals**: likely something you find in the world, with more ways of crossing added over time, perhaps portals players build (D72). Which other ways, and whether found portals are fixed places in a world's generation, are open.
- **Loading**: no traditional loading screen. The crossing itself can cover loading if it needs to (D72). Proposed, technical: a portal is known before you reach it, so the world beyond can start loading as you approach, and the crossing only covers what is left.
- **Each world is still a round world** you can walk all the way around (D4).
- **What it asks**: how the server hands a player between worlds (D52).

## Weather

<a id="weather"></a>Proposed in [climate.md](climate.md): weather and seasons as swings in a few quantities that everything responds to, different on each world (D78, D149). With a thousand players (D52), weather is shared state the server sends.

Open: which kinds of weather there are, and whether they hurt, help or only look.

## What it asks of the engine

- **Caverns** are no extra work for the shape of the ground: smooth voxels describe the inside of a world as easily as its surface (D9). They are extra work for generation (shaping them so they are worth exploring), for light (it is dark underground), and for streaming (what is below you matters as much as what is beside you).
- <a id="water"></a>**The sea** (D75) is a sphere round the world's centre, which is what a level surface is on a round world with gravity toward its centre. Water fills the ground below it that the sea can reach, so depth is whatever the ground makes it. A hollow below sea level that the sea cannot reach stays dry; working out what the sea reaches is a fill over the terrain, done once when a world is generated and again only when digging opens a way in. The sandbox's stand-in is not yet this: it draws the sea everywhere below sea level, reached or not.
- **Water features** go beyond one sea level. Lakes at different heights and water underground need water to be asked for by place, which the engine's contract already allows (D45). Lakes are planned as the generated ground's basins filled to their spill points (D322, [demos.md](demos.md#1-a-mountain-lake)).
- **Gravity** pulls toward a world's centre with a strength the world sets. The player's movement already treats "down" as toward the centre.
- **Any number of worlds** (D17), so every world has to come from rules and its seed alone, its factions, cultures and portals included, with nothing tuned by hand that the game depends on; and a world nobody is in costs nothing, existing only as a seed until someone arrives.
- **Each world has its own coordinate frame** (D36), and only the one a player is in is generated in full. A crossing is a hand-off between worlds, and on a server of a thousand players (D52) between parts of the server. Nothing has crossed between worlds yet; how a world is loaded, left and freed is [instances.md](instances.md) (D468 to D471).
- **Digging** is the terrain's job (D9); what you get from it is harvesting (D43).

## Open questions

1. Answered: **how many worlds.** No fixed limit (D17).
2. Answered: **how worlds differ.** In the obvious ways (gravity, air, temperature, terrain, water, life, magic), each a theme (D59). Whether any differ in less obvious ways, such as day length or resources found nowhere else, is settled theme by theme.
3. Answered in part: **how is the next world reached?** Likely by a portal you find, with other ways over time (D72).
4. **Caverns**: a feature of some worlds or all? Do they hold anything the surface does not?
5. **Water**: does it flow, or only sit at a level? The sea sits at its level (D75); whether it pours into a hole dug below that level is the open part.
6. Answered in part: worlds differ in size, set by their theme. The numbers above are a proposal to walk and judge (D13).
