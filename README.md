# Terra

A co-op survival sandbox about terraforming a dead planet, for up to four players. You land on a world that kills you by being dead, make it live, and then have to live with what you grew.

This repository is where the project is thought through: design docs, research notes and tech demos. It is in pre-production and there is no game to play yet.

| Folder | What it is |
|---|---|
| [docs](docs) | Decisions and plans. Start with [docs/decisions.md](docs/decisions.md). |
| [research](research) | What we looked into and found, one dated note per question. |
| [demo](demo) | A playable tech demo: walk a voxel planet, dig, terraform, fly to orbit. Built on Godot as a reference. |
| [prototypes](prototypes) | The planet simulation and a globe to watch it on. |
| [spike](spike) | Throwaway experiments. |

The plan is to build everything in Rust on Bevy, with our own voxel terrain. See [docs/tech.md](docs/tech.md).
