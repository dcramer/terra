# Gameplay

Status: draft. The tone and the dead-to-alive arc are Decided; the specifics below are Leaning until a prototype tests them.

## Core loops

| Loop | What the player does |
|---|---|
| 30 seconds | Move, mine, place, watch a meter (oxygen first). |
| 10 minutes | Leave base for a resource, return, build a machine, see a local value move. |
| Session | Cross a regional or global threshold. The world visibly changes, a new threat appears, new tech unlocks. |

If the 30-second loop isn't fun in a grey-box build, content won't fix it.

## Survival needs

Oxygen, water, food, temperature, radiation. Each one follows the same arc:

1. **Manual**: carry it, ration it, run back to base.
2. **Local infrastructure**: a machine or a pipe covers your base.
3. **Solved by the planet**: terraforming removes the need (breathable air, drinkable water).

## Terraforming phases

Draft names and order. Phases are gated by global values; what a region looks like is set by its local values. See [planet-sim.md](planet-sim.md).

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

## Automation and defence

Every threat has a built answer: oxygen lines, heaters, perimeter walls, turrets, flora suppressors. A well-built base survives without attention. There is no fail state; neglect costs you progress and equipment.

## Harvesting

Leaning, not decided (D43): resources are taken from the world itself. Rock and ore by mining the ground, trees by chopping, grass by cutting. What each yields and what it is for is open.

## Open questions

- Can terraforming regress, either locally (a region dries out) or globally (runaway greenhouse)?
- How do four players split work? Is there any role specialisation?
- What do players build with: fixed modular pieces, or something freer?
- What is the end of the game on one planet?
