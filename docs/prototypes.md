# Prototypes and milestones

Small builds, each answering one question, with criteria written before it starts.

Builds 1 and 2 and the tech demo below were made on Godot before the move to Rust (D11). They were deleted on 2026-10-12 and are in git history; their numbers are in [research/godot-spike.md](../research/godot-spike.md). The Rust milestones at the end are what comes next.

## 1. Tech spike

**Question:** can Godot with Voxel Tools do a seamless, diggable, co-op planet, and is it pleasant to develop with agents? This closed D9 and D12.

The spike answers five questions, in order of how likely each is to disqualify the engine. Each gets the smallest test that can answer it, on a bare planet with no game systems.

| # | Question | Smallest test | Pass when |
|---|---|---|---|
| 1 | Can two players dig the same planet and see each other's edits? | Two headless processes, one bare planet, dig operations sent over the network. | Both end with identical voxel data, and a client that joins late catches up. |
| 2 | Can our simulation drive the terrain and change it over time while keeping player edits? | Change a terrain input at runtime on a planet that has a dug hole. | The terrain changes and the hole survives. |
| 3 | Does the surface look and perform well from orbit to ground, and does digging feel right? | A developer flies the stock solar-system demo. | Both developers judge it good enough; 60 fps on a mid-range GPU. |
| 4 | What do we get for fluids? | Read the module's docs. | We know what exists and what is ours to build. |
| 5 | Can both developers work on it with agents, on WSL and macOS? | Run tests 1 and 2 from both machines. | An agent can build, run and test without the editor GUI on both. |

**Status, 2026-10-04:** questions 1, 2 and 4 are answered and the engine is chosen (D12). Question 3 is answered for rendering but not for feel, and question 5 is answered for WSL but not macOS. Results are in [research/godot-spike.md](../research/godot-spike.md).

Also record, without a pass bar: how it behaves at 2 km and 20 km radius, and whether a double-precision engine build is needed.


## 2. Simulation toy

**Question:** is local-to-global terraforming legible and fun on its own? This tests D6.

Build: the coarse cell simulation on a globe with no game around it. Place heaters, gas generators and seeders; watch values spread; fast-forward.

**Status, 2026-10-04:** built, in `prototypes/sim_toy`, since deleted (2026-10-12; in git history). The rules are a first draft. It runs and its tests pass; whether it meets the bar below is for the developers to judge by playing it.

Pass when:

- You can tell what your actions did without reading numbers.
- There are real choices about where to act, not one best move.
- Different strategies give visibly different planets.

## 3. Beauty corner

**Question:** what does the game look like? This closes D14.

The direction is now set by [art.md](art.md): Forever Ago is the inspiration. The beauty corner is one fixed scene with terrain, a rock, a plant, a machine, a player and the sky, at three phases (dead, thawing, alive), in that style.

A first look test exists as the sandbox's `look` experiment. It has the light, haze and faceted shapes, and none of the foliage or designed models.

Pass when both developers would buy the game in one of the screenshots.

## Tech demo

Added 2026-10-04 at the developers' request, ahead of the beauty corner: the first build that joins the pieces into something playable. It was in `demo/`, deleted on 2026-10-12 and in git history. You could walk a voxel planet in first person, dig, place the three machines, watch the simulation change sea, ground and sky, and fly to orbit.

It is not a prototype of the game's design: there is no survival, no co-op and no art in it.

## Outpost scene

Added 2026-10-10 at the developers' request: a proof of concept worth showing someone. It is the sandbox's `play` experiment, in Rust on Bevy: an animated figure with a builder tool at a furnished base by a lake. See the [sandbox README](../crates/terra_sandbox/README.md) and [research/outpost-scene.md](../research/outpost-scene.md).

It is a picture of where the game could end up, not a prototype of its design: there is no survival, no terraforming, no voxel terrain and no co-op in it.

## Rust milestones

A working plan, still to be determined. The engine would be built as a foundation and four parallel tracks, described in [engine.md](engine.md). These are the checkpoints. Each is measured against the Godot numbers in [research/godot-spike.md](../research/godot-spike.md), on the RTX 5090 and on the integrated GPU.

| # | Track | Checkpoint | Done when |
|---|---|---|---|
| F1 | Foundation | The shared contracts and the generation toolkit | The contracts compile and are used by a trivial stand-in. The toolkit gives identical output on Linux, macOS and the Windows build, checked in CI. |
| A1 | Terrain | Terrain core, no window: shape and material, detail levels, sphere edits with harvest yields | The criteria in [terrain.md](terrain.md). |
| A2 | Terrain | Terrain on screen, streaming as the camera moves | The flight tests from the Godot spike pass: worst frame no higher than Godot's at 100 and 400 m/s, and terrain keeps up. A dig causes no visible hitch and shows the material underneath. |
| B1 | Stand-in world | The app shell: window, scripted camera paths, frame-time capture, screenshots | A scripted run prints frame times and saves a screenshot on Linux, on the Windows build and on macOS. |
| B2 | Stand-in world | A walkable stand-in planet, with sky and sea | A scripted walk and a fast camera pass over the planet have no frame over 16 ms on the RTX 5090. The scripted checks pass. (Was the transition to orbit and back, dropped with space, D4.) |
| C1 | Things on the ground | Scattering on the stand-in planet, with procedural rocks | 100 thousand instances within 150 m cost no more than they did in the Godot load test. Removing one is an operation. |
| D1 | Networking | Two peers in one process share ordered operations and snapshots | Both end in the same state. A peer that joins late catches up. No window needed. |
| M | All | Real terrain replaces the stand-in | Everything the Godot demo did, at frame rates at least as good on both GPUs. |

**Status, 2026-10-10:** F1 is done, apart from the simulation snapshot contract, which waits for the simulation's rules. The toolkit's pinned outputs pass on Linux, on macOS, natively on Windows, and on the Windows build we cross-compile from WSL.

The sandbox (D46) covers most of B1: a scripted run prints frame times and saves a screenshot on Linux under the software renderer and on the Windows build on the RTX 5090. macOS and the integrated GPU are not yet measured.

The beauty corner follows M, since the art should be built on the engine we ship on.

Every checkpoint reports frame times from the real play loop, not only from scripted runs (D25).

## MVP: the first playable

Proposed 2026-10-11, for the developers to edit. Being built as the tech demo, the sandbox's play experiment: `cargo xtask play` plays it, `cargo xtask loop` plays its loop by script and checks every step. Where it stands is below.

**Question:** is the core loop of [game design 001](proposals/game-design-001/README.md) fun, with almost nothing else around it? It is a prototype of that proposal (D89), built to answer this question before the proposal is adopted or anything is spent on scale.

If the loop is not fun with one monster and one settlement, more content will not fix it.

### Where it stands

As of 2026-10-11.

| Part | State |
|---|---|
| One small world, day and night | Voxel terrain, dug with the pickaxe for stone and ore, saved; no night yet |
| Gathering with separate tools (D96) | Built |
| Crafting at stations, timed and queued (D109) | Built: workbench, smelter, forge, campfire |
| A claim, upgradable (D94, D104) | Built |
| Building on the voxel grid (D48) | Rooms and items on the build grid; a room cuts into the ground it stands on, once trees and rocks are cleared (D131). Blocks on the voxel grid are missing |
| Backpack, equipment, action bar (D84); a belt quiver (D118) | Built; wear and repair (D99) missing |
| One wildlife, hunted for meat and hide | Built: deer, with the bow |
| One monster, forming where magic pools (D103), holding a prisoner | Built: spiders at a glowing hollow |
| Combat: melee, a bow, block, dodge, stamina (D97) | Built; hit zones and critical hits (D119) missing |
| One settlement, a request, the prisoner to rescue, who runs the forge (D90, D95) | Built, the village from building parts (D108) |
| Skills by doing (D88) | Built, archery among them (D117) |
| Hunger and food, death and the dropped pack (D101) | Built |
| The map, only where you have been (D76) | Built |
| A local server (D51) | Missing |
| Saving and loading | Built |
| Feel: swing animations for the axe and pickaxe, a better harvesting experience, a small notice of what you gained when you harvest or pick something up, hit effects when damage lands, and the like | Asked for by the developers on 2026-10-11. Built: what you gained, listed at the right as it comes in; a blow sprays blood or ichor and jolts what it struck; being hurt reddens the edges of the view and shakes it. Swing animations are under way separately |
| Camera: zoom in and out with the mouse wheel, later all the way to first person | Asked for by the developers on 2026-10-11. Built: the wheel, or = and -, from 1.4 to 9 m behind; while the hammer has a part out that the wheel turns, it turns that instead. First person is to come |
| Building: move what is being placed up and down, not only turn it, except what must stand on the ground | Missing; asked for by the developers on 2026-10-11 |
| Done when: a developer plays it, an agent drives it, save and reload, frame times | The agent's run passes; the developers' play-test, frame times on the integrated GPU, and a save-and-reload check in the script are to do |

### In it

- **One small world**, smooth voxel terrain (D9), one biome, day and night. Everything in reach within a few hundred metres; the developers want the MVP small in scale, not a large world.
- **Gathering** with separate tools (D96): an axe for trees, a pickaxe for rock and ore, a knife for fibre and hide.
- **Crafting** at a workbench, a smelter and a forge, taking time and queued (D109), from a handful of materials: wood, stone, fibre, hide and one ore. Tools made of better material work more (crafting.md).
- **A claim** you place, 40 metres on a side, upgradable to 80 and 160 (D94); building on the voxel grid with shaped rooms (D48); the world heals outside it.
- **The backpack, equipment and action bar** (D84), with wear and repair (D99).
- **One kind of wildlife** that grazes, flees and can be hunted for meat and hide.
- **One kind of monster**, forming at one place where magic pools (D103), dangerous to go near, holding a prisoner.
- **Combat**, third person (D97): one melee weapon and a bow; attack, block, dodge; stamina.
- **One settlement**, built from the same parts as the player's buildings (D108), with a few people, a request in words, and the prisoner to rescue (D95). Rescued, they come home with you and run the forge (D90).
- **Skills by doing** (D88): a few, such as chopping, mining, a weapon, crafting.
- **Hunger** and food (D101); **death** drops the pack, costs a little skill progress, and you wake at your bed.
- **The map**: only where you have been, and where you are (D76).
- **A local server**: the game talks to a server inside it (D51).
- **Saving** and loading.

### Not in it

Essences and abilities, raids, factions beyond the one settlement, portals and other worlds, taming and mounts, trade, many players, the spread. Essences are the first addition after it.

### Done when

- **A developer plays it from a fresh start**, with nothing explained beyond the controls, and finds their way through: gather, craft, claim, build a shelter, eat, find the settlement, rescue the prisoner, bring them home, have them make a better tool. Both developers say whether it was fun, which is the question.
- **An agent can drive the same loop** (D69) and see that each step worked.
- **Save and reload** restores the claim, what is built, what is carried and the recruit.
- **Frame times** meet D25 in the play loop on the RTX 5090, and are reported on the integrated GPU.

### What it needs first

All engine work, mine: the voxel terrain (A1, A2), building on a voxel grid, collision and a character controller on voxels (D53), creatures with behaviour and pathfinding, combat, the local server, and saving. Placeholder content (materials, recipes, weapons, numbers) is chosen as it is built, as the inventory's was.
