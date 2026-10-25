# Can a fluid-simulated campfire move as real ones do?

2026-10-25, fire round g22. Question: five rounds (g17-g21) tuned the marched noise field's lobe clocks, pinch and tear, and its splits stay at 1.65 or more a tongue a second against the real campfires' 0.99-1.19 ([bar](2026-10-25-fire-motion.md#tongues-fire-round-g18)), its dusk lifetime at 0.16 s against 0.04-0.12. The tracker counts lobes rising past each other as forks. Does a fluid-simulated fire, filmed in the same view, reach the real rates where the noise field can't, and if so, what would it cost to use one in the game?

## How shipped games get their fire's motion

Their motion comes from a fluid simulation run offline, never from noise tuned by hand. Few games run that simulation at play time.

- **Flipbooks of baked simulations are the norm.** The flame is a gas simulation made in Houdini (Pyro) or EmberGen, rendered to a sprite-sheet flipbook. That flipbook is played on particles, often with motion vectors so fewer frames read smoothly. The games surveyed in [2026-10-19-how-games-draw-fire.md](2026-10-19-how-games-draw-fire.md) all draw flipbook sprites: The Witcher 3, Cyberpunk 2077, Frostbite, Decima, Doom, God of War, Enshrouded and Valheim. EmberGen's game output is flipbooks with fire, smoke, normal and motion-vector channels, as well as OpenVDB ([JangaFX](https://jangafx.com/software/embergen), [realtimevfx thread](https://realtimevfx.com/t/embergen-beta-released-real-time-fluid-sims-for-instant-flipbook-generation/9915)).
- **RDR2 has no public account of its fire.** Bauer's SIGGRAPH 2019 talk on RDR2's atmosphere covers its raymarched clouds and fog, and "particle fog volumes" for fires and smoke-filled rooms ([slides](https://advances.realtimerendering.com/s2019/slides_public_release.pptx), [course page](http://advances.realtimerendering.com/s2019/index.htm)). It says nothing of how the flames themselves move. Particles with baked flipbooks are the likely answer, but that is inference.
- **Unreal offers both paths and recommends baking for games.**
  - Niagara Fluids simulates gas on a grid at play time. Epic's guidance is that 2D grids "are more efficient and better suited for games". 3D grids cost more memory and GPU and suit hero effects and cinematics, and either kind "can be baked into a flipbook" ([Niagara Fluids reference](https://dev.epicgames.com/documentation/unreal-engine/niagara-fluids-reference-in-unreal-engine)).
  - Since 5.3, Sparse Volume Textures play an imported OpenVDB sequence as a looping volume, drawn by the Heterogeneous Volumes path, with frames prefetched ([Heterogeneous Volumes](https://dev.epicgames.com/documentation/en-us/unreal-engine/heterogeneous-volumes-in-unreal-engine), [StreamableSparseVolumeTexture](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/StreamableSparseVolumeTexture)).
  - Fire packs on Fab ship as SVTs baked from VDB caches ([example](https://www.fab.com/listings/4502bbd6-fc7c-4665-a97d-af1fa0f39500)).
  - The published guidance gives no milliseconds or megabytes for either path.
- **Real-time grid fire exists but has not shipped as everyday fire.** NVIDIA's Flow is an adaptive sparse voxel grid for fire and smoke ([docs](https://docs.nvidia.com/gameworks/content/artisttools/flow.htm)), and its forerunner FlameWorks ([SIGGRAPH](https://history.siggraph.org/?p=72939)). Neither has a shipped campfire we could find. GPU Gems 3, chapter 30, ran a grid fluid simulation in Hellgate: London for smoke. It gives the memory as about 41 bytes a cell: 32 for the simulation's fields and 9 for voxelising obstacles ([chapter](https://developer.nvidia.com/gpugems/gpugems3/part-v-physics-simulation/chapter-30-real-time-simulation-and-rendering-3d-fluids)). Far Cry 2's fire spreads by simplified rules, because the research methods "can rarely run in real-time". How its flames are drawn is not described ([interview](https://gamedeveloper.com/game-platforms/interview-how-i-far-cry-2-i-s-fire-fuels-spreads)).
- **Baked loops are cut and blended.** EmberGen sets a loop range, and artists tune it until the exported book's seam is gone. A higher timestep can break deterministic looping ([forum](https://forums.jangafx.com/t/looping-correctly-in-timeline-but-not-in-flipbook/1800)). Another artist averaged groups of four of 288 frames into 72 frames, as motion blur, so the book would not strobe ([realtimevfx](https://realtimevfx.com/t/bruno-afonseca-sketch-48/19028)).

Our field is the departure. It makes motion from noise clocks where shipped games bake it from a simulation, so the spike asks whether a simulation's motion clears the real campfires' rates.

## Answer

No sim met the bar. A Mantaflow campfire stays under the splits bar of 1.2, at 0.0-0.8 a tongue a second, where our field reads 1.65 or more. It meets dusk lifetime (0.03-0.06 s against 0.04-0.12) and, with a sharp flame edge, the tongue count and the area's pumping. It misses sheds (0.2-0.9 against 1.0 or more) and lean (2-5 deg against main's 8.8 and the real 7.6-24.5). Its tongues are born at 1-7 a second, against the real fires' 12-19.

Its splits are low because nothing in it forks. It is one body of flame rising straight up in still air. That bears out the g21 reading that our forks are lobes crossing each other. It does not give the real campfire's tongues, which tear off, lean and shed.

Of the levers tried:

- **Vorticity matters.** Raising it from 0.1 to 0.4, with flame vorticity from 0.5 to 1.0, lifted sheds from 0.31 to 0.91 and lean from 2.6 to 3.8 deg.
- **The flame's edge matters as much as the sim.** Emission as flame^3 instead of ^1.2 is the reaction zone's sharp falloff, as a blackbody's T^4 gives. It raised tongues above half height from 1.3 to 1.9 and births from 3 to 6-7 a second, and the area's sd/mean from 0.08 to 0.15. Both of the last two moved into the real range.
- **Resolution alone made it calmer.** Going from 96 to 160 cells on the long axis cut splits from 0.27 to 0.00 and births from 2.0 to 1.4. At 160 the field stays smooth unless vorticity or noise is raised with it.

What none of the three changed is lean, and it limits sheds. Real campfires burn outdoors in moving air, and these sims burn in still air, so a sim's next lever is a gusting side wind and patchier fuel.

## Method

- **Scene.** Blender 5.2.2, Mantaflow gas, on this machine. Scripts are kept outside the repository in `~/.cache/terra-review/fire/g22/tools`: `sim.py`, `run.sh` to bake, `render.sh`, `measure.sh` and `desk.sh`. Five 0.62 m logs stand as a teepee over a 0.34 m ember bed. The logs and bed are fire inflows with fuel 1.0 and temperature 1.5, made patchy by a drifting clouds texture. The domain is 1.6 x 1.6 x 2.2 m and adaptive, with burning rate 0.45, flame smoke 0.3 and max temperature 1.5.
- **Shading.** Emission from the flame field goes through a red-orange-yellow-pale ramp, with strength 160 x flame^p. Cycles OptiX renders it, 16-24 samples, denoised, Standard view transform.
- **View.** The camera is clipm.ts's, 2.6 m up and 3.95 m back, looking at 0.65 m over the fire, 29 mm on the 1600-wide frame. The 600x600 crop is rendered directly over dark ground.
- **Film.** Each sim runs 20 s at 30 fps, after 3 s to settle, and is measured with `node tools/fire/motion.ts <film> --seconds 20`, with no matte, as the real clips are.
- **Time.** Mantaflow's time is in grid units, not seconds. At 1x the sims flicker at 0.8-1.2 Hz median and peak at 0.6-0.8 Hz. Played 2.5 times faster, every sim's dominant peak lands at 2.0 Hz, the puffing two real clips show (1.9-2.2 Hz). That is the Froude-scaled rate a 0.3-0.5 m fire puffs at, about 1.5/sqrt(D) Hz. So 2.5x is the calibrated row: 8 s by dropping frames, which loses one-frame shed blobs, so its sheds read low. A bake with a larger `time_scale` would be the clean fix.
- **Cost.** Bakes ran on 24 CPU cores: 22 min at 96 cells, 28 min at 96 with vorticity, 74 min at 160. The caches take 1.1-3.5 GB. Each 600-frame render took 20-23 min on the RTX 3080.

| Sim (res, vorticity / flame vort., edge p) | speed | flicker med Hz | area sd/mean | tongues > 0.5 H | born /s | life s | sheds | splits | lean deg | r at no lag | top red % | tracks |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| real range (bar) | | 1.83-3.83 | 0.09-0.19 | 1.6-3.6 | 12-19 | 0.04-0.12 | ≥ 1.0 | ≤ 1.2 | ≥ 8.8 | ≤ 0.12 | 3-47 | 120-190 |
| main (g21 fixed film, dusk/night) | | 2.50 / - | 0.24 / 0.18 | 2.4 / - | 13.7 / - | 0.16 / 0.14 | 1.00 / 1.00 | 2.03 / 1.83 | 8.8 / 8.6 | -0.06 / -0.01 | 41 / 31 | 271 / - |
| A 96, 0.1 / 0.5, p 1.2 | 1x | 1.00 | 0.08 | 1.2 | 2.0 | 0.06 | 0.31 | 0.27 | 2.6 | 0.06 | 6 | 41 |
| A | 2.5x | 2.50 | 0.08 | 1.3 | 2.8 | 0.04 | 0.34 | 0.34 | 3.0 | 0.11 | 6 | 22 |
| B 160, 0.1 / 0.5, p 1.2 | 1x | 0.83 | 0.08 | 1.2 | 1.4 | 0.04 | 0.29 | 0.00 | 2.2 | 0.06 | 3 | 28 |
| B | 2.5x | 2.00 | 0.08 | 1.2 | 0.9 | 0.03 | 0.12 | 0.12 | 2.1 | 0.14 | 3 | 7 |
| C 96, 0.4 / 1.0, p 1.2 | 1x | 1.17 | 0.08 | 1.3 | 3.0 | 0.05 | 0.91 | 0.27 | 3.8 | 0.07 | 7 | 59 |
| C | 2.5x | 2.83 | 0.08 | 1.3 | 3.9 | 0.04 | 0.92 | 0.57 | 3.7 | 0.11 | 7 | 31 |
| B, p 3 | 1x | 1.00 | 0.21 | 1.4 | 3.4 | 0.04 | 0.22 | 0.36 | 5.3 | 0.04 | 0 | 66 |
| B, p 3 | 2.5x | 2.50 | 0.20 | 1.4 | 4.0 | 0.04 | 0.33 | 0.77 | 5.2 | 0.17 | 0 | 32 |
| **C, p 3** (best) | 1x | 1.17 | 0.15 | 1.9 | 5.8 | 0.05 | 0.76 | 0.52 | 5.2 | 0.03 | 0 | 116 |
| **C, p 3** | 2.5x | 3.00 | 0.15 | 1.9 | 7.3 | 0.04 | 0.31 | 0.61 | 4.7 | 0.11 | 0 | 58 |

The sims' rates rest on fewer tracks than the real clips' 120-190, so the rates are rough to about ±0.3. Their top-third red of 0% is the ramp, not the motion. The [one-sim blind test](2026-10-24-campfire-one-sim.md) found a whole fire drawn as one emissive volume reads as "a soft orange haze". This spike measured motion only, not look.

## What using one would take (an estimate; not built)

A sim that met the bar would come into the game as shipped games bring theirs, baked and played back, not simulated at play time.

- **A baked looping volume.**
  - **Size.** One 8-bit flame channel over the flame's box, 0.6 x 0.6 x 1.2 m. At about the marcher's step of 1 cm, 96 steps through it, that box is 64 x 64 x 128, 0.5 MB a frame raw or 0.25 MB as BC4. A 4 s loop of 120 frames is about 31 MB as BC4. At the sim's own 2.3 cm cells it is 32 x 32 x 64, about 8 MB. A second channel for soot doubles either.
  - **Sharing.** Every campfire shares one loop, offset in time and mirrored or rotated each, so a village costs the loop once.
  - **Looping.** Either crossfade the bake's last second into its first offline, as EmberGen and Houdini loops are cut, which costs one fetch a step at play time. Or play two copies half a loop apart with triangle weights, so each copy's seam falls at zero weight, which costs two fetches.
  - **Marching.** `fire.wesl`'s campfire marcher takes 96 steps and evaluates seven lobes and noise at each (`FIELD_STEPS`, `FIELD_LOBES`). Sampling the texture would replace that with one or two trilinear fetches a step, so its cost should be at most the field's. That field is inside docs/fire.md's 0.3 ms for one near campfire and 1.5 ms for a burning village. This is an estimate until a bench measures it.
- **A real-time grid, against it.**
  - **Size.** At C's 2.3 cm cells, a 32 x 32 x 64 grid over the flame (65k cells) takes about 2.7 MB at GPU Gems 3's 41 bytes a cell.
  - **Time.** Advection, vorticity, divergence and 20-40 pressure iterations make about 50 small dispatches a step. Dispatch overhead dominates at that size, so about 0.3-0.5 ms a fire on the RTX 3080 at 30 Hz, my estimate. Ten campfires would be 3-5 ms, two to three times the whole village's fire budget.
  - **Gain.** It would gain fire that reacts to wind and to the player. Epic keeps 3D grids for hero effects and cinematics for the same cost.
- **Recommendation.** Build neither yet: no sim meets sheds and lean. The next spike is one bake of C with a gusting side wind (a wind force field, a few m/s varying over seconds) and its `time_scale` set so 1x puffs at 2 Hz. If that meets sheds ≥ 1.0 and lean ≥ 8.8 with splits under 1.2, the path is the baked loop above, marched by our existing campfire marcher. That needs a design note before any game code. The time-varying wind would then have to be baked into the loop, which is a known limit of baked fire, against the grid's live wind.
