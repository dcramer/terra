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
| [aaa-forest-look.md](aaa-forest-look.md) | What makes a forest read as AAA (no pops, bark that is not black, light under the canopy), and what Bevy 0.20 has for it? |
| [forest-lod-and-mood.md](forest-lod-and-mood.md) | How does the Witcher 3 change distant trees' detail without pops, what makes a forest feel like being inside one, and how do The Forest and ARK make their trees? |
| [far-forest.md](far-forest.md) | How do shipped open worlds carry a forest past the trees they draw to the horizon, and what made the lush forest's horizon a dark slab? |
| [2026-10-14-sotf-forest-look.md](2026-10-14-sotf-forest-look.md) | Measured against Sons of the Forest's own daytime forest frames, how do the lush forest's light, colour, ground, leaves and trees differ, and what closes each gap? |
| [2026-10-15-biome-demos.md](2026-10-15-biome-demos.md) | How do shipped games and engines make mountain lakes and tree lines, tundra, and towns bustling with people and animals, what does the real world say each should be, and what in our code stands in the way? |
| [2026-10-15-seasons.md](2026-10-15-seasons.md) | Drawn from the climate, does the lush forest read as the same wood in each season, and what does each look like, measured against Sons of the Forest's summer and snowy forests? |
| [2026-10-15-mountain-lake-reference.md](2026-10-15-mountain-lake-reference.md) | What should the mountain lake demo look like, measured by band against Far Cry 5's valleys and photographs of Morskie Oko, and what do the photographs say of where trees, scree, rock and snow lie? |
| [2026-10-15-tundra-reference.md](2026-10-15-tundra-reference.md) | What should the tundra demo look like, measured by band over photographs of open tundra in autumn, what colours are its lichen, shrub, sedge, moss and snow, and what do The Long Dark and Skyrim show of its composition and wind? |
| [2026-10-15-dragons-and-fire.md](2026-10-15-dragons-and-fire.md) | How do shipped games spread fire through grass, wood and buildings, how does real fire behave on timber and stone, how do games fly a dragon and draw its breath, and how are flames, embers, smoke and their light drawn and capped? |
| [2026-10-16-real-time-fire-rendering.md](2026-10-16-real-time-fire-rendering.md) | What makes real-time fire read as real: what colour and brightness real flames are and why ours go salmon through PBR Neutral, how shipped games shape and move flames, and what numbers our flame, smoke, haze and firelight should use? |
| [2026-10-17-large-flier-motion.md](2026-10-17-large-flier-motion.md) | How do shipped games and films move a large flier so it reads as heavy and smooth: bounded-curvature courses, banking with the turn, the head leading and the tail trailing, flapping against gliding, its speed, its flare into a hover and the tell before a breath? |
| [2026-10-14-scots-pine-crowns.md](2026-10-14-scots-pine-crowns.md) | How do shipped games build a Scots pine's crown so it reads as a pine, not a broadleaf, from the ground at 15 to 30 m, and what does the lush forest as a tall conifer stand measure? |
| [foliage-near-camera.md](foliage-near-camera.md) | How do shipped games keep foliage from filling a third-person camera and from clipping through the player, and its boom from pumping among trunks, and what does doing the same measure on the lush forest's films? |
| [integrated-gpu.md](integrated-gpu.md) | What do the village, yard, world and forest cost on the integrated GPU at the low preset and on the fast GPU at high, and is a section laid and merged in under 2 ms? |
| [2026-10-14-building-fluidity.md](2026-10-14-building-fluidity.md) | As a base is built, changed and knocked down, does everything on screen adjust when it should, with nothing popping, blinking, floating or hitching? What the QA films found |
| [building-audit.md](building-audit.md) | Does every rule of building hold in the game as built (structure, physics, drawing), and does it meet its bars for cost and looks? `cargo xtask rules` |
| [building-drawing.md](building-drawing.md) | What do fifty buildings cost to draw, laid and merged a section at a time on the workers, against drawn whole on the main thread, and do they meet the building plan's bars? |
| [building-scale.md](building-scale.md) | What do three hundred buildings, a castle of 140,000 cubes and bases out to 2 km cost to draw on the fast and the integrated GPU, and where does the time go? |
| [texture-detail-2026-10-15.md](texture-detail-2026-10-15.md) | Why do the construction materials' textures read soft a metre or two off, measured from source scan to pixel, and what do shipped games do about it? |
| [colour-balance-2026-10-15.md](colour-balance-2026-10-15.md) | Measured against a matte grey card at noon and in the evening, is the whole picture warm, and is it the sun, the sky, the grade, the tone mapping or the materials? |
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
| [block-modules.md](block-modules.md) | Can stone built of pieces chosen by their neighbours, cut from Tripo's stone, look as good as whole sections, and how does a displaced stone surface on the cubes compare? |
| [timber-walls.md](timber-walls.md) | Which way of drawing the timber kind from its cubes looks most beautiful and thematic: generated round logs, generated hewn timber, or a timber surface, against today's Tripo logs? |
| [voxel-building-look.md](voxel-building-look.md) | How does Enshrouded draw its building so touching cubes blend and each cube's look changes by its neighbours, how do other good-looking grid builders do it, and what path takes Terra to D211's bar? |
| [role-pieces.md](role-pieces.md) | Which way of giving each exposed face, edge and corner of a cube a look by its role and neighbours brings stone, logs and roofs on par with Enshrouded: role layers and masks, swept profiles and corner pieces, or modules? |
| [goals-review-2026-10-13.md](goals-review-2026-10-13.md) | Does the building system as built match the developers' goals: Enshrouded's voxel approach, building by hand, the cube as the truth, the art guide, 300 buildings? |
| [goals-review-2026-10-14.md](goals-review-2026-10-14.md) | Since the first review, has building kept to the developers' goals: hand-made pieces (D251), building by hand, the cube as the truth, performance at scale? |
| [goals-review-2026-10-15.md](goals-review-2026-10-15.md) | Since the second review: are D251's modelled pieces what the developers asked for, were its findings fixed or papered over, and what is measured at scale? |
| [goals-review-2026-10-15b.md](goals-review-2026-10-15b.md) | Since the third review: is the baseline the developers set (wood and stone, construction and destruction) flawless and performant, is a material simple to add, is any kit customised, and has scope crept? |
| [goals-review-2026-10-16.md](goals-review-2026-10-16.md) | Since the fourth review: does the baseline hold flawless and performant, does the voxel contract hold in the code, is anything customised per kit, has scope crept, and is the code kept organised? |
| [building-workflow-retro-2026-10-16.md](building-workflow-retro-2026-10-16.md) | Across building's work of 13–16 October, where did time, tokens and rework go, and what tools and abstractions would make it easier, safer and faster? |
| [building-tooling-review-2026-10-16.md](building-tooling-review-2026-10-16.md) | Do the tools the building workflow retro asked for work when an agent drives them, cost nothing in play and hold D333's baseline without baggage? |
| [building-code-health-2026-10-15.md](building-code-health-2026-10-15.md) | After many changes, does building carry dead code, data, options or tools, logic in two places, monoliths or rules on the wrong side of a layer, and what is left to tidy, in what order? |
| [kit-review-2026-10-14.md](kit-review-2026-10-14.md) | What visual imperfections are left in the wood and stone kits at 2, 5 and 12 m, noon and evening, that the mesh checker does not catch; which are shared code and which a kit's data? |
| [enshrouded-parity.md](enshrouded-parity.md) | Feature by feature, how does Enshrouded's building compare with ours as built, and what closes each gap on our cube store, operations and layouts without rebuilding them? |
| [uniform-materials.md](uniform-materials.md) | How do shipped games draw every building material one way and swap one fast, and does a material drawn as only its cubes' surface and textures (logs, shakes, thatch) read as well as modelled pieces? |
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
| [melee-feel.md](melee-feel.md) | Why did the troll fight's first film read as slow and weightless, and how do shipped melee games make a blow feel heavy (wind-up, hit-stop, cancels, shake)? |
| [2026-10-17-movement-and-fighting.md](2026-10-17-movement-and-fighting.md) | After five rounds of the troll fight, how do shipped games and engines move and fight a player, people and monsters of every size on one system (root motion, motion matching, blend spaces, IK, procedural and physical layers, attack data, body volumes), and what do we port? |
| [hwfwm-magic.md](hwfwm-magic.md) | How do the magic mechanics of *He Who Fights With Monsters* work (essences, awakening stones, abilities, ranks, skill books, other magic), so we can design our own? |
| [planning-mode-cameras.md](planning-mode-cameras.md) | Which games give building, and planning in particular, a camera other than the character's, and what does each choice cost in a persistent multiplayer world? |

## Adding a note

- One question per note, stated at the top with the date.
- Say what was measured or read, and what was only inferred.
- End with the limits: what the note does not show.
- Link sources.
| [skin-deformation.md](skin-deformation.md) | Why did characters' limbs knot and squeeze in our animations, and what makes them deform correctly through every clip? |
| [2026-10-14-animation.md](2026-10-14-animation.md) | Why does the dressed character's run look wrong and what makes it look right, and how do we animate creatures whose bodies are not a person's without an animator? |
| [2026-10-15-75fps-budget.md](2026-10-15-75fps-budget.md) | Which benchmark scenes hold to D25's 13.3 ms at 75 fps, where is the frame lost, and what would win it back at what cost to the look? |
| [2026-10-15-run.md](2026-10-15-run.md) | Why did the player's jog read as squatting on our bodies, and what fixes its legs, arms and hands without remaking the take? |
| [2026-10-16-far-mountains.md](2026-10-16-far-mountains.md) | What made the range's wall behind the castle look so regular, and how do shipped open worlds give far mountains their structure? |
| [procedural-loot-and-spells.md](procedural-loot-and-spells.md) | How do shipped games generate loot procedurally, and spells, so our essence powers can be built the same way? |
| [enshrouded-combat.md](enshrouded-combat.md) | How does Enshrouded's combat play, now that it is the baseline for ours? |
| [worktree-builds.md](worktree-builds.md) | Agents work in many worktrees, each building Bevy from nothing. What makes a new one quick, and what is safe? |
| [character-systems-industry.md](character-systems-industry.md) | How do shipped games and public tools let players shape a body and wear gear that fits every body, at the level of mechanisms we can rebuild? |
| [bevy-character-capabilities.md](bevy-character-capabilities.md) | What does Bevy 0.19 give a character system, at what cost, and what is ours to build? |
| [bevy-0-20-upgrade.md](bevy-0-20-upgrade.md) | What does moving from Bevy 0.19 to 0.20 take, and does the game look, skin and run the same afterwards? |
| [blender-pipeline-practice.md](blender-pipeline-practice.md) | Is running our asset pipeline as Python scripts in a headless Blender how game studios do it, and what do they do that we do not? |
