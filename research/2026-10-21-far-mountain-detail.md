# Far mountain detail: how shipped games make a wall 1 to 5 km off read as rock (2026-10-21)

The question: five rounds of terrain-shape work on the mountains (ledges, benches, horn heights, snow share; D426) gained tenths in review, the last one changing under 0.4% of the pixels. At the lake (dusk, 2 to 4 km) and the valley (morning, 1 to 3 km) the horns' rock reads as "smooth plaster" and their snow as soft blotches. How do shipped games give mid and far mountains their detail, and which of their ways can we check by measurement?

## What shipped games and tools do

- **Witcher 3** (Gollent, [Landscape creation and rendering in REDengine 3](https://gdcvault.com/play/1020394/Landscape-Creation-and-Rendering-in), GDC 2014). The abstract says the terrain material dropped conventional layers and linear blends. The slides were not reachable, so how it handles cliffs is unconfirmed here.
- **Horizon Zero Dawn** ([GPU-based procedural placement](https://www.guerrilla-games.com/read/gpu-based-procedural-placement-in-horizon-zero-dawn)). Rocks and cliff assets are placed by rule, over 500 asset types. Nothing public covers how its far terrain is shaded.
- **Ghost of Tsushima, Ghost of Yōtei** ([Samurai Landscapes](https://gdcvault.com/play/1027352/Samurai-Landscapes-Building-and-Rendering), GDC 2021, abstract only; [Yōtei's distant mountains](https://blog.playstation.com/?p=411201)). Yōtei bakes its models and detailed terrain materials into textures for the far mountains. That is an offline bake of the detail into colour.
- **Far Cry 5** ([Terrain Rendering in Far Cry 5](https://gdcvault.com/play/1025261/Terrain-Rendering-in-Far-Cry), [Procedural world generation](https://gdcvault.com/play/1025215/Procedural-World-Generation-of-Far), GDC 2018). It has procedural cliffs with displacement geometry, cliff shading of their own, and cliff rocks generated in Houdini. These are meshes and offline generation.
- **Frostbite**
  - Andersson, [Terrain rendering in Frostbite](https://www.advances.realtimerendering.com/s2007/Andersson-TerrainRendering(Siggraph07)-CourseNotes.pdf), SIGGRAPH 2007. Materials are placed by slope ramps.
  - Widmark, [Terrain in Battlefield 3](https://media.contentapi.ea.com/content/dam/eacom/frostbite/files/gdc12-terrain-in-battlefield3.pdf), GDC 2012, read in full. Far off, the detail splats are gone after 50 to 100 m. To hide a blurry far heightfield and normal map, put detail such as occlusion into the cheap colour map.
- **UE5 landscape**
  - Height-blended layers ([landscape material expressions](https://dev.epicgames.com/documentation/en-us/unreal-engine/landscape-material-expressions-in-unreal-engine)).
  - Macro variation: one texture sampled at three scales and multiplied in ([how to](https://versluis.com/2023/05/how-to-use-macro-variations-for-unreal-engine-landscape-materials)).
  - Triplanar on cliffs ([80.lv](https://80.lv/articles/ue5-triplanar-deep-dive-from-worldalignedtexture-to-high-quality-normals-part-1/)).
- **Gaea and World Machine** ([erosion](https://docs.quadspinner.com/Guide/Using-Gaea/Erosion.html), [colour](https://docs.quadspinner.com/Guide/Using-Gaea/Color-Production.html)). Erosion bakes wear, deposits and flow masks from the heightfield, and Occlusion and Curvature masks drive texturing without any erosion. The docs warn that flow alone as the texture driver looks "CG".
- RDR2: no public source was found.

## What ours already has, and what it lacks

The ground shader already draws, from world-space noise and the fall line:
- far ribs and couloirs, three sets, darker in the gullies;
- the fault blocks' tilted beds, as tiers, joints and slabs;
- weathering patches;
- aspect tints;
- wet streaks.

These are macro variation and strata in UE's and Gaea's sense (D394, D395, D426). Each set fades out before a pixel holds one of its stripes, and each is masked by noise, so few of them survive at 2 to 4 km.

The first guess was that a far wall lacks any measure of its own shape. A far chunk keeps alpha 1, so no vertex sees less sky (`terra_world::terrain`, `SHADED_LEVELS`; the near pits only, 0.8 to 2 m reaches). Its mesh is 16 to 32 m a cell, and cascade shadows do not reach it. At dusk the sky's light is nearly all the light, so every gully, bench corner and notch is lit alike, and the wall reads as one smooth tone.

That would be the gap Battlefield 3's advice and Gaea's Occlusion mask fill, and Yōtei's bake too. They put the shape's occlusion into the far ground's light and colour.

## The approaches against ours

| Approach | Who | Verifiable at these views by measurement | Fits a voxel field (D198) and D196 |
|---|---|---|---|
| More terrain shape (ledges, benches) | ours, D426, rounds 18-22 | No: under 0.4% of pixels changed | Yes |
| Macro rock texture, triplanar, several scales | UE5, Far Cry 5 | Yes, but a texture's tiling and alias are judged by eye | Needs a cliff texture at 50-200 m scale, which we lack |
| Occlusion and cavity from the shape, in the far ground's light and colour | BF3, Gaea Occlusion, Yōtei's bake | **Yes**: local contrast and edge density in the rock, before against after | Yes: asked of the field per vertex, no bake |
| Flow and wear masks | World Machine, Gaea | Partly | Needs a flow accumulation we do not have (our gullies are per point); "CG" alone |
| Kitbashed cliff meshes on steep ground | Far Cry 5, Horizon, Witcher 3 | Only by eye, and only after modelling | Needs assets and placement rules (models skill) |

## Tried, and measured

The fold occlusion was built (`terra_voxel::openness`; patch in the mountains r23 folder) and rendered twice against main ad930678. The measures are the rock's local contrast (high-pass std over mean) and its edge density, in crops of the lake's massif and the valley's walls.

1. **Within 14 and 42 m, half of it into the colour.** A far wall's vertices averaged 0.93 open, 0.08 either way. 0.02 to 0.16% of pixels changed by more than 3%.
2. **Within 40 and 120 m, stretched, 0.8 into the colour.** The vertices averaged 0.84, 0.12 either way, and the walls' sky was dimmed by about a quarter. 0.04 to 0.2% of pixels changed by more than 3%. The rock lost contrast: the lake at 15 s fell from hp 0.053 to 0.038 and edges 0.049 to 0.026 on the low rock. The folds hold the snow, and dimming them dimmed it against the rock.

Neither landed. A harsh review scored the second 0.1 to 0.3 better in every view (lake 5.2 to 5.5, 4.7 to 5.0; valley 5.7 to 5.8; settlement 5.6 to 5.7). But it was told what had changed, and it named the shader's old fall-line stripes as the new folds.

## Why nothing on the surface shows: the wall is mostly air

The ground shader's own aerial perspective and foot mist (`ground.wesl`, end of the fragment) take this share of a wall's foot, within 80 m of the eye's height (the mist thins to none 750 m over it):

| View | Air | Rock left |
|---|---|---|
| Lake at dusk, 3 km | 0.87 | 0.13 |
| Valley in the morning, 2 km | 0.93 | 0.07 |

Bevy's distance fog comes on top of that. The arithmetic:
- `AERIAL_M` is 2400 m from 250 m.
- `FOOT_MIST_DEPTH_M` is 900 m under 80 m over the eye: at a 0.3 share at dusk, and whole by day.
- At the valley, 1750/2400 + 1750/900 = 2.67, and exp(-2.67) = 0.07.

That matches the renders. A 25% change on the surface, times 0.07 to 0.13 of the pixel, is 2 to 3%, under the diff's fuzz.

Real clear air at 2 to 3 km leaves 70 to 85% of a wall (20 to 40 km visibility). The references' walls show their dark rock in shade. The five shape rounds and these two shading candidates were all hidden by the haze, not lost in the mesh.

## Next

The foot mist and the aerial perspective have to leave a wall's foot at least a third of its own light at 2 to 3 km, held by a test on the shader's CPU twin of `through`. Without that, no surface detail can show.

Then measure the same crops again, and reapply the fold occlusion with snow left out of its dimming.
