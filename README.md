# Terra

**First and foremost, we are building an extremely ambitious sandbox engine, with multitudes of procedural worlds and intricate systems for each component of the game.** The game is a multiplayer survival sandbox with RPG in it, across realms reached through portals, in the spirit of Kenshi and Enshrouded, on lore from *He Who Fights With Monsters*'s world-building, for up to a thousand players on one persistent server. Magic pools and makes monsters, settlements live with it, and you hold ground with your tribe and grow by rank. The design is [docs/gameplay.md](docs/gameplay.md).

It is in pre-production. Everything here records current thinking, and any of it may change.

## Goals

| Goal | More |
|---|---|
| **Many very different worlds**, each with its own terrain, caverns, water, plants, air, gravity, seasons and cultures, made from rules and a seed. | [docs/worlds.md](docs/worlds.md), [docs/settings.md](docs/settings.md) |
| **A living world**: monsters form where magic pools, animals roam and hunt, settlements trade, grow and fall, whether or not you are there. | [docs/gameplay.md](docs/gameplay.md) |
| **Always get better**: skills by doing, techniques by study, powers from essences, ranks over all of them. | [docs/progression.md](docs/progression.md), [docs/magic.md](docs/magic.md) |
| **Survive each world**: health, stamina, mana, breath, hunger, thirst and warmth, against seasons and weather. | [docs/survival.md](docs/survival.md), [docs/climate.md](docs/climate.md) |
| **Hold your ground**: claims, buildings of walls, floors and roofs, and a tribe of recruits who work and defend on their own. | [docs/building.md](docs/building.md), [docs/tribe.md](docs/tribe.md) |
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

To get a new machine ready and play, on the real GPU whatever the machine:

```bash
make setup   # once: installs Rust and what it needs, builds the game, checks the machine
make play
```

`make` lists the everyday tasks, such as `make fresh` for a new game and `make update` for the latest; they run `cargo xtask` tasks, and `cargo xtask tools` lists all of those. More on setting up a machine is in [docs/development.md](docs/development.md). Coding agents should read [AGENTS.md](AGENTS.md).

## Licence

[MIT](LICENSE). Code adapted from other projects is listed in [THIRD_PARTY.md](THIRD_PARTY.md).
