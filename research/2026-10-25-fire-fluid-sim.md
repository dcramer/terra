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

## Round g23: a gusting side wind, in real seconds

Question: started from C (96 cells, vorticity 0.4, flame vorticity 1.0, emission flame^3), with the sim's clock in real seconds and a gusting side wind, does the sim meet every motion target? Scripts are in `~/.cache/terra-review/fire/g23/tools` (`sim.py` takes `key=value` settings).

### Setting it up

- **Real seconds.** `time_scale` 2.5, the speed g22 calibrated by dropping frames, here baked in, with up to 10 substeps a frame for the CFL number. The baked velocity grid confirms the clock: Mantaflow's frame is 0.1 × 25/fps × `time_scale` of its time units, and with that the flame gas rises at 3.2-3.4 m/s. A 30 kW fire's intermittent flame rises at about 1.9 Q^(1/5), 3.8 m/s ([McCaffrey 1979](https://nvlpubs.nist.gov/nistpubs/Legacy/IR/nbsir79-1910.pdf)). The fuel patches drift 2.5 times as fast a frame, as C's did at 2.5x.
- **Wind is a push on the flame gas, not a moving air mass.** Four low-resolution probes (48 cells, 10 s held at each strength, 2-3 min each) set it:
  - Blender's wind field with its default `flow` 1 drags the gas's whole velocity toward the wind's, the rise included. The flame stopped rising (0.1-0.2 m/s) and lay over 59-88° at every strength.
  - A wind-tunnel inflow (a slab over the upwind face, a smoke inflow of no density, its initial velocity the wind) moved no air, with no density and with a trace of it.
  - The wind field with `flow` 0 pushes only where there is gas, as Blender applies fields to smoke, and leaves the rise at 2.6-3.4 m/s. Strength 0.25, 0.5, 1 and 2 gave the flame gas a mean drift of 2.4, 3.6, 6.5 and 12 m/s, from the baked velocity grid. This one is used.
- **Gusts.** The strength is a mean times a lognormal envelope exp(0.6x - 0.18) of a smooth signal x of periods 2.7-11 s, so lulls are about a third of the mean and gusts about two and a half times it, the gust factor of wind near the ground. The wind veers ±20° about the side axis on its own slow signal, and Blender's per-sample noise on the strength equals the mean. D is a breezy evening: mean 0.09, for a drift of about 0.8 m/s gusting to 2.5. E is a still night at half that. Both blow across the view, because the camera sees lean only across it. A breeze of 2-4 m/s at 10 m is about 1.2-2.5 m/s at a campfire's height, by the log wind profile.
- **The top's red.** motion.ts counts a pixel red if its hue is under 25° and it is above the flame cut, a luma of about 127. At that brightness the Standard view clips red, so a plain deep red reads as orange. Above red_lo the emission colour turns, by height, toward linear (1, 0.1, 0.06), which clips to a red hue over a wide range of brightness. That is the cooler, sootier top of a real flame. Tried on C's frames 100-159, it turned red from 0.4 to 0.7 m for a top-third red of 46%, and from 0.5 to 0.85 m for 22%; the second is used. It reads slightly pink rather than orange-red, a look to judge, not a motion measure.

### Result

No sim met every target. F met eight of the nine and missed sheds, at 0.27 against 1.0 or more. Each sim ran 20 s at 30 fps after 3 s to settle, at its own speed with no frames dropped, and was measured with `motion.ts --seconds 20`. The drift is the flame gas's mean sideways speed over the film, from the baked velocity grid, with its sd over time.

| Sim (vorticity / flame vort., wind strength, noise; drift) | flicker med Hz | area sd/mean | born /s | life s | sheds | splits | lean deg | r at no lag | top red % | tracks |
|---|---|---|---|---|---|---|---|---|---|---|
| target | 1.83-3.83 | ≤ 0.19 | 12-19 | 0.04-0.12 | ≥ 1.0 | ≤ 1.2 | ≥ 8.8 | ≤ 0.12 | 3-47 | |
| C p3 at 2.5x (g22, frames dropped) | 3.00 | 0.15 | 7.3 | 0.04 | 0.31 | 0.61 | 4.7 | 0.11 | 0 | 58 |
| E 0.4 / 1.0, 0.045, 0.045; 1.09 ± 0.46 m/s | 2.67 | 0.18 | 14.2 | 0.07 | **0.56** | 1.18 | **8.0** | 0.10 | 16 | 283 |
| D 0.4 / 1.0, 0.09, 0.09; 1.59 ± 0.53 m/s | 2.50 | **0.20** | 12.7 | 0.06 | **0.27** | **1.33** | **8.2** | 0.10 | 10 | 252 |
| **F** 0.8 / 1.5, 0.06, 0.18; 1.77 ± 0.56 m/s | 2.50 | 0.19 | 13.1 | 0.05 | **0.27** | 0.85 | 9.5 | 0.05 | 11 | 261 |

Bold is a miss. All three dominant peaks are 2.17-2.19 Hz, the real puffing. Bakes took 31-40 min on 24 cores, and 1.6-1.8 GB each. Renders took 15-19 min on the RTX 3080.

- **Real seconds and wind fixed what g22 could not reach.** Births rose from 6-7 to 13-14 a second, inside the real 12-19. The tracker found 250-280 tracks, against the real clips' 120-190. Lean reached 9.5 deg in F.
- **Wind costs sheds.** Sheds were 0.56 in the lightest wind and 0.27 in the two stronger ones. Shed blobs last 0.03-0.05 s, one or two frames. A push applied only where there is gas carries a torn-off blob along with its tongue instead of leaving it behind, so fewer detach. Real wind moves the air around the flame too.
- **The tongues' lean lags the gas's.** The gas leans 17-28 deg, but the tongues' median is 8-9.5 deg. Most measured tongues stand among the logs, where the gas is still rising straight from the fuel. Stronger wind (D) moved it 0.2 deg and broke splits and area.
- **Vorticity bought splits back.** Doubling vorticity with more wind noise (F) took splits from 1.18-1.33 to 0.85 and lean from 8.0-8.2 to 9.5. Sheds did not move.

The recommendation stands: build neither a baked loop nor a live grid yet. A sim's last miss is sheds, which needs wind that moves the air and not only the flame's gas. Mantaflow in Blender gives no inflow for that, as the probes found. The next try would be a guiding velocity field (Mantaflow's guides) or a sim in EmberGen or Houdini, whose wind is an ambient velocity. That is a step up in tooling; it is the developers' call whether sheds alone is worth it.

## Round g34: the fire carried upwind through still air

Question: g23's wind pushed only the flame's gas, so torn blobs rode along with their tongue. If the air itself moves past the fire, do tongues shed? Mantaflow has no ambient wind, so the whole fire (ground, logs, ember bed, camera) is carried upwind through still air, which in the fire's frame is the same thing (Galilean). The domain stays still and holds the whole path, with F's cell size, and the adaptive domain simulates only the cells round the gas. A domain that moved with the fire did not carry its gas: a 48-cell probe's flame died. The carry speed is F's settings' mean times g23's lognormal gust envelope, veering ±20°. Everything else is F's (96 cells, vorticity 0.8, flame vorticity 1.5, `time_scale` 2.5), with no wind field. Scripts are in `~/.cache/terra-review/fire/g34/tools` (g33's, with the sims' folder settable for the render worker).

- **G**: mean 1.18 m/s, 0.22-3.06 m/s, 25.8 m travelled. **H**: mean 1.96 m/s, 0.36-5.11 m/s, 43.0 m travelled.
- Baked on the render worker (Blender 5.2.3, 32 cores, both at once) in 63 and 65 min, 7.2 and 7.6 GB. Mantaflow printed "Invalid dt" on 17 and 55 steps and went on. Rendered as F was: frames 91-690, Cycles OptiX, 24 samples, flame^3, red from 0.5 to 0.85 m. That took 11 min each on the RTX 4090.
- All three films were measured by today's motion.ts with `--seconds 20`. F measures as it did in g23.

| Sim (carry m/s) | flicker med Hz | area sd/mean | born /s | life s | sheds | splits | lean deg | r at no lag | top red % | tracks | targets met |
|---|---|---|---|---|---|---|---|---|---|---|---|
| target | 1.83-3.83 | ≤ 0.19 | 12-19 | 0.04-0.12 | ≥ 1.0 | ≤ 1.2 | ≥ 8.8 | ≤ 0.12 | 3-47 | | |
| F (g23; pushed gas, 1.77 m/s drift) | 2.50 | 0.19 | 13.1 | 0.05 | **0.27** | 0.85 | 9.5 | 0.05 | 11 | 261 | 8 |
| G (1.18) | 2.33 | **0.37** | 18.3 | 0.10 | **0.33** | 0.67 | 11.2 | 0.11 | **0** | 363 | 6 |
| H (1.96) | 2.33 (peak 0.50) | **0.31** | 12.5 | 0.08 | **0.19** | 0.55 | 11.0 | 0.02 | **0** | 248 | 6 |

Bold is a miss.

- **Moving air did not bring sheds.** They were 0.33 and 0.19 against F's 0.27; the target is 1.0. Shed blobs still last 0.04 s. Pushing only the gas was not what held sheds down.
- **The flame lies low and clings to the logs.** It is about 10% shorter in frame (168 px against 185) and wraps the logs instead of standing as one body. Its top third stays under the red band's 0.5 m, so top red falls to 0. That is a render setting and could be fixed, but sheds and area could not.
- **The gusts swing the whole flame.** Area sd/mean is 0.31-0.37, about twice F's 0.19. The carry swings from a fifth to two and a half times its mean, and the flame's size follows it. H's dominant flicker became the 0.5 Hz gust swing.
- **Lean and births rose.** Lean was 11.0-11.2 deg against F's 9.5. G's births were 18.3 a second, near the top of the real 12-19.
- F remains the best sim, missing sheds alone. The recommendation in g23 stands. Sheds are not a matter of which frame the wind is in, so the sim path's remaining lever is the turbulence that tears a tongue (resolution, or a solver with ambient turbulence, such as EmberGen or Houdini). Which, if either, is the developers' call.

## Round g35: turbulence that tears tongues

Question: g34 found sheds are not a matter of which frame the wind is in. Does more turbulence (finer cells, more vorticity, Blender's noise upres) tear tongue tips off so they rise away? The bakes and calls were set before measuring (`~/.cache/terra-review/fire/g35/rule.md`, scripts in `g35/tools`).

- **I**: F's settings at 128 cells (F 96), vorticity 1.6 and flame vorticity 3.0, substeps up to 14. Mantaflow's vorticity confinement adds strength × (N × curl) in grid units, so at finer cells F's numbers act 96/128 as strongly. Undo that and 1.6 / 3.0 is F's × 1.5.
- **J**: the same bake with noise upres 2 (a 256-cell noise grid) at Blender's default strength 1.0. One `bake_all` into an ALL cache bakes both. I renders it with the noise off, J with the noise on. A modular noise bake on a finished data bake came out empty in two 32-cell probes, and its film was black.
- Baked on the render worker (Blender 5.2.3, 32 cores) in 80 min, 4.6 GB of base grids and 9.9 GB of noise. Rendered as F was, in 13 min (I) and 16 min (J) on the RTX 4090, and measured by today's motion.ts with `--seconds 20`. F measures as it did in g23.

| Sim | flicker med Hz | area sd/mean | born /s | life s | sheds | splits | lean deg | r at no lag | top red % | tracks | targets met |
|---|---|---|---|---|---|---|---|---|---|---|---|
| target | 1.83-3.83 | ≤ 0.19 | 12-19 | 0.04-0.12 | ≥ 1.0 | ≤ 1.2 | ≥ 8.8 | ≤ 0.12 | 3-47 | | |
| F (g23; 96 cells, 0.8 / 1.5) | 2.50 | 0.19 | 13.1 | 0.05 | **0.27** | 0.85 | 9.5 | 0.05 | 11 | 261 | 8 |
| I (128 cells, 1.6 / 3.0) | 2.50 | **0.23** | **10.4** | 0.04 | **0.21** | 0.46 | 11.1 | 0.06 | 7 | 209 | 6 |
| J (I + noise upres 2) | 2.83 (peak 6.0) | **0.84** | **9.8** | 0.08 | **0.00** | 0.00 | **-** | -0.03 | 47 | 197 | 5, degenerate |

Bold is a miss. By the calls set beforehand, neither is a gain: sheds stayed under twice F's (0.54). Neither the noise strength 2 bake nor the 160-cell one was earned.

- **Finer cells calm the flame, even with confinement scaled up.** As g22 found, I's flame is a smoother body. Fewer tongues are born (10.4 against 13.1), fewer split (0.46), and sheds fell to 0.21. Shed blobs still last 0.03 s, one frame.
- **Mantaflow's noise does not tear a flame; it powders it.** J's flame is a dim, speckled cloud (the flame cut falls to luma 67, no pale or yellow at all). The tracker finds no attached tongues to shed from, so its zeros mean nothing. Blender's wavelet noise adds detail to the upres'd grids along a noise field, and on a flame field that detail comes out as sparkle. It is made for smoke.
- **The offline tools that make torn flames do it with a solver term Mantaflow lacks.** Houdini's pyro *shredding* pushes the velocity along the heat field's gradient, which gives fire its streaks and licks, and its *disturbance* breaks the flame up at a set scale ([SideFX, Pyro Burn shelf tool](https://www.sidefx.com/docs/houdini/shelf/pyro_burn.html)). EmberGen's flipbooks are what shipped fire is mostly baked from today (about 90% of Back 4 Blood's fire flipbooks, [Cherry 2021](https://realtimevfx.com/t/justin-cherry-realtime-vfx-reels-2021-back-4-blood/18495)).
- **Shipped real-time fire gets its pieces from particles, not from one volume.** A flame is a SubUV flipbook on many short-lived, velocity-aligned sprites that spawn over the fuel and rise ([Epic, adding visual effects](https://dev.epicgames.com/documentation/unreal-engine/at12-adding-visual-effects-to-your-game)), so each card's tip leaves on its own. Effects built on one baked flipbook add pieces over it: Afonseca's EmberGen campfire, one flipbook on a bent plane with no particles, names "secondary flame licks" as its next step ([RTVFX sketch 48](https://realtimevfx.com/t/bruno-afonseca-sketch-48/19028)). Torches are often a teardrop mesh with scrolling UVs and a few particles to break up its outline ([Polycount](https://polycount.com/discussion/comment/1136113/)). No studio talk on RDR2's or AC's fire describes it.

The sim path's sheds chase is closed for Mantaflow: wind in either frame (g23, g34), finer cells and noise (g35) all leave sheds at 0.2-0.3. F stays the bake. Sheds would come as shipped games get them, as separate short-lived pieces over the loop. docs/fire-module.md's D493 section sketches them as wisps drawn by the same marcher; nothing is built.

## Round g38: the baked campfire against the marched one, judged

The developers chose the baked campfire for the game (review desk `r-fire-sim`, on round g28's build), so round g38 put the campfire on the baked loop with g28's light tones, its loop played 1.25x and its thin edge thickened, and the shed wisps (rounds g36-g37), and held it to the switch's gates. On motion.ts it meets 8/6 of the nine targets, sheds 0.81/0.57 (dusk/night), and its fire sheet passes. Shedding 3.75 wisps a tip a second instead of 3 lowered sheds to 0.65/0.49: the extra wisps rise past the flame and raise the measured flame's height, so fewer tongues stand above half of it.

The motion panel (D472, 16 fresh judges a build, motion strips against anchors with RDR2 at 9) scored the marched campfire 5.08 and the baked one 4.19, a difference of -0.89 [-1.07, -0.71], so the switch did not land. In the strips the baked flame is a low orange mass on the logs, where the marched one stands about twice as tall in several separate tongues. Three harsh reviewers, comparing a 12 s film of the baked campfire with RDR2's frames, scored it 3, 4 and 4 out of 10. All three named the same faults:

- The logs read as a leopard print: even orange spots along their whole length, where RDR2's are charred black with glowing cracks only where the flame touches.
- The flame is opaque orange-red, with no white-yellow core and no see-through tips. Its wisps fly off as long straight streaks.
- The stones and the light are weak: smooth pink stones on a flat disc, and a small pool of light at night.

The motion measures, which are about how the flame moves, do not see how big or how bright it is. Before the baked body is tried for the switch again, it needs the marched flame's height and brightness.
