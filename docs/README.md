# Terra design docs

Working title: **Terra**. First and foremost an extremely ambitious sandbox engine: multitudes of procedural worlds, and intricate systems for each part of the game that act on one another (D187). The game, survival and RPG in a living world across realms reached through portals, is what those systems make together.

The source of truth, then the docs about the game as a whole, then one doc per part of the game, then parked docs, then the engine and how we work.

| Doc | What it covers |
|---|---|
| [decisions.md](decisions.md) | What is settled, what is a working assumption, what is open. Source of truth. |

**The game as a whole**

| Doc | What it covers |
|---|---|
| [vision.md](vision.md) | What we are building (D187), the pitch, pillars, our inspirations, tone, non-goals, and how the scope is reached. |
| [gameplay.md](gameplay.md) | **The game's design, and the source of truth for it** (D89): a living world, after Enshrouded and *He Who Fights With Monsters*; what goes on in the realms, the loops, factions and settlements, holding ground, no hand-holding. Links a page per system, below. |
| [lore.md](lore.md) | The foundational lore: how the universe works, from *He Who Fights With Monsters*'s world-building, written in our own words. The basis for every system. |
| [naming.md](naming.md) | Proposed rules for making names and lore: a shared tongue for how the universe works, in the books' names and plain English, and generated tongues for each realm's peoples. |
| [proposals](proposals/README.md) | Where new numbered proposals go. |

**Parts of the game**

| Doc | What it covers |
|---|---|
| [worlds.md](worlds.md) | The realms as physical worlds: what a world's theme sets, terrain, caverns, water, size, gravity, the starting realm, portals between them. |
| [biomes.md](biomes.md) | Draft: how a world fills with life, from grass to canopy: goals, how placement and drawing could work on voxel terrain, open questions. |
| [climate.md](climate.md) | Leaning: seasons and weather as swings in temperature, daylight and precipitation that bodies, ground, plants and creatures respond to; the year's length. |
| [survival.md](survival.md) | Needs, rhythms and death: hunger, thirst, warmth, breath, rest, mana; day and night; coming back at a hearth. |
| [combat.md](combat.md) | A fast action RPG on stamina, after Enshrouded; weapon skills by doing; abilities from essences; rank decides more than skill. |
| [creatures.md](creatures.md) | Wildlife with routines and a food chain; monsters that form where magic pools, and what they leave. |
| [magic.md](magic.md) | Magic as a whole, with examples: core essences, attributes, mana, confluences by rule, awakening as a weighted draw of generated powers, combat styles and studied crafts from skill books, old-school spellcraft (reagents, words of power, Mark and Recall, scrolls and wands), and magic beyond these. |
| [crafting.md](crafting.md) | Resources by region and rank, tools, stations run by recruits, learning what to make. |
| [people.md](people.md) | Settlements with days, quests as what people say, recruits, trade. |
| [tribe.md](tribe.md) | Your tribe: its players and recruits, and its claims as outposts; recruits who work, carry and defend on their own, without being managed. |
| [player.md](player.md) | No class, creation (D111), moving and acting on the world, getting around big round realms, the map, what is on screen and what is not, options (D80). |
| [arc.md](arc.md) | The fiction, the start without a tutorial, the long game by rank, playing together. |
| [settings.md](settings.md) | How any realm's theme and cultures work: gameplay roles every setting fills, rank the only power axis, how generated realms fit the lore, and one setting worked through, a realm of machines that run on magic without guns. |
| [items.md](items.md) | Items and containers: equipment, backpack, action bar and storage, on a Subnautica baseline; icons; how they work in code. |
| [characters.md](characters.md) | Bodies and what they wear: how every item fits every body without fitting each by hand (D112, D128), the base body, the game's side, tools and checks, order of work, risks and questions for the developers. |
| [building.md](building.md) | Building as the player meets it: walls, floors and roofs of half-metre cubes (D152, D181), drawn the Sims way (D154), planned then constructed (D139), claims, the controls, sizes, and open questions. |
| [building-tech.md](building-tech.md) | The technical design for building: cubes as shared state (D160, D181), walls laid from kits by rule, roofs as a height field, buildings and indoors, drawing many buildings, and the order of work. |
| [building-kits.md](building-kits.md) | How each kind of construction looks and is drawn, planned, going up and built, logs first: corners, joins and openings (D177 to D179). |
| [progression.md](progression.md) | Three ways to grow (skills, studied techniques, essences), skills on the rank ladder, crossing a rank, and how things are unlocked. |
| [multiplayer.md](multiplayer.md) | Server-first multiplayer, built toward a thousand players on one persistent server. |
| [art-style.md](art-style.md) | **The style guide and art direction for anything the game draws**, the same in every realm and setting: why direction D, what a realm brings, life size, shape, colour, light, budgets, the concept prompt, a checklist, how the renderer makes the look and open questions. |
| [audio.md](audio.md) | Sound: how it is put together, what it sounds like now, and questions for the developers. |

**Parked**

| Doc | What it covers |
|---|---|
| [planet-sim.md](planet-sim.md) | Parked with terraforming (D6, D89): the planetary simulation, cells, fields, how local feeds global. |

**The engine and how we work**

| Doc | What it covers |
|---|---|
| [tech.md](tech.md) | The stack, how the code is split, how co-op shares state, terrain, workflow, risks. |
| [engine.md](engine.md) | The engine's parts, what each needs, and the parallel tracks they are built in. |
| [terrain.md](terrain.md) | Brief for the first component, the voxel terrain crate. |
| [development.md](development.md) | Setting up a machine, everyday commands, seeing and measuring output, developer tools, and QA our agents can run. |
| [prototypes.md](prototypes.md) | The builds so far and the milestones ahead, with their pass criteria. |

Findings, surveys and measurements live in [research](../research), not here. These docs say what we decided; research says what we found.

## How we keep these useful

- **decisions.md wins.** If another doc disagrees with it, the other doc is wrong; fix it.
- **Every claim has a status.** *Decided* means don't reopen without a reason. *Leaning* means a working assumption that is untested or unconfirmed. *Open* means nobody has chosen yet.
- **Change the doc in the same change as the thing it describes.** A decision made in chat that isn't written here didn't happen.
- **Keep them short.** These describe intent and constraints, not implementation. Code explains itself.
- **Record why.** A decision without its reason gets relitigated.
