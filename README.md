# Terra

**An extremely ambitious sandbox engine**: multitudes of procedural worlds, realms reached through portals, each run by intricate systems for every component of the game (ground, water, climate, plants, creatures, people and factions, magic, items, building) that act on one another. The game is what those systems make together: survival and RPG in a living world, in the spirit of Kenshi and Enshrouded, on lore from *He Who Fights With Monsters*'s world-building, for up to a thousand players on one persistent server. Magic pools and makes monsters, settlements live with it, and you hold ground with your tribe and grow by rank. The design is [docs/gameplay.md](docs/gameplay.md); why and how ambitious is [docs/vision.md](docs/vision.md).

It is in pre-production. Everything here records current thinking, and any of it may change.

## Goals

| Goal | More |
|---|---|
| **Systems all the way down**: every part of a world is a system that works on any world from data, and the systems act on one another. | [docs/vision.md](docs/vision.md) |
| **Many very different worlds**, each with its own terrain, caverns, water, plants, air, gravity, seasons and cultures, made from rules and a seed. | [docs/worlds.md](docs/worlds.md), [docs/settings.md](docs/settings.md) |
| **A living world**: monsters form where magic pools, animals roam and hunt, settlements trade, grow and fall, whether or not you are there. | [docs/gameplay.md](docs/gameplay.md) |
| **Always get better**: skills by doing, techniques by study, powers from essences, ranks over all of them. | [docs/progression.md](docs/progression.md), [docs/magic.md](docs/magic.md) |
| **Survive each world**: health, stamina, mana, breath, hunger, thirst and warmth, against seasons and weather. | [docs/survival.md](docs/survival.md), [docs/climate.md](docs/climate.md) |
| **Hold your ground**: claims, buildings of walls, floors and roofs, and a tribe of recruits who work and defend on their own. | [docs/construction.md](docs/construction.md), [docs/tribe.md](docs/tribe.md) |
| **Fight like an action RPG**, with Enshrouded as the baseline. | [docs/combat.md](docs/combat.md) |
| **One art style for every realm**: bold simple shapes, flat colour by palette role, shading left to the light. | [docs/art-style.md](docs/art-style.md) |
| **A thousand players on one persistent server**, with a few friends as the small case. | [docs/multiplayer.md](docs/multiplayer.md) |
| **Built by two people and their coding agents**: Rust on Bevy, our own voxel terrain, everything procedural and data-driven, and QA the agents run themselves. | [docs/tech.md](docs/tech.md), [docs/development.md](docs/development.md) |

How firm each of these is, and who decided it, is in [docs/decisions.md](docs/decisions.md). What is built so far is in [docs/engine.md](docs/engine.md).

## What is here

| Folder | What it is |
|---|---|
| [docs](docs) | Decisions and design docs. Start with [docs/README.md](docs/README.md). |
| [research](research) | What we looked into and found, one dated note per question. |
| [assets](assets) | What the game loads: models, sounds, shaders, icons and data. Credits in [THIRD_PARTY.md](THIRD_PARTY.md). |
| [art](art) | Source art the asset pipeline turns into the game's files. |
| [crates](crates) | The Rust workspace: the game, its engine-free rules, and the [sandbox](crates/terra_sandbox) of testbeds. |
| [tools](tools) | Team tooling outside the game. |
| [spike](spike) | Throwaway experiments. |

To play the current build, on the real GPU whatever the machine:

```bash
cargo xtask play
```

`cargo xtask play --fresh` starts a new game; `cargo xtask tools` lists the other tasks. To set up a machine, see [docs/development.md](docs/development.md). Coding agents should read [AGENTS.md](AGENTS.md).

## Licence

[MIT](LICENSE). Code adapted from other projects is listed in [THIRD_PARTY.md](THIRD_PARTY.md).
