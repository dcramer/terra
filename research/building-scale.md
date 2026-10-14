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

What was changed for it, and the numbers after, follow in this note as the work lands.
