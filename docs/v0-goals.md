# v0 goals

Status: **proposal, 2026-11-01** (D589, Open). The eight areas are the developers' list; the tracks, the gaps, the numbers and the order are the agent's recommendations, for the developers to edit. Nothing here overrides a decision until they settle it. v0 is the next milestone, not the entire game: **the foundations of the engine, with one thin demo on top**.

The developers, 2026-11-01: "heres our major TODO, these are the things we think we need for a v0 (but its not the entire game)". Then: "we're not really solving the reason to cross or actual gameplay yet, just trying to get foundations of an engine in place". Then, on the demo: "when you beat the boss he drops an artifact, that artifact allows you to open the portal, and go to the other world, and thats basically the end of the demo (the other world is playable, but no more progression etc)".

## What v0 is

An engine that can carry the game: a seed makes a large procedural world, a friend can join it, and a gate leads to a second world of another theme and back. Every system runs for real, through the server. v0 proves each of them works. Its gameplay is one short line through them, the demo arc below; the rest of the game's design waits.

**v0 is done when this session works, start to finish, on two machines over the internet:**

1. On the title screen, a player makes a world: a name, Survival or Creative, and a seed, rolled or typed.
2. They host it; a friend joins from their own title screen.
3. Both walk through at least three biomes of the first realm, in weather that changes as they walk, with the wind in the trees.
4. They use what the game already has: dig, gather, craft, move items, equip gear, and meet wildlife and monsters that fit the place, each seeing the same thing.
5. They beat the boss, take the artifact it drops, and open the gate with it.
6. They cross, together or apart, and come back. The second realm's land, sky, plants, creatures and sound are its own, and what they carry goes with them.
7. They quit and return days later. Both realms and both characters are as they were left.

An agent drives the same session with two scripted clients and checks each step (D69). The frame budget (D25) holds throughout.

The earlier MVP (D105, [prototypes.md](prototypes.md)) asks whether the core loop is fun, and leaves out portals and many players. v0 asks a different question: is the engine under it sound? The word MVP stays D105's.

## The demo arc

The developers' outline, 2026-11-01, with the details still theirs (Q7 to Q9):

1. **Prepare**: the player gets ready for the boss by "some stuff (maybe level up or something, progress around? find clues?)".
2. **Beat the boss**, in the main world. It drops an artifact.
3. **Open the gate** with the artifact.
4. **Cross**, and the demo is over. The other world is playable, with no more progression.

The arc is a line through the engine, not a progression system: it should add as little as it can.

| Track | What the arc adds |
|---|---|
| Land, Realms | The boss's place and the gate placed from the seed, each reachable on every seed |
| Realms | A gate that starts shut and opens, its state saved with the realm and shared by everyone in the world |
| Animation, Creatures | One boss: a big body, its attacks and tells, and fights that two players can share |
| Play | The artifact, an item that opens the gate; beating the boss stays beaten (D499), so it must not be possible to lose (Q8) |
| Server | The fight's outcome and the gate's state decided once, for everyone, and kept across a disconnect |
| Proof | Scripted runs open the gate directly, so testing a crossing never means beating the boss; the session runs the whole arc once |

## Out of v0

Proposed, to edit. What already works stays in the game and keeps working; it just gets no new work.

- Gameplay beyond the demo arc: progression past it, anything to do after the gate, balance, the economy. The developers said so; the engine has to allow these, not answer them.
- Building, as the developers said: on hold. Villages and settlements with it, so no quests or recruits.
- Essences and abilities (D105 has them first after the MVP), raids, factions, taming and mounts, trade.
- A third realm, dungeons as content, a thousand players (the design keeps D52 possible; v0 does not measure it).
- First person, gamepad, voice chat, a store page.

## Three seams that come first

Most tracks can start today. These three contracts cost little now and a great deal later, so they go first, and every track then builds on them.

1. **One realm description.** `WorldSpec` holds a key, a kind and a seed, and today's data is one world's (`climate.ron`, `flora.ron`, `creatures.ron`). A realm needs one file naming its size, land recipe, climate, flora, creatures, sound bed, sky and asset set (D59, D471). A new realm is then a file, not code.
2. **One seed.** The planet is `Seed(1)` and no option changes it. A world's seed becomes the only input; each realm's seed derives from it (`terra_gen::hash`). Every bake that assumes one land (the horizon map, D416) has to run for a new seed, on the player's machine, when the world is made.
3. **One way for state to change.** D138's stages already separate intent, tick and what is shown. From now on every track holds this rule: input becomes intent, only the tick changes shared state, what is shown only reads it, and sound and effects come from state or a `Noise`. New gameplay that breaks it is rework for Server.

## Tracks

Each track can run beside the others. "Waits on" is what it truly needs first. Agents at once stay within AGENTS.md's limit of three or four, so this is what *can* run side by side; the order below says what *should*.

| Track | From the list | Waits on | Works in | Clashes with |
|---|---|---|---|---|
| **Front** | 1 | Seam 2; Server for join | `terra_game::play::title`, `worlds` | Nothing |
| **Realms** | 2 | Seams 1, 2 | `terra_world::instance`, each module's `entering`, `play::save` | Land, Living world, at each module's entry |
| **Realm two** | 2 | Realms for the crossing; can start its art now | `assets`, `art`, the realm's data | Nothing |
| **Land** | 3 | Seam 2 | `terra_land`, `terra_voxel`, `terra_world::planet` | Realms, at the planet |
| **Living world** | 3 | Nothing | `terra_climate`, `terra_world` sky, weather, wind, flora | Realms, at each module's entry |
| **Body** | 4 | Server, for prediction | `terra_motion`, `terra_walker` | Animation, on the player's body |
| **Animation** | 5 | Nothing | `tools/characters`, `tools/creatures`, clips | Body, Creatures: one owner a body (animation.md) |
| **Creatures** | 5, gap | Animation for new bodies; seam 3 | `terra_creatures`, `assets/data/creatures.ron` | Play, in the fight's code |
| **Server** | 6 | Seam 3 | New `terra_net`, the seams of `terra_walker`, `terra_survival`, `terra_creatures` | Everything lightly; keep its edits small and landed often |
| **Sound** | 7 | Nothing | `terra_world::audio`, `play::sounds`, `play::soundscape` | Nothing |
| **Play** | 8 | Seam 3 | `terra_items`, `terra_survival` | Creatures, Server |
| **Proof** | gap | Server | `tools/`, `xtask`, bots | Nothing |

### Front (1)

- **Have**: the title screen (D579, `--title`): Continue, Worlds, New world, The yard, Settings, Quit. Several worlds, each with a name and mode, kept on disk. Deleting asks first.
- **Need**: a seed on New world, rolled by default, typed if wanted, kept in `world.ron` and shown in the list. New world options kept few (name, mode, seed, Q6). Host and Join. A screen for the creator of a few sliders (D111), which has none yet (`terra_walker::appearance` holds the look), so two players can tell each other apart. Quit to title without restarting the program, which a crossing's leaving should now make possible.
- **Done when**: a scripted run makes two worlds from different seeds, sees different land, and returns to the title and loads the first, in one process.
- **Note**: D579 says no seed is offered, because the land was one world. Generating from a seed removes that reason, but the decision is the developers' to amend (Q2).

### Realms (2)

- **Have**: `terra_world::instance` built: states, scoping, leaving, the census, crossing in one frame, and a round-trip test with no GPU. Most modules are built on entering.
- **Need**: the rest of the modules moved (fire, the map, the soundscape). A gate entity, `Door`, with its model and effect. Asset sets (D471). The save split into a character's and an instance's (D471). The return stack (D470). Warming as you approach. A gate placed by generation in each realm, from the seed, on reachable ground (Q3).
- **Done when**: instances.md's test passes: five round trips with the census equal and memory within 2%; the cover under 3 s (D501), aiming under 1 s for a warmed realm.

### Realm two (2)

The proof that a realm is data. instances.md's playground proves loading and nothing else; this proves a world of another theme comes out of the same rules. It needs enough of each part to show that, not a full realm's worth.

- **Need**: its land recipe, ground materials, plants, sky and light, wildlife, monsters and sound, each by the existing pipelines and the art style (D14). A realm's parts are new; the rules are not.
- **Done when**: stills and a film pass review beside the first realm, and a stranger could tell them apart at a glance while seeing one game (D14). The generator states how many variants it makes (D182).

### Land (3)

- **Have**: smooth voxels, dug and saved (`terra_voxel`). The land by its rules (`terra_land`). Forest, mountain lake, tundra and town, made as demos on the 30 km world at places their queries found. `play` itself runs a 2 km world with one seed; mountains need `--radius 30000` (Q4).
- **Need**: the generator holding up on *any* seed, not on the demos' pinned places: a walkable start, biomes reached by walking, a blend between them, the gate reachable. The moving origin (D36), which is not built, though the mover is already in 64-bit (D482). Streaming and load times at that size.
- **Done when**: a sweep over at least 100 seeds (`cargo xtask fields`, by CPU, `terra_gen::par::map`) finds every world valid, and a walked film crosses three biomes with no visible seam. Terrain still digs everywhere.

### Living world (3)

- **Have**: the climate's fronts, rain, snow and wind as fields (D314, D315). The weather at the eye (D418). Volumetric clouds on high (D515). Snowfall and falling leaves. One wind for grass, bushes and trees, gusts travelling across them, through one shared buffer (D255, D374), so wind already costs little per plant.
- **Need**: weather.md's stages from the storm on: storm cells, falling rain and lightning (none are built), puddles and rain on water, then a whole winter, then a front arriving in one take. Plants for every biome at every distance, the far forest included. The sky the same for every player, which D540 makes nearly free: clock and seed are all that travels.
- **Done when**: a clear-to-storm film with no visible step passes review in each of three biomes, on both realms' terms, and frame time holds in the worst weather on the 3080 (D25).

### Body (4)

"Fully implemented" needs a list, or it never finishes. Proposed:

| Move | State |
|---|---|
| Walk, jog, sprint, jump, fall, land, stairs, steep ground, swim, dive, pushing | In the game on the mover (D53) |
| Dodge | In the fight |
| Starts, stops, turns, lean into turns, blends by speed and direction | To do (animation.md, step 2) |
| Climb, pull up, lunge | The mover has them; off in the game until the figure has clips (D477) |
| Crouch or sneak | Not built; waits for gameplay |
| Prediction: the client moves at once, the server corrects | Not built; the mover's plain, fixed-tick code is ready for it (D138) |

- **Done when**: `cargo xtask controller` shows every row passing, a before-and-after film earns the developers' yes (animation.md), and the same walk feels right at 100 ms of latency.

### Animation (5)

- **Have**: the method (animation.md, D584, D585). People by the character pipeline (D115). Sixteen animal kinds on two body plans, their skeletons placed by hand (creature-rigs.md).
- **Need**: creature-rigs.md's order of work: the creature check, body plans as data, skeletons fitted to the mesh, weights, paces from size. Then one template body for each plan monsters need. The player's locomotion and fight sets finished, one job a body at a time. Which plans come first is D436, how monsters move D438, and blows from a pose sheet D588.
- **Done when**: for each plan v0 uses, a new creature goes from a concept to the game with no hand edits, passes the creature check, and scores 7 or more on review (D437). The time and credits a creature takes are reported, since that sets how many a realm can have.

### Creatures (5, a gap in the list)

An animation pipeline gives a body that moves; nothing in the list says what it chooses to do.

- **Have**: wildlife with routines, startled by rule, put out in cells as a player nears (D380, D384). Spiders at a place where magic pools. The fight's rules, and the duel.
- **Need**: behaviour as data: sensing, hunting, fleeing, packs, each kind's attacks and tells, leashing back home. Finding a way over dug ground, which has no solution yet for creatures. Spawning by a realm's theme and where magic pools (D59, D103). All of it on the server, with cells shared between players rather than kept round one eye.
- **How many**: enough hostile and harmless kinds to prove each body plan in each realm's theme. The count is what the pipeline can carry, once a creature's cost is measured.
- **Done when**: a new kind's behaviour is a data file, a duel run per plan passes, and two players at opposite ends of a pack see the same fight.

### Server (6)

The largest risk and the only track at zero. No `terra_net` exists yet.

- **Have**: the fixed tick (D138), rules in plain crates (D28), operations in order (D21), the same world on every machine (D30), and the plan to use `renet` with our own messages (D34).
- **Need**:
  - `terra_net`, a server program with no window, and the same server started inside the game (D51).
  - **Always a client.** Even solo, the drawn game is a client of a server. Otherwise there are two code paths.
  - **Two realms at once.** One player can be in each realm, so a host's server runs both while its own client draws one. instances.md's split (a headless `App` per instance on the server, the client's world in the main `World`) means a host holds its realm twice. Measure that memory before building on it.
  - Sending by area, a snapshot then changes, others shown between ticks, late join, leaving and rejoining mid-fight, and a version check on connect.
  - Each player's character kept by the server under an identity (Q1).
  - Every action checked: in reach, owned, allowed (D51).
  - Text chat and a player list.
- **Done when**: engine.md's test passes (two peers end the same, a late joiner catches up), then four clients for 30 minutes at 100 ms and 1% loss show no lasting mismatch, with the bytes per player per second reported.

### Sound (7)

- **Have**: `Noise` as a fact about the world, the mix with its levels, `Footsteps`, `Ambient` within earshot, recorded and generated sounds, blows by what met what, loudness checks (D113, D447). Bevy's spatial listener on the camera: left and right, with volume by distance.
- **Need**: a choice of engine, since Bevy's has no reverb or muffling. Candidates: `bevy_seedling`, already named in audio.md, Steam Audio, and FMOD or Wwise as paid options (D526); the first job checks each against Bevy 0.20 and reports. Then, by the developers' three words:
  - **Directional**: front from back and above from below, not only left from right.
  - **Distance**: a curve per kind, far sounds losing their highs and arriving late (thunder at 343 m/s, as weather.md wants).
  - **Ambient**: a bed per biome and realm, blended by place; caves and rooms sounding like themselves (audio.md's question 5).
  - **The pipeline**: a clip goes from a source to the game through one step that names, measures and checks it. Others' noises arrive from the server.
- **Done when**: a test renders a moving source to a file and measures its level, side and tone against position, with no person listening; then the developers approve a listening film in each realm.

### Play (8)

- **Have**: backpack, equipment, action bar, quiver, wear and repair, crafting at stations, gathering by tool, skills by doing, hunger, death and the dropped pack, the map (prototypes.md). Sword and shield and the bow on the combat scorecard (D452).
- **Need**: the baseline as engine, not as balance: every change to an inventory an operation the server checks, items and gear carried across a crossing, death and return working in either realm, and what another player sees of your gear. Beyond the artifact, which items, tiers and numbers exist is gameplay, for later.
- **Done when**: two players trade blows with the same monster, swap items through a chest, die and recover their packs, with nothing duplicated or lost across a crossing or a disconnect.

### Proof (a gap in the list)

- **Need**: headless clients that log in and play by script (multiplayer.md's agents as players), `cargo xtask loop` for several clients, latency and loss on demand, and the v0 session as one command. A Windows build a friend can run, with a crash report they can send (D399).
- **Done when**: the session above runs unattended on main, and someone not on the team joins a game from a build they were sent.

## Order

1. **Now, one or two agents**: the three seams. A bounded spike answers whether a host can hold two realms.
2. **Then, the four that carry the most risk**: Server up to two clients walking together, Land on any seed, Animation's creature check and first plan, Realms to a working gate between placeholder realms.
3. **As agents come free**: Front, Sound, Living world, Body, in any order. None block the others, and Front's seed and Sound's engine choice are small.
4. **Once a gate and a server stand**: Creatures and the boss, Play on the server, Realm two's content.
5. **Last**: Proof's session end to end, fixing what it finds.

Each track gets a bounded plan of its own, and a skill or scorecard if it lacks one (controller, combat and environment have theirs), before its first job.

## Questions for the developers

Design calls, each with a recommendation; none is settled until they say so. Q1 to Q4 shape the engine; Q7 to Q9 shape the demo.

| # | Question | Recommended |
|---|---|---|
| Q1 | Who is v0 for, and how do they connect? | The two developers and a few friends, 2 to 4 at once. A player hosts or a server runs alone (D51). Direct address first, Steam's relay when strangers' routers get in the way (D34). A player is identified by a key kept on their machine |
| Q2 | Does a world have a seed? D579 says no | Yes: D579's reason lapses once land comes from the seed. Amend it |
| Q3 | What is the second realm? "Scifi" meets settings.md's "no science fiction toolsets" and "no machine guns" | A dead realm of magic-machine ruins under a strange sky, from settings.md's techno realm: alien and technological, yet still magic. Its builders made the gate, which fits the stargate. Its monsters are guardians and things grown in the ruins. Its gate is found in the world and walked through (D72) |
| Q4 | What is "real scale"? `play` is 2 km in radius today; the mountains and D13 say 30 km | 30 km for the first realm, since biomes need the room. 10 to 20 km for the second |
| Q5 | How many biomes? | At least three in the first realm (forest, mountain lake, tundra) and two in the second |
| Q6 | Which options has a new world? | Name, mode and seed. No more until there is something to tune |
| Q7 | What does a player do to reach the boss? | Nothing new: skills by doing (D88) and gear crafted in the first realm, with a boss that starting gear cannot beat. The gate is found early, dark and shut, so a player knows what they are after. The boss's place is found by looking, with no markers (D92): something on the land seen from far off. Clues only if a playtest shows players lost |
| Q8 | What are the artifact's rules? | Used once on the gate, it stays there, and the gate stays open for everyone in that world. Until then, dropped on a death it never decays, an exception to D84, or a lost artifact would shut a world out of the second realm for good |
| Q9 | What is the boss, and where? | One great monster at the heart of the worst place magic pools (creatures.md's question 3, D103), never coming back once beaten. Start from the 4 m troll of the troll fight (D402), whose model and clips were made for a film |

Open decisions v0 will meet, by track: D134 day length (Living world), D436 and D438 monsters (Animation), D580 load time (Land, Front), D588 pose sheets (Animation), audio.md's music and voices (Sound).
