# How shipped games fill a world with plants

2026-10-12. Question: how do shipped games place and draw vegetation over large, procedural terrain so that it reads as full and organic, and what does Bevy give us toward it? Asked after the developers named Sons of the Forest's world design as an inspiration (D85). It informs [docs/biomes.md](../docs/biomes.md).

This is a survey made through web research on the day; nothing was tried. Most GDC recordings are paywalled, and only Horizon Zero Dawn's slides were read in full.

**Labels on each claim:**
- **developer**: the developer's own slides, abstract, blog or docs;
- **second-hand**: press, retold interviews, community write-ups;
- **unverified**: not confirmed.

## Answer

- **Placement is rules over maps, not hand work.** The best-documented shipped system, Horizon Zero Dawn's, places all its nature from density maps and blue-noise patterns, deterministically, with three people making the nature assets and one making the ecosystems. Far Cry 5 and Ghost Recon Wildlands did the same offline in Houdini, with species competing for ground and big trees seeding smaller ones.
- **Fullness comes from layers, density near the camera, and clumping.** It doesn't come from detailed models.
- **Grass is generated on the GPU.** In Ghost of Tsushima it is one curve per blade, built in the vertex shader.
- **Far trees are impostors.**
- **Only changes are stored.** Valheim, Astroneer and Rust all store the edits on top of the generated world.
- **Bevy has the engine pieces but no foliage layer.** It has GPU-driven instancing, dithered cross-fades between levels of detail, and GPU occlusion culling. Placement, grass, impostors and wind would be ours to build. The community crates are early or stale.

## Placement

**Horizon Zero Dawn** (Guerrilla, GDC 2017; [slides](https://www.guerrilla-games.com/media/News/Files/GDC2017_VanMuijden_GPUBasedProceduralPlacementInHorizonZeroDawn.pdf); developer)
- **Goals:** "Quick iterations, Large variety, Believable look, Art Directable, Data driven, Deterministic, Locally stable."
- **Scale:** used for all nature. 500+ asset types and 100,000+ objects in a scene, for about 250 µs of GPU time.
- **Ecotopes:** an ecotope is a kind of place. It decides which assets grow, their distribution and colour, and the weather, effects, sound and wildlife.
- **World maps:** the world is a stack of 2D maps, generated or painted:
  - height and erosion (flow, deposition, wear);
  - cavity;
  - roads and water;
  - placement maps for trees and undergrowth;
  - colour-variance and lichen maps.

  Most are at 0.5 to 1 m resolution, about 4 MB per km².
- **Steps, for each layer:**
  1. An artist's rule graph is evaluated into a density texture.
  2. The density is thresholded against a precomputed pattern, spaced to the asset's footprint. This is effectively blue noise.
  3. Each point gets a random orientation and scale, seeded by its pattern index.
- **Layers never collide.** Layers with the same footprint share a pattern and take separate threshold ranges, so no collision pass is needed.
- **Locally stable:** changing one map changes only what is near the change, so artists could move rivers and mountains without redressing the world.

**Far Cry 5** (Ubisoft, GDC 2018; [summary](https://blog.playstation.com/2018/03/22/the-procedural-world-generation-of-far-cry-5/); developer)
- Houdini tools made the biomes, terrain texturing, rivers and cliff rocks over 100 km².
- "Each species of vegetation has its own reason for growing at any specific location, and several species will fight for the ground."
- Grass reads the terrain's humidity and lies along a wind map.
- In an interview, the producer said they simulated light, moisture and temperature gradients and inferred the vegetation from them ([GamesBeat](https://gamesbeat.com/ubisoft-winnipeg-guides-ubisofts-use-of-ai-and-cloud-computing-to-make-bigger-better-games/); second-hand).

**Ghost Recon Wildlands** (Ubisoft, 2017; [80.lv interview](https://80.lv/articles/vegetation-generation-in-ghost-recon-wildlands/); developer)
- **Two systems:**
  - grass spawned from the terrain's material;
  - trees and bushes scattered by a Houdini tool.
- **Cascade:** big trees spawn medium trees, which spawn small trees and bushes.
- **Rules:**
  - density, scale, slope and spacing first;
  - then sun direction and which species grow on which material;
  - then exclusion masks for riverbeds, cliffs and roads.
- **Species:** each has several growth stages, and each rule uses only a few species, which looks natural and saves memory.
- **Levels of detail:** dense areas switch to impostors sooner.

**Ghost of Tsushima** (GDC 2021, ["Samurai Landscapes"](https://gdcvault.com/play/1027352/Samurai-Landscapes-Building-and-Rendering); developer abstract): millions of instances, placed by rules that artists write in a small language run on the GPU.

**No Man's Sky** (GDC 2017, [abstract](https://gdcvault.com/play/1024265/Continuous-World-Generation-in-No); developer): voxels, then polygons, then texturing, then "population". How flora is placed is unverified; the talk is paywalled.

**Not found:**
- Sons of the Forest is Unity HDRP ([PCGH](https://www.pcgameshardware.de/Sons-of-the-Forest-Spiel-72418/Specials/Test-Steam-Anforderungen-Benchmark-Release-1414211/); second-hand). Nothing public says whether its foliage is placed by hand or by rule.
- Enshrouded: nothing public.
- Death Stranding uses Decima, as Horizon does, but nothing says it uses the same placement.

**The building block:** Poisson-disc or blue-noise sampling ([Bridson 2007](https://www.cs.ubc.ca/~rbridson/docs/bridson-siggraph07-poissondisk.pdf)). Horizon's precomputed pattern is a version that needs no neighbour search and gives the same result for a tile every time.

## Drawing

**Grass in Ghost of Tsushima** (GDC 2021, [abstract](https://gdcvault.com/play/1027214/Advanced-Graphics-Summit-Procedural-Grass); developer). The details come from talk summaries ([1](https://gist.ly/youtube-summarizer/procedural-grass-systems-in-ghost-of-tsushima-achieving-art-direction), [2](https://tigerabrodi.blog/grass-in-ghost-of-tsushima); second-hand):
- **Blades:**
  - A compute shader lays out blades per tile, culled by view and distance.
  - Each blade is a curve built in the vertex shader from its index, with no vertex buffer.
  - Blades clump by Voronoi cells, which vary their height and facing.
  - Normals are tilted outward to look round.
- **Wind** is one 2D noise field, read by the CPU and the GPU alike.
- **Distance:**
  - Two levels of detail blend by moving vertices.
  - Far tiles use painted textures, so the distance isn't flat green.
- **Shadows:** a full shadow pass was too expensive, so grass uses a height impostor and screen-space shadows.
- **Flowers** are separate instanced meshes.

**Impostors.** Unreal's [Impostor Baker](https://dev.epicgames.com/documentation/en-us/unreal-engine/impostor-baker-plugin-in-unreal-engine) (developer) bakes views from a hemisphere as the last level of detail; it is used for most of Fortnite's assets. Wildlands' last level is a billboard that shows a different side by angle, with light and translucency channels.

**Nanite foliage** (Unreal 5.7, [release](https://www.unrealengine.com/news/unreal-engine-5-7-is-now-available); developer, labelled experimental):
- **Approach:** distant canopies drawn as pixel-sized voxels with no hand-made levels of detail, repeated branch parts instanced, and wind per bone.
- **Forum reports on 5.7** (second-hand): holes in voxelised trunks, and a high cost with many skinned instances.

**Wind:**
- **Crysis** ([GPU Gems 3, ch. 16](https://developer.nvidia.com/gpugems/GPUGems3/gpugems3_ch16.html); developer): a main bend of the whole plant plus a detail bend of the leaves, with each instance's own stiffness, all in the vertex shader.
- **SpeedTree** ([docs](https://docs.unity3d.com/speedtree-modeler/manual/wind-games.html)): stores wind weights per vertex, for each level of trunk, branch and leaf.

**Overdraw.** Leaves on alpha-tested cards cost overdraw and alias; real geometry, as Tsushima's blades are, avoids that but needs culling and levels of detail. Guerrilla's GDC 2018 talk on Horizon's vegetation ([Vault](https://gdcvault.com/play/1025066/Between-Tech-and-Art-The)) has the details; it wasn't read.

## Digging, cutting and regrowth

- **Digging:**
  - **Astroneer** ([blog](https://blog.astroneer.space/p/augments/); developer): removing terrain also clears plants and rocks.
  - **7 Days to Die:** grass doesn't come back on dug ground (player reports).
  - **Valheim:** started by storing every strike as a networked object replayed on load, and replaced that in 0.150.3 with something lighter ([GameSpot](https://gamespot.com/articles/valheim-update-0-150-3-notes-new-terrain-system-for-faster-loading/1100-6490339/); second-hand). Either way, only edits are stored over the seeded world.
- **Cutting:**
  - **Valheim** (second-hand): trees become stumps, which are separate objects, and don't regrow.
  - **Rust** ([devblog 192](https://rust.facepunch.com/blog/devblog-192); developer): replaced SpeedTree trees with its own models so a tree can be cut in half and leave a stump. Its server refills resources to a target density (modding forums; unverified in detail).
- **Regrowth:**
  - **The Forest** ([v0.42](https://endnightgames.com/node/687); developer): added regrowth, a share of trees each time you sleep, stopped by removing the stump.
  - **Sons of the Forest:** has it on by default, 10% when sleeping (second-hand).
- **Grass giving way to the player:** usually the player is written into a texture that the grass shader reads and that fades over time (tutorials; no shipped game confirmed).

## Bevy

Checked in the pinned 0.19.1 source and the release notes. Bevy 0.20 came out on 2026-10-08; we pin 0.19.

**Built in:**
- **GPU-driven rendering** (since [0.16](https://bevy.org/news/bevy-0-16/)): indirect drawing, bindless materials, and culling on the GPU, on by default for standard meshes.
  - Full support is on Vulkan only.
  - **A custom material must be `#[bindless]`, or it falls back to the CPU path.** Our wind material would need this.
- **[0.19](https://bevy.org/news/bevy-0-19/):**
  - visibility ranges checked on the GPU;
  - `NoCpuCulling`;
  - only changed transforms uploaded, which suits static scatter.
- **`VisibilityRange`:** dithered cross-fades between levels of detail. Its own example goes from a detailed mesh to a simple one to a billboard impostor.
- **`OcclusionCulling`:** two-phase, on the GPU, off by default.
- **0.20 adds mesh shaders.** The release notes name procedural grass with dynamic levels of detail as a use. No web support.

**Not usable for foliage:**
- **Meshlets (virtual geometry)** are still experimental. They take only opaque materials, with no alpha mask and no vertex shader of your own, so no wind.
- **There is no built-in grass, scatter, impostor baker or wind.**

**Community crates**, checked on crates.io the same day:

| Crate | Bevy | State |
|---|---|---|
| [bevy_feronia](https://github.com/NicoZweifel/bevy_feronia) 0.8.4 | 0.18 | Scatter layers, instancing, wind, chunked GPU culling. Its author calls it early and not for production; no 0.19 release |
| bevy_eidolon 0.5.0 | 0.19 | Its instanced material, from the same author |
| [bevy_flora](https://github.com/Ladvien/bevy_flora) 0.14.0 | 0.20 | Parametric stylised trees from a seed; two days old |
| bevy_symbios 0.11.0 | 0.19 | L-system plants |
| bevy_foliage_tool 0.16.2 | 0.16 | Stale |
| warbler_grass, bevy_procedural_grass | 0.13 and older | Unmaintained |

## What this suggests for us

Inferred, for [docs/biomes.md](../docs/biomes.md); it decides nothing.

- **Placement:** Horizon's model fits our rules almost exactly: data-driven, deterministic, locally stable, many kinds as data, few people. That means:
  - ecotopes (our biomes) as data;
  - density from the land;
  - footprint-spaced blue noise per cell;
  - separate threshold ranges instead of collision passes.

  Add the cascade and species competition from Far Cry 5 and Wildlands. One difference: our objects must be the same on the server, so they are placed on the CPU in shared code. Cover can be on the GPU.
- **Edits:** placement is regenerated per cell from the seed, and only changes are stored. Rust's refill to a density, and The Forest's regrowth from stumps, are shipped models for regrowth if the developers want it (D43).
- **Drawing:**
  - Tsushima-style grass;
  - instanced meshes with a bindless wind material in the Crysis style;
  - `VisibilityRange` fading to impostors;
  - not meshlets.

  Mesh shaders in 0.20 are worth a look when we upgrade.
- **Read before building:** Tsushima's grass talk, which is free on GDC's YouTube, and Guerrilla's 2018 vegetation talk.

## Limits

- **Nothing here was tried.** No placement or grass was built or measured.
- **Most talks were read as abstracts or summaries.** Only Horizon Zero Dawn's slides were read in full. Tsushima's numbers (vertices per blade, how many blades are dropped far off) are from summaries.
- **Nothing public was found on Sons of the Forest or Enshrouded:** how they place or draw their plants, or how they sync cut trees.
- **None of the sources is a stylised game** with a technical account of its foliage. Breath of the Wild, Genshin Impact and Sea of Thieves were looked for and not found.
- **Crates change monthly.** The table is as of 2026-10-12.
