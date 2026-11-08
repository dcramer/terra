# Iso: the top-down game on the game's own systems

Status: **design, 2026-11-07; A decided 2026-11-08 (D635)**: one game with an iso camera, "just make sure its modularized enough to not collide". The camera is D629's question, the characters' budgets D630's; the art pipeline is D628 and the biome kit and its bar D634.

## Intent

The iso experiment (`crates/terra_iso`, once `spike/iso`) is the developer's "high quality, realistic topdown game", "realistic meaning leaning into realism" (D634). In a few weeks it found an art pipeline (D628) and a biome kit judged toward realism (D634). It shares almost nothing with the game: Bevy, the `patches/`, the assets and `terra_core::cooked`'s mips. Its hero is a point moved each frame, its fight is a cone and a count of blows, and its ground is a closure. The developer: "make sure that they are majorly using the same systems, same networking, same ecs etc, ik lots of things will be different like models etc."

This doc says what iso shares with the game and what stays its own, the order the shared parts are taken in, and what iso's kit gives back.

## The choice (D635)

| Option | What it is | What is shared |
|---|---|---|
| **A. One game with an iso camera** (chosen, D635) | Iso is a camera, its controls and a look over the game's world: one binary, one world and one server. Iso's biomes become realms' themes and sites. | Everything below the camera. |
| B. A sibling game on shared crates | Iso has its own binary and its own authored scenes, on the game's rules crates and plugins. | The tick, intent, mover, fight, creatures and net. Not the world: iso keeps its own world layer and server. |
| C. Keep them separate | Iso stays an art and look lab. | Only what flows back. |

The developers chose A on 2026-11-08: "one game, just make sure its modularized enough to not collide". Why A: the developer asked for one ECS, one set of systems and one network. A server decides everything (D51), so B means a second world layer and a second server to keep in step for as long as both exist, and C does not answer the ask. Steps 0 to 6 below are the same under A and B, so the choice between them is not needed before step 7. Under C they are not built at all, so C has to be ruled out before step 1.

## Constraints

- **Modular enough not to collide** (D635, the developers): iso's camera, controls, look kit and figures are their own plugins and modules beside the third-person game's, talking through components, events and small shared types. Neither camera's work forks or breaks the other's: what assumes a horizon or a third-person eye takes the camera as an input, and a change for one camera keeps the other's checks passing.
- **Assets are reused, made with Tripo, never bought** (the developers, 2026-11-08: "i am not paying for assets right now. we will use tripo generally. we can reuse existing assets as much as we can"). Reuse what the repository has first, else make it with Tripo (D628). A licensed or non-redistributable file (a paid pack, a Bestiary-style licence) goes in the private repository `dcramer/terra-private` (D599's licences; `tools/characters/vendored.ts` finds it at `~/src/terra-private` or `$TERRA_PRIVATE`), never this public one. The non-fantasy biomes' people are iso's own cast, made with Tripo ("Own cast, Tripo").
- **The rules are plain Rust** (D28, D447): iso's movement and fight run in `terra_motion`, so a server can decide them (D51) and a client can predict them.
- **One tick** (D138): movement and the fight run in `FixedUpdate` at 64 Hz. The camera, clips and effects run each frame and only read.
- **Intent is the seam** (D51): iso's input does nothing but write the walker's `Intent`. The server writes other players' intents.
- **Layers** (D198): iso's camera and controls replace the walker layer's. Gameplay is never forked.
- **Kinds are data** (D136): the knight, ogre, treant and moose are entries in the bestiary and the move sets, never tables in iso's code.
- **The game never depends on a spike** (D133): anything that flows back moves into a game crate first.
- **The game's rules hold**: `libm` (D30), no `unsafe`, files and functions under their sizes (D396). Iso breaks D30 today, and D396 in `kit/effects.rs` (1,129 lines).

## What stays per camera

| Thing | Why it differs |
|---|---|
| The camera rig and framing | D629 asks whether it follows the hero or stays fixed. Settled within it: the hero stands about a tenth of the frame's height, with about 18 by 12 m of ground in view. |
| The controls | WASD moves relative to the screen (`screen_move` at the camera's yaw). In play a left click turns the hero to the cursor's ground point and swings there; clicks do not move him. The fixed camera's clicks select and send the party (`party.rs`, `command.rs`), with formations and pause. All of them write `Intent`. |
| Models, their LOD and budgets | Iso's figures are Tripo's one-mesh characters (D628). Their budgets are D630's open question; its recommendation for iso characters that are Terra's, as under A, is main Terra's budgets with levels of detail. The game's bodies are converted onto our skeleton. Either way the kind is shared: its capsule, speeds, moves and hurt zones are the kind's data, whichever mesh draws it. |
| The look kit's dressing for a frame seen from above | Contact grime, holes cut through crowns and roofs over the hero, light presets exposed for a frame that is all ground. |
| What a horizon needs | The iso camera never sees one, so it needs no far bake (D416), no far woods, and no sky except in reflections. |

## What is shared

| System | The game's | What iso takes |
|---|---|---|
| ECS world and tick | `terra_world::tick` (D138), instances (D468, D469) | An iso scene is an instance on the same tick |
| Intent and controls | `terra_walker::intent::Intent`, `controls::Action` (D80) | The hero's input, so it can be rebound |
| Controller | `terra_motion::mover` (D53), `Kind`, `gaits` (D585) | Every body's movement, held by the course (D457) |
| Combat rules | `terra_motion` moves, blade, contact, stamina (D452), `terra_creatures::melee` | The swing, the hit and the blow, decided by the server |
| Animation | The walker's `Acting`, inertialization (D631), pace (D585) | Clip choice and blends, once these are lifted off the player's body |
| Creatures and NPCs | `terra_creatures` (bestiary, mind, gait), `terra_folk` | Wildlife, foes and townsfolk |
| World and terrain (A only) | `terra_voxel`, `terra_land`, `terra_climate`, `terra_world`'s ground, water and flora | The realm under the camera |
| Server and replication, when built | D34, D51, D471 | One set of messages. Iso is a client that differs only in its camera |

## Order of work

Each step is small and lands on its own with its check, movement first (developers: "same systems").

0. **Into the workspace** (done 2026-11-08). `spike/iso` becomes `crates/terra_iso`, a front end allowed Bevy as `terra_sandbox` is (D28, amended in the same commit), run by `cargo iso`, with the `f32` trigonometry moved to `libm` and `kit/effects.rs` split. `loop.ts`, `noise.json`, and the biomes move with it, and the iso-art skill, `tools/iso-art/sheet.ts` and D634 are pointed at the new path. *Check:* D634's loop shows every still unchanged, within its noise.
1. **Input to intent, on the tick.** `play::control` splits in two. A reader in `RunFixedMainLoop`, before the fixed loop, writes `Intent`, as `intent::read` does. `Intent` carries no direction: it holds the actions held and pressed and an aim ray, and the walk turns Forward, Back, Left and Right into a wish along `View.forward`. So WASD becomes those actions, and the click becomes a press with the ray from the cursor. The walk runs in `FixedUpdate` under `Sim::Act`, and what is shown is interpolated (`tick::Smoothed`). *Check:* a test that one held key gives the same path at 30 and at 144 frames a second; `--press` counts in ticks.
2. **The mover.** The hero becomes a `terra_motion::mover::Bodies` body, with an iso `Query`:
   - an exact-distance ground: a plane, or a layout's height field whose distance bound is honest;
   - `footprint` circles as `Obstacles`;
   - walls as boxes;
   - the pond as water.

   `ask()` is private to `terra_walker::movement` and reads the walker's `Intent`, `View`, `Stance` and `Stamina` components. It is cut to plain inputs, then moved into `terra_motion` and made public. The speeds come from a `Kind` fitted to the knight's clips (D585), not from constants. *Check:* the controller course runs the iso kind; a test that the hero, walking each layout's route, never enters a footprint; the films are judged (D634) and the stills stay unchanged.
3. **Split the walker.** `WalkerPlugin` becomes a movement-and-intent plugin and a camera plugin, so iso can bring its own camera. The walk also builds its query from `Res<Planet>`, flora's obstacles and the builds, with up from the planet's centre. Its ground source becomes the world's to give, since in steps 2 to 6 the hero walks iso's layouts under A too. `ArriveAt` is already set from above, as building and crossings set it, but `spawn` also reads the `Planet` and `Start`. *Check:* the game's walker tests and the controller scorecard are unchanged, and the sandbox shot is within noise.
4. **The hero is a player.** It gets `Player`, `LocalPlayer`, `Motion`, `Stance` and `View`, with `View.forward` set to the camera's yaw on the ground. A swing steers the walk through `Stance.steer`, as melee does, and clips are chosen by `Motion`'s velocity. `View.forward` is both the walk's frame and where a body faces while it works (`toward`), and melee's lock turns it toward its target. Iso moves by the screen and faces the cursor, so the walk's frame needs a field of its own first. *Check:* a test that a swing holds the hero in place, facing the click.
5. **The fight's rules.** An iso knight move set is written, timed by `phases.ts` on the Tripo attack. `moves::Runner`, `blade::Edge` swept against hurt capsules, and `contact::Fighting` replace the cone and `BLOWS`. On the game's side, `phases.ts` already takes `--socket` and `--tip` but finds the hips by the name `pelvis`. `moves.ts` must accept a Tripo skeleton, and the blade's `HILT_M` and `TIP_M` (`terra_motion::blade`) become per-kind data. *Check:* `cargo xtask combat` over the iso set, and a duel of the knight against the ogre (D530).
6. **Creatures, people and animation**, a step each:
   - the Tripo kinds become bestiary entries;
   - the mind (`think`, `Pace`, bolt, temper) is lifted out of `terra_creatures` behind a ground-and-nav query trait;
   - `nav.rs` moves into a plain crate whose paths become mover wishes, beside `terra_town`'s A* on a town's path graph, with crowd avoidance (Recast/Detour's `dtCrowd`, zlib);
   - inertialization, masked layers and pace are lifted off the player's body (D542 for Tripo rigs).

   This is most of the game-side work, and the Bevy coupling of `terra_creatures`, `terra_folk` and `terra_walker` goes to the agents as a technical decision. *Check:* each system's own scorecard.
7. **The world.**
   - Under A, the iso camera looks at the game's realm. First the forest start, shot as a still at the iso camera and judged by D634's loop against the forest's references. Then the kit's ground, scatter and finishes become the realm theme's data on `terra_land`, and iso's authored layouts (a ruin, a camp, a street) become sites placed by rule.
   - Under B, iso keeps its layouts as its own world source, behind the same `Query`, instance and tick.
8. **The server**, when D34 is built. Iso is one more client: it sends `Intent` and draws what is replicated. Nothing in the server is iso's.

## What flows back to the game

- **Ground** (`kit/ground.rs`, `ground.wesl`): scanned layers laid at their true size by masks and blended by height per pixel. The game's ground is coloured per vertex at 0.5 m, with the layers' sizes in code.
- **Scatter as data** (`kit/scatter.rs`): density, clumps, tilt and sinking by mask, for flora's and cover's rules.
- **Finishes** (`kit/finish.rs`): each model's materials regraded at load for its biome's light, so packs' showroom finishes weather.
- **Reflections** (`kit/reflect.rs`): the scene captured into a cubemap from a point, for a town's glass and wet stone, beside `skylight.rs`.
- **Contact grime** (`kit/contact.rs`) under props, beside the screen-space contact shadows.
- **Effects** (`kit/effects.rs`): one fixed pool of quads and lights, with nothing spawned at run time. **Weather** (`kit/weather.rs`): rain as streaks and mist lying low, for [weather.md](weather.md). **Laid floors** (`kit/laid.rs`) for towns' paving.
- **Play**:
  - `nav.rs`, the first grid pathfinder round props (the game's other A* runs on a town's path graph or carves a dungeon's corridors);
  - `footprint.rs`, colliders from a mesh's bounds;
  - `strike.rs`'s per-body copy of materials for the hit flash (glTF materials are shared, so one flash lit every ogre);
  - `bite()`'s contact height;
  - `react()`'s flinch for a kind with no hit clip.
- **The loop** (D634): the adversarial judge and the no-loss ratchet, for the environment skill.

## Risks

- **The voxel ground under a top-down camera.**
  - The frame is all ground. At 0.5 m voxels (`terra_voxel::mesh::VOXEL_M`) there are about 36 by 24 cells in view, so the transvoxel facets and the blotches of per-vertex cover show where iso's masks are per pixel, and triplanar tiling repeats.
  - Caves, overhangs, roofs and canopies hide the hero; `foliage.rs`'s holes have to become a general cut-away.
  - The camera's yaw holds against the local tangent frame of a round planet, so screen north drifts near a pole.
  - Digging remeshes the ground right under the camera.
- **Budgets for a thousand players** (D52). A crowd seen from above is all close. D628's caps are each kind's share of about a million triangles a frame, and hundreds of 24k-triangle figures pass that million. So LOD goes by how many are on screen, not by distance. The server's cost does not depend on the camera; a smaller view only allows a nearer radius of interest.
- **What the 2.5D camera needs.** The game's systems that assume a horizon have to take the camera as an input: the far bake, distance fog, clouds, and LOD by the eye's `Viewpoint`. A click aims by a ray onto the real ground, not onto a plane, and figures, hits and paths have to read from above (D634).
- **The walker's split is the real work.** `terra_walker` is 20k lines and shaped by the planet; taking it whole brings the planet, flora, the building layout and instances.
- **The mover on iso's ground.** The mover's crease rule assumes an exact distance, and a height field's distance overestimates on a slope. `movement_mover.rs` proves the rule on the game's own field, and iso's ground needs the same proof.
- **Films change.** With the mover, bodies take new paths round props, so the D634 ratchet sees differences. The stills stay unchanged by construction, and the films are judged.
- **Rigs.** Tripo's bones, scale and root motion differ from our skeleton's, which is the cost of lifting the animation. D628 plays Tripo's clips as made and never converts them, so each Tripo kind gets its own move set timed on its own clips (step 5). Carrying the game's clips across by D542's retarget would go against D628.
- **Intent on the wire.** Under D51, `Intent` is what the server receives. Click-to-move, or a stick's analogue direction, cannot be put in held actions. Either a route follower writes them client-side, as `follow_route` does, or `Intent` gains a wish or a goal.

## Questions for the developers

1. ~~A, B or C (D635).~~ A, 2026-11-08.
2. Under A, is the iso camera the game's only camera, or one of two? Building, for instance, may want the third-person view. With two cameras, players in one world see differently: from above, iso sees round corners and behind a body, and third-person sees to the horizon. That matters wherever players fight players.
3. One game has one art style (D14), but D634's bar leans further into realism than D14's lightly stylised realism. Which holds?
4. Pause cannot exist on a shared server (D51). Party orders can, as intents for companions, but only in real time. Keep pause for solo play, or drop it with the fixed camera (D629)?
5. Are iso's other places (the neon street, the wasteland) realms by theme ([settings.md](settings.md)), or iso's alone?
