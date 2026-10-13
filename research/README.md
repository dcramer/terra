# Research

What we looked into and what we found: surveys, measurements and experiments. Each note is dated and says what question it answers.

Plans and decisions live in [docs](../docs). A finding here does not change the plan until a decision in [docs/decisions.md](../docs/decisions.md) says so.

| Note | Question |
|---|---|
| [rust-stack-spike.md](rust-stack-spike.md) | Do Bevy and the transvoxel crate build and run from our machines, and at what cost? |
| [rust-voxel-ecosystem.md](rust-voxel-ecosystem.md) | What exists in Rust to build a voxel planet game on? |
| [voxel-tools-port.md](voxel-tools-port.md) | How much of Voxel Tools would we need to port, and what makes that tractable? |
| [forest-scale-test.md](forest-scale-test.md) | Can the world hold a very large number of objects that are each interactive? |
| [lush-forest-spike.md](lush-forest-spike.md) | Can the world feel like a dense, rich forest, as The Witcher 3 or Sons of the Forest do, within the frame budget? How big games make foliage fast. |
| [benchmarks.csv](benchmarks.csv) | Frame times of the benchmark scenes, recorded by `cargo xtask bench --record` (D185) |
| [subnautica-2-building.md](subnautica-2-building.md) | How does Subnautica 2's construction work, and what would the same idea look like here? |
| [subnautica-inventory.md](subnautica-inventory.md) | How do items and containers work in Subnautica? |
| [enshrouded.md](enshrouded.md) | What do Enshrouded's voxel terrain and building do, and where do they meet our decisions? |
| [reference-games.md](reference-games.md) | Which existing games could game design 001 be based on, so its core mechanics are known to be fun? |
| [outpost-scene.md](outpost-scene.md) | With no artist, how far do free models, our own colours and the engine's built-in effects get? |
| [character-creators.md](character-creators.md) | How do the best character creators and equipment systems work, with Enshrouded as the model, and what fits Terra? |
| [ai-models-for-art.md](ai-models-for-art.md) | With no artists, can an agent make our models, and which AI tools, free or paid, make good 3D art? |
| [art-explorations.md](art-explorations.md) | What looks did we try or point at before direction D, and what did each teach? |
| [realistic-building-textures.md](realistic-building-textures.md) | Do building pieces look real enough, and less cartoony, with a realistic concept and Tripo's textures kept? |
| [adjacency-pieces.md](adjacency-pieces.md) | How do games draw each cell of a grid from its neighbours, as Enshrouded's building does, and which way suits our cubes, ghosts and models? |
| [tripo-studio-credits.md](tripo-studio-credits.md) | Can scripts spend our Tripo subscription's credits instead of API credits? |
| [tripo-studio-api.md](tripo-studio-api.md) | What does each Studio operation take and return, so a script can do everything the web app does? |
| [agents-playing-games.md](agents-playing-games.md) | How do AI agents play games, from the screen or through the game's state, and what would make them efficient at Terra? |
| [godot-spike.md](godot-spike.md) | What did the Godot build prove, and what are the numbers to beat? |
| [terrain-meshing.md](terrain-meshing.md) | Can `terra_voxel` mesh a real planet's chunk within budget on Linux and Windows, and what stands in the way? |
| [item-icons.md](item-icons.md) | How do games make the icons of items in slots, and which way keeps one style across procedural variants? |
| [agent-debugging-tools.md](agent-debugging-tools.md) | What tools and patterns help a coding agent debug and check a game, and what should we build or adopt? |
| [repo-layout.md](repo-layout.md) | Where should each kind of thing in this repository live, what do our folders actually hold, and what does the industry call these things? |
| [one-mesh-for-every-body.md](one-mesh-for-every-body.md) | How do we fit every item to every body and template without a model per pair, and still look good? |
| [modern-character-variety.md](modern-character-variety.md) | In modern games with deep creators or varied crowds, how are body variety and worn gear combined, and what is authored per body versus once? |
| [body-morphs.md](body-morphs.md) | Can build and sex morphs be made from designs of the base person, and does gear made once pass on every morph? |
| [gear-every-clip.md](gear-every-clip.md) | Do the realistic bodies and every worn item pass when the checks play every clip the game plays, and what fixes each failure? |
| [agents-and-blender.md](agents-and-blender.md) | What is the current outside advice on agents in Blender, with skeletons and generated models, and are our playbooks current? |
| [character-pipeline-audit.md](character-pipeline-audit.md) | Across the character pipeline, what is done as the industry does it, what is a bug, and what is duct tape? |
| [ecs-audit.md](ecs-audit.md) | Does the game's code keep to our ECS rule and to many of everything, and is ECS the right model for a thousand players? |
| [building-systems-prior-art.md](building-systems-prior-art.md) | How do the building systems of Enshrouded, Rust, Ark, Subnautica and The Forest work, from keys to claims, so ours misses nothing players expect? |
| [procedural-vegetation.md](procedural-vegetation.md) | How do shipped games place and draw vegetation over large procedural terrain so it reads full and organic, and what does Bevy give us toward it? |
| [procedural-buildings.md](procedural-buildings.md) | How do games turn a drawn plan into a detailed building (roofs over any footprint, walls from a kit, drawing many pieces, building piece by piece, indoors), and how hard is each for us? |
| [recruits-at-the-base.md](recruits-at-the-base.md) | How do shipped games handle recruits who live and work at your base, and which work without the player managing each one? |
| [web-build.md](web-build.md) | Can the sandbox's tools run in a web browser on the game's own renderer, opened and controlled by a link? |
| [hwfwm-magic.md](hwfwm-magic.md) | How do the magic mechanics of *He Who Fights With Monsters* work (essences, awakening stones, abilities, ranks, skill books, other magic), so we can design our own? |
| [planning-mode-cameras.md](planning-mode-cameras.md) | Which games give building, and planning in particular, a camera other than the character's, and what does each choice cost in a persistent multiplayer world? |

## Adding a note

- One question per note, stated at the top with the date.
- Say what was measured or read, and what was only inferred.
- End with the limits: what the note does not show.
- Link sources.
| [skin-deformation.md](skin-deformation.md) | Why did characters' limbs knot and squeeze in our animations, and what makes them deform correctly through every clip? |
| [procedural-loot-and-spells.md](procedural-loot-and-spells.md) | How do shipped games generate loot procedurally, and spells, so our essence powers can be built the same way? |
| [enshrouded-combat.md](enshrouded-combat.md) | How does Enshrouded's combat play, now that it is the baseline for ours? |
| [worktree-builds.md](worktree-builds.md) | Agents work in many worktrees, each building Bevy from nothing. What makes a new one quick, and what is safe? |
| [character-systems-industry.md](character-systems-industry.md) | How do shipped games and public tools let players shape a body and wear gear that fits every body, at the level of mechanisms we can rebuild? |
| [bevy-character-capabilities.md](bevy-character-capabilities.md) | What does Bevy 0.19 give a character system, at what cost, and what is ours to build? |
| [bevy-0-20-upgrade.md](bevy-0-20-upgrade.md) | What does moving from Bevy 0.19 to 0.20 take, and does the game look, skin and run the same afterwards? |
