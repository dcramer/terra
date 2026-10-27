# Research

What we looked into and what we found: surveys, measurements and experiments. Each note is dated and says what question it answers.

Plans and decisions live in [docs](../docs). A finding here does not change the plan until a decision in [docs/decisions.md](../docs/decisions.md) says so.

[benchmarks.csv](benchmarks.csv) holds the frame times of the benchmark scenes, recorded by `cargo xtask bench --record` (D185).

The table is rendered from the notes themselves (D409): each note's title and the date at its top. Add a note and run `node tools/docs/docs.ts --render`; never edit the table.

<!-- rendered from research/ by `node tools/docs/docs.ts --render`: edit that, not this -->

| Note | Dated |
|---|---|
| [Layered clothing: how shipped games put a shirt with trousers, tucked or not](2026-10-27-layered-clothing.md) | 2026-10-27 |
| [Animation sourcing: a library, a service or a suit for every clip, under a licence a public repository can hold](2026-10-26-animation-sourcing.md) | 2026-10-26 |
| [How do major games theme and scale their dungeons, and which themes should Terra's have?](2026-10-26-dungeon-themes.md) | 2026-10-26 |
| [Selling a slow giant's weight: what shipped work does, and the Cetri's walk measured against it](2026-10-26-giant-walker-weight.md) | 2026-10-26 |
| [The head on the aim while the body swings: how shipped engines lay a look-at after the clips](2026-10-26-head-look-at.md) | 2026-10-26 |
| [The Bevy volumetric clouds branch measured against the sky's budget](2026-10-26-volumetric-clouds-spike.md) | 2026-10-26 |
| [Volumetric clouds and a real sky: how games and engines draw them](2026-10-26-volumetric-skies.md) | 2026-10-26 |
| [Climbing in shipped games and engines: what is climbable, what it costs, how the body follows the face](2026-10-25-climbing.md) | 2026-10-25 |
| [Do vaults and arched niches lift the crypt demo's score?](2026-10-25-dungeon-vaults.md) | 2026-10-25 |
| [Can a fluid-simulated campfire move as real ones do?](2026-10-25-fire-fluid-sim.md) | 2026-10-25 |
| [How real campfires move, measured against ours](2026-10-25-fire-motion.md) | 2026-10-25 |
| [Why the forest walk's summer night was black](2026-10-25-forest-night.md) | 2026-10-25 |
| [Loadouts and animation layers: how shipped games put items in two hands and move every body with them](2026-10-25-loadouts-and-animation-layers.md) | 2026-10-25 |
| [The shield arm in a swing: what shipped sword-and-shield sets do with the shield while the sword cuts](2026-10-25-shield-arm-in-a-swing.md) | 2026-10-25 |
| [The shield in a blow, frame by frame: what floats in ours, and where shipped games and historical fighters carry it](2026-10-25-shield-prior-art.md) | 2026-10-25 |
| [Space Engineers 2's terrain, mining and building, against ours](2026-10-25-space-engineers-2.md) | 2026-10-25 |
| [Weathering a building by wave function collapse: a blind test against noise masks](2026-10-25-wfc-weathering.md) | 2026-10-25 |
| [What a landed hit shows: the contact-point burst, not a whole-body flash](2026-10-25-what-a-landed-hit-shows.md) | 2026-10-25 |
| [Campfire history panel: which rounds since 10-21 made it look worse from the camp view](2026-10-24-campfire-history-panel.md) | 2026-10-24 |
| [Campfire as one whole-fire sim on a few cards: a blind test](2026-10-24-campfire-one-sim.md) | 2026-10-24 |
| [Character controller: Jolt's CharacterVirtual and Godot's CharacterBody3D](2026-10-24-character-controller.md) | 2026-10-24 |
| [Combat clip sources: a bake-off scored by the move check](2026-10-24-combat-clip-sources.md) | 2026-10-24 |
| [Enshrouded's melee, measured](2026-10-24-enshrouded-combat-measured.md) | 2026-10-24 |
| [Campfire as one body of flame: how shipped games lay it on the logs](2026-10-23-campfire-one-body.md) | 2026-10-23 |
| [Creature rigs: how shipped games make many creatures move well](2026-10-22-creature-rig-pipeline.md) | 2026-10-22 |
| [Far terrain shadows and the lake's off-screen reflection: how shipped engines shade land past the shadow map (2026-10-22)](2026-10-22-far-terrain-shadows.md) | 2026-10-22 |
| [Fire review calibration: what AAA frames score from our harsh reviewer](2026-10-22-fire-review-calibration.md) | 2026-10-22 |
| [Far mountain detail: how shipped games make a wall 1 to 5 km off read as rock (2026-10-21)](2026-10-21-far-mountain-detail.md) | 2026-10-21 |
| [Agent throughput: where the hours went](2026-10-19-agent-throughput.md) | 2026-10-19 |
| [Fire step back: a professional flipbook in our engine](2026-10-19-fire-step-back.md) | 2026-10-19 |
| [How games draw fire: four surveys combined](2026-10-19-how-games-draw-fire.md) | 2026-10-19 |
| [Mountains that look proper: ours against real ranges and shipped games (2026-10-19)](2026-10-19-mountain-look.md) | 2026-10-19 |
| [A raymarched breath for the cinematic tier](2026-10-19-raymarched-breath.md) | 2026-10-19 |
| [Winter that reads: how shipped games draw a snowy forest](2026-10-19-winter-aaa.md) | 2026-10-19 |
| [Binary art out of git history](2026-10-18-binary-art-out-of-history.md) | 2026-10-18 |
| [How the crates should be cut](2026-10-18-crate-layout.md) | 2026-10-18 |
| [Dragon breath and a burning castle: what makes them read as real](2026-10-18-dragon-breath.md) | 2026-10-18 |
| [How structures collapse, in games and in the world](2026-10-18-how-structures-collapse.md) | 2026-10-18 |
| [Recurring film defects: what came back, how often, and how engines stop it](2026-10-18-recurring-film-defects.md) | 2026-10-18 |
| [Storms and winter: how games draw weather](2026-10-18-storms-and-winter.md) | 2026-10-18 |
| [Large fliers: how games and films make a heavy flier move smoothly](2026-10-17-large-flier-motion.md) | 2026-10-17 |
| [How shipped games move and fight their characters and monsters](2026-10-17-movement-and-fighting.md) | 2026-10-17 |
| [How shipped melee makes a blow feel heavy](melee-feel.md) | 2026-10-17 |
| [Far mountains: what made the wall behind the castle regular (2026-10-16)](2026-10-16-far-mountains.md) | 2026-10-16 |
| [Real-time fire rendering: colour, shape, smoke, haze and light](2026-10-16-real-time-fire-rendering.md) | 2026-10-16 |
| [Building's new tools, reviewed against the goals](building-tooling-review-2026-10-16.md) | 2026-10-16 |
| [Building's workflow: what cost the most, and what to change](building-workflow-retro-2026-10-16.md) | 2026-10-16 |
| [Building against the developers' goals: the fifth review](goals-review-2026-10-16.md) | 2026-10-16 |
| [75 fps: where the frame goes](2026-10-15-75fps-budget.md) | 2026-10-15 |
| [Mountain lakes, tundra and living towns: how shipped games build them](2026-10-15-biome-demos.md) | 2026-10-15 |
| [Dragons and fire: how shipped games spread, draw and breathe it](2026-10-15-dragons-and-fire.md) | 2026-10-15 |
| [The mountain lake's reference: Far Cry 5's valleys and Morskie Oko's photographs](2026-10-15-mountain-lake-reference.md) | 2026-10-15 |
| [The player's run: legs, then arms and hands](2026-10-15-run.md) | 2026-10-15 |
| [The lush forest through a year](2026-10-15-seasons.md) | 2026-10-15 |
| [The tundra's reference: photographs of open tundra in autumn, measured](2026-10-15-tundra-reference.md) | 2026-10-15 |
| [Building's code health](building-code-health-2026-10-15.md) | 2026-10-15 |
| [Colour balance, measured against a grey card (2026-10-15)](colour-balance-2026-10-15.md) | 2026-10-15 |
| [Building against the developers' goals: the third review](goals-review-2026-10-15.md) | 2026-10-15 |
| [Building against the developers' goals: the fourth review](goals-review-2026-10-15b.md) | 2026-10-15 |
| [Texture detail of the construction materials, near](texture-detail-2026-10-15.md) | 2026-10-15 |
| [Animation: a dressed run that looks right, and creatures with bodies of their own](2026-10-14-animation.md) | 2026-10-14 |
| [Is building fluid? What the QA films found (2026-10-14)](2026-10-14-building-fluidity.md) | 2026-10-14 |
| [Scots pine crowns that read from the ground, and the lush forest as a tall conifer stand](2026-10-14-scots-pine-crowns.md) | 2026-10-14 |
| [The lush forest beside Sons of the Forest: light, ground, leaves, trees](2026-10-14-sotf-forest-look.md) | 2026-10-14 |
| [Building at scale: three hundred buildings, a castle, and bases to the horizon](building-scale.md) | 2026-10-14 |
| [Carrying a forest to the horizon](far-forest.md) | 2026-10-14 |
| [Foliage at the camera and at the player](foliage-near-camera.md) | 2026-10-14 |
| [Building against the developers' goals: the second review](goals-review-2026-10-14.md) | 2026-10-14 |
| [Kit review: the wood and stone kits at 2, 5 and 12 metres](kit-review-2026-10-14.md) | 2026-10-14 |
| [One way to draw every construction material](uniform-materials.md) | 2026-10-14 |
| [What makes a forest read as AAA, and what Bevy 0.20 has for it](aaa-forest-look.md) | 2026-10-13 |
| [Building audit: rules, physics, drawing, cost and looks](building-audit.md) | 2026-10-13 |
| [Drawing bases a section at a time: the village bench](building-drawing.md) | 2026-10-13 |
| [Building against Enshrouded's, feature by feature](enshrouded-parity.md) | 2026-10-13 |
| [Distant trees without pops, the mood of a forest, and how The Forest and ARK make theirs](forest-lod-and-mood.md) | 2026-10-13 |
| [Building against the developers' goals](goals-review-2026-10-13.md) | 2026-10-13 |
| [The integrated GPU at 30 fps, and sections under 2 ms](integrated-gpu.md) | 2026-10-13 |
| [Role pieces: a look for every exposed part of a cube](role-pieces.md) | 2026-10-13 |
| [How good-looking voxel builders draw their blocks](voxel-building-look.md) | 2026-10-13 |
| [Pieces chosen by their neighbours](adjacency-pieces.md) | 2026-10-12 |
| [Debugging and QA tools for our agents](agent-debugging-tools.md) | 2026-10-12 |
| [Agents in Blender: outside advice against our playbooks](agents-and-blender.md) | 2026-10-12 |
| [Moving to Bevy 0.20](bevy-0-20-upgrade.md) | 2026-10-12 |
| [What Bevy gives a character system, and what we build](bevy-character-capabilities.md) | 2026-10-12 |
| [Blender in the pipeline: how studios do it](blender-pipeline-practice.md) | 2026-10-12 |
| [Block modules: conformed pieces or a displaced surface](block-modules.md) | 2026-10-12 |
| [Body morphs on our own base](body-morphs.md) | 2026-10-12 |
| [Building in survival games](building-systems-prior-art.md) | 2026-10-12 |
| [Character pipeline audit: what is sound, what is a bug, what is duct tape](character-pipeline-audit.md) | 2026-10-12 |
| [How games make customisable characters and gear that fits every body](character-systems-industry.md) | 2026-10-12 |
| [ECS audit](ecs-audit.md) | 2026-10-12 |
| [Enshrouded's combat](enshrouded-combat.md) | 2026-10-12 |
| [Gear through every clip](gear-every-clip.md) | 2026-10-12 |
| [How magic works in He Who Fights With Monsters](hwfwm-magic.md) | 2026-10-12 |
| [Item icons](item-icons.md) | 2026-10-12 |
| [A forest as rich as big games make them](lush-forest-spike.md) | 2026-10-12 |
| [How modern games vary bodies and fit gear to all of them](modern-character-variety.md) | 2026-10-12 |
| [One mesh for every body: how gear fits many bodies without a model per pair](one-mesh-for-every-body.md) | 2026-10-12 |
| [Cameras for planning](planning-mode-cameras.md) | 2026-10-12 |
| [Generating buildings from a drawn plan](procedural-buildings.md) | 2026-10-12 |
| [How games generate loot and spells](procedural-loot-and-spells.md) | 2026-10-12 |
| [How shipped games fill a world with plants](procedural-vegetation.md) | 2026-10-12 |
| [Realistic textures on building pieces](realistic-building-textures.md) | 2026-10-12 |
| [Recruits who live and work at your base](recruits-at-the-base.md) | 2026-10-12 |
| [Repository layout](repo-layout.md) | 2026-10-12 |
| [Skin deformation: why limbs twisted and squeezed, and the fix](skin-deformation.md) | 2026-10-12 |
| [Timber walls: generated logs, hewn timber, or a timber surface](timber-walls.md) | 2026-10-12 |
| [Our tools in a browser](web-build.md) | 2026-10-12 |
| [Building in many worktrees: what makes a new one quick](worktree-builds.md) | 2026-10-12 |
| [Making our own models with AI](ai-models-for-art.md) | 2026-10-11 |
| [How character creators work, and what fits Terra](character-creators.md) | 2026-10-11 |
| [Games to base game design 001 on](reference-games.md) | 2026-10-11 |
| [Meshing the terrain: how fast, and what it took](terrain-meshing.md) | 2026-10-11 |
| [Tripo Studio's API, endpoint by endpoint](tripo-studio-api.md) | 2026-10-11 |
| [Using our Tripo subscription from scripts](tripo-studio-credits.md) | 2026-10-11 |
| [How AI agents play games](agents-playing-games.md) | 2026-10-10 |
| [Art explorations before direction D](art-explorations.md) | 2026-10-10 |
| [Enshrouded's voxels and building](enshrouded.md) | 2026-10-10 |
| [A planet of choppable trees](forest-scale-test.md) | 2026-10-10 |
| [Can the sandbox show something worth showing?](outpost-scene.md) | 2026-10-10 |
| [Subnautica 2's base building, and a first try at it](subnautica-2-building.md) | 2026-10-10 |
| [Subnautica's inventory](subnautica-inventory.md) | 2026-10-10 |
| [Rust stack toolchain check](rust-stack-spike.md) | 2026-10-06 |
| [What exists in Rust for a voxel planet game](rust-voxel-ecosystem.md) | 2026-10-05 |
| [Porting Voxel Tools to Rust](voxel-tools-port.md) | 2026-10-05 |
| [Godot spike: what we tested and learned](godot-spike.md) | 2026-10-04 |

<!-- end of rendered -->

## Adding a note

- One question per note, stated at the top with the date.
- Say what was measured or read, and what was only inferred.
- End with the limits: what the note does not show.
- Link sources.
