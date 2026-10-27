# The Bevy volumetric clouds branch measured against the sky's budget

2026-10-26, sky.md stage 0. Question: can Máté Homolya's `volumetric-clouds-nubis` branch of Bevy (github.com/mate-h/bevy at 0d852d0a7931, 2026-10-22, MIT or Apache-2.0) draw cumulus and an overcast deck within D515's budget (clouds, their shadow map and shafts within 1.5 ms at the high preset on D25's RTX 3080, 2 ms at the low preset on the integrated GPU), with the eye below, inside and above cloud (D519), and does it look photographic (D516)? The method and the alternatives are [2026-10-26-volumetric-skies.md](2026-10-26-volumetric-skies.md).

## Answer

1. **It looks right.** From below at noon its cumulus are convincing: flat grey bases, bright tops, a cirrus veil; the overcast deck reads as a broken grey sheet from below and a sunlit field from above, with its gaps' shadows on the ground.
2. **It does not fit the budget where D519 needs it.** On the RTX 5090, a card some 2 to 2.5 times a 3080, its clouds cost 1.1 ms under cumulus and about 2 ms in and above the cumulus layer: over the 3080's whole 1.5 ms on the faster card. Only the overcast deck (0.73 to 0.76 ms) is near the line, and on a 3080 it would still be about 1.5 to 1.9 ms.
3. **Port its parts, not the branch** (D515): its march, its coarse distance field and its time-sliced cloud shadow map into our patches, with the cost in and near cloud cut and measured against the budget when stage 2 builds it.

## How it was measured

A spike (deleted, D133) built the branch on its own Bevy (main of 2026-10-20, newer than ours) for Windows and drew a plain scene in it: a meadow, three domed hills, Bevy's atmosphere in its ray-marched mode (the branch's clouds need it; Terra uses lookup textures, D212) and the branch's cumulus and cirrus layers. Overcast was forced onto its weather map (the DYCOMS-II stratocumulus profile, cover 1, base 575 m, about 270 m deep), which has no painted map wired. Offscreen at 1920x1080, noon, the eye still, per-pass GPU timestamps through Bevy's render diagnostics (spans added on the branch's cloud field, shadow filter and haze passes); a run is 240 frames to settle and 600 measured. "Clouds" is the frame's sum of the cloud passes (march, reconstruction, shadow map and its filter, the distance field amortised); "sky" is the atmosphere's lookup tables, haze, sky and environment passes, which the clouds add to.

Every run is on the RTX 5090. The machine was busy with other agents' work (CPU load 10 to 190, the GPU 0 to 27% busy before each run): the medians agree with short checks on a quieter machine (cumulus in the layer 1.99 ms over 30 frames against 2.21 over 600), the 95th percentiles do not, so read the medians.

| Clouds, ms (median / p95) | Below (1.8 m) | In the layer | Above |
|---|---|---|---|
| Cumulus, high preset | 1.14 / 2.54 | 2.21 / 3.95 | 1.93 / 3.72 |
| Cumulus, low preset | 0.94 / 1.06 | 1.72 / 3.42 | 1.48 / 3.10 |
| Overcast, high preset | 0.73 / 0.97 | 0.76 / 1.17 | 0.74 / 2.62 |
| Overcast, low preset | 0.63 / 0.67 | 0.64 / 0.86 | not run |

- The march is most of it: 0.90 ms of the 1.14 below cumulus, 1.98 of the 2.21 in the layer. Its step budget rises about elevenfold for a ray that starts inside the layer (96 to 1024 at the medium preset), which is the in-layer cost. Twelve probes at 1.2 and 2 km in a first round gave 1.85 to 2.28 ms where cloud was round the eye and 0.55 ms below the local base.
- The cloud shadow map is cheap, as its time slicing promises: 0.17 ms under cumulus, 0.10 under the deck (0.12 and 0.06 at the low preset). The distance field is about 0.02 ms a frame amortised.
- The sky's own passes take 0.34 to 0.54 ms, of which the haze (marched per pixel in front of cloud) 0.19.
- The low preset saves only 15 to 25% on the 5090.

**On the RTX 3080** (D25's target), estimated, not measured: the 3080 has 29.8 TFLOPS and 760 GB/s against the 5090's 104.8 and 1792, and is some 2 to 2.5 times slower in games. Cumulus would cost about 2.2 to 2.8 ms below and 4 to 5.7 ms inside, against 1.5.

**The look**, in stills at noon (`~/.cache/terra-review/sky-stage0/`): cumulus from below and from within the layer are photographic, with shadows on the ground; the overcast deck from below is grey with blue in its gaps, from above a sunlit field. Inside a cloud the frame is an even grey fog with faint banding near the ground; cumulus edges close to the eye are soft rather than sharp, which D519's close detail asks for.

## Not measured

Stopped by the developer once the numbers above answered the question: the integrated GPU (so whether the low preset can reach 2 ms there is unknown, though a preset that saves a fifth on the 5090 makes it unlikely), the lookup-texture atmosphere, a moving eye (reprojection's cost and smear in flight), a low sun, overcast above the deck at the low preset, the branch's path-traced reference, and our own scenes at our Bevy.

## What porting costs

The branch's footprint in the engine is small: the atmosphere module of bevy_pbr (about 11,000 lines added, the debug dumps and reference tracer included), bevy_light's atmosphere (+238 lines), pbr_lighting.wesl (+86, cloud shadows on surfaces) and one mesh-view binding for the cloud shadow map, plus small changes to volumetric fog and light probes. Between Bevy 0.20 and the branch's base the atmosphere files moved little (under 60 lines), but the mesh-view bindings were reworked, so the binding and the branch's renaming of the atmosphere's GPU data are redone on 0.20. It would go into the bevy_pbr patch we already carry (D184; D485 and D496 change it, the fog shader among it) and a new bevy_light patch.

Gaps against sky.md: no weather map from data (the branch's texture field is documented but not wired, and its cloud kind is unused), cloud time from Bevy's clock rather than the world's (D315), heights that would have to ignore our atmosphere's scaling transform (D334), and an upstream pull request (#26253) closed on 2026-10-20 as work in progress, unreviewed.

## What stage 2 must cut and measure

The branch already traces at half resolution, a pixel in four a frame (a sixteenth of the full frame, as Horizon Zero Dawn), and rebuilds the rest by checkerboard reprojection, so the cut is in the march itself:

- fewer, longer steps inside cloud and far from the eye, as Nubis Evolved does, instead of an elevenfold budget for every ray in the layer; leaping empty air on the distance field from the eye;
- the cloud round the eye as fog lit as the cloud, as sky.md's Inside and above says, so the march starts past it;
- a quarter-resolution trace at the low preset, and the 2D layer on the same weather map if the integrated GPU still cannot hold 2 ms (D515).

Measured then, with `cargo xtask bench` on a quiet machine: both GPUs, high and low presets, cumulus and overcast below, inside and above, a moving eye through cloud (reprojection's smear and cost), the lookup-texture atmosphere, the 95th percentile as well as the median, and stills against the path-traced reference.

## Sources

- Homolya, `volumetric-clouds-nubis`, github.com/mate-h/bevy at 0d852d0a7931; bevyengine/bevy pull request #26253.
- NVIDIA's published specifications of the GeForce RTX 3080 and RTX 5090.
- Schneider, "Nubis, evolved", SIGGRAPH 2022 Advances.
