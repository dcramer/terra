# Why our trees read fake: how shipped games draw trees, grass and foliage

2026-11-02. The developer: our trees look fake beside Enshrouded's; branches move as one unit; canopies feel unhealthy. Question: how shipped games solve the "hair" problem of plants too many to model one by one (wind, shading, levels of detail), what each costs, and concretely why ours read fake. Research only: nothing here changes the game. The choices are Open decisions for the developers: [D597](../docs/decisions/D597.md) (wind in levels) and [D598](../docs/decisions/D598.md) (fuller crowns of smaller sprays, warmer). The forest's light was measured too and is not the gap on a clear day, so it gets none.

## In plain words

- **Our crowns slide as one block.** In the shader every vertex of a tree, trunk included, moves the same way downwind, by its height squared, with a phase that changes only over 27 to 90 m. No branch turns about its own root and no branch has its own timing. On a still camera under the pines, 85% of the crowns' motion is one shared shift, and neighbouring patches move almost alike (r 0.91). Enshrouded's crowns on a still camera: 11 to 24% shared, r 0.36 to 0.43. Its trunks stand still and its sprays stir on their own.
- **Leaves barely flutter.** Flutter is 1.5 cm (D282 cut it from 5 cm when crowns shook "like jelly"), which is under a pixel from 10 m. The jelly came from phase taken per vertex, not from flutter as such. The cure the industry uses is a phase per leaf and per branch (Crysis, SpeedTree), not less motion.
- **The pine crowns read as pads on poles.** From outside the stand, each Scots pine is a straight pole with a few flat round pads of one dark green on top, like bonsai or topiary. Enshrouded's conifers are full cones of many sprays, catching the sun spray by spray. The crown is the top quarter to third of a 24 to 34 m tree (D318), with sky between the pads.
- **Close up, the sprays are big smeared cards.** Under a pine you see a few large fan cards, magnified and repeated. Real crowns and Enshrouded's are many smaller sprays, with dark gaps between the layers.
- **The crowns are greener and bluer than the references.** Their median hue is 77° (67 to 106 by view) against Enshrouded's 62° and Sons of the Forest's 63°, and red against green is 0.93 against Enshrouded's 1.11. On the rainy day the worker drew, the crowns went teal: 90 to 116°.
- **The shading is not the main gap.** We already have every crown-shading technique the industry uses: crown normals, a baked crown occlusion, light through leaves, dark undersides, occluded sky. On a clear day our brightness, leaf spread and leaves catching the light are near Enshrouded's.

## What we have, as built

Read from the code on main at `2fd0e0736`.

- **Trees** are grown by rules (D222), SpeedTree's way: tubes of bark for trunk and limbs, leaves as cards of real sprays (D269, D276). The lush forest's Scots pines, Norway spruces and oaks (`crates/terra_sandbox/src/experiments/lush/species.rs`, `conifers.rs`) are also the play world's woods (`crates/terra_world/src/woods.rs`); the generic flora's trees are `crates/terra_world/src/grown/tree.rs`.
- **Wind** is one vertex function, `sway` in `assets/shaders/foliage.wesl` (and `wind.wesl`, `grass.wesl`), fed by the world's wind in the shared buffer (D374). Each vertex carries two numbers in its second texture coordinate: `bend` = (height over the foot / the tree's height)², and `flutter`, 0 on trunk and main limbs, 0.15 on twigs, 0.25 to 1.0 on cards. The vertex moves by
  - `bend × direction × (0.45 + gust) × strength × 0.25`: a **translation downwind**, the same direction for every vertex, its phase from the vertex's place in the world at wavelengths of 27 to 90 m (`downwind × 0.07`, `× 0.23`), with the gusts' fronts 40 and 90 m long (D255, D282);
  - plus a flutter of 1.5 cm, its phase from the world position at about 20 m a cycle (`world × 0.29`), so a spray moves as one (D282).
- **Crown shading** is already the shipped recipe on paper: card normals bent out of the leaf mass and the crown (`species.rs`, the "out_mass/out_crown" blend), a per-card tint darker inside the mass and low in the crown (a baked AO), leaves lit through by the sun and the sky in a yellower colour (`lit_through`, `sky_through` in `foliage_fragment.wesl`, D247, D264, D272), undersides darker and without sheen (D264), the sky's light and reflections occluded by a sky occlusion volume (D270, D289), each leaf tilted by the card's normal map (D269).
- **Levels of detail**: crowns drop cards with distance, the next card to go shrinking into its middle (D223); levels crossfade by a dither TAA resolves (D214); shadows from a sparse stand-in (D203, D234); past the last level, billboards drawn procedurally by `far_forest.wesl` (D273), not captured from the tree. Leaf cards are alpha-tested (`AlphaMode::Mask`) under TAA with MSAA off; their mip chains keep coverage (`terra_bake/src/picture.rs`, `mips::cut_out_chain`), Castaño's fix.

## How shipped games do it

Each technique: how it works, what it costs the GPU and the content, and its source. **[P]** is read in a primary source (a paper, a talk's slides, official docs, the pinned engine's code), **[S]** from a secondary one (a write-up, a reimplementation, a search extract), **[I]** inferred. Unreal is cited by its docs and talks only (D196). Enshrouded, Ghost of Tsushima and Horizon's talks are behind the GDC Vault's login or not published; what is marked [S] there should be checked against the video before anything is built on it.

### Wind: a hierarchy, each level about its own pivot, each with its own phase

| Technique | How it works | GPU cost | Content cost |
|---|---|---|---|
| **Crysis** (Sousa, GPU Gems 3 ch. 16, 2007) [P] | *Main bending*: the whole plant leans by `wind × height × bendScale`, then the vertex is put back at its old distance from the root, so the tree bends, not shears. *Detail bending*: vertex colours, painted by artists: R a leaf edge's stiffness (edges flap along the normal), G each leaf's own phase, B the leaf's overall stiffness (it bobs up and down); A is baked AO. The object's phase is `dot(position, 1)`, so no two trees move alike. Motion is four smoothed triangle waves (frequencies 1.975, 0.793, 0.375, 0.193), cheaper than sines and never in step. | A few dozen vertex ALU ops; the source gives no number. | Four channels painted a model. |
| **SpeedTree** (its Modeler's "Games" wind, and the Unity and Unreal importers) [P] | *Shared motion*: the whole tree sways from a start height, woody trunks barely flexing. *Branch motion* per branch level (two levels in games), each branch bending along its own length by a weight curve, with **Independence**: 0 and every branch of a level moves together, typical 1 to 10, each branch out of phase with its neighbours. *Ripple* on leaves and fronds, along their normal or the wind, by noise, with its own independence per vertex. *Gusting*: frequency, strength, duration, rise and fall. Every number a curve from calm to the strongest wind. Shader switches name the motions: `BRANCH_DIRECTIONAL`, `_TURBULENCE`, `_WHIP`, `_OSC_COMPLEX`, `LEAF_RIPPLE`, `LEAF_TUMBLE`, `LEAF_TWITCH`, `FROND_RIPPLE` (Unreal's `FSpeedTreeWind::EOptions`). | Unity's tiers: Fastest (global only), Faster (+ leaf ripple), Better (+ branches), Best (+ leaf tumble), Palm (+ frond ripple), settable per level of detail. The per-vertex data takes several texture coordinates (a vendor says five extra UV sets [S]). | Tuned per model in the Modeler; the generator knows each branch, so the data is free. |
| **Unreal's Pivot Painter 2** (Epic's docs) [P] | Each element (trunk, branch, twig, leaf) gets its pivot, its X axis and length, its parent's index and its depth, baked into small textures read by the element's index from one extra UV set. `PivotPainter2FoliageShader` runs the same wind four times, a level deeper each, each element **rotating about its own pivot** by a turbulence and gust texture's vector; the offsets add (cheaper than chaining the rotations, a little less exact). | Vertex work only; Epic: pixel cost matters more, turn off levels that need not move. | A baking step in the DCC tool (MaxScript or Houdini); we would bake it in the generator. |
| **Unreal 5.7's Nanite Foliage** (experimental, off by default) [P] | Trees as *assemblies* of instanced parts (up to 65k per tree), far away as voxels of near-pixel size instead of cards or billboards, and wind driving a **skeleton** (the Dynamic Wind plugin) rather than moving vertices. Epic's example tree: 41M triangles in 2,160 part instances on 850 bones; 100k bones updated in about 0.1 ms on the GPU; the asset from 3.5 GB to about 29 MB. Shown in The Witcher 4's Unreal Fest demo (PS5, 60 fps) [S]. | Needs Nanite; Epic advise caution shipping it. Earlier Nanite (5.1+) took masked and moving foliage only at a price: moving vertices widen culling bounds and split raster bins, masks add overdraw. | A skeleton per tree. |
| **Ghost of Tsushima** (Rockenbeck, GDC 2021) [S] | One authored wind, its strength varied over the map by Perlin noise moving with time; gusts read across grass fields; local disturbances (*vorticles*: a place, a radius and a turn, up to hundreds, summed) push grass, cloth and particles. How the trees respond is not confirmed. | Not published. | One field for everything. |
| **Horizon Zero Dawn** (Sanders, GDC 2018) [S] | A global wind field; three tiers: large bend, branches and leaves, small interaction; SpeedTree models. | Not published. | |

Every one of these shares the thing ours lacks: **motion at the branch's scale, about the branch's own root, with the branch's own phase**. SpeedTree's Independence is exactly the knob for "branches move as one unit".

### Grass

**Ghost of Tsushima** (Wohllaib, GDC 2021): every blade generated on the GPU, its own shape and motion, acres of it in budget [P, the Vault abstract]. From write-ups [S]: a blade is a cubic Bézier of four control points, 15 vertices near and 7 far, near blades blending to the far shape before they swap; far tiles twice as large for the same count; clumps from Voronoi cells set height, facing, colour and bend; blades seen edge-on are widened in view space; about 83,000 blades visible for about 2.5 ms (platform not given). Ours (`grass.wesl`, D268, D293) is already merged clumped blades round the camera, with the same gusts; the grass is not what the developer named.

### Shading a crown

| Technique | How it works | Cost |
|---|---|---|
| **Light through a leaf** | Crysis: the back face lit by wrapped diffuse `saturate(dot(-N,L) × 0.6 + 0.4)`, mixed with a view term, times a painted thickness texture as small as 128² [P]. Frostbite: a local-thickness map (Barré-Brisebois, GDC 2011) [P]. Unreal's *Two Sided Foliage* model: a subsurface colour for the light passed through, masking stems and veins [P]. Godot: *Backlight* and SSS *Transmittance* [P]. Bevy 0.20: `diffuse_transmission` with a texture and `TransmittedShadowReceiver` [P]. | A little pixel ALU; no pass. |
| **Crown normals** | SpeedTree's *Global* lighting points leaf normals away from a shared centre, with a puffiness control, "the most efficient way to light models for real-time", the default since Modeler 9.4 [P]; artists transfer normals from a sphere or hemisphere onto big cards [S]. A realistic look blends part way between the card's and the crown's [I]. | Baked; none at run time. |
| **AO from the crown's volume** | Crysis bakes it into vertex alpha [P]; SpeedTree renders it in the Modeler, with a ground term and a remap [P]. | Baked. |
| **Specular** | Crysis: Phong on the front only [P]. No shipped source found for sheen models on leaves. | |
| **Shadows in the canopy** | Horizon: shadow levels of detail fixed per cascade (not by distance) so they do not pop; shadow casters separate meshes [S]. Forbidden West: a visibility buffer for foliage and alpha-tested geometry, shading in compute [P]. | |

We already have every row of this table (above, *What we have*), several measured and tuned; shading is not where ours is furthest behind.

### Levels of detail and far trees

- **Crossfades**: SpeedTree and Unreal dither between levels [P]; ours does, in a pattern TAA resolves (D214).
- **Octahedral impostors** (Ryan Brucks, *Octahedral Impostors*, 2018, used in Fortnite): the tree rendered from many directions over a (hemi-)octahedron into an atlas of colour, normal and depth, the frames nearest the view blended, so a far tree keeps its own shape and lighting from every side; the hemi-octahedral layout spends all frames on the upper half [S; the post could not be read here]. Far Cry 4: modelled leaves near, "tessellated displaced billboards" far [P, the Vault abstract].
- **Alpha mipmaps** (Castaño, *Computing Alpha Mipmaps*, 2010) [P]: each mip's alpha scaled so as many texels pass the test as at the top, so far crowns do not thin out. Ours does this already.
- **Alpha to coverage** needs MSAA [P, Godot docs; Bevy falls back to a plain mask with `Msaa::Off`]; our TAA needs MSAA off, so it is not open to us without giving up TAA.
- **Nanite Foliage's voxels** replace both cards and billboards far away (above).

### What the engines give

- **Bevy 0.20** (pinned) [P, its source]: `diffuse_transmission`, `specular_transmission`, `thickness` and their textures; `AlphaMode::AlphaToCoverage` (a mask with MSAA off); `VisibilityRange` with margins for a dithered crossfade, its docs' example high-poly → low-poly → impostor; GPU-driven instancing, culling and multi-draw since 0.16, automatic for entities sharing a mesh and material; mesh tags (a u32 per instance a vertex shader reads). Its meshlets (virtual geometry) cannot take foliage: no vertex shader for materials, so no wind, and no alpha mask.
- **Godot 4** (MIT) [P]: MultiMesh instancing; visibility ranges whose fade is alpha-blended, dithered only through a per-level material; Backlight and Transmittance in StandardMaterial3D; alpha antialiasing (alpha to coverage) and scissor; ProtonScatter and Spatial Gardener as add-ons for placement [S]. No wind system of its own.
- **Enshrouded** (Keen Games' own Holistic engine): no talk, devlog or interview on its vegetation, wind or tree rendering was found (GDC, Digital Dragons, devcom, Keen's site, 80.lv, Steam news). On the engine in general [S, PCGH's 1.0 test]: the world is voxels meshed into smooth polygons; light is fully real-time, nothing baked; no hardware ray tracing: it raymarches with the voxels as the acceleration structure, with SDFs, probes and caches for indirect light; a "brick renderer" for far voxel detail since 1.0 [S, Wccftech]. What its trees do is read from its frames below.

## Ours beside the references

**What was shot.** On the render worker at `2fd0e0736`, in the lush forest (`--experiment play --experiment lush --level yard`), at 15:00. There were five stills (up into the pines, up under a crown, the tree line from outside, toward the sun, the clearing) and two 12 s films from a still camera (`tools/sandbox/film.ts --frames 300-1020 -- … --vantage …`). They were shot on the default late-August day (clear, wind 1.7 m/s) and on 15 July, which the climate made a rainy day (cloud 1.0, 2.5 mm/h, wind 8.4 m/s). The vantage sheet came from the rainy day. The pictures are kept outside the repository, in `~/.cache/terra-review/veg-research/`.

**The references.** Enshrouded's 19 Steam store screenshots and its 1.0 launch trailer, plus two 12 s pieces of a still-camera ambience video of its Springlands conifer forest (YouTube `1Q9A2_Nhb54`, at 5:00 and 30:00). They are kept outside the repository, in `~/.cache/terra-review/refs/enshrouded/`. The YouTube frames are compressed, which blurs fine detail and lowers their spread measures.

**Still pictures** (`tools/sandbox/look.ts measure`, sRGB):

| | brightness | below 0.08 | red/green | flat foliage | leaves catching light | crown hue |
|---|---|---|---|---|---|---|
| Enshrouded, Springlands conifers (2 frames) | 0.24 to 0.27 | 6 to 11% | 1.09 to 1.15 | 12 to 22% | 4.6 to 5.5% | 57 to 60° |
| Enshrouded, broadleaf forest (store shot) | 0.35 | 4% | 1.10 | 2% | 14% | 68° |
| Sons of the Forest, `sotf-forest-day` | 0.148 | 46% | 1.10 | 18% | 9.7% | 63° |
| Ours, clear day (5 views) | 0.12 to 0.30, mean 0.22 | 18% | 0.93 | 28% | 12.8% | 67 to 106°, mean 77° |
| Ours, rainy day (3 views) | 0.14 to 0.32 | 37% | 0.92 | 20% | 11.4% | 90 to 116° |

**Wind on a still camera.** A scratch block-matcher (`target/job/flow.ts`, not committed) splits the upper 60% of the picture (the crowns) into 32-pixel tiles at 640×360 and finds each tile's shift over 0.1 s. It reports three numbers:
- *shared*: how much of the motion is one shift common to the whole view;
- *r*: how alike neighbouring tiles' motion is;
- *amplitude*: the median shift.

| Clip | wind | amplitude (p90) | shared | r, neighbours | r, 4 tiles apart |
|---|---|---|---|---|---|
| Enshrouded, Springlands at 5:00 | calm, unknown | 0.03 px (0.06) | 0.11 | 0.43 | 0.19 |
| Enshrouded, Springlands at 30:00 | calm, unknown | 0.04 px (0.08) | 0.24 | 0.36 | 0.34 |
| Ours, under the pines' crowns, clear | 1.7 m/s | 0.08 px (1.52) | **0.85** | **0.91** | **0.90** |
| Ours, toward the sun, clear | 1.7 m/s | 0.02 px | nothing moved above 0.05 px | | |

A frame-difference map (`target/job/motion.ts`) shows where each clip moves.
- **Enshrouded:** motion is a fine speckle at the scale of sprays all over the foliage, and the trunks are dark (still).
- **Ours:** motion comes in broad sheets where whole crowns slide over gaps of sky, and the trunks show too.

On the rainy day the rain streaks swamp both measures.

**Why ours read fake, concretely:**
1. **Wind: the crown is one rigid sheet.** `sway` in `foliage.wesl` is a translation along the wind for every vertex, scaled by (height / tree height)². Every limb therefore moves in step with the trunk at its height. The trunk slides rather than bends, since nothing keeps its length. The phase varies only at 27 to 90 m, longer than a crown. The flutter (1.5 cm, its phase at about 20 m a cycle) is under a pixel past 10 m. Result: 85% of the crowns' motion is shared and neighbouring patches correlate at 0.91, against Enshrouded's 0.11 to 0.24 and 0.36 to 0.43.
2. **Crown form: pads on poles.** From outside, a Scots pine is a bare pole with a few flat pads of one dark green, as in `clear/still-00590.png`. That is a crown of a quarter to a third of the tree (D318), its pads few and large. Enshrouded's conifers are full, layered cones of many sprays, with deep shade between the tiers. There are no branch stubs or dead lower limbs on our boles, which real pines in a stand carry.
3. **Spray scale: big magnified cards.** Looking up, the sprays are a few large fan cards with visibly repeated pictures (`clear/still-00390.png`, flat foliage 45%, flat crowns 59%). Rule 3 of the art guide asks for "many car-sized leaf clumps, not five house-sized ones"; at spray scale we break it.
4. **Colour: the crowns are teal.** Crown hue is 77° (to 106°) against 62 to 63°, and red against green 0.93 against 1.10. A rainy day pushes them to 90 to 116°. A blue-green conifer reads as sickly beside the warm olive of Enshrouded's and Sons of the Forest's.
5. **Not the cause:** how bright the forest is on a clear day (ours 0.22 against Enshrouded's 0.24 to 0.35), light through leaves (leaves catching the light 12.8% against 4.6 to 14%), or how the alpha holds up at distance (we keep coverage).

Side by side: `~/.cache/terra-review/veg-research/side-by-side.jpg`.
- Top row: Enshrouded's Springlands toward the sun, beside ours toward the sun.
- Bottom row: Enshrouded's broadleaf forest, beside our tree line.

Motion maps: `motion-side-by-side.png` (ours on top). Our film: `wind-film-ours.mp4`.

## What fits our engine and art style

- **Hierarchical wind fits best** ([D597](../docs/decisions/D597.md)).
  - We grow every tree, so the generator knows each branch's root, axis, level and parent at no cost. That is what Pivot Painter 2 bakes into textures and SpeedTree writes into extra UV sets.
  - Bevy takes custom vertex attributes, and the main, depth and shadow passes already share `foliage.wesl`'s vertex function.
  - Costs vertex work only, in the main pass and in each shadow cascade: two rotations about a pivot a vertex, about 30 to 60 ALU ops [I]. Plus 16 to 32 bytes a vertex. A near tree is 2,000 to 8,000 triangles (the art guide's budget).
  - To be measured on the walk bench (D185).
- **Crowns as many smaller sprays in layered masses** ([D598](../docs/decisions/D598.md)) is generator and content work, not engine work.
  - It may cost triangles and overdraw. Smaller cards mean more of them, so the near crown needs measuring against its budget, and the far levels already drop cards (D223).
  - It also asks a design question: how full a Scots pine's crown is, against D301's tall open stand.
- **Warmer crown colour** is a constant in the card tint and grading: free. It is part of D598, and is judged by the looks check's crown hue.
- **Octahedral impostors** would replace the procedural far billboards (D273) with captures of each grown variant. Not the gap the developer named; later.
- **Nanite Foliage** is not open to us. Bevy's meshlets take neither moving vertices nor alpha. It shows where the industry is going: parts instanced, skinned wind, voxels far away.
- **Art style (D14).** All of this keeps "textured, lightly stylised realism": real sprays, true sizes, nothing cartoon. Rule 3 (simplify, don't shrink) argues *for* more, smaller clumps. Rule 12 (leaves read as leaves) is met card by card already.

## A ranked plan

1. **A wind measure first.** The still-camera film and the block-matcher above, made a tool. Hold it to Enshrouded's numbers: shared motion under about 0.3, neighbour r under about 0.5, trunks still. That gives the change something to fail on main and pass after. A day's work; no frame cost.
2. **Hierarchical wind** (D597):
   - the trunk rotated about its foot with its length kept;
   - limbs about their roots with a phase each;
   - leaf cards about their stalks with a phase each.
   
   It answers "branches move as one unit" directly. Vertex cost only; to be benched.
3. **Warmer crowns:** crown hue back to about 62°, and red against green to about 1.1, in sun and in rain. Free: constants. The looks check holds it.
4. **Fuller crowns of smaller sprays** (D598). The larger content change, and a design call on how full the pines' crowns are. It costs triangles, to be measured against the near tree's budget.
5. **Later:** octahedral impostors captured from the grown variants for the far forest, and dead branch stubs on the boles.

## Sources

- Sousa, "Vegetation Procedural Animation and Shading in Crysis", GPU Gems 3 ch. 16, NVIDIA/Crytek, 2007. https://developer.nvidia.com/gpugems/gpugems3/part-iii-rendering/chapter-16-vegetation-procedural-animation-and-shading-crysis
- SpeedTree Modeler manual, "Fan: Games" wind; "Import to Unreal"; "Lighting leaves and fronds"; "Light object reference". https://docs.unity3d.com/speedtree-modeler/manual/fan-games.html, …/import-to-unreal.html, …/lighting-leaves-and-fronds.html, …/light-object-reference.html
- Unity Manual, SpeedTree Importer, Model tab. https://docs.unity.cn/Components/SpeedTreeImporter-Model.html
- Epic, `FSpeedTreeWind::EOptions` (UE 5.1 API). https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/Engine/FSpeedTreeWind/EOptions?application_version=5.1
- Epic, "Pivot Painter Tool 2.0"; "World Position Offset material functions"; "Shading Models"; "Nanite Foliage"; UE 5.7 release notes; `UDynamicWindSkeletalData`. https://dev.epicgames.com/documentation/unreal-engine/pivot-painter-tool-2.0-in-unreal-engine, https://dev.epicgames.com/documentation/en-us/unreal-engine/nanite-foliage, https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-5-7-release-notes
- Wihlidal (Epic), "Bringing Nanite to Fortnite Battle Royale in Chapter 4", 2023. https://www.unrealengine.com/tech-blog/bringing-nanite-to-fortnite-battle-royale-in-chapter-4
- Wohllaib, "Procedural Grass in Ghost of Tsushima", Sucker Punch, GDC 2021. https://gdcvault.com/play/1027214/Advanced-Graphics-Summit-Procedural-Grass
- Rockenbeck, "Blowing from the West: Simulating Wind in Ghost of Tsushima", GDC 2021. https://gdcvault.com/play/1027124/Blowing-from-the-West-Simulating ; write-up: https://www.gamedeveloper.com/design/using-vorticles-to-simulate-wind-in-i-ghost-of-tsushima-i-
- van Muijden, "GPU-Based Procedural Placement in Horizon Zero Dawn", Guerrilla, GDC 2017. https://www.guerrilla-games.com/read/gpu-based-procedural-placement-in-horizon-zero-dawn
- Sanders, "Between Tech and Art: The Vegetation of Horizon Zero Dawn", GDC 2018. https://gdcvault.com/play/1025066/Between-Tech-and-Art-The
- McLaren, "Adventures with Deferred Texturing in Horizon Forbidden West", GDC 2022. https://www.guerrilla-games.com/read/adventures-with-deferred-texturing-in-horizon-forbidden-west
- Barré-Brisebois, "Approximating Translucency for a Fast, Cheap and Convincing Subsurface Scattering Look", DICE, GDC 2011. https://colinbarrebrisebois.com/2011/03/07/gdc-2011-approximating-translucency-for-a-fast-cheap-and-convincing-subsurface-scattering-look/
- Castaño, "Computing Alpha Mipmaps", 2010. https://ludicon.com/castano/blog/articles/computing-alpha-mipmaps/
- Golus, "Anti-aliased Alpha Test: The Esoteric Alpha To Coverage", 2017. https://bgolus.medium.com/anti-aliased-alpha-test-the-esoteric-alpha-to-coverage-8b177335ae4f
- Brucks, "Octahedral Impostors", shaderbits, 2018. https://shaderbits.com/blog/octahedral-impostors
- McAuley, "Rendering the World of Far Cry 4", GDC 2015. https://gdcvault.com/play/1022234/Rendering-the-World-of-Far
- Godot docs, "Standard Material 3D", "Visibility ranges (HLOD)". https://docs.godotengine.org/en/stable/tutorials/3d/standard_material_3d.html, https://docs.godotengine.org/en/stable/tutorials/3d/visibility_ranges.html
- Bevy 0.20's source (`bevy_pbr` `pbr_material.rs`, `meshlet/`; `bevy_camera` `visibility/range.rs`; `bevy_anti_alias` `taa`), and "Bevy 0.16", 2025. https://bevy.org/news/bevy-0-16/
- PCGH, "Enshrouded 1.0 im Test", 2026. https://www.pcgameshardware.de/Enshrouded-Spiel-74267/Specials/1-0-Technik-Test-Benchmarks-Release-1556566/
- Enshrouded on Steam (store screenshots, launch trailer), app 1203620; "Enshrouded Ambience & Music | Sunshine in the Springlands", YouTube `1Q9A2_Nhb54`.
