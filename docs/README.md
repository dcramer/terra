# Terra design docs

Working title: **Terra**. A multiplayer survival sandbox across a dozen or so procedural worlds, reached through portals.

| Doc | What it covers |
|---|---|
| [decisions.md](decisions.md) | What is settled, what is a working assumption, what is open. Source of truth. |
| [vision.md](vision.md) | Pitch, pillars, what we take from each inspiration, non-goals. |
| [worlds.md](worlds.md) | The worlds: themes, cultures, terrain, caverns, water, foliage, gravity, the starter planet, portals between them. |
| [survival.md](survival.md) | Draft: the survival attributes (health, stamina, oxygen, hunger, thirst) and how they vary by world. |
| [progression.md](progression.md) | Draft: player skills, the research system, and how things are unlocked. Mostly open questions. |
| [gameplay.md](gameplay.md) | Core loops, survival needs, hostile life, automation; terraforming phases, parked. |
| [living-world.md](living-world.md) | Draft: the direction being considered, a living world in the spirit of Enshrouded and *He Who Fights With Monsters*. Proposals and questions for the developers. |
| [player.md](player.md) | Draft: the player's body, tools, equipment, backpack and action bar, acting on the world, building. Open questions for the developers. |
| [items.md](items.md) | Items and containers: equipment, backpack, action bar and storage, on a Subnautica baseline, and how they work in code. |
| [construction.md](construction.md) | Draft: what the developers want from building, a proposed shape for it, and open questions. |
| [multiplayer.md](multiplayer.md) | Server-first multiplayer, built toward a thousand players on one persistent server. |
| [planet-sim.md](planet-sim.md) | The planetary simulation: cells, fields, biomes, how local feeds global. |
| [tech.md](tech.md) | The stack, how the code is split, how co-op shares state, terrain, workflow, risks. |
| [engine.md](engine.md) | The engine's parts, what each needs, and the parallel tracks they are built in. |
| [development.md](development.md) | Setting up a machine, everyday commands, seeing and measuring output, developer tools, and QA our agents can run. |
| [terrain.md](terrain.md) | Brief for the first component, the voxel terrain crate. |
| [art.md](art.md) | Art direction, constraints, the beauty-corner test. |
| [prototypes.md](prototypes.md) | The builds so far and the milestones ahead, with their pass criteria. |

Findings, surveys and measurements live in [research](../research), not here. These docs say what we decided; research says what we found.

## How we keep these useful

- **decisions.md wins.** If another doc disagrees with it, the other doc is wrong; fix it.
- **Every claim has a status.** *Decided* means don't reopen without a reason. *Leaning* means a working assumption that is untested or unconfirmed. *Open* means nobody has chosen yet.
- **Change the doc in the same change as the thing it describes.** A decision made in chat that isn't written here didn't happen.
- **Keep them short.** These describe intent and constraints, not implementation. Code explains itself.
- **Record why.** A decision without its reason gets relitigated.
