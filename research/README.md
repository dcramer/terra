# Research

What we looked into and what we found: surveys, measurements and experiments. Each note is dated and says what question it answers.

Plans and decisions live in [docs](../docs). A finding here does not change the plan until a decision in [docs/decisions.md](../docs/decisions.md) says so.

| Note | Question |
|---|---|
| [rust-stack-spike.md](rust-stack-spike.md) | Do Bevy and the transvoxel crate build and run from our machines, and at what cost? |
| [rust-voxel-ecosystem.md](rust-voxel-ecosystem.md) | What exists in Rust to build a voxel planet game on? |
| [voxel-tools-port.md](voxel-tools-port.md) | How much of Voxel Tools would we need to port, and what makes that tractable? |
| [forest-scale-test.md](forest-scale-test.md) | Can the world hold a very large number of objects that are each interactive? |
| [subnautica-2-building.md](subnautica-2-building.md) | How does Subnautica 2's construction work, and what would the same idea look like here? |
| [subnautica-inventory.md](subnautica-inventory.md) | How do items and containers work in Subnautica? |
| [enshrouded.md](enshrouded.md) | What do Enshrouded's voxel terrain and building do, and where do they meet our decisions? |
| [reference-games.md](reference-games.md) | Which existing games could game design 001 be based on, so its core mechanics are known to be fun? |
| [outpost-scene.md](outpost-scene.md) | With no artist, how far do free models, our own colours and the engine's built-in effects get? |
| [character-creators.md](character-creators.md) | How do the best character creators and equipment systems work, with Enshrouded as the model, and what fits Terra? |
| [ai-models-for-art.md](ai-models-for-art.md) | With no artists, can an agent make our models, and which AI tools, free or paid, make good 3D art? |
| [tripo-studio-credits.md](tripo-studio-credits.md) | Can scripts spend our Tripo subscription's credits instead of API credits? |
| [tripo-studio-api.md](tripo-studio-api.md) | What does each Studio operation take and return, so a script can do everything the web app does? |
| [agents-playing-games.md](agents-playing-games.md) | How do AI agents play games, from the screen or through the game's state, and what would make them efficient at Terra? |
| [godot-spike.md](godot-spike.md) | What did the Godot build prove, and what are the numbers to beat? |
| [terrain-meshing.md](terrain-meshing.md) | Can `terra_voxel` mesh a real planet's chunk within budget on Linux and Windows, and what stands in the way? |
| [item-icons.md](item-icons.md) | How do games make the icons of items in slots, and which way keeps one style across procedural variants? |
| [agent-debugging-tools.md](agent-debugging-tools.md) | What tools and patterns help a coding agent debug and check a game, and what should we build or adopt? |
| [repo-layout.md](repo-layout.md) | Where should each kind of thing in this repository live, what do our folders actually hold, and what does the industry call these things? |
| [one-mesh-for-every-body.md](one-mesh-for-every-body.md) | How do we fit every item to every body and template without a model per pair, and still look good? |

## Adding a note

- One question per note, stated at the top with the date.
- Say what was measured or read, and what was only inferred.
- End with the limits: what the note does not show.
- Link sources.
