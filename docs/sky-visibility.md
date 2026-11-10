# Sky visibility that follows the camera

Proposal (D658, Leaning), for the developers to read before any code (D49). It answers the first change of [the lighting review](../research/2026-11-09-lighting-system-review.md): the forest's sky visibility, and every rule that reads it, exists only in the `lush` experiment. Nothing here is built. Every cost below is arithmetic or an estimate, marked as such; none is measured.

## Intent

How much of the sky reaches a spot is worked out wherever the camera is, in `play` first: under the mountain world's woods, against its cliffs and in its hollows, the diffuse sky light, the sky's reflection, the haze, the wooded night's eye, shelter from rain and what creatures take as shade all read one field that the world fills as it streams.

## What we have

- `terra_world::sky_occlusion` (D270, D336): one Bevy irradiance volume over a known square. `SkyOccluders` (the crowns as ellipsoids, inserted only by `lush.rs`) is baked once on another thread: each 4 by 2.5 m cell looks 65 ways through a grid of leaf thickness, to 90 m, and keeps the share of sky each of its six sides sees, plus straight up. Lush's is 145 x 24 x 145 cells, 504,600.
- The texture is not visibility but light: each side holds `sky * share + leaf glow * (1 - share) + ground bounce`, as RGB9E5, filled again on a thread whenever the sky's colours move 3%, and sent whole (12.1 MB for lush's).
- `den_light` (D591): the same for a den, 3 m cells, looking through the ground's distance field (`planet.voxels().distance`).
- Readers: the lit pass through Bevy's volume and our patch (D485); `OpenOverhead` for the haze (`air::follow`, `daylight::shine`) and the eye (`eye::open_exposure`, `wooded_night_ev`); `SkySeen::open_at` for climate's shelter and wildlife.
- Play streams grown trees in 104 m blocks out to 360 m on the streaming pool (`woods/near.rs`, each `Planted` with its kind, height and crown), fells them (`woods/felled.rs`), and streams 16 m voxel chunks. No tree grows over time yet. Nothing gives any of it to the sky.

## Prior art

Looked up 2026-11-10 (D196, D654) by a helper of this job, which opened each page marked [P] and reported what it could not reach; those lines are [S] or [M] and nothing below rests on them. What may be ported is the last column; the rest is read from documents and talks only.

| Source | What it stores and how it moves | Fit | Code |
|---|---|---|---|
| [P] Unity 6, Adaptive Probe Volumes' [sky occlusion](https://docs.unity.com/en-us/engine/6000.0/manual/lighting-overview/lighting/probevolumes/probe-volumes-change-lighting-at-runtime/skyocclusion) and its [shader](https://raw.githubusercontent.com/Unity-Technologies/Graphics/master/Packages/com.unity.render-pipelines.core/Runtime/Lighting/ProbeVolume/ProbeVolume.hlsl) | Four numbers a probe (visibility as L0 and L1 harmonics), multiplied at run time by the live sky's ambient probe; cells stream from disk | The storage we want: visibility, lit by today's sky. Its bake is offline | No (Unity's licence) |
| [P] Ghost of Tsushima ([Patry, SIGGRAPH 2021](https://www.advances.realtimerendering.com/s2021/jpatry_advances2021/index.html)) | Nine numbers a probe (sky visibility as second-order harmonics) and a bounced-sky term tinted by the sky's mean colour; a 12.5 m grid in 200 m tiles; relighting 73 tiles took 123 microseconds on a PS4 | The same storage, shipped in an open world with a day | Talk only |
| [M] Far Cry 4 ([McAuley, GDC 2015](https://www.gdcvault.com/play/1022235/Rendering-the-World-of-Far)) | The talk's page says "high-resolution directional sky occlusion"; its slides were not reachable, so that it multiplies the sky's harmonics (as D485 cites) is from memory | A lead, not a ground | Talk only |
| [P] DDGI (Majercik 2019, the [author's overview](https://morgan3d.github.io/articles/2019-04-01-ddgi/overview.html); [2021](https://ar5iv.arxiv.org/html/2009.10796)); [RTXGI](https://github.com/NVIDIAGameWorks/RTXGI-DDGI/blob/main/docs/DDGIVolume.md) | Probe grids locked to the camera: only the plane entering on the axis of motion is new, the inside keeps its data; levels blend over their last cell, the densest first until the weight is whole; probes with nothing near sleep | The scrolling, blending and sleeping rules. Its rays need ray tracing hardware | Papers yes; RTXGI no (NVIDIA's SDK licence forbids it) |
| [P] Godot 4 SDFGI ([docs](https://docs.godotengine.org/en/stable/tutorials/3d/global_illumination/using_sdfgi.html), [`gi.h`](https://raw.githubusercontent.com/godotengine/godot/master/servers/rendering/renderer_rd/environment/gi.h)) | Up to 8 cascades of 128 cells cubed round the camera, each twice the last, probes scrolled with their history; a moving camera's shifts can show | The one reference whose code we may read for scrolling cascades. Far more than we need: full bounce light from a distance field | Yes, MIT |
| [P] Unreal: [volumetric lightmap](https://dev.epicgames.com/documentation/en-us/unreal-engine/volumetric-lightmaps-in-unreal-engine), [distance field ambient occlusion](https://dev.epicgames.com/documentation/en-us/unreal-engine/distance-field-ambient-occlusion-in-unreal-engine) | Baked bricks of harmonics streamed with their level; or a bent normal and occlusion a pixel, cone-traced through four camera clipmaps of a distance field that update only what changed, 2.6 to 3.7 ms on a PS4 | The bricks need an offline bake of a fixed world. The clipmap update rule is ours too; the per-pixel tracing is too dear | Docs only |
| [C] Minecraft ([sky light](https://minecraft.wiki/w/Light)) | 4 bits a block, flooded down from the open sky and sideways losing one a block; an edit refloods locally; the day changes a curve, never the stored value | Proof that stored visibility, lit at run time, survives edits cheaply. No direction, blocks not metres | Idea only |
| [P] Teardown ([Gustafsson](https://blog.voxagon.se/2018/10/17/from-screen-space-to-voxel-space.html)) | Rays a pixel through a one-bit voxel occupancy volume, only the edited region sent again, about 9 ms | Where we would go for full bounce light. Not for this | Blog only |
| [S] Enshrouded (Keen Games, Graphics Programming Conference 2024, from the [programme's abstract](https://www.khronos.org/events/graphics-programming-conference); no slides found) | Moved from ray tracing to its own distance-field rays, since nothing baked survives a world players dig | The same | Talk only |

The fit for a streamed, generated voxel world with instanced trees and a sky that changes all day: **store visibility as Unity and Tsushima do, move it as DDGI's camera-locked grids do, fill it as we already do** (a worker looking through leaf thickness and the ground's distance field, not hardware rays). Bevy's Solari needs ray tracing hardware and is not this.

## The design

**A cell stores visibility, not light.** Six shares of sky, one a side (the ambient cube we have, as numbers not colours), and how lit the ground under it is: Rg8, 12 bytes a cell in Bevy's six-sided layout, against 24 for today's colours. The shader makes the light: the sky's own harmonics toward the normal (the environment map's diffuse light, already read) times the share, plus the leaf glow for the share hidden and the ground's bounce, from a few colours in a uniform. Why: the sky repaints all day and the volume never needs filling again for it; D485's `sky_light_over_its_cube` (seven cube reads a fragment, there because a cube of light and the map's harmonics disagree off the axes) goes away, since there is one sky and one multiplier; and an edit touches bytes, not a relight. Six shares, not harmonics, because Bevy's sampler already blends that layout and our bake already makes them.

**Two levels round the camera**, boxes in the frame of the ground under it, snapped to whole cells so a cell is always the same place in the world:

| Level | Cell | Cells | Box | Why |
|---|---|---|---|---|
| 0 | 4 x 2.5 x 4 m | 64 x 40 x 64 = 163,840 | 256 x 100 x 256 m | D270's proven cell; 100 m tall because a 30 degree slope climbs 74 m across the half box |
| 1 | 16 x 8 x 16 m | 64 x 32 x 64 = 131,072 | 1,024 x 256 x 1,024 m | Past the 360 m trees are grown to; cliffs and valley walls |

Past level 1 the open sky, as now. Each level fades over its outer cells into the next (Bevy's `falloff`), the finer first.

**Filling.** A level is a list of columns wanted and columns held. A column is asked for when its block's trees are known (from the woods' own rule for the block, `terra_land::woods`, not from the drawn entities, so it never waits on meshes) and its ground is generated; it is worked out on the streaming pool under its budget (`order::Meter`), nearest first, and lands as bytes in the level's array. Each cell looks 65 ways as now, through leaf thickness and now also the ground's distance field, so one field covers canopy, cliff and overhang. A cell with no crown or ground within reach above it is 1 without looking (DDGI's sleeping probes): most of the air.

**What changed only (D127).** A tree felled or regrown, a season thinning the broadleaves, or ground dug marks the columns within its reach dirty (a crown hides sky for cells below and beside it out to about its height over the tangent of the lowest look; bounded at 32 m for level 0); they are worked out again and the bytes replaced. When trees come to grow, the same mark. The den keeps its own bake until digging can reach it.

**Scrolling without a pop.** The box moves by whole cells, four at a time, when the camera is two cells from the step. Cells keep their world place, so what a fragment reads does not change when the box moves; the columns entering are in the outer fade, where the level weighs nothing, and are filled before the fade reaches them (a level's fade is wider than a step). The texture and the box's place change in one frame. First version: the level's bytes are shifted on the CPU and the texture sent whole (1.97 MB for level 0, once a step, about every 3 s at a jog); wrapped addressing with only the new slab sent is a later saving if the bench asks, since Bevy's stacked six-side layout cannot wrap through its sampler. Loading holds the screen until level 0 is whole, so the eye meets no arrival.

**Dens and rooms keep winning (D485).** A room's reflection probe smaller than every volume holding the fragment gives its light, unchanged. Among irradiance volumes Bevy averages by weight, which would dilute a den's cells with the clipmap's: the patch changes that to the densest volume first, each next one taking only the weight left (DDGI's rule), which is also what orders level 0 over level 1. Ranking is by distance to the box, not its middle (the review's item 7), so the camera's own levels are never dropped.

**What reads it.** `OpenOverhead` is level 0's straight-up share at the camera from the CPU's copy, as now, and now always present in play: a yard reads 1 because it was looked at, not because nothing was worked out. `SkySeen::open_at` reads the finest level holding the point, else 1. The eye's rules do not change; that they step when the camera crosses a cell is the review's change 3, not this.

**Cost, by arithmetic.**
- Memory: 294,912 cells. CPU copy 8 bytes a cell (six sides, up, ground) 2.4 MB, leaf thickness 4 bytes 1.2 MB; GPU 12 bytes 3.5 MB. Lush's one volume is 12.1 MB on the GPU today.
- Filling: a look is at most 36 steps of 2.5 m. Level 0 whole is at most 163,840 x 65 x 36 = 383 million steps; a step's slab (4 x 40 x 64 cells) 24 million. At an assumed 5 ns a step that is 1.9 s and 0.12 s of one core, off the frame, less where air sleeps. The ground's distance field costs more a step than the leaf grid and is the number to measure first.
- Frame: no relight ever; one texture upload a step; per fragment three taps a level (six in a fade) where today it is three taps and seven cube reads. To be benched (D185), not claimed.

**Bevy 0.20.** It gives: irradiance volumes as 3D textures of any filterable format, moved by their transform each frame, with `falloff`; 8 volume textures and (our patch) 64 probe slots a view; a volume list on every view (D485). To patch in `patches/bevy_pbr` (D184): a flag on a volume saying its texture is visibility, and the lit pass's light for it; densest-first weights among volumes; ranking by distance to the box. Not patched: the upload (an `Image` changed from the main world, as now).

## Stages

Each ends on a number an agent reads (D69); a new check gates only after a planted fault (D632).

1. **The field, no GPU.** The clipmap as plain data and the fill, tested beside the code: its cells equal the one-volume bake of the same crowns within 0.02; a cell reads the same after any scroll; an edit changes only cells within reach. Logs ns a step and seconds a level, replacing the assumption above.
2. **Play has level 0**, still as colours (today's shader). A new continuous check, `woodlight`: in play, a patch under the woods against one in a meadow at 13:00, the ratio D270 measured in lush; it reads 1.0 on main and must fall. `turnlight`, `turnflash` and `volumelight` hold; the walk bench against main.
3. **Level 1, densest first, the fades.** A walk across level 0's edge and into the den's mouth: no step in a patch's light over a bar (the review's change 6 check, used here); `volumelight` in the den unchanged; `indoorlight` unchanged.
4. **Edits.** Fell a tree in a scripted run: the patch under it brightens within a second, cells worked out and their milliseconds reported.
5. **Visibility in the texture.** The patch's flag and light; `sky_light_over_its_cube` and the relight thread deleted. `volumelight`'s trunk nearer 1.00 than its 0.98; the bench no worse; lush's looks hold their rows.
6. **Lush on the clipmap**, `SkyOccluders`' one volume deleted; D270, D273, D286, D310 and D336's numbers measured again in play, each amended with what it reads there.

## What it changes in the decisions

Proposed, none made: D270 (one volume over a known forest becomes the clipmap; light in the texture becomes visibility; the 70 m seam past the edge goes); D336 (the stems' share moves into the shared fill); D485 (volumes densest first rather than averaged; `sky_light_over_its_cube` removed at stage 5; the den's "open cells hold the whole sky" stays); D310 and D273 (the rule for "no canopy worked out" goes, since play always has one; their numbers remeasured at stage 6); D591 (the den's volume stays, and joins the clipmap only when digging must light it).

## Open questions

1. **Storage: visibility (recommended) or today's colours?** Colours need no shader patch and ship at stage 2; visibility is what Unity and Tsushima store and removes the relight and seven reads a fragment. Recommended: both, in that order, as staged.
2. **Does the ground hide the sky outside dens?** Recommended yes: the mountain world's cliffs and gullies are most of what is not wood. It is the fill's dearest part; if stage 1 measures it too slow at level 0, level 1 alone looks through the ground.
3. **Is lush kept on its own volume?** Recommended no: one system, lush moved at stage 6, so what is tuned is what is played.
4. **Built roofs and dug shafts.** Not in this design: rooms have their probes (D485, D486), and a dug den is dark as now. Recommended: after stage 4, edits to the ground mark columns as felled trees do.
