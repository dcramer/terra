# Terra design docs

Working title: **Terra**. A multiplayer survival sandbox across a dozen or so procedural worlds, reached through portals.

The source of truth, then the docs about the game as a whole, then one doc per part of the game, then the engine and how we work.

| Doc | What it covers |
|---|---|
| [decisions.md](decisions.md) | What is settled, what is a working assumption, what is open. Source of truth. |

**The game as a whole**

| Doc | What it covers |
|---|---|
| [vision.md](vision.md) | Pitch, pillars, our inspirations (Kenshi, Enshrouded, Subnautica 2, *He Who Fights With Monsters*, Soulmask), non-goals. |
| [proposals](proposals/README.md) | Numbered proposals, each one candidate. So far one for the whole game: [game design 001, a living world](proposals/game-design-001.md) (working name), in the spirit of Enshrouded and *He Who Fights With Monsters*. |

**Parts of the game**

| Doc | What it covers |
|---|---|
| [worlds.md](worlds.md) | The worlds: themes, cultures, terrain, caverns, water, foliage, gravity, the starter planet, portals between them. |
| [gameplay.md](gameplay.md) | Core loops, survival needs, hostile life, bases and defence; terraforming phases, parked. Older than game design 001, whose loops would replace these if adopted. |
| [survival.md](survival.md) | Draft: the survival attributes (health, stamina, oxygen, hunger, thirst) and how they vary by world. |
| [player.md](player.md) | Draft: the player's body, tools, equipment, backpack and action bar, acting on the world, building. Open questions for the developers. |
| [items.md](items.md) | Items and containers: equipment, backpack, action bar and storage, on a Subnautica baseline, and how they work in code. |
| [construction.md](construction.md) | Draft: what the developers want from building, a proposed shape for it, and open questions. |
| [progression.md](progression.md) | Draft: player skills, the research system, and how things are unlocked. Mostly open questions. |
| [multiplayer.md](multiplayer.md) | Server-first multiplayer, built toward a thousand players on one persistent server. |
| [planet-sim.md](planet-sim.md) | Parked with terraforming: the planetary simulation, cells, fields, how local feeds global. |
| [art.md](art.md) | Art direction, constraints, the beauty-corner test. |

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
