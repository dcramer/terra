# The integrated GPU at 30 fps, and sections under 2 ms

2026-10-13. Building step 3 left three bars unmet ([building-drawing.md](building-drawing.md)): a section laid and merged on a worker took 3.5 to 5 ms against 2 ms; on the integrated GPU fifty buildings added about 60 ms; and the integrated GPU missed its budget with nothing built. What do they cost now, after D238 (a base laid whole once a change, sections cut from it and made only at the detail they are seen at), D239 (quality presets, low for the integrated GPU) and D240 (crossfades TAA resolves, eyes that open indoors)?

## How it was measured

As `cargo xtask bench` runs its scenes (D185): release builds, offscreen at 1920x1080, 240 frames to settle and 600 timed, on the RTX 5090 (three runs, middle run's median and 99th percentile, worst of any) and on the Ryzen 9 9950X3D's integrated Radeon, two compute units (`WGPU_POWER_PREF=low`; one run each, the frames are 30 to 400 ms). "Before" is main at 592d00d, which has no presets and draws everything at what is now high; "after" is this change, high on the 5090 and low on the Radeon, as each chooses. The bench now prints each render pass's GPU time, which is where the costs below come from. The machine was shared with other agents' builds (load 1 to 25 on 32 cores), so the 5090's numbers move by a millisecond or so between runs; the forest was run back to back to compare.

## Results

Frame times in ms, median / 99th percentile / worst:

| Scene | 5090, before | 5090, after | Radeon, before | Radeon, after (low) |
|---|---|---|---|---|
| village | 7.52 / 15.31 / 28.55 | 7.23 / 12.35 / 15.13 | 287.6 / 367.3 / 463.7 | 34.3 / 40.8 / 43.8 |
| village-bare | 6.61 / 12.49 / 44.05 | 6.44 / 11.58 / 13.75 | 188.2 / 236.6 / 262.2 | 25.3 / 29.0 / 31.6 |
| yard | 6.26 / 10.38 / 13.00 | 6.51 / 10.87 / 26.70 | 187.0 / 229.1 / 273.9 | 25.1 / 29.5 / 35.3 |
| world | 8.62 / 12.93 / 54.97 | 8.93 / 13.89 / 18.55 | 324.0 / 388.4 / 2207 | 71.2 / 87.4 / 97.6 |
| forest | 9.95 / 15.71 / 19.68 | 10.12 / 15.99 / 19.18 | 375.5 / 444.3 / 484.8 | 90.6 / 111.4 / 218.4 |

The 5090's forest is the back-to-back pair; the 5090 is bound by the game's own work (4 to 6 ms) and the high preset draws as before, so it is unchanged within noise, and screenshots from the same vantages match.

Sections, by `cargo xtask sections` on one thread: the fifty bases laid whole in a median 2.1 ms each; a section with anything in it cut in 0.01 ms and merged in a median 1.9 ms near (full and middle detail), 1.1 ms middle, 0.3 ms far (before: about 1.4 ms laying and 4.5 ms in all near). The slowest, a stone house's whole ground section near, is still 60 to 120 ms, most of it the block surface's 10 steps a face displaced on the CPU. In the game, with the workers busy together, the median textured section is 2.3 to 3.8 ms.

## What it shows

- **Where the integrated GPU's frame went**, before, in the village: volumetric haze 90 ms, the main pass 76, the prepass 20, ambient occlusion 15, then TAA, bloom and the rest; about 50 ms more not timed (the shadow cascades). With nothing built the haze was 92 ms of 195.
- **Low takes each of those down**: no haze shafts, ambient occlusion, contact shadows or bloom (about 110 ms); the world drawn at three fifths of the screen and scaled up (about two thirds of every pixel's cost); two cascades to 60 m; flat relief; the ground's layers at one size; the air's tables halved; detail and grass at half. The bare yard holds 30 fps (25 ms) and fifty buildings add about 9 ms (the main pass 4.6, the prepass 2.1, their shadows), leaving the village just over (34 ms).
- **The far buildings were too many triangles**: 2.9 million at their far and proxy details, logs, deck planks and dress drawn in full section at 180 m. As boxes and three-point profiles they are 1.1 million.
- **The world and the forest stay near 13 fps.** What is left is foliage: the main pass and the prepass draw the trees' and plants' cards and blades (the forest's main pass 44 ms, prepass 15, at three fifths), and the world's trees have no levels of detail at all. Thinning the grass to a tenth moved the world by less than 2 ms. Shipped games at this end draw distant trees as impostors and merge foliage by area (D47); that is the next step.
- **Two traps found on the way**: a second camera showing the scaled picture was a view of the world to Bevy, so the sun cast its cascades for it too, and it stood at the planet's middle, where Bevy measured every `VisibilityRange` from it, so no shadows were drawn; it stands on its own render layer and with the main camera now. And halving the atmosphere's transmittance table was left alone, as the sun's light through the air is read from it.
