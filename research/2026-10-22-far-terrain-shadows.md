# Far terrain shadows and the lake's off-screen reflection: how shipped engines shade land past the shadow map (2026-10-22)

The question (D416): Terra's sun cascades end at 300 m (`Quality::shadows`), so a range's evening shadow across a valley a kilometre off is not drawn, and the lake's reflection, a march through the screen's depth, falls back off screen to the skyline's flat colours (D322). How do engines shade terrain past the dynamic shadow distance, and what does it cost us?

## What others do (read)

- **Horizon mapping.** Max, "Horizon mapping: shadows for bump-mapped surfaces", The Visual Computer 4, 1988: the elevation of the horizon is stored at each texel in a few ways; a light is in shade where it stands under it. One lookup a pixel, the light free to move.
- **Height-field self-shadowing.** Timonen and Westerholm, "Scalable Height Field Self-Shadowing", Eurographics 2010: horizons of a height field in many ways, swept along lines, the sun's elevation compared against the horizon interpolated between the two ways either side of its azimuth. Our bake follows it, marching each cell along great circles instead of sweeping.
- **Andrew Willmott**, height-map shadow notes ([andrewwillmott.com/tech-notes](https://andrewwillmott.com/tech-notes)): horizon angles from a height map as a cheap far shadow for a moving sun.
- **Fritsch et al.**, "Fast Planetary Shadows using Fourier-Compressed Horizon Maps", HPG 2025 ([diglib.eg.org/handle/10.2312/hpg20251171](https://diglib.eg.org/handle/10.2312/hpg20251171)): horizon maps on a planet, the curve in the geometry, compressed to a few Fourier terms; the same idea at planetary scale.
- **Precomputed far, traced near**: the Eurographics 2015 short paper on terrain shadows split between a precomputed far term and ray-traced near ones (Eurographics Digital Library, egsh.20151014.057-060; read as its abstract only).
- **Unreal.** Landscape has no distance-field shadows of its own; it casts into the directional light's Far Shadow cascades (`bCastFarShadow`, the light's far cascade count), an extra low-resolution cascade drawn again as the sun moves ([forum thread 445122](https://forums.unrealengine.com/t/445122), issue UE-71042).
- **Reflections off screen** fall back in shipped engines from screen-space reflections to captured reflection probes, then to the sky. Ours: SSR, then the skyline's parametric picture (D322), now shaded by the horizon map.

Bevy 0.20 allows four cascades a light (`MAX_CASCADES_PER_LIGHT`); stretching the last to kilometres redraws every chunk's shadow each frame and coarsens the near ones. A horizon map costs a lookup a pixel and nothing a frame.

## What we built

- `terra_land::horizon` (no Bevy): a gnomonic plane at the map's middle, the ground's heights sampled onto it once, then a march from each of 512 x 512 cells of 24 m along great circles out to 6 km in 16 ways (a cell a step, then 3% of the distance), as four RGBA8 layers of elevations from -0.1 to 0.7 rad. Tested against a 1 m ray march on a synthetic ridge and hill: p95 0.17 degrees, worst 1.21 on a steep flank a cell off.
- `terra_world::horizon`: baked on the async pool round the land's middle, again when the camera is a kilometre off.
- `assets/shaders/horizon.wesl`: `horizon_lit`, read by the ground, the far forest's cards and the lake, faded in over the last cascade's last fifth, the sun's half degree as its penumbra.

## Measured

- **Bake**: `horizon map: 512x512 of 24 m baked in 6.64 s` to 6.71 s over three runs (release, one async worker, the 30 km world at the lake). Until it lands the ground is lit as before.
- **Orientation** (a throwaway debug tint, the ground past the last cascade coloured red by `1 - lit` and green where the slope faces away from the sun): at the lake valley, 06-10 19:00, the far peaks come out yellow (the map and the slope's facing agree), the faces under them red (cast shade past 300 m), and green only as speckle from detail normals. A map turned the wrong way would paint the faces toward the sun.
- **Look**: before and after stills from the same five vantages at 18:00 and 19:00, and two on the water looking down so the far shore is reflected from the fallback. Mean luma difference 0.6 to 1.7 of 255 on the valley views, 2.2 and 3.5 on the water ones: the far faces under the big peak darken, and the fallback's mirrored slopes darken where the map shades them. At 18:00 little is in a range's shade (the sun about 10 degrees up); the haze over the far land softens what there is.
- **Cost**: `cargo xtask bench world` on the RTX 3080, main 592a708e against the branch, both on a busy machine (processors 70-100% busy by others): 26.8 ms median against 25.7 ms. No cost shows; the shader adds a texture lookup a far pixel, nothing a frame on the CPU. The `lake` and `lake-walk` scenes stuck at their start ("stuck after 300 s") on main and on the branch alike, so they gave no numbers.

## Limits

- The map is 12 km across; past it the ground is lit as before.
- 16 ways at 24 m: a narrow notch in a ridge is missed, and a shadow edge is soft over a cell or two.
- The lake's fallback places the slope it mirrors from the skyline's height and distance, an estimate; a reflection probe captured at the lake would replace it.
