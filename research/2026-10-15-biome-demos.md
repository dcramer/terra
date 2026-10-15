# Mountain lakes, tundra and living towns: how shipped games build them

2026-10-15. Question, from the developers: "more biome demos": a forest with a lake in the mountains, with both forested slopes and high peaks; a tundra; and a town "bustling with life", with NPCs and animals. How do shipped games and engines make each, what does the real world say the place should be, which parts can we port (D196), and what in our code stands in the way? The plan built on it is [docs/demos.md](../docs/demos.md).

Tags: **[P]** primary (the developers' talk, docs, paper, or source read), **[S]** secondary (press, wikis, forums), **[I]** inferred. Most GDC Vault talks are members-only, so several findings rest on their abstracts and slides.

## Answer

- **Mountains are eroded height fields with rules on top.** Shipped open worlds bake erosion offline (World Machine, Houdini) and place by slope, height, curvature and wetness rules. A per-point erosion term can stand in for the bake on a world generated where you stand.
- **The tree line is a temperature, not a height.** It sits where the growing season's mean is about 6.4 to 6.7 °C (Körner and Paulsen). So the climate fields we already have, with their 6.5 °C a kilometre lapse rate, give it for free. What they need is a kilometre of relief.
- **Lake water is a medium, not a mirror.** It absorbs and scatters by depth, has a foam and wet band where depth is small, ripples by the wind, and reflects the far land. Engines draw it as a single layer after the opaque pass.
- **Tundra is wind, wetness and patterned ground.** Lichen on exposed ridges, dwarf shrubs and sedges in the lee and the wet, pools along ice-wedge polygons, and snow drifted into the same lee hollows every year.
- **A bustling town is a few dozen full actors and many cheap ones.** Shipped towns run time-windowed schedules over smart objects or pooled spots, simulate by distance, and draw people in tiers. Assassin's Creed Unity carries 10,000 people on 40 full AIs and 120 high-detail models.
- **Our blockers, from reading the code:**
  - **Relief.** The world's relief is about 150 m and the lush forest's 6 m, about 1 °C of lapse. No climate tree line can show until the ground rises a kilometre.
  - **Air scale.** The 18× haze scale (D212, D299) would wash 5 km peaks to the sky.
  - **Water.** Only one sea sphere, and nothing local (`terra_core::Water` has no implementation).
  - **Missing systems.** No particles, no navigation, no schedules, no character LOD.
  - **Budget.** The lush forest walk already uses the whole frame budget on the RTX 3080: p99 14.8 to 16.0 ms against 13.3 (D309).

## Mountains and valleys

**Terrain shaping**

- **[P] Far Cry 5** (GDC 2018, Carrier):
  - Houdini tools fill about 100 km² of wilderness: biomes, texturing, freshwater networks and cliff rocks.
  - Water, cliffs and vegetation are generated together, because water shapes the land.
  - Rules: grassland on exposed hills, pines in valleys, denser forest on north slopes where shade keeps the soil moist.
  - Species compete for ground. Grass is turned along a wind-vector map.
  - https://gdcvault.com/play/1025215/Procedural-World-Generation-of-Far , https://blog.playstation.com/2018/03/22/the-procedural-world-generation-of-far-cry-5/
- **[P] Far Cry 5 terrain rendering** (GDC 2018, Moore): a GPU pipeline for heightfield LOD, culling and stitching, with procedural cliff geometry on top. https://gdcvault.com/play/1025261/Terrain-Rendering-in-Far-Cry
- **[S] Ghost Recon Wildlands** (GDC 2017): a 64k World Machine terrain, rebuilt every three days on a farm; Houdini roads, villages and rocks by slope, curvature and cliff masks. https://80.lv/articles/landscape-and-material-pipeline-of-ghost-recon-wildlands/
- **[P] Horizon Zero Dawn** (GDC 2017, van Muijden):
  - Placement runs on the GPU round the player, from rule graphs.
  - It is deterministic and locally stable, so moving a river does not re-dress the world.
  - It places sounds, effects and wildlife as well as plants.
  - https://www.guerrilla-games.com/media/News/Files/GDC2017_VanMuijden_GPUBasedProceduralPlacementInHorizonZeroDawn.pdf
- **[P] Ghost of Tsushima** (GDC 2021, "Samurai Landscapes"): placement and texturing rules compiled to bytecode on the GPU, millions of instances. https://gdcvault.com/play/1027352/Samurai-Landscapes-Building-and-Rendering
- **[P] Erosion where you stand.** Our world is generated per point, so an offline bake does not fit. Three references:
  - **Quilez's derivative fBm:** each octave is damped by the slope so far, giving smooth valleys and rough ridges. https://iquilezles.org/articles/morenoise
  - **Runevision's erosion filter** (March 2026): gullies along the downhill gradient, branching by octave. It works per point on top of any height function. Its code is **MPL-2.0**, not a licence D196 lets us take, so work from the write-up only. https://blog.runevision.com/2026/03/fast-and-gorgeous-erosion-filter.html
  - **The simulation baseline:** Mei, Decaudin and Hu 2007, "virtual pipes" hydraulic erosion. https://hgpu.org/?p=2256

**Tree line and slopes**

- **[P] The tree line.** Körner and Paulsen 2004, 46 sites from 68°N to 42°S:
  - It sits at a growing-season mean root-zone temperature of 6.7 ± 0.8 °C, and 6.4 °C in the 2012 revision for air temperature.
  - Season length and thermal sums predict it worse.
  - https://edoc.unibas.ch/8594
- **[S] Tree-line heights:**
  - About 2,100 m in the Alps at 46°N, 1,100 m in Norway at 61°N, 800 m in Sweden at 68°N.
  - Higher on sunny sides. Krummholz is the stunted, wind-shaped fringe above the closed forest.
  - https://en.wikipedia.org/wiki/Tree_line
- **[S] Scree** rests at 33 to 37°, below cliffs, in cones and concave fans. https://en.wikipedia.org/wiki/Scree
- **[I] The Tatras.** Morskie Oko, a lake at 1,395 m under Rysy at 2,499 m, is the whole brief in one valley: spruce to the water, dwarf pine above about 1,550 m, scree fans (Wielki Piarg) to the shore, rock and snow patches above. It is about 1.1 km of relief over 2 to 3 km.

**Far peaks**

- **[P] Terrain LOD:** geometry clipmaps (Losasso and Hoppe 2004, https://hhoppe.com/proj/geomclipmap/) and CDLOD (Strugar 2009, https://github.com/fstrugar/CDLOD).
- **[P] Ghost of Yōtei** bakes its distant mountains into textures, and culls over a million trees, rocks and bushes to about 60,000 in a shot. https://blog.playstation.com/?p=411201
- **[P] Octahedral impostors** (Epic's Impostor Baker; Fortnite uses upper-hemisphere tree impostors). https://dev.epicgames.com/documentation/unreal-engine/impostor-baker-plugin-in-unreal-engine
- **[P] Bevy's atmosphere.** `bevy_pbr/src/atmosphere/mod.rs` in the pinned Bevy implements Hillaire 2020 with aerial perspective in front of the ground (https://diglib.eg.org/handle/10.1111/cgf14050). Read locally.
  - Ours draws it Earth-sized with distances 18 times the world's (D212).
  - That was right for 150 m hills. It would make a peak 5 km off as hazy as 90 km of real air.

## Water

- **[P] Unreal, Single Layer Water:**
  - The body is a homogeneous medium with absorption and scattering coefficients and a phase function.
  - It is drawn in its own pass after the opaque scene, with plain translucency as the low-end fallback.
  - Bodies (lake, river, ocean) are authored as splines.
  - https://dev.epicgames.com/documentation/en-us/unreal-engine/single-layer-water-shading-model-in-unreal-engine , https://dev.epicgames.com/documentation/en-us/unreal-engine/water-system-in-unreal-engine
- **[P] Unity HDRP water** has three presets: pool, river and ocean. A pool uses only the ripples band of its FFT, a river ripples and agitation. Lakes need no swell. https://unity.com/blog/engine-platform/new-hdrp-water-system-in-2022-lts-and-2023-1
- **[P] Valve's flow maps** (SIGGRAPH 2010): two normal-map phases half a cycle apart hide the reset, at about 4 texels a metre. https://advances.realtimerendering.com/s2010/index.html
- **[P] Bevy 0.20, read locally:**
  - Its screen-space reflections need the deferred renderer and reflect only what is on screen.
  - The `ssr` example's water is an `ExtendedMaterial` with animated ripples.
  - Our renderer is forward (nothing in `crates/` asks for deferred), so SSR is not free to us.
- **[P] Crates and code to port:**
  - `bevy_water` 0.18.1 (MIT/Apache) supports Bevy 0.18, not 0.20.
  - Godot water shaders under MIT exist (foam, refraction, depth fog), e.g. https://github.com/Chrisknyfe/boujie_water_shader.
  - Jolt (MIT) has buoyancy impulses ready to port (`Body::ApplyBuoyancyImpulse`). https://github.com/jrouwe/JoltPhysics/blob/master/Jolt/Physics/Body/Body.h
  - No current O3DE water gem was found.
- **[P] Planar reflections** in Unreal take a screen percentage and an LOD distance factor, so the mirrored view can be drawn smaller and coarser than the main one. Engine docs.
- **[S] Wet ground** darkens and smooths the dry material rather than changing its lighting model (Lagarde). https://www.fxguide.com/?p=43362
- **[I] Shores** compare scene depth with the water's surface to drive foam, absorption (exp(−σd)) and a wet band above the waterline. Every engine above does this.
- **[P] Lakes as filled basins** (GIS practice): Priority-Flood (Barnes, Lehman and Mulla 2014) fills each depression of a height field to its spill point in one pass. That is the "basin filled to its own level" our [engine.md](../docs/engine.md#water-and-melting) already asks for. https://arxiv.org/abs/1511.04463

## Tundra

**Ecology**

- **[P] Ground cover:**
  - In the far north, mosses and lichens dominate. Further south come dwarf shrubs and graminoids.
  - Shrub tundra: *Betula nana*, *Empetrum nigrum* and *Cladonia* lichens. https://eunis.eea.europa.eu/habitats/8144
  - Tussock tundra: *Eriophorum vaginatum* with dwarf birch, willow, Labrador tea, lingonberry and *Sphagnum*. https://www.arcticatlas.org/atlas/aatvm/aatvmvg/s11
- **[P] Patterned ground.** Ice-wedge polygons are about 30 m across with under 50 cm of relief. Thaw leaves metre-scale thermokarst pools along their edges. https://www.ncbi.nlm.nih.gov/pmc/articles/PMC7064580/
- **[P] Wind:**
  - Lee slopes hold about twice the snow of windward ones, and late snow patches recur in the same places every year.
  - Cornices form at crests.
  - https://www.cambridge.org/core/journals/annals-of-glaciology/article/distribution-of-snow-drifts-on-ridge
  - The standard terrain index for this is Winstral's Sx, the greatest upwind slope angle within a search distance.
- **[S] The forest-tundra edge** (*Waldtundra*): stunted spruce and larch in islands, krummholz mats, getting shorter and sparser to the open tundra. Wikimedia Commons, Category:Waldtundra.

**Snow and wind in games**

- **[P] Snow by normal** (Company of Heroes 2): snow blends in by how far a surface faces up, with a top-down snow-shadow map so none lies under overhangs. https://www.slideshare.net/proyZ/daniel-barrero-coh2renderingtech
- **[P] Rise of the Tomb Raider** (deferred snow deformation, GPU Pro 7): trails pushed down and raised at their edges, refilled in storms, 4 MB, dozens of NPCs. https://history.siggraph.org/wp-content/uploads/2022/10/2015-Talks-Michels_Labs-R_D-Rendering-Techniques-in-Rise-of-the-Tomb-Raider.pdf
- **[P] Ghost of Yōtei:** tessellated snow with a displacement buffer; snow knocked off branches as particles; sparkle from stable screen-space noise. https://blog.playstation.com/?p=411201
- **[P] Ghost of Tsushima** (GDC 2021, "Blowing from the West"): one wind drives cloth, hundreds of thousands of particles and millions of grass blades. We already follow it for gusts (D255). https://gdcvault.com/play/1027350/Blowing-from-the-West-Simulating
- **[S] The Long Dark** is painterly and bright. Valheim is deliberately low-detail, carried by its light and post-processing. Neither is our look (D14), but both show how open cold country reads: big skies, wind you can see, sparse verticals. https://www.pcgamer.com/uk/the-long-darks-creative-director-explains-why-it-doesnt-need-zombies

## Towns

**Simulation**

- **[P] Kingdom Come: Deliverance II:**
  - Nearly 2,400 NPCs, about four times the first game's, half of them in one city.
  - Every NPC keeps its routine even when the player is far, through AI level of detail.
  - It combines GOAP with modular behaviour trees.
  - https://gdcvault.com/play/1035576/Game-AI-Summit-Combining-GOAP
- **[P] Assassin's Creed Unity** (GDC 2015): 40 full AIs and 120 high-detail models carry about 10,000 crowd NPCs on screen, swapped through a pool without the player seeing. https://gdcvault.com/play/1022141/Massive-Crowd-on
- **[P] Hitman: Absolution** (GDC 2012): crowds of 1,200 at 30 fps on PS3 and 360. https://gdcvault.com/play/1016443/Crowds-in-Hitman
- **[S] The Witcher 3:**
  - NPCs choose among pooled spots instead of fixed routes, e.g. 200 spots for about 50 sailors in Novigrad's harbour.
  - Its job trees are sequences of actions with their animations.
  - https://cdprojektred.atlassian.net/wiki/spaces/W3REDkit/pages/6328487/TOOL%3A+Job+Tree+editor
- **[S] Skyrim's Radiant AI:** time-windowed packages per NPC (where to work, eat, sleep), with "sandbox" idles at markers. https://wiki.beyondskyrim.org/wiki/Arcane_University:NPC_Packages
- **[P] The Sims:** objects advertise what needs they satisfy, and a Sim picks among the best offers by its motives. These are smart objects. https://qrg.northwestern.edu/papers/Files/Programming_Objects_in_The_Sims.pdf
- **[P] F.E.A.R.:** GOAP over three states (go to, animate, use a smart object). https://pages.cs.wisc.edu/~dyer/cs540/handouts/gdc2006_orkin_jeff_fear.pdf

**Drawing crowds**

- **[P] Instanced skinning** through vertex texture fetch: about 10,000 characters on 2007 hardware (GPU Gems 3, ch. 2). https://developer.nvidia.com/gpugems/GPUGems3/gpugems3_ch02.html
- **[P] Vertex animation textures** for crowds (SideFX). https://www.sidefx.com/learn/talks/vertex-animation-textures-for-crowds/
- **[P] Unity's Animation Instancing** skins on the GPU but cannot blend clips. https://blog.unity.com/engine-platform/animation-instancing-for-skinnedmeshrenderer
- **[P] Unreal's Animation Budget Allocator:** a millisecond budget per platform, ranked by significance, lowering tick rate, interpolating, or sharing a leader pose. https://dev.epicgames.com/documentation/en-us/unreal-engine/animation-budget-allocator-in-unreal-engine
- **[P] Ours, measured 2026-10-12** ([bevy-character-capabilities.md](bevy-character-capabilities.md)), dressed characters of 73 joints:
  - RTX 5090: 100 cost a 5.8 ms frame (3.8 ms of game CPU); 1,000 cost 13.3 ms, bound by the CPU's animation and transforms.
  - Integrated GPU: 2.5 to 4 ms each.
  - Nothing has been measured on the RTX 3080.

**Navigation** (crates.io, checked 2026-10-15)

- **[P] Ports:** Recast/Detour with DetourCrowd is zlib, and RVO2/ORCA is Apache-2.0; both can be ported.
- **[P] Rust crates:**
  - `landmass` 0.9.3 has no Bevy dependency, but `bevy_landmass` needs Bevy 0.19.
  - `vleue_navigator` needs Bevy 0.19.
  - `oxidized_navigation` stopped at Bevy 0.15.
  - None targets 0.20.

**Layout and sound**

- **[P] Emilien et al. 2012**, used for Ghost Recon Wildlands: interest maps seed a village and its roads, land is divided into parcels along them, and a grammar fits buildings to the slope. https://perso.liris.cnrs.fr/egalin/Articles/2012-villages.pdf
- **[S] Watabou's generator** builds Voronoi wards of market, craft and gate. https://watabou.itch.io/medieval-fantasy-city-generator
- **[P] The Witcher 3's crowd sound** (Wwise Tour 2016): the town round the player is split into four quadrants, each counting the people in it and how they are (calm, afraid), sent as parameters to a crowd bed; ambient zones are volumes. https://blog.audiokinetic.com/ (the Wwise Tour 2016 article on The Witcher 3)
- **[P] Ghost of Tsushima's ambience** scales with time, weather and region (GDC 2019).

## Measuring and judging

- **[P] The Division** (GDC 2019): bots take the player's input and wander the streets to gather performance data. https://80.lv/articles/gdc-using-ai-controlled-players-to-test-the-division/
- **[P] Unreal's Gauntlet** runs sessions unattended, and its CSV profiler keeps per-frame times by thread. https://dev.epicgames.com/documentation/en-us/unreal-engine/gauntlet-automation-framework-overview-in-unreal-engine
- **[I] Heat maps** of frame time over the world map are common practice; no studio's own was found published.
- **[P] Look references:** studios gather reference boards per location, then a colour script (Cyberpunk 2077: Phantom Liberty, GDC 2024).

## Where reference frames come from

- **Games.** Steam store screenshots and trailers, sampled as in [2026-10-14-sotf-forest-look.md](2026-10-14-sotf-forest-look.md). The app IDs are from memory and should be checked against the store API before use.
  - Mountain lake: Sons of the Forest (1326470: snowy peaks and lakes, already our reference), Red Dead Redemption 2 (1174180), Far Cry 5 (552520), Horizon Zero Dawn (1151640).
  - Tundra: Red Dead Redemption 2 (the Grizzlies), Skyrim Special Edition (489830), The Long Dark (305620), Valheim (892970).
  - Towns: Kingdom Come: Deliverance II (1771300), Kingdom Come: Deliverance (379430), The Witcher 3 (292030), Mount & Blade II: Bannerlord (261550).
- **Photographs** from Wikimedia Commons, checking each file's licence (mostly CC BY-SA):
  - Mountain lake: Category:Morskie_Oko (296 files, with Wielki Piarg's scree), Category:Pragser_Wildsee, Category:Moraine_Lake.
  - Tundra: Category:Tundra (with "Tundra lakes and ponds", "Alpine tundras", "Waldtundra") and Category:Tundra_by_country.
  - Towns: Category:Medieval_markets.
- **Keep everything outside the repository**, under `~/.cache/terra-review/refs/`.
