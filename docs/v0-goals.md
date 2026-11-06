# v0 goals

Status: **decided, 2026-11-01** (D589). The eight areas and the demo are the developers'; the tracks, the gaps, the numbers and the order are the agent's, and change as the work teaches. v0 is the next milestone, not the entire game: **the foundations of the engine, with one thin demo on top**.

The developers, 2026-11-01: "heres our major TODO, these are the things we think we need for a v0 (but its not the entire game)". Then: "we're not really solving the reason to cross or actual gameplay yet, just trying to get foundations of an engine in place". Then, on the demo: "when you beat the boss he drops an artifact, that artifact allows you to open the portal, and go to the other world, and thats basically the end of the demo (the other world is playable, but no more progression etc)".

## What v0 is

An engine that can carry the game: a seed makes a large procedural world, a friend can join it, and a gate leads to a second world of another theme and back. Every system runs for real, through the server. v0 proves each of them works. Its gameplay is one short line through them, the demo arc below; the rest of the game's design waits.

**v0 is done when this session works, start to finish, on two machines over the internet:**

1. On the title screen, a player makes a world: a name, Survival or Creative, and a seed, rolled or typed.
2. They host it; a friend joins from their own title screen.
3. Both walk through at least three biomes of the first realm, in weather that changes as they walk, with the wind in the trees.
4. They use what the game already has: dig, gather, craft, move items, equip gear, and meet wildlife and monsters that fit the place, each seeing the same thing.
5. They beat the boss, which imprints a magic mark on their arms (D590), and open the gate with it.
6. They cross, together or apart, and come back. The second realm's land, sky, plants, creatures and sound are its own, and what they carry goes with them.
7. They quit and return days later. Both realms and both characters are as they were left.

An agent drives the same session with two scripted clients and checks each step (D69). The frame budget (D25) holds throughout.

The earlier MVP (D105, [prototypes.md](prototypes.md)) asks whether the core loop is fun, and leaves out portals and many players. v0 asks a different question: is the engine under it sound? The word MVP stays D105's.

## The demo arc

The developers' outline, 2026-11-01, the details left for later:

1. **Prepare**: the player gets ready for the boss by "some stuff (maybe level up or something, progress around? find clues?)".
2. **Beat the boss**, in the main world. A magic mark imprints on your arm (D590).
3. **Open the gate** with the mark.
4. **Cross.** The demo doesn't end formally: the other world is playable, with no more progression.

The arc is a line through the engine, not a progression system: it should add as little as it can.

| Track | What the arc adds |
|---|---|
| Land, Realms | The troll's cave (D591) and the gate placed from the seed, each reachable on every seed |
| Realms | A gate that starts shut and opens, its state saved with the realm and shared by everyone in the world |
| Animation, Creatures | One boss, the troll: it starts from the troll fight's 4 m body and clips (D402), made for a film, and needs its attacks, tells and a fight two players can share, in a cave |
| Play | The mark (D590): a fact on the character, kept by the server and read by the gate, shown on the arm. It is a small general record of marks, not a case for one boss, since a new mark is data (D136) |
| Server | The fight's outcome and the gate's state decided once, for everyone, and kept across a disconnect |
| Proof | Scripted runs open the gate directly, so testing a crossing never means beating the boss; the session runs the whole arc once |

## Out of v0

Proposed, to edit. What already works stays in the game and keeps working; it just gets no new work.

- Gameplay beyond the demo arc: progression past it, anything to do after the gate, balance, the economy. The developers said so; the engine has to allow these, not answer them.
- Building, as the developers said: on hold. Villages and settlements with it, so no quests or recruits.
- Essences and abilities (D105 has them first after the MVP), raids, factions, taming and mounts, trade.
- A third realm, dungeons as content, a thousand players (the design keeps D52 possible; v0 does not measure it).
- A host-less dedicated server as a way to play, which the developers expect at some point.
- First person, gamepad, voice chat, a store page.

## Three seams that come first

Most tracks can start today. These three contracts cost little now and a great deal later, so they go first, and every track then builds on them.

1. **One realm description.** `WorldSpec` holds a key, a kind and a seed, and today's data is one world's (`climate.ron`, `flora.ron`, `creatures.ron`). A realm needs one file naming its size, land recipe, climate, flora, creatures, sound bed, sky and asset set (D59, D471). A new realm is then a file, not code.
2. **One seed.** A world's seed is its only input (D596): kept in `world.ron`, given by `--seed`, the home realm's own, each other realm's derived from it (`terra_gen::hash`), held by a lint and a fingerprint test. The start is searched for from the seed (`terra_land::spawn`), the sweep (`cargo xtask worlds`) finds every seed but the first's kept demo start valid, and New world rolls one. Every bake that assumes one land (the horizon map, D416) has to run for a new seed, on the player's machine, when the world is made.
3. **One way for state to change.** D138's stages already separate intent, tick and what is shown. From now on every track holds this rule: input becomes intent, only the tick changes shared state, what is shown only reads it, and sound and effects come from state or a `Noise`. New gameplay that breaks it is rework for Server.

## Tracks

Each track can run beside the others. "Waits on" is what it truly needs first. Agents at once stay within AGENTS.md's limit of three or four, so this is what *can* run side by side; the order below says what *should*. What each track has, needs and is done when: [v0-tracks.md](v0-tracks.md), a section a track.

| Track | From the list | Waits on | Works in | Clashes with |
|---|---|---|---|---|
| **Front** | 1 | Seam 2; Server for join | `terra_game::play::title`, `worlds` | Nothing |
| **Realms** | 2 | Seams 1, 2 | `terra_world::instance`, each module's `entering`, `play::save` | Land, Living world, at each module's entry |
| **Realm two** | 2 | Its theme, settled later; Realms for the crossing | `assets`, `art`, the realm's data | Nothing |
| **Land** | 3 | Seam 2 | `terra_land`, `terra_voxel`, `terra_world::planet` | Realms, at the planet |
| **Living world** | 3 | Nothing | `terra_climate`, `terra_world` sky, weather, wind, flora | Realms, at each module's entry |
| **Body** | 4 | Server, for prediction | `terra_motion`, `terra_walker` | Animation, on the player's body |
| **Animation** | 5 | Nothing | `tools/characters`, `tools/creatures`, clips | Body, Creatures: one owner a body (animation.md) |
| **Creatures** | 5, gap | Animation for new bodies; seam 3 | `terra_creatures`, `assets/data/creatures.ron` | Play, in the fight's code |
| **Server** | 6 | Seam 3 | New `terra_net`, the seams of `terra_walker`, `terra_survival`, `terra_creatures` | Everything lightly; keep its edits small and landed often |
| **Sound** | 7 | Nothing | `terra_world::audio`, `play::sounds`, `play::soundscape` | Nothing |
| **Play** | 8 | Seam 3 | `terra_items`, `terra_survival` | Creatures, Server |
| **Proof** | gap | Server | `tools/`, `xtask`, bots | Nothing |

## Budgets

D25 holds a client's frame and D501 a crossing. Nothing yet holds the rest, and each needs a number before its track ends. The first measures set them, then a ratchet holds them (D452).

| Budget | Stands at |
|---|---|
| A host's frame: client, server and both realms on one machine | Unset. D25 counts only a client |
| A server's tick for one realm with players in it | Unset; 64 ticks a second leaves 15.6 ms for everything |
| Bytes per player per second, standing and travelling | Unset, multiplayer.md's first measure |
| Memory on a host and on a client | Unset |
| Load to first playable frame | D580, Open |
| Making a new world, bakes included | Unset |
| A crossing's cover | Under 3 s (D501), aim under 1 s warmed |

## Order

1. **Now, one or two agents**: the three seams. A bounded spike answers whether a host can hold two realms.
2. **Then, the four that carry the most risk**: Server up to two clients walking together, Land on any seed, Animation's creature check and first plan, Realms to a working gate between placeholder realms.
3. **As agents come free**: Front, Sound, Living world, Body, in any order. None block the others, and Front's seed and Sound's engine choice are small.
4. **Once a gate and a server stand**: Creatures and the boss, Play on the server, Realm two's content.
5. **Last**: Proof's session end to end, fixing what it finds.

Each track gets a bounded plan of its own, and a skill or scorecard if it lacks one (controller, combat, environment and anim have theirs), before its first job. Server, Sound and Creatures start from a research note and a design, since they have neither (D49, D196).

## Left for later

The developers, 2026-11-01: "who cares who v0 is for, its a demo. the seed is just an rng thing like normal procedural generation goes. the second realm someone else will figure out later, world size idk big enough?, we will figure out boss/boss fight rules/etc later.. demo doesnt \"end\" formally".

- **The second realm's theme.** Whoever takes it has "scifi or something else" to start from, and settings.md's standing "no science fiction toolsets" and "no machine guns" to square with it; that doc's techno realm is worked through already.
- **The way to the boss and the fight's rules**: what a player does first, how the fight goes.

Open decisions v0 will meet, by track: D134 day length (Living world), D436 and D438 monsters (Animation), D580 load time (Land, Front), D588 pose sheets (Animation), audio.md's music and voices (Sound).
