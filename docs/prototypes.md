# Prototypes and milestones

Small builds, each answering one question, with criteria written before it starts.

Builds 1 and 2 and the tech demo below were made on Godot before the move to Rust (D11). They stay as working references. The Rust milestones at the end are what comes next.

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

**Status, 2026-10-04:** built, in [prototypes/sim_toy](../prototypes/sim_toy). The rules are a first draft. It runs and its tests pass; whether it meets the bar below is for the developers to judge by playing it.

Pass when:

- You can tell what your actions did without reading numbers.
- There are real choices about where to act, not one best move.
- Different strategies give visibly different planets.

## 3. Beauty corner

**Question:** what does the game look like? This closes D14.

Build: one fixed scene with terrain, a rock, a plant, a machine, a player and the sky, at three phases (dead, thawing, alive), in both treatments from [art.md](art.md).

Pass when both developers would buy the game in one of the screenshots.

## Tech demo

Added 2026-10-04 at the developers' request, ahead of the beauty corner: the first build that joins the pieces into something playable. It is in [demo](../demo). You can walk a voxel planet in first person, dig, place the three machines, watch the simulation change sea, ground and sky, and fly to orbit.

It is not a prototype of the game's design: there is no survival, no co-op and no art in it.

## Rust milestones

The path back to the Godot demo, in Rust, then past it. Each milestone is measured against the Godot numbers in [research/godot-spike.md](../research/godot-spike.md), on the RTX 5090 and on the integrated GPU.

| # | Milestone | Done when |
|---|---|---|
| R1 | Terrain core: `terra_voxel` meshes a planet at several levels of detail, with sphere edits, and no window | Tests pass without a GPU. Output matches Voxel Tools on the same density function within a stated tolerance. No cracks between detail levels. |
| R2 | Terrain on screen: `terra_game` draws it from orbit to ground and streams it as the camera moves | The flight tests from the Godot spike pass: worst frame no higher than Godot's at 100 and 400 m/s, and terrain keeps up. A dig causes no visible hitch. |
| R3 | Playable: first-person player, digging, the simulation driving ground, sea and sky, the three machines | Everything the Godot demo does. The scripted checks pass (land, walk, dig, refill). Frame rates on the ground at least match Godot's on both GPUs. |
| R4 | Co-op: two players in one world | Both see each other move, dig and place machines. A player who joins late catches up. |

The Godot build is retired when R3 passes. The beauty corner follows R3, since the art should be built on the engine we ship on.

Every milestone reports frame times from the real play loop, not only from scripted runs (D25).
