# Terra

A survival sandbox across a dozen or so procedural planets, played from the ground with tools in your hands. You start on a world that is easy to live on, then go to others that are not. Some are dead, and can be made to live; there you have to live with what you grew.

This repository is where the project is thought through: design docs, research notes and experiments. It is in pre-production and there is no game to play yet. Everything here is provisional: it records current thinking, and any of it may change.

## Goals

| Goal | More |
|---|---|
| **Explore a dozen or so very different worlds**, each with its own terrain, caverns, water, plants, air, gravity and cultures, reached through portals or something like them. | [docs/worlds.md](docs/worlds.md) |
| **Always get better**: many skills, gained by doing and improved by doing more. | [docs/progression.md](docs/progression.md) |
| **Survive each world**: health, stamina, oxygen, hunger and thirst, which each planet treats differently. | [docs/survival.md](docs/survival.md) |
| **Play as a person in the world**, walking around with tools, gathering and building, in the manner of No Man's Sky or Subnautica. | [docs/player.md](docs/player.md) |
| **Round planets you can dig into**, procedurally generated, with no loading screens within a world. | [docs/engine.md](docs/engine.md) |
| **Take resources from the world itself**: mine the rock, chop the tree, cut the grass. | [docs/engine.md](docs/engine.md#harvesting) |
| **Gear up and carry**: an equipment grid, a backpack that grows, and an action bar of things to use. | [docs/player.md](docs/player.md) |
| **Research new technology** to unlock more to build and craft. | [docs/progression.md](docs/progression.md) |
| **Meet NPCs** on the worlds. | [docs/gameplay.md](docs/gameplay.md) |
| **Build bases you shape**, from a build menu on a tool you equip, as in Subnautica. | [docs/construction.md](docs/construction.md) |
| **A fully procedural world**, computed from rules and very cheap to compute, send and store. | [docs/engine.md](docs/engine.md) |
| **Look like a carefully lit model**: simple shapes, flat colour, warm light and cool shadow, and never chunky. | [docs/art.md](docs/art.md) |
| **A thousand players on one persistent server**, with a few friends playing together as the small case. | [docs/multiplayer.md](docs/multiplayer.md) |
| **Stay fast with very many things on screen**, any of which the player can interact with. | [research/forest-scale-test.md](research/forest-scale-test.md) |
| **Write everything in Rust**, on Bevy, with the game modelled in ECS and our own voxel terrain. | [docs/tech.md](docs/tech.md) |
| **Be buildable by two people working with coding agents**, with every asset free, bought or generated, developer tools to refine each system, and QA the agents can run themselves. | [docs/development.md](docs/development.md) |
| **Playable by an agent alone**, headless, through an MCP server. | [docs/multiplayer.md](docs/multiplayer.md#agents-as-players) |
| **Design before building**, and write down what was decided and why. | [docs/README.md](docs/README.md) |

How firm each of these is, and who decided it, is in [docs/decisions.md](docs/decisions.md).

## Planned

What the developers want in the game. **None of it is built yet.** A few pieces have stand-ins in the [sandbox](crates/terra_sandbox) to play with, marked *(stand-in)*.

**Worlds** ([worlds.md](docs/worlds.md))
- Procedurally generated terrain on real, round planets.
- About a dozen planets, each different, mostly in the obvious ways; perhaps more over time.
- Always round and finite, but large: walking all the way round should be ridiculous. About 30 km radius proposed for the starter planet.
- Thematic variety on each: terrain, caverns, water features, foliage, palette.
- A starter planet that is easy: normal, breathable air.
- Gravity, toward each planet's centre.
- A sea level on each planet with water, behaving as physics says: level all round, filling what the sea can reach, as deep as the ground makes it.
- Digging into the ground, as in No Man's Sky.
- Travelling between worlds, probably through portals. No space.
- Weather, different on each world.
- Probably no terraforming: the notes are kept ([planet-sim.md](docs/planet-sim.md)) but it is not a concern for now.

**Surviving** ([survival.md](docs/survival.md))
- Survival attributes: health, stamina, oxygen, hunger, thirst, and perhaps more. Each world can treat them differently.
- Swimming, at any depth, using stamina. Under water you use up oxygen, as on a planet with thin or no air.

**The player and their gear** ([player.md](docs/player.md))
- An equipment grid: a character sheet.
- A space suit made of parts, such as a suit and an oxygen tank, for breathing where you otherwise could not.
- An inventory that is your backpack: small at first, bigger with a bigger backpack in the backpack slot *(stand-in: raw items as counts)*.
- An action bar of things to use *(stand-in)*.
- Tools for gathering, such as different kinds of cutter *(stand-in: one cutter)*.
- A map. What it shows and its limits are not decided.
- An options menu with the usual settings, including audio and key remapping.

**Building** ([construction.md](docs/construction.md))
- A build menu brought up by a building tool you equip, as in Subnautica *(stand-in: a dial)*.
- Rooms you shape on a half-metre grid, interior walls, windows and doors *(stand-in)*.
- Pieces whose look changes with what they connect to, such as rounded corners; fixed-size buildings such as round rooms, joined by connectors; materials.
- Building, taking apart and cutting take time: hold to complete *(stand-in)*.

**Progress** ([progression.md](docs/progression.md))
- A research system that unlocks technology and things to craft and build. How you research is not decided.

**People**
- NPCs ([gameplay.md](docs/gameplay.md#npcs)).
- A thousand players on one persistent server ([multiplayer.md](docs/multiplayer.md)).
- Playing headless, as an agent only, through an MCP server ([multiplayer.md](docs/multiplayer.md#agents-as-players)).

**How it is built** ([tech.md](docs/tech.md), [development.md](docs/development.md))
- The game modelled in ECS.
- QA our coding agents can run: drive the game, look at it, ask it what is true *(stand-in: scripted runs)*.
- Developer tools for refining each system, starting with a model workbench.
- An in-game debug menu behind a launch flag, opened with the backtick key, for things like warping.
- Frame times on screen at all times against the budget, and logs that record how each session ran *(built in the sandbox)*.

## What is here

| Folder | What it is |
|---|---|
| [docs](docs) | Decisions and design docs. Start with [docs/decisions.md](docs/decisions.md). |
| [research](research) | What we looked into and found, one dated note per question. |
| [crates](crates) | The Rust workspace, including the [sandbox](crates/terra_sandbox) where experiments run. |
| [demo](demo) | An earlier playable demo on Godot, kept as a reference. |
| [prototypes](prototypes) | A globe for watching the planet simulation, on Godot. |
| [spike](spike) | Throwaway experiments. |

To set up a machine, see [docs/development.md](docs/development.md). Coding agents should read [AGENTS.md](AGENTS.md).

## Licence

[MIT](LICENSE). Code adapted from other projects is listed in [THIRD_PARTY.md](THIRD_PARTY.md).
