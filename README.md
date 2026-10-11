# Terra

A sandbox about making a dead planet live, played from the ground with tools in your hands. You land on a world that kills you by being dead, make it live, and then have to live with what you grew.

This repository is where the project is thought through: design docs, research notes and experiments. It is in pre-production and there is no game to play yet. Everything here is provisional: it records current thinking, and any of it may change.

## Goals

| Goal | More |
|---|---|
| **Turn a dead planet into a living one**, and then contend with the life you made. | [docs/vision.md](docs/vision.md) |
| **Play as a person in the world**, walking around with tools, gathering and building, in the manner of No Man's Sky or Subnautica. | [docs/player.md](docs/player.md) |
| **Real round planets you can dig into**, with no seam between standing on the ground and flying in space. | [docs/engine.md](docs/engine.md) |
| **Take resources from the world itself**: mine the rock, chop the tree, cut the grass. | [docs/engine.md](docs/engine.md#harvesting) |
| **Build bases you shape**, not assemble from fixed parts. | [research/subnautica-2-building.md](research/subnautica-2-building.md) |
| **A fully procedural world**, computed from rules and very cheap to compute, send and store. | [docs/engine.md](docs/engine.md) |
| **Look like a carefully lit model**: simple shapes, flat colour, warm light and cool shadow, and never chunky. | [docs/art.md](docs/art.md) |
| **A thousand players on one persistent server**, with a few friends playing together as the small case. | [docs/multiplayer.md](docs/multiplayer.md) |
| **Stay fast with very many things on screen**, any of which the player can interact with. | [research/forest-scale-test.md](research/forest-scale-test.md) |
| **Write everything in Rust**, on Bevy, with our own voxel terrain. | [docs/tech.md](docs/tech.md) |
| **Be buildable by two people working with coding agents**, with every asset free, bought or generated. | [docs/decisions.md](docs/decisions.md) |
| **Design before building**, and write down what was decided and why. | [docs/README.md](docs/README.md) |

How firm each of these is, and who decided it, is in [docs/decisions.md](docs/decisions.md).

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
