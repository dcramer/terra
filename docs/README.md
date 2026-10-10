# Terra design docs

Working title: **Terra**. A co-op survival sandbox about terraforming a dead planet.

| Doc | What it covers |
|---|---|
| [decisions.md](decisions.md) | What is settled, what is a working assumption, what is open. Source of truth. |
| [vision.md](vision.md) | Pitch, pillars, what we take from each inspiration, non-goals. |
| [gameplay.md](gameplay.md) | Core loops, survival needs, terraforming phases, hostile life, automation. |
| [player.md](player.md) | Draft: the player's body, tools, acting on the world, carrying, building. Open questions for the developers. |
| [multiplayer.md](multiplayer.md) | Draft: what four-player co-op and a hundred players on a persistent server each mean, and a path that keeps both open. |
| [planet-sim.md](planet-sim.md) | The planetary simulation: cells, fields, biomes, how local feeds global. |
| [tech.md](tech.md) | The stack, how the code is split, how co-op shares state, terrain, workflow, risks. |
| [engine.md](engine.md) | The engine's parts, what each needs, and the parallel tracks they are built in. |
| [development.md](development.md) | Setting up a machine, everyday commands, seeing and measuring output. |
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
