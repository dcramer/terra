# Terra design docs

Working title: **Terra**. A multiplayer survival sandbox across many procedural worlds, reached through portals.

The source of truth, then the docs about the game as a whole, then one doc per part of the game, then the engine and how we work.

| Doc | What it covers |
|---|---|
| [decisions.md](decisions.md) | What is settled, what is a working assumption, what is open. Source of truth. |

**The game as a whole**

| Doc | What it covers |
|---|---|
| [vision.md](vision.md) | Pitch, pillars, tone, our inspirations, non-goals. |
| [gameplay.md](gameplay.md) | **The game's design, and the source of truth for it** (D89): a living world, after Enshrouded and *He Who Fights With Monsters*; what goes on in the realms, the loops, factions and settlements, holding ground, no hand-holding. Links a page per system, below. |
| [lore.md](lore.md) | The foundational lore: how the universe works, from *He Who Fights With Monsters*'s world-building, written in our own words. The basis for every system. |
| [naming.md](naming.md) | Proposed rules for making names and lore: a shared tongue for how the universe works, with magic spoken of as water, and generated tongues for each realm's peoples. |
| [proposals](proposals/README.md) | Numbered proposals, each one candidate. Game design 001 was adopted on 2026-10-12 and is now gameplay.md and its system pages. |

**Parts of the game**

| Doc | What it covers |
|---|---|
| [worlds.md](worlds.md) | The realms as places: themes, terrain, caverns, water, size, gravity, the starting realm, portals between them. |
| [biomes.md](biomes.md) | Draft: how a world fills with life, from grass to canopy: goals, how placement and drawing could work on voxel terrain, open questions. |
| [climate.md](climate.md) | Draft: seasons and weather as swings in temperature, daylight and precipitation that bodies, ground, plants and creatures respond to; the year's length. |
| [survival.md](survival.md) | Needs, rhythms and death: hunger, thirst, warmth, breath, rest; day and night; coming back. |
| [combat.md](combat.md) | Real time on stamina, after Enshrouded; weapon skills by doing; abilities from essences; rank decides more than skill. |
| [creatures.md](creatures.md) | Wildlife with routines and a food chain; monsters that form where magic pools. |
| [essences.md](essences.md) | Essences, abilities and ranks; skills, studied techniques and powers as three ways to grow; mana. |
| [crafting.md](crafting.md) | Resources by region and rank, tools, stations run by recruits, learning what to make. |
| [people.md](people.md) | Settlements with days, quests as what people say, recruits, trade. |
| [your-character.md](your-character.md) | No class, getting around big round realms, what is on screen and what is not. |
| [arc.md](arc.md) | The start without a tutorial, the long game by rank, playing together. |
| [settings.md](settings.md) | How any realm's theme works: gameplay roles every setting fills, rank the only power axis. |
| [techno-realm.md](techno-realm.md) | One setting worked through: a realm of machines that run on magic, without guns. |
| [player.md](player.md) | Draft: the player's body, tools, equipment, backpack and action bar, acting on the world, building. Open questions for the developers. |
| [items.md](items.md) | Items and containers: equipment, backpack, action bar and storage, on a Subnautica baseline, and how they work in code. |
| [characters.md](characters.md) | How bodies, clothing, armour and weapons are made so every item fits every body without fitting each by hand (D128). Questions for the developers. |
| [character-system-plan.md](character-system-plan.md) | The character system from scratch: the base body asset, bodies as shapes, gear made once, the game's side, tools, order of work and risks. |
| [construction.md](construction.md) | Draft for the developers: the baseline building system, everything players will expect from Enshrouded, Rust, Ark, Subnautica 2 and The Forest, the controls, what is built, and open questions. |
| [progression.md](progression.md) | Draft: player skills, the research system, and how things are unlocked. Mostly open questions. |
| [multiplayer.md](multiplayer.md) | Server-first multiplayer, built toward a thousand players on one persistent server. |
| [planet-sim.md](planet-sim.md) | Parked with terraforming: the planetary simulation, cells, fields, how local feeds global. |
| [art.md](art.md) | Art direction, constraints, the beauty-corner test. |
| [audio.md](audio.md) | Sound: how it is put together, what it sounds like now, and questions for the developers. |

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
