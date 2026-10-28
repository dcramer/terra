# Terra design docs

Working title: **Terra**. First and foremost, we are building an extremely ambitious sandbox engine, with multitudes of procedural worlds and intricate systems for each component of the game (D187). The game is a multiplayer survival sandbox with RPG in it, across realms reached through portals, built toward a thousand players on one persistent server (D52, D89).

## Where things are

| Looking for | Go to |
|---|---|
| What is decided, and who decided it | [decisions.md](decisions.md), the index by area; each decision in full in [decisions/](decisions/D1.md), or `cargo xtask decision D28`; [the log](decisions-log.md) says what changed when |
| How a part of the game works and why | The design docs below |
| A design being weighed | [proposals](proposals/README.md) |
| What we tried, measured or surveyed | [research](../research/README.md), dated notes; [benchmarks.csv](../research/benchmarks.csv) for frame times |
| An experiment in progress | [spike](../spike/README.md) for throwaway code; the sandbox's testbeds and experiments in [terra_sandbox](../crates/terra_sandbox/README.md) |
| How to run, check or measure something | [development.md](development.md); each tool's README in [tools](../tools) |
| The steps for a repeated job (a character, a model, an icon) | The skills in [.agents/skills](../.agents/skills) |

## The docs

The source of truth, then the docs about the game as a whole, then one doc per part of the game, then parked docs, then the engine and how we work.

| Doc | What it covers |
|---|---|
| [decisions.md](decisions.md) | What is settled, what is a working assumption, what is open: the index of `decisions/`, a file per decision, which are the source of truth. Written by `node tools/docs/docs.ts --render`; edit the files, not it. |
| [decisions-log.md](decisions-log.md) | One line per decision added or changed, newest last. Git merges it by union, so agents adding lines at once never conflict. |

**The game as a whole**

| Doc | What it covers |
|---|---|
| [vision.md](vision.md) | What we are building (D187), the pitch, pillars, our inspirations, tone, non-goals, and how the scope is reached. |
| [gameplay.md](gameplay.md) | **The game's design, and the source of truth for it** (D89): a living world, after Enshrouded and *He Who Fights With Monsters*; what goes on in the realms, the loops, factions and settlements, holding ground, no hand-holding. Links a page per system, below. |
| [lore.md](lore.md) | The foundational lore: how the universe works, from *He Who Fights With Monsters*'s world-building, written in our own words. The basis for every system. |
| [naming.md](naming.md) | Proposed rules for making names and lore: a shared tongue for how the universe works, in the books' names and plain English, and generated tongues for each realm's peoples. |
| [proposals](proposals/README.md) | Where new numbered proposals go. |

**Parts of the game**

| Doc | What it covers |
|---|---|
| [worlds.md](worlds.md) | The realms as physical worlds: what a world's theme sets, terrain, caverns, water, size, gravity, the starting realm, portals between them. |
| [instances.md](instances.md) | Leaning (D468 to D471): how a world is loaded in and out: a world instance as a unit (a realm; a dungeon is a place in it, D497), what it owns and what is global, its life on the server, scoping its entities and resources in the ECS, the crossing through a portal and the way back, asset sets, per-instance saves and replication, the first slice (a portal from the forest to a second realm) and the test that nothing leaks, the developers' answers on dungeons (D497 to D502). |
| [dungeons.md](dungeons.md) | Leaning (D473, D474), themes Decided (D527): dungeons by rule in stages: the mission graph (roles, links, a lock and its keys, a loop, a shortcut, mobs and loot by danger), placement on 3 m cells and 4 m levels with corridors and stairs, dressing by wave function collapse from a theme's socketed modules, themes as data, the sheet, what comes next and open questions |
| [biomes.md](biomes.md) | Draft: how a world fills with life, from grass to canopy: goals, how placement and drawing could work on voxel terrain, open questions. |
| [demos.md](demos.md) | Proposal: the next biome demos, a mountain lake, a tundra and a living town, each made by the world's rules: references, what exists, parts and variants, budgets, checks and order of work (D321). |
| [climate.md](climate.md) | Leaning: seasons and weather as swings in temperature, daylight and precipitation that bodies, ground, plants and creatures respond to; the year's length. |
| [weather.md](weather.md) | Proposal (D418): the weather as it is drawn and checked, storms and winter first: the weather at the eye driving the sky's deck, the light under cloud, the haze and what falls, each rule with its owner and check, named weathers as queries, the climate's missing storms, the staged plan and open questions. |
| [sky.md](sky.md) | Design (D515 to D522): the sky as the climate's picture, photographic and physically lit: clouds as lit volumes in the air (kinds fair-weather first, shades, wind, shadows, the sky light), seen to the horizon and flown through, heightened storms, the moon's phases and path, the sky's effects, realms, the same sky for every player, the budget, checks and the staged plan. |
| [survival.md](survival.md) | Needs, rhythms and death: hunger, thirst, warmth, breath, rest, mana; day and night; coming back at a hearth. |
| [combat.md](combat.md) | A fast action RPG on stamina, after Enshrouded; weapon skills by doing; abilities from essences; rank decides more than skill. |
| [movement.md](movement.md) | Proposal: how every body moves and fights on one system, in layers (intent, mover, actions, pose, fix-up, physics): root motion, moves as data, bodies that do not pass through, creatures of every size, what to port, the checks, open questions and the order of work. |
| [holding.md](holding.md) | Proposal: what each hand holds and how every body moves with it: items' grips and classes, the stance from the pair by rule, a move set per class and stance over shared locomotion, the buttons by stance, mirroring for left hands, timing by weight, hand IK and the strap, variants and their count, the checks, the order of work and open questions (D459 to D465). |
| [controller.md](controller.md) | The mover (D53): a capsule moved by a port of Jolt's CharacterVirtual over our own collision queries (voxel SDF, buildings, props, trees, bodies; water as a medium), gravity toward the centre, mass-weighted pushing, the numbers that are data, the production criteria and courses, climbing's design (Open, D477), open questions. |
| [scene-rules.md](scene-rules.md) | Proposal: the defects the films keep showing, each owned by a system that gets it right in every scene (wildlife, steering, shadow and reflection casters, fire, colour, camera rig, town layout, far bake, destruction, render runs) and failed by an automatic check on every film before a person sees it; the order of work and open questions. |
| [creatures.md](creatures.md) | Wildlife with routines and a food chain; monsters that form where magic pools, and what they leave. |
| [magic.md](magic.md) | Magic as a whole, with examples: core essences, attributes, mana, confluences by rule, awakening as a weighted draw of generated powers, combat styles and studied crafts from skill books, old-school spellcraft (reagents, words of power, Mark and Recall, scrolls and wands), and magic beyond these. |
| [crafting.md](crafting.md) | Resources by region and rank, tools, stations run by recruits, learning what to make. |
| [people.md](people.md) | Settlements with days, quests as what people say, recruits, trade. |
| [tribe.md](tribe.md) | Your tribe: its players and recruits, and its claims as outposts; recruits who work, carry and defend on their own, without being managed. |
| [player.md](player.md) | No class, creation (D111), moving and acting on the world, getting around big round realms, the map, what is on screen and what is not, options (D80). |
| [arc.md](arc.md) | The fiction, the start without a tutorial, the long game by rank, playing together. |
| [settings.md](settings.md) | How any realm's theme and cultures work: gameplay roles every setting fills, rank the only power axis, how generated realms fit the lore, and one setting worked through, a realm of machines that run on magic without guns. |
| [creature-rigs.md](creature-rigs.md) | Proposal: from a generated mesh to a creature that moves well: body plans as data, skeleton fitting, automatic weights, gaits from body size, and the scored creature check that tells a good creature from a bad one; where we depart from shipped games, an honest estimate per plan, questions for the developers (D436 to D443). |
| [items.md](items.md) | Items and containers: equipment, backpack, action bar and storage, on a Subnautica baseline; icons; how they work in code. |
| [characters.md](characters.md) | Bodies and what they wear: how every item fits every body without fitting each by hand (D112, D128), the base body, the game's side, tools and checks, order of work, risks and questions for the developers. |
| [attachment.md](attachment.md) | The attachment contract: the body's sockets and their axes, and for each kind of held or worn look its origin, axes, facing, size and glTF extras, with the check that holds them (D546). |
| [assembled-items.md](assembled-items.md) | Proposal: weapons and tools assembled from parts with named connection points, by rules from maker, realm and material; kinds own holding, carrying and animation; how it is checked by parts and kinds on what the game draws; the questions for the developers (D565). |
| [building.md](building.md) | Building as the player meets it: walls, floors and roofs of half-metre cubes (D152, D181), drawn the Sims way (D154), planned then constructed (D139), claims, the controls, sizes, and open questions. |
| [building-tech.md](building-tech.md) | The technical design for building: cubes as shared state (D160, D181), walls laid from kits by rule, roofs as a height field, buildings and indoors, drawing many buildings, and the order of work. |
| [building-plan.md](building-plan.md) | The technology plan for building (D195): where each part lives, what is stored, operations, drawing, collision, the art pipeline, how it is checked and measured, and the order of work. |
| [building-kits.md](building-kits.md) | How each kind of construction looks and is drawn, planned, going up and built, logs first: corners, joins and openings (D177 to D179). |
| [building-drawing.md](building-drawing.md) | How building is drawn, from cubes to the screen, where each kind of junk on screen comes from and what keeps it out (`cargo xtask artifacts`), and adding a material as data. |
| [building-voxels.md](building-voxels.md) | The voxel contract (D348): the cube and its shapes, who draws each face, edge and corner between cubes, the invariants that keep a building closed, every junction with the rule that closes it and the check that proves it, and the gaps. |
| [progression.md](progression.md) | Three ways to grow (skills, studied techniques, essences), skills on the rank ladder, crossing a rank, and how things are unlocked. |
| [multiplayer.md](multiplayer.md) | Server-first multiplayer, built toward a thousand players on one persistent server. |
| [art-style.md](art-style.md) | **The style guide and art direction for anything the game draws**, the same in every realm and setting: textured, lightly stylised realism, what a realm brings, life size, shape, colour, light, budgets, the concept prompt, a checklist, how the renderer makes the look and open questions. |
| [fire.md](fire.md) | Leaning (D415): one Fire module for every fire, from a candle to a burning castle, a breath and an explosion: its layers, the flipbook channels and our Mantaflow baker, one colour ramp, the exposure rule, the tonemapper bake-off, light, smoke and haze rules, presets as data, LOD and the cinematic tier, the check against real fire, and the staged plan. |
| [fire-module.md](fire-module.md) | Leaning (D466): the fire module's boundaries and small surface, the general flame every kind shares (a marched flame field in place of cards), wind, rain and snow, the fire bench and the measure across kinds; proposed (D493): the flame's motion from a baked looping simulation |
| [audio.md](audio.md) | Sound: how it is put together, what it sounds like now, and questions for the developers. |

**Parked**

| Doc | What it covers |
|---|---|
| [planet-sim.md](planet-sim.md) | Parked with terraforming (D6, D89): the planetary simulation, cells, fields, how local feeds global. |

**The engine and how we work**

| Doc | What it covers |
|---|---|
| [tech.md](tech.md) | The stack, how the code is split, how co-op shares state, terrain, workflow, risks. |
| [engine.md](engine.md) | The engine's parts, what each needs, and the parallel tracks they are built in. |
| [terrain.md](terrain.md) | Brief for the first component, the voxel terrain crate. |
| [development.md](development.md) | Setting up a machine, everyday commands, seeing and measuring output, developer tools, and QA our agents can run. |
| [ci-agent.md](ci-agent.md) | The CI agent: the container that runs the slow checks whenever main moves and files what breaks as GitHub issues. |
| [render-worker.md](render-worker.md) | The render worker: the GPU box that takes agents' renders (films, sheets, shots, benches) over ssh at a commit and hands back their files. |
| [trailer.md](trailer.md) | The trailer: one run through each of the four places, the hour and route of each and why, and how it is filmed (D357). |
| [troll-fight.md](troll-fight.md) | The troll fight, a demo scene on the lake shore: a 4 m troll against a man with sword and shield, its beats and how it is filmed (D402). |
| [prototypes.md](prototypes.md) | The builds so far and the milestones ahead, with their pass criteria. |

Findings, surveys and measurements live in [research](../research), not here. These docs say what we decided; research says what we found.

## How we keep these useful

- **The decisions win.** If another doc disagrees with one in [decisions/](decisions.md), the other doc is wrong; fix it.
- **Every claim has a status.** *Decided* means don't reopen without a reason. *Leaning* means a working assumption that is untested or unconfirmed. *Open* means nobody has chosen yet.
- **Change the doc in the same change as the thing it describes.** A decision made in chat that isn't written here didn't happen.
- **Keep them short.** These describe intent and constraints, not implementation. Code explains itself.
- **Say each thing once.** A decision's file says what and why in a few sentences and links its doc; the doc says how the system works; a tool's README has its commands and files; a skill has the steps and links the README rather than copying it; a research note is what was found on its date and isn't rewritten. Everywhere else cites the decision or links the place.
- **Checked, not trusted.** `cargo xtask check` fails on a dead link or heading, a doc, note, spike or script missing from its index, a malformed or unknown decision, and a decisions index that isn't its files ([tools/docs](../tools/docs/README.md)).
- **Record why.** A decision without its reason gets relitigated.
