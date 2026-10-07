# Rust stack toolchain check

2026-10-06. Question: do Bevy and the `transvoxel` crate build and run from our machines, how long do builds take, and what does meshing cost?

A throwaway app in [spike/rust_stack](../spike/rust_stack) built a small planet from 512 independently meshed blocks with Bevy 0.19 and `transvoxel`.

| Measurement | Result |
|---|---|
| Clean build, Linux | 3 min 36 s with full debug information; 2 min 29 s with line numbers only |
| Clean cross-build for Windows from WSL | 3 min 49 s with full debug information; 2 min 19 s with line numbers only |
| Rebuild after a one-line change | 3.5 s |
| First compile against Bevy 0.19, by an agent that had not seen this version | Passed first time, after reading the examples that ship with the crate |
| Render inside WSL with no GPU | Works: 55 ms a frame at 1080p, screenshot saved |
| Render on the RTX 5090, two runs | Median frame 1.85 and 1.79 ms. 99th percentile 3.35 and 10.28 ms, so the tail is noisy; the developer's machine was in use. |
| Meshing one 32-cubed block, Linux | 6 ms |

Two problems found and fixed:

- **Full debug information makes the Windows build larger than 2 GB, and Windows refuses to load it.** Debug builds now keep line numbers only.
- **The system maths library on the Windows build was about six times slower.** Switching to the pure-Rust `libm` crate fixed it, as the table below shows.

| Meshing, 512 blocks on 32 threads | Linux | Windows build |
|---|---|---|
| With the system maths library | 173 ms | 1,129 ms |
| With `libm` | 162 ms | 189 ms |
| One surface block, system maths | 6.0 ms | 38.0 ms |
| One surface block, `libm` | 6.0 ms | 5.8 ms |

`libm` also gives the same answer on every platform, which the system libraries do not promise. The same slowdown is the likely reason the simulation tick was three times slower in the Windows build under Godot; that was not re-measured.

## Limits

- One small static planet with no level of detail, no streaming and no edits. It says nothing about the terrain system we still have to write.
- macOS was not tested.
- The mid-range and integrated GPUs were not tested.
