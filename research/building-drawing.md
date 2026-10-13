# Drawing bases a section at a time: the village bench

2026-10-13. What do fifty buildings cost to draw now that bases are laid and merged a section at a time on the workers (D213, step 3 of [building-plan.md](../docs/building-plan.md)), against what they cost drawn whole on the main thread, and do they meet the plan's bars?

## How it was measured

`cargo xtask bench village` lays fifty buildings on the yard (every kind: logs, stone, hewn timber, stone with logs and stone with hewn timber; every roof: gable, hip and lean-to; every plan: rectangle, L, T and U; turned every way) and looks at them from the village's corner; `village-bare` is the same view with nothing built. Release builds, offscreen at 1920x1080, three runs each, on the RTX 5090 and on the Ryzen 9 9950X3D's integrated Radeon (`WGPU_POWER_PREF=low`). "Before" is the commit before step 3 (9892cc9) with only the village and its scenes added. The machine was shared with another agent's builds and runs throughout (load 5 to 22 on 32 cores), so frame times are noisy; the differences below are well past the noise.

## Results

Frame times, median / 99th percentile / worst, in ms:

| | RTX 5090 | Integrated Radeon |
|---|---|---|
| Before (9892cc9): `village-bare` | 10.1 / 32.8 / 77.8 | 95.5 / 110.3 / 120.7 |
| Before: `village`, whole bases redrawn on the main thread, no detail by distance | 20.0 / 43.6 / 75.3, 594 hitches in 600 frames | did not finish: over 350 ms a frame |
| After (be6d22f): `village-bare` | 4.62 / 8.66 / 11.10 | 90.8 / 97.6 / 122.2 |
| After: `village` | 4.74 / 8.79 / 11.43, no hitches, within the budget | 152.4 / 201.0 / 235.9 |

The bare view got cheaper between the two on the fast GPU (D214's temporal anti-aliasing and eye adaptation came in), so each `village` is compared with the bare view of its own build.

What the bench says beside its frames (after, on the workers of the 5090's machine):

- A section with anything in it is laid and merged on a worker in a median 4.1 to 5.0 ms with its textured detail and 3.5 ms far only; a section of a log cabin near, all detail, about 8 to 15 ms (measured in `terra_build` on Linux).
- An operation (a window carved into a house's wall, a step of it taken out, and both undone) is applied and checked in a median 0.055 to 0.069 ms, the slowest 0.34 ms (0.12 to 0.15 ms before, on the same machine).
- The main thread does no laying or merging: a change marks its sections and the workers do the rest.

## What it shows

- **On the fast GPU** fifty buildings now cost about 0.1 ms a frame over the bare view, its 99th percentile and worst frame within 3% of the bare view's, with no hitches; before, they cost 10 ms and stuttered every frame.
- **On the integrated GPU** the buildings still cost about 60 ms a frame over a bare view that is already 91 ms, almost all of it drawing triangles: the depth prepass, the shadow cascades and the main pass each draw the buildings again (measured with F3: the prepass rose by about 25 ms, the shadow pass by about 35 ms and the main pass by about 60 ms with the buildings in). Detail by distance (every piece within 25 m, a textured middle detail to 60 m, one baked mesh a section far off) took the buildings from 104 ms to 62 ms; before step 3 the scene did not run at all. Next: fewer shadow casters far off, an impostor past 180 m, a cheaper block surface near, and a middle detail for roofs' coverings.
- **The bars**: an operation applied and checked in under 0.1 ms is met. "No frame above the yard's for the same view" is met on the fast GPU, within noise, and not on the integrated GPU. A section laid and merged in under 2 ms is not met for sections with buildings in them: laying is about 1 ms (a roof is laid whole for every section it reaches), and merging a section near takes a few ms, a log cabin's full detail 8 to 15 ms, most of it generating its logs (D200). The middle and far details spare that for every section past 25 m, and none of it is on the main thread.
