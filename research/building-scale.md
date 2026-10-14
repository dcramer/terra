# Building at scale: three hundred buildings, a castle, and bases to the horizon

2026-10-14. What does building's drawing cost with hundreds of buildings near and far, and with one very large base, on the fast GPU and the integrated one, and where does the time go? The developers: "we're also going to need to make sure our building system is performant, both when they're in the distance (idk if they'd even be rendered?) and up close with dozens of buildings and LOTs of voxels", "also i want you to measure like 300 buildings", "im pretty sure we already have perf issues".

## How it is measured

Scenes in `cargo xtask bench` (D185, [development.md](../docs/development.md#measuring-performance)), on the yard at D13's leaning size (a 30 km radius, so the ground curves away as it will):

- `town300`: three hundred buildings from `terra_build::village::town`, every kit, plan (rectangle, L, T, U, two storeys, shed, deck) and roof of cubes, on eight streets, walked down a street through the middle at 6 m/s for 1,200 frames. `town300-rise` sees it from 30 m up at a corner, `town300-fly` flies over it 60 m up, `town300-bare` is the rise's view with nothing built.
- `castle`: one base of 141,502 cubes (`terra_build::castle`): an 80 m ward inside a curtain 3 m thick and 12 m high, six towers of six storeys, a keep of five, a hall, slate roofs; walked through the gate toward the keep. `castle-field` sees it from 270 m.
- `far`: three hundred buildings in hamlets from 200 m to 2 km round the start, looked round from 60 m up.

Release builds offscreen at 1920x1080; the RTX 5090 at the high preset, the Ryzen 9 9950X3D's two-unit Radeon at low (`WGPU_POWER_PREF=low`). Each run also reports triangles a frame by pass (the GPU's pipeline statistics), meshes drawn, process memory and the GPU's mesh memory, how long the bases took to lay cold, and how long sections took to stream in as the camera moved.

The machine was shared with other agents' builds throughout (load 35 to 60 on 32 cores), which makes the CPU-bound numbers noisy: `town300-bare` on the 5090 measured 17.8 ms median in one run, slower than the town from the same place.

## Before (1578a91 with the scenes added), one run each

Frame times, median / 99th percentile / worst, in ms:

| Scene | RTX 5090, high | Radeon, low |
|---|---|---|
| `town300` (street) | 16.3 / 32.7 / 39.8, 11 hitches | did not finish in 300 s: about 80 ms a frame |
| `town300-rise` | 8.7 / 20.7 / 22.1 | 80.9 / 116.0 / 137.9 |
| `town300-fly` | 6.6 / 17.9 / 19.2 | 78.3 / 111.4 / 117.8 |
| `town300-bare` | 17.8 / 44.1 / 49.4 (load 46) | 28.4 / 39.7 / 42.7 |
| `castle` (through the gate) | 5.8 / 15.3 / 18.5 | 83.4 / 143.7 / 152.1 |
| `castle-field` | 14.7 / 34.2 / 66.2 | 32.4 / 41.7 / 44.7 |
| `far` | 13.4 / 28.2 / 30.8 | 28.0 / 49.5 / 62.6 |

What the town holds and draws (5090):

| | `town300` | `town300-rise` | `castle` | `far` |
|---|---|---|---|---|
| Triangles a frame (prepass + main) | 10.9 M | 9.2 M | 8.2 M | 0.8 M |
| Meshes drawn | 4,732 | 3,002 | 941 | 351 |
| Meshes on the GPU | 2,923 MiB in 11,680 | 1,050 MiB in 6,359 | 1,347 MiB in 2,159 | 859 MiB in 5,879 |
| Process memory | 2,266 MiB | 2,198 MiB | 2,085 MiB | 2,170 MiB |
| Game's work on the CPU, median | 12.2 ms | 7.0 ms | 4.2 ms | 9.7 ms |
| GPU, median | 3.0 ms | 2.9 ms | 2.5 ms | 2.3 ms |

The bare view holds 103 MiB of meshes and draws 0.85 M triangles. Per base, then, the town costs about 3.1 to 9.4 MiB of the GPU's memory (1,050 MiB seen from the rise, 2,923 MiB once the street walk has made near detail of a street's houses) and about 1 MiB of the process's.

Laying from cold (the bases put up on the main thread, then laid whole and cut into sections on the workers):

- Putting up 300 bases from their shortcuts on the main thread: 12.7 s (40 to 110 ms a house), from working out every wall and room after every shortcut, the roofer's one cube at a time. Now settled once a run of fills (`Base::apply_shortcuts`); the castle's shortcuts went from never finishing in a test to 0.9 s.
- 300 bases laid whole in a median 6.8 ms, 2,235 sections cut and merged in a median 8.7 ms textured and 3.2 ms far, on four workers: 27 s on the 5090's run and 97 to 147 s on the Radeon's (the machine's load) from the first to the last on screen.
- The castle: laid whole in 0.74 s, 244 sections, 3.4 s to all on screen.
- Streaming as the camera moved: the street walk laid sections again 49 times, a median 82 ms each time and the slowest 3.3 s; the castle's walk 10 times, the slowest 1.1 s.

## What it shows

- **The fast GPU is not the limit; the CPU is.** The GPU spends 2.5 to 3 ms a frame on any of these views; the frames are the game's work on the main thread, 4 to 12 ms, and stutters as sections are put on screen.
- **Far bases cost memory for nothing.** Every section of every base is kept laid at least at its far detail, wherever the camera is: 860 MiB of meshes for 300 hamlet houses seen as specks, and bases vanish entirely past 600 m, so the far view shows none of the hamlets past it.
- **Near detail is heavy.** A street of houses near makes 2.9 GiB of mesh and 11 M triangles a frame, half of it the depth prepass.
- **The integrated GPU is GPU-bound**: 28 ms bare, 80 ms with the town in view at the low preset.

Profiled with Bevy's Chrome trace on the street walk (5090): the main thread's update was 17.5 ms a frame and the render thread's 14 ms, both pipelined, the render thread's in graph encoding and submission (5.5 and 3.2 ms), which grow with the meshes drawn; the build systems themselves under 1 ms. The integrated GPU's frame at the rise was its main pass (48.7 ms) and depth prepass (20 ms) over 2.9 M triangles: past 90 m (its preset halves distances) every house was its proxy of 5,400 triangles on a few dozen pixels, as `cargo xtask sections` counts (near 307,000 a house, middle 96,000, far 20,000, proxy 5,400).

## What was changed (D278)

- **A hull past the far detail**: from 180 m a section is the hull of its cubes (`terra_build::hull`, Unreal's approximated mesh), coplanar faces merged, roofs at their pitch, its kits' mean colours: a few hundred triangles a house.
- **Merged cells to the horizon**: from 600 m each cell of a base, four sections a side, is one hull, to 3 km. Bases are seen as silhouettes out to where the haze takes them, where before nothing was drawn past 600 m.
- **Streaming**: sections past 740 m are let go and not laid, and laid again inside 690 m; a base with nothing laid lets go of its copy and its pieces.
- **A memory count** per base, CPU and GPU (`build.memory`, and `Builds hold:` at the end of a benchmark).

## After (D278), one run each, the RTX 5090 at high (load about 20)

| Scene | Before | After |
|---|---|---|
| `town300` (street) | 16.3 / 32.7 / 39.8, 11 hitches | 12.9 / 33.6 / 45.1, 13 hitches |
| `town300-rise` | 8.7 / 20.7 / 22.1 | 8.4 / 19.4 / 21.3 |
| `town300-fly` | 6.6 / 17.9 / 19.2 | 11.1 / 34.3 / 43.1 |
| `town300-bare` | 17.8 / 44.1 / 49.4 (load 46) | 8.1 / 19.6 / 26.7 |
| `castle` | 5.8 / 15.3 / 18.5 | 10.1 / 26.2 / 35.1 |
| `castle-field` | 14.7 / 34.2 / 66.2 | 9.8 / 21.8 / 32.8 |
| `far` | 13.4 / 28.2 / 30.8 | 8.5 / 19.2 / 30.9 |

The bare view, nothing built, measured 8.1 ms median with the game's own work 6.1 ms: on this shared machine the world alone is over the 99th-percentile budget, and single runs on it move by tens of percent either way (the fly-over and the castle's walk, which D278 does not change near, came out slower), so the frame times above are indications. What changed for certain, as the runs count it:

| | Before | After |
|---|---|---|
| `far`: meshes on the GPU | 859 MiB | 253 MiB, the bases' 106 MiB of it: 0.2 MiB a base far off, 3.2 MiB the most |
| `far`: from the first section laid to the last on screen | 20.9 s | 1.8 s, 2,584 of 2,872 sections set aside |
| `far`: CPU held by the bases | | 53 MiB, 0.15 MiB a base far off |
| `town300-rise`: triangles a frame | 9.2 M | 5.8 M |
| `town300-rise`: meshes on the GPU | 1,050 MiB | 829 MiB |
| `town300`: first laid to last on screen, cold | 27.4 s | 23.8 s |
| `castle`: what the base holds | | 1,088 MiB on the GPU walking in, 63 MiB from the field; 41 MiB on the CPU |

The town's street walk is still over the budget in its 99th percentile and hitches: its near detail, 300,000 triangles a stone house, is put on screen a section at a time as the walk reaches it (2.8 GiB of meshes, the slowest burst 3.5 s), which D278 does not touch.

## After (D278) on the integrated Radeon at low, one run each

| Scene | Before | After | GPU, after | Triangles a frame |
|---|---|---|---|---|
| `town300` (street) | did not finish in 300 s | 86.5 / 143.5 / 349.0 | 57.1 ms | 3.7 M |
| `town300-rise` | 80.9 / 116.0 / 137.9 | 56.2 / 69.5 / 81.6 | 48.3 ms (82.1 before) | 2.6 M (5.8 M) |
| `castle` | 83.4 / 143.7 / 152.1 | 66.0 / 123.7 / 133.1 | 44.5 ms | 3.3 M |
| `castle-field` | 32.4 / 41.7 / 44.7 | 30.7 / 43.4 / 47.0 | 24.2 ms | 1.0 M |
| `far` | 28.0 / 49.5 / 62.6 | 26.7 / 38.7 / 42.5 | 22.2 ms | 0.8 M |

`far` and `castle-field` meet D239's 33.3 ms median, as the bare view's 28.4 ms nearly does alone.

Then, from the same runs' counts: the hull's roof courses merged into one plane each and the sides between cut cubes dropped, 630 triangles a house (`cargo xtask sections`: 31,564 for the village's fifty, against the old proxy's 267,770); and at the low preset the far detail handing over to the hull at 120 m rather than 180 (60 m as drawn there). On the Radeon, one run each:

| Scene | After the first change | After both | GPU | Triangles a frame |
|---|---|---|---|---|
| `town300-rise` | 56.2 / 69.5 / 81.6 | 36.2 / 58.2 / 91.4 | 30.9 ms | 1.6 M |
| `castle` | 66.0 / 123.7 / 133.1 | 67.0 / 111.2 / 121.6 | 44.9 ms | 2.8 M |
| `far` | 26.7 / 38.7 / 42.5 | 21.6 / 33.6 / 74.4 | 17.6 ms | 0.7 M |

The town from the rise is within 3 ms of D239's 33.3 ms, its GPU at 30.9 ms; the castle walked through is its near and middle detail, the stone's surface displaced a few centimetres a step, which D278 does not change.

A `town` view in `cargo xtask lodcheck` (forty buildings down a street from 25 m up, the detail scale swept from half to twice) finds nothing popping as the far detail hands over to the hull: no frame changes by a patch, worst 0.00%. The town and the castle near do not: their frames are the main pass and depth prepass over the far and middle detail of the houses within 90 m (its preset halves distances), 20,000 and 96,000 triangles a house, and the street walk's worst frames are sections put on screen as it reaches them.
The town's street walk is still over the budget in its 99th percentile and hitches: its near detail, 300,000 triangles a stone house, is put on screen a section at a time as the walk reaches it (2.8 GiB of meshes, the slowest burst 3.5 s), which D278 does not touch. Numbers on the integrated GPU follow.

## An edit, and the game's CPU time a frame (D281)

The second goals review's findings 4 and 5: what one edit costs, and where the game's CPU time goes among buildings.

**One cube taken out of the castle** (`terra_build`, release, one thread; a throwaway driver, not kept):

| | Before | After (D281) |
|---|---|---|
| The operation applied and checked (`Base::act`) | 0.14 to 0.16 ms | 0.13 to 0.19 ms: the columns are kept cube by cube now, which was not where the time went |
| Sections the edit sets waiting | 342, every one of the castle's | 198: those round it and the rows through it |
| The base laid again, on a worker | 337 ms (laid whole: 343 to 418) | 270 ms |
| The sections waiting cut to see which lay the same | 55 ms on the main thread | on the worker, after laying |

A castle edit still waits about 300 ms on a worker before it is drawn: a row of logs and a band of dress are laid from the whole row, and laying a row's sections takes in every row crossing them, so the castle's rows through the edit look at most of it. Laying only the pieces of the rows an edit is on, kept by row, is what would take it further.

**A stone house going up** (`going-up`, the 5090, load 10): 122 changes, each on screen a median 17.3 ms after it was made, 21.4 ms at the 90th percentile and 240 ms at worst; the slowest section a median 11.1 ms on its worker. Frames 4.13 / 9.03 / 12.12 ms, no hitch, the game's own work 3.4 ms.

**The street walk, system by system** (`town300`, Bevy's Chrome trace, the 5090, load 27 to 42): the game's update 14.4 ms a frame, the render thread's 12.9 ms, pipelined. What the main schedule spends a frame, the costliest first:

| System | ms a frame | Worst |
|---|---|---|
| `bevy_time::fixed::run_fixed_main_schedule` (the walker's `movement::walk` 0.7 of it) | 1.96 | 13.6 |
| `bevy_ui::layout::ui_layout_system` | 0.95 | 6.0 |
| `bevy_render::mesh::allocator::allocate_and_free_meshes` | 0.78 | 10.4 |
| `bevy_render::view::visibility::collect_visible_cpu_culled_entities` | 0.46 | 3.8 |
| `bevy_camera::visibility::calculate_bounds` | 0.41 | 5.1 |
| `terra_world::terrain::choose` | 0.33 | 17.6 |
| `bevy_transform` (dirty trees and propagation) | 0.52 | 9.3 |

and on the render thread `render_system` 4.7 ms, 2.8 of it submitting command buffers. No one system of the game's is the frame: the update is many engine systems over many entities (some 4,700 meshes drawn), on a machine whose load stretched every one of them, so the frames' worst moments were systems of a few hundredths of a millisecond taking 20 to 50 ms when their threads were not run. The one that was ours, `terrain::choose`, worked out which chunks of ground are wanted on the main thread every 4 m walked, 17 ms each time; it is done on a worker now (76a68a3), and the next run, at load 20, had the game's update at 9.8 ms, frames 11.9 / 29.3 / 132 ms, with `choose` gone from the costly systems. The rest is entities and draws, for D278's merging and the render thread's encoding to bring down.

## Up close: the block surface (D284)

Near detail was the street walk's cost: the village's fifty buildings drawn near come to 14.2 million triangles (`cargo xtask sections`), and every exposed face of a block kind's cube was ten by ten squares, 200 triangles, its stone's relief displaced into it, wherever it was within 25 m.

Now a face has points only where its shape bends, across the rounding of its open edges, and is otherwise one flat square; the relief is the material's normal map and parallax; faces of different detail share their points along the lines they meet on, so the surface stays closed. The village's near detail is 10.8 million triangles; what is left, by what it is drawn in:

| Near | Triangles | MiB |
|---|---|---|
| Logs: logs, ends, decks, hewn members, daub | 6.9 M | 354 |
| Roofs: coverings, boards, logs | 2.2 M | 256 |
| Block kinds: surface and floors | 0.17 M (3.97 M before) | 13 |
| Block kinds: dress (corner stones, plinths, copings, lintels) | 0.85 M | 64 |

Cubes cut to a roof or a shape then took points only where their cut crosses or bends, and those near a bedded lintel or sill one every 10 cm: the block kinds' surface fell from 0.78 M to 0.17 M, and with their dress they come to 109 triangles a cube of them and 1.55 MiB a building. The budget (D284), held by tests in `cargo xtask check` and by `cargo xtask sections`: 64 triangles a square metre of surface open to the air (a stone house had 800 before, 172 after the first pass), 120 a cube with dress, 4 MiB a building. The dress modules (D258), 0.85 M, are now most of what is left of the block kinds; the timber kinds' logs, at 5.5 million triangles, and the roofs' modules are the street walk's next cost, and are not block surfaces.

Stone, earth and brick, before and after, at 2, 5 and 10 m on the 5090: the same, the stone's joints as deep by parallax as by displacement at 2 m, earth's lifts and lumps kept in its shape, a wall's silhouette along its open edge straighter where the stone's displacement had broken it.

**The street walk after D284** (`town300`, d8e8cfe, recorded in `benchmarks.csv`), on a machine that did not go quiet, so the load is noted with each:

| GPU | Load | Median / p99 / worst | Hitches | Triangles a frame | Frame's parts |
|---|---|---|---|---|---|
| 5090, high | 71 to 104 | 15.4 / 29.9 / 77.2 ms (three runs, the middle; the quietest of them 8.6 / 24.8 / 77.2) | 62 | 9.05 M | the game's update 12.1 ms, the GPU 3.2 |
| Radeon, low | 24 to 86 | 78.9 / 115.7 / 541.5 ms (one run of three; two stuck past 300 s) | 1,197 | 2.36 M | the game's update 21.0 ms, the GPU 55.3: its main pass 36.0, the depth prepass over the same triangles 6.5 |

The 5090's frame is the main thread's, the CPU systems the performance work owns. The Radeon's is its main pass, at five times the depth prepass over the same 1.2 M triangles: shading the pixels, not the triangles' number, is most of what it spends, so near triangles cut further show there only in the prepass and the vertex work. Before D284 the walk was 9.2 / 72.5 / 123.2 ms on the 5090 (5d73d6b, load 20 to 40) and 64.3 / 96.5 / 130.8 on the Radeon.
## The castle's edits, operations and aim (the third goals review, finding 6)

Throwaway drivers in `terra_build`, release, one thread, on the castle's 141,502 cubes.

**An operation** (`Base::act`, a cube taken out at the top of a wall, five places): 0.11 to 0.45 ms before, 0.026 to 0.079 ms after, under the 0.1 ms bar. The time was the check that nothing is left hanging: a flood from each of the cube's neighbours looking for the ground, depth first, which from the top of a curtain wall wandered along the castle before it went down. It now goes lowest first (a best-first search on height), finding the ground down the wall in a few dozen steps; what is found joined or lost is the same either way.

**The hammer's aim** (`Base::raycast`, which the aim casts twice a frame): every cube of the base was tested against the ray, 3.0 ms a ray in the castle, 6 ms of every frame spent aiming. It now walks the ray cell by cell from where it enters the base's bounds (Amanatides and Woo's grid traversal, as voxel engines cast rays), 0.6 µs a ray; a test checks it meets the cube testing every cube does, on 3,600 rays round twelve of the village's buildings.

**A cube taken out of the castle, to the screen**: the castle is no player's base, so it was timed off the screen, each step as the game's workers do it, every section drawn once near and kept first, as the player's own base is. On a machine at load 55 (laying the castle whole took 1.38 s, against 0.34 s quiet): 153 to 192 sections reached; the base laid again in 1.1 to 1.8 s; those reached cut in 48 to 99 ms; 1 to 5 sections changed, drawn again from how they were in 56 to 559 ms, the slowest 174 ms. Laying again is most of it, as long as laying the castle whole: the rows through the edit reach across all of it, and laying any section of them takes in every row crossing it. Quiet, that is about 0.3 to 0.5 s from the edit to the screen. Laying again only the rows an edit is on, with each piece kept by the row it came from, is what would bring it down to the house's 17 ms.
