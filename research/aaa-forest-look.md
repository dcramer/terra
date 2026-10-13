# What makes a forest read as AAA, and what Bevy 0.20 has for it

2026-10-13. Question: the developers watched a video of the lush forest walk and found the bark very dark, things popping in and out, and the whole not yet feeling like a AAA game. What do shipped games do that we do not, and which of it does Bevy 0.20, as pinned, already have?

## Answer

Three things in our renderer explain most of it, and Bevy has the fix for each:

1. **No temporal anti-aliasing.** Every reference game resolves foliage with TAA. Level-of-detail crossfades are dithered (Bevy's `VisibilityRange` uses a fixed 4×4 ordered dither, as Unreal's foliage does), and without a temporal resolve the dither shows as a crawling screen-door: things fizzing in and out. TAA also stops leaf cards and grass blades shimmering. We used SMAA. Bevy 0.20 has `TemporalAntiAliasing`; wind-animated vertex shaders must write last frame's position for its motion vectors (`globals.time - globals.delta_time`), which ours did not.
2. **A fixed exposure set for open sun.** Shade under a canopy is several stops below the open sky, and every reference game adapts its exposure, clamped, as an eye does. Bevy 0.20 has `AutoExposure`, a histogram pass with speeds, a range and a compensation curve. Extra contrast in the shadows after the tone map darkened the shade further.
3. **Bark at the bottom of the plausible range, with no relief.** DONTNOD's chart for Unreal puts dielectric albedo between sRGB 50 and 243; bark is about 90 to 135 (linear 0.10 to 0.25). Ours averaged about 85, with no normal map to catch the sky light, so in shade it went nearly black.

## What shipped games do, by impact per cost

| # | What | How shipped games do it | Bevy 0.20 | Cost |
|---|---|---|---|---|
| 1 | TAA, with motion vectors that include the wind | Unreal, Decima and RAGE all lean on it for foliage; GPUOpen on velocity for foliage | `TemporalAntiAliasing` (needs `Msaa::Off`; adds the motion-vector prepass); `ShadowFilteringMethod::Temporal` goes with it | 1 to 3 days |
| 2 | Clamped eye adaptation; no extra shadow contrast after the tone map | Every reference game | `AutoExposure { range, speed_brighten, speed_darken, compensation_curve, metering_mask }` | under a day |
| 3 | Bark albedo in range, with normal and occlusion maps (occlusion mostly white) | DONTNOD and Far Cry 3 calibration; SpeedTree's guidance that per-vertex and texture occlusion multiply | material change | hours |
| 4 | Sky occlusion under the canopy: per-vertex occlusion baked when a tree is grown, and a coarse sky-visibility field dimming the sky light, clamped about 0.3 to 0.5 and tinted green | Crysis (occlusion in vertex alpha), Unity's Book of the Dead occlusion probes, RDR2's sky irradiance probe grid | ours to build; `GeneratedEnvironmentMapLight` is global | 2 to 5 days |
| 5 | Leaf shading: crown-shaped normals, translucency by thickness, wrapped light, a floor on how dark a crown's inside gets | Crysis (GPU Gems 3, ch. 16), SpeedTree global lighting | `diffuse_transmission`, `thickness` | 1 to 2 days |
| 6 | Volumetric fog with sun shafts, aerial perspective | RDR2's integrated volumetrics; Hillaire's atmosphere | `VolumetricFog`, `VolumetricLight`, `FogVolume`; the atmosphere is already on | 1 to 2 days, and GPU time |
| 7 | Shadow casters fixed per cascade, never dropped inside the shadow distance | Horizon Zero Dawn: one shadow LOD per cascade, separate casters, a height-field far cascade | ours | 2 to 4 days |
| 8 | Octahedral impostors for far trees | Fortnite (Brucks), Horizon's billboard LODs; Nanite Foliage in UE 5.7 later | none; `VisibilityRange` to the last level | 1 to 2 weeks |
| 9 | Grass thinning blade by blade with distance, the ground tinted to the grass far off | Ghost of Tsushima | ours (done, D202) | done |
| 10 | Variety and wind: per-instance tint, more undergrowth layers, gusts that travel, grass bending round the player | Ghost of Tsushima's one wind system, Horizon's three scales, the Witcher 3's 200 plant kinds | ours | weeks, mostly data (D136) |

Not for us now: Solari, Bevy's ray-traced lighting, is experimental and leans on NVIDIA's DLSS Ray Reconstruction for denoising; screen-space reflections need deferred shading and add little to a forest; meshlets cannot draw alpha-cut leaves or run a wind shader.

## Sources

- Lagarde, DONTNOD physically based rendering chart for Unreal Engine 4: <https://seblagarde.wordpress.com/2014/04/14/dontnod-physically-based-rendering-chart-for-unreal-engine-4/>
- McAuley, Calibrating lighting and materials in Far Cry 3 (SIGGRAPH 2012): <https://blog.selfshadow.com/publications/s2012-shading-course/mcauley/s2012_pbs_farcry3_notes_v2.pdf>
- Sousa, Vegetation procedural animation and shading in Crysis (GPU Gems 3, ch. 16): <https://developer.nvidia.com/gpugems/gpugems3/part-iii-rendering/chapter-16-vegetation-procedural-animation-and-shading-crysis>
- SpeedTree materials and lighting: <https://docs.unity3d.com/speedtree-modeler/manual/materials-and-maps.html>, <https://docs.unity3d.com/speedtree-modeler/manual/lighting.html>
- Lagarde and de Rousiers, Moving Frostbite to PBR (specular occlusion): <https://seblagarde.wordpress.com/wp-content/uploads/2015/07/course_notes_moving_frostbite_to_pbr_v32.pdf>
- Bauer, Creating the atmospheric world of Red Dead Redemption 2 (SIGGRAPH 2019): <https://advances.realtimerendering.com/s2019/index.htm>
- Unity probe volume sky occlusion: <https://docs-alpha.unity3d.com/Manual/urp/probevolumes-skyocclusion>
- Hillaire, A scalable and production-ready sky and atmosphere (2020): <https://blog.selfshadow.com/publications/s2020-shading-course/hillaire/s2020_pbs_hillaire_slides.pdf>
- Brucks, Octahedral impostors: <https://shaderbits.com/blog/octahedral-impostors>
- Wohllaib, Procedural grass in Ghost of Tsushima (GDC 2021): <https://gdcvault.com/play/1027214/Advanced-Graphics-Summit-Procedural-Grass>
- van Muijden, GPU-based procedural placement in Horizon Zero Dawn (GDC 2017): <https://www.guerrilla-games.com/media/News/Files/GDC2017_VanMuijden_GPUBasedProceduralPlacementInHorizonZeroDawn.pdf>
- Bevy 0.20 source, read in `~/.cargo/registry`: `bevy_anti_alias` (TAA), `bevy_post_process::auto_exposure`, `bevy_pbr` (prepass motion vectors, `VisibilityRange` dither, volumetric fog, contact shadows), `bevy_light::cascade`.

The Ghost of Tsushima and Horizon vegetation talks are behind GDC Vault; what is said of them here comes from their abstracts and from public re-creations.
