# Gameplay

Status: draft. The tone is Decided. Terraforming, and the dead-to-alive arc with it, is parked (D60). The specifics below are Leaning until a prototype tests them. The worlds are in [worlds.md](worlds.md).

The style of play, and the player's body, tools and actions, are in [player.md](player.md).

## Core loops

| Loop | What the player does |
|---|---|
| 30 seconds | Move, mine, place, watch a meter (oxygen first). |
| 10 minutes | Leave base for a resource, return, build a machine, see a local value move. |
| Session | Cross a regional or global threshold. The world visibly changes, a new threat appears, new tech unlocks. |

If the 30-second loop isn't fun in a grey-box build, content won't fix it.

## Survival needs

The survival attributes are health, stamina, oxygen, hunger and thirst (D61); see [survival.md](survival.md). Temperature and radiation are things about a place that wear them down. Each need follows the same arc:

1. **Manual**: carry it, ration it, run back to base.
2. **Local infrastructure**: a machine or a pipe covers your base.
3. **Solved by the place**: on the starter planet some needs are solved from the start (D62). Solving them by terraforming is parked (D60).

## Terraforming phases

Parked (D60): probably no terraforming. Kept for reference. Draft names and order. Phases are gated by global values; what a region looks like is set by its local values. See [planet-sim.md](planet-sim.md).

The developers floated a **terraform rating** for each such planet (D79, Leaning): one measure of how far along it is, whose top might mean the air is breathable. It could be what those global values add up to, and what the phases and the weather (D78) follow.

| Phase | The planet | What threatens you |
|---|---|---|
| Dead | Airless, frozen or scorched, irradiated | Suffocation, cold, radiation |
| Thawing | Pressure and heat rising, ice melting | Storms, floods, unstable ground |
| Wet | Standing water, rain | Weather; first microbial hazards |
| Green | Flora spreading | Hostile plants overrunning machines and bases |
| Swarming | Insects | Swarms |
| Wild | Animals | Predators |

## Hostile life

- **What you grow decides what you face.** A hot wet region and a cold dry one produce different flora and fauna.
- **Your output provokes it.** As in Factorio, where pollution provokes the biters, biomass feeds and escalates the ecosystem.
- **Order: plants, then swarms, then animals.** This matches the fiction and the cost. Plants need no pathfinding or animation. Swarms are particle systems. Walking creatures on deformable round terrain are the most expensive thing in the design.
- **Variation by parameters**, not by an editor: a small set of archetypes that differ by biome.

## NPCs

The developers want NPCs (D70); what they are is open. People who trade, give work or tell the story? Machines that help? Survivors on other planets? The developers have said, with "maybe", that you might recruit NPCs from the cultures found on other planets (D86, [worlds.md](worlds.md#breathable-planets-and-a-setting-per-planet)). Each kind asks something different of the engine: a figure that stands and talks is cheap, one that walks a planet and works is close to the cost of a creature (see Hostile life).

## Bases, outposts and defence

From the developers (D90, Leaning): part of survival is establishing bases and outposts and holding them. Recruited NPCs help defend them; built defences, automation, may come later. There is no fail state (D18); neglect costs you progress and equipment. See [living-world.md](living-world.md).

## Harvesting

Leaning, not decided (D43): resources are taken from the world itself. Rock and ore by mining the ground, trees by chopping, grass by cutting. What each yields and what it is for is open, as is whether things regrow and whether a player can strip a planet bare. The engine can afford any answer: see [engine.md](engine.md).

## Open questions

- Can terraforming regress, either locally (a region dries out) or globally (runaway greenhouse)?
- How do four players split work? Is there any role specialisation?
- What do players build with: fixed modular pieces, or something freer?
- What is the end of the game on one planet, and across several?
