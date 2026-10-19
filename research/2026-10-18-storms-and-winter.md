# Storms and winter: how games draw weather

2026-10-18. The question: a new workstream builds weather and seasons, storms and winter above all, and the developer wants films that look as real as The Witcher 3's storms, Red Dead Redemption 2's weather, Sons of the Forest's winter and Ghost of Tsushima's wind. What do shipped games do for rain, wind, storm skies, lightning, fog, snow and the turn of the seasons, what makes each read as real or fake, and what is the cheapest way to get each in Terra? It builds on [docs/climate.md](../docs/climate.md), which already has the quantities (temperature, precipitation, wind, wetness) and the climate maps round the camera (D327), the one wind with gust fronts (D255), and blown snow cards (D345). The reference frames for judging the films are in `~/.cache/terra-review/weather/refs` (sheet.jpg, CREDITS.txt).

## Answer

No game draws a storm with one effect. Each layers a dozen cheap ones, all driven by a few shared numbers, and the storm reads as real when every layer agrees. The rules:

1. **Light falls first.** Under a storm the sun is gone and the sky light drops by two orders of magnitude: about 100,000 lux in direct sun, 1,000 on an overcast day, 100 on a very dark one (Wikipedia, Lux). A storm that keeps the sun's shadows, or that exposure meters back to noon, reads as a grey filter over a sunny day. Drop the sun to nothing, make the light flat and from above, and let exposure win back only part of the loss.
2. **Distance goes first.** Rain, snow and spray fill the air, so contrast and colour fall off within a few hundred metres, and the far forest becomes flat grey-blue layers. This does most of the work in every storm frame. It is height and distance fog, not particles.
3. **Everything wet changes at once.** Ground, bark, rock and roofs darken (porous ones to about a fifth of their dry diffuse) and turn glossy, and water gathers in the low places first (Lagarde, Water drop 3b). Wetness rises and dries over minutes, not instantly.
4. **One wind moves everything, and it travels.** Grass, crowns, rain, snow, leaves and smoke all read the same wind, and gusts are visible as fronts rolling across a field (Ghost of Tsushima; Terra's D255). Rain and snow lean with it: snow, falling at about 1 m/s, flies almost flat in a gale; rain, at about 9 m/s, only tilts.
5. **Precipitation is mostly near the camera and mostly blur.** Rain is long motion-blurred streaks in a few layers round the camera, plus splashes where it lands, plus sheets in the distance; it is never thousands of modelled drops (Lagarde, Water drop 2a). Snow is slow, varied flakes, the nearest ones big and soft.
6. **Lightning is a flicker, then a lit cloud, then thunder late.** A flash is three or four strokes over a third of a second, not one pulse (Atmosphere 2016 study). Most flashes light the cloud deck from inside. Thunder comes about 3 s per kilometre later.
7. **Snow lies by physics, and is broken.** It lies on what faces up, thins under crowns, drifts on the lee side, and its edge is broken by noise and by what pokes through. Even, white, full cover reads as a texture swap (Batman: Arkham Origins; Rise of the Tomb Raider; Terra's D327 already does the first half).

## 1. Rain falling

**What games do.**
- Lagarde's survey of shipped rain (Remember Me, ToyShop, Flight Simulator 2004, Space Marine, BioShock, Saints Row: The Third) finds two families: world particles near the camera, or textured layers on geometry fixed to the camera (Water drop 2a).
- Remember Me drew four layers on a cylinder round the camera, each the same pre-blurred streak texture scrolled and scaled at its own speed, so they read as rain at different depths. Particles were rejected as they scale poorly with heavy rain. It cost about 1.7 ms on PS3 and Xbox 360 (Water drop 2a).
- ATI's ToyShop demo drew rain as screen-aligned layers too, with splashes, drips and streaming water on windows (Tatarchuk, SIGGRAPH 2006).
- A streak is what a drop looks like through an exposure: its length is the fall speed times the shutter time, and its brightness is the sky refracted through it, as Garg and Nayar showed when they rendered streaks from a database of drop oscillations (SIGGRAPH 2006).
- Rain stops under cover: Remember Me rendered a depth map from above, along the rain's direction, 256 by 256 over 20 m, and tested each drop against it; layers further away used noise masks instead (Water drop 2a).
- Splashes come from the same depth map: random spots on the geometry, 20 to 60 at once by intensity, each a short crown mesh and a few droplet sprites. A corona splash lasts 10 to 20 ms (Water drop 2a).
- Drops are 0.5 to 10 mm and fall at 3 m/s (1 mm) to 9 m/s (over 4 mm) (Water drop 2a, after Gunn and Kinzer 1949).

**What reads as real.** Streaks that are thin, faint and lit by the sky, not white; several depths moving at different speeds; rain absent under roofs and dense crowns; splashes and ripples on every flat surface and water; grey sheets of rain hanging under the cloud base kilometres off, which are fog, not drops. **What reads as fake.** Bright, thick, evenly spaced lines; one layer; rain inside a cabin; rain that does not lean with the wind; no splashes.

**For Terra:** a camera-centred cylinder of three or four scrolled streak layers in our own WGSL, leaned by the one wind, faded by depth against the scene, masked by a small top-down occlusion map from the rain's direction (or, first, the sky occlusion volume of D270, which already knows what is under a crown); splash cards at random lit points on that map; the far sheets as dark FogVolumes under the cloud base. The technique is Lagarde's published one; nothing to port.

## 2. Wet surfaces, ripples and puddles

**What games do.**
- Lagarde's model, used in Remember Me and set out in Water drop 3a and 3b: wetness only changes the dry material's own numbers.
  - The diffuse is multiplied by lerp(1, 0.2, porosity); metals and smooth materials do not darken.
  - Roughness falls toward water's; the specular reflectance hardly moves (water's F0 is about 0.02 against 0.04 for most dielectrics).
  - Three stages: damp (darker, glossier), drenched (a thin film that flattens the normal), puddle (flat water over the surface).
  - Puddles are masked by a height map or vertex colour, so water fills the low places first; drying is uneven and slower in puddles, roughly a minute for games.
- He quotes the aim as tweaking the dry BRDF's numbers rather than a new lighting model (fxguide, Part C: making wet environments).
- Ripples on puddles are a normal map of rings made each frame from a procedural circle texture, in time-shifted layers that grow with the rain; a canned animation repeats too visibly, a simulation costs too much (Water drop 2b).
- Reflections are the strongest cue of a wet world, and they are only the ordinary lighting of a smoother surface; ripples stretch lights into streaks toward the viewer, and wet surfaces look more saturated (Water drop 4a).

**What reads as real.** Darkening that differs by material (soil and bark a lot, rock less, metal none); gloss you see as sky in the rough; puddles in ruts and hollows only; wet patches surviving under crowns' drip lines after the rain stops. **What reads as fake.** A uniform gloss on everything, including grass and cloth; puddles on slopes; instant wet and dry.

**For Terra:** the ground map already holds wetness (D327) and the ground already darkens; add Lagarde's porosity per material (a number in the material data, D136), the roughness drop, and puddle masks from the terrain's own height so hollows fill first; a rain-ripple normal on the lakes and puddles by rain falling now. All in our shaders.

## 3. Wind in foliage and in the air

**What games do.**
- Ghost of Tsushima (Rockenbeck, GDC 2021; Game Developer's write-up):
  - One authored base wind direction, its strength varied by Perlin noise in time and by the weather; Perlin gusts visible as waves crossing the grass; curl noise for particles' finer motion.
  - Vorticles: invisible spheres with a direction that make swirls, straight gusts or blasts, summed brute force by every particle, hundreds of them; particles add the wind to their acceleration and look ahead on the height map to lift over hills.
  - Grass, cloth, leaves, mud and blood all read the same wind; its VFX lead says everything "was moving from it" (80.lv).
  - The grass is blades made on the GPU, each with its own shape and sway, so acres read as a field (Wohllaib, GDC 2021).
- God of War simulates a dynamic, spatially varying 3D wind, and trees and leaves sway from it without bones (Feeley, GDC 2019).
- Crysis bent vegetation in the vertex shader from a few painted weights: main bending of the whole plant, then branch and leaf-edge detail, cheap enough for every instance (Sousa, GPU Gems 3 ch. 16). Zioma's GPU Gems 3 chapter does the same for trees with noise and periodic functions.
- Unreal and SpeedTree wind: a global wind with gust settings, read by SpeedTree's hierarchical sway or by Pivot Painter's stored pivots (Unreal docs, Pivot Painter Tool 2.0). RDR2 and The Witcher 3 have no published wind talk; The Witcher 3 used SpeedTree's trees.

**What reads as real.** The whole field and the crowns move together, more at the tops, with gusts you can watch arrive; leaves and needles flutter faster than branches; in a storm, whole trees lean and recover, rain sheets swing, and loose leaves, needles and snow stream past. **What reads as fake.** Every plant swaying in phase or in place; grass moving with no gust pattern; a storm whose trees move as on a calm day.

**For Terra:** D255 already has the gust fronts across grass and crowns. Make the weather drive their amplitude, speed and length (a gale's fronts faster, stronger, closer together), add leaves, needles and loose snow as instanced cards that read the same formula (as the blown snow of D345 does), and save vorticles for local events (a dragon's wing beat, a falling tree). Ported technique: Rockenbeck's vorticles, from the talk.

## 4. Storm skies and their light

**What games do.**
- Horizon Zero Dawn raymarches clouds through a low-resolution noise volume shaped by a weather map (coverage, type, precipitation), lit by Beer's law with a powder term for dark edges, updated over several frames (Schneider, SIGGRAPH 2015). Nubis later let artists author cloudscapes and storm fronts (Schneider, SIGGRAPH 2017).
- RDR2 voxelises clouds, fog and the sky's light into one system, raymarches scattering and transmittance, and lights the scene's ambient from it, so overcast is one state of the same model (Bauer, SIGGRAPH 2019).
- Ghost of Tsushima's lighting, atmosphere and tonemapping were built for a filmic look (Patry, SIGGRAPH 2021).
- The real light: direct sun 32,000 to 100,000 lux; daylight without the sun 10,000 to 25,000; an overcast day about 1,000; a very dark overcast day about 100 (Wikipedia, Lux; Bevy's `light_consts::lux` has the same numbers).

**What reads as real.** A low, heavy, textured deck, darker underneath than at its edges; light from the whole sky, so no hard shadows and soft dark contact under trees; the far horizon sometimes brighter where the deck thins; the scene darker than a person expects. **What reads as fake.** A flat grey sky over sun-lit ground; sun shadows under a storm; full exposure compensation.

**For Terra:** first the light, which costs nothing: in a storm, fade the sun's illuminance by the cloud cover to near zero, take the sky's ambient from about 10,000 to 1,000 lux or less, and let exposure adapt by only part of the drop. Then a cloud deck: a 2D layer of noise shaped by the climate's cloud field, drawn into Bevy's Atmosphere sky. Raymarched volumes (Schneider's method, published) only if the deck fails review.

## 5. Lightning and thunder

**What the world does.**
- A negative cloud-to-ground flash has on average 3.67 strokes, a third have only one, and flashes with several strokes last on average 371 ms (study of 1,174 flashes, Atmosphere 2016). Strokes are tens of milliseconds apart (Rakov and Uman, Lightning: Physics and Effects).
- Most flashes stay in the cloud, so what a person mostly sees is the deck lit from inside.
- Sound travels at about 343 m/s, so thunder comes about 3 s per kilometre after the flash: the crack first from near strikes, a long roll from far ones.

**What games do.** No shipped game has published its lightning. In footage, The Witcher 3 and RDR2 both flash the whole scene: the sky and cloud deck brighten, the land takes a brief cold directional light from the bolt's side, and a bolt mesh shows for a few frames (observed, not documented).

**What reads as real.** A flicker of three or four pulses with a dim glow between; a branching bolt seen for a fraction of a second; most flashes as glows in the cloud; shadows from the bolt's direction; thunder late. **What reads as fake.** One even pulse; every flash a visible bolt; thunder at the same instant.

**For Terra:** an event in the weather system with a place, a time and a stroke pattern drawn from those numbers; it drives a short directional light from its place, a boost to the sky's and cloud deck's brightness near it, a bolt made of branching segments drawn as a bright ribbon for the strokes, and a thunder sound delayed by distance. All ours; small.

## 6. Fog, haze and visibility

**What games do.**
- Assassin's Creed IV computed fog in a frustum-aligned voxel grid (froxels): density, then lighting with shadows, then a march front to back, all in compute (Wronski, SIGGRAPH 2014).
- Frostbite made it physically based and unified with particles and surfaces: participating media in froxels, scattering and extinction, temporal reprojection (Hillaire, SIGGRAPH 2015).
- Unreal's volumetric fog is that froxel method inside its exponential height fog (Unreal docs, Volumetric Fog). Godot 4's volumetric fog is froxel-based too, with local FogVolumes (Godot docs; MIT).
- Bevy 0.20 has DistanceFog, a raymarched VolumetricFog with FogVolumes (64 steps by default), and Hillaire's 2020 sky (Atmosphere); Terra uses the first two.
- How far one sees: heavy snow is visibility of 400 m or less, moderate 400 to 800 m (FAA Aeronautical Information Manual, 7-1-17); a blizzard is 400 m or less in snow and wind of 35 mph (about 16 m/s) or more for three hours (NWS). Rain hides much less: in Mexican downpours the optical range fell below 1 km only above about 60 mm/h (Montero-Martínez and García-García 2025), so heavy rain leaves a few kilometres, and what greys the distance in a storm is mostly mist, spray and the cloud base coming down.

**What reads as real.** Height fog that pools in valleys and over water; distant ridges as flat layers of one grey-blue; colour lost with distance before shape is; light shafts only where the sun breaks through. **What reads as fake.** Fog that is the same everywhere; a sharp horizon behind falling snow; fog brighter than the sky.

**For Terra:** drive DistanceFog's density and colour from the weather (visibility as the number: a blizzard's 200 to 400 m, heavy snow's 400 to 800 m, rain's 2 to 5 km with the deck down), height-falloff so valleys fill first, and keep VolumetricFog for near air. If the raymarch's cost or banding fails, port Godot's froxel fog (MIT), named in the code.

## 7. Snow falling

**What the world does.**
- Snowflakes, as aggregates, fall at about 1 m/s, from about 0.5 to 1.5 m/s by type; graupel faster (Locatelli and Hobbs 1974). Rain falls 3 to 9 m/s.
- Aggregates are a few millimetres to a couple of centimetres across, largest near freezing, when they stick together.
- So wind decides the look: in 10 m/s of wind snow travels ten times faster sideways than down. Loose snow already lying starts to drift at about 5 m/s and blows in sheets by 10 (Li and Pomeroy 1997, as D345 has it).

**What games do.** Snow is particles or textured layers round the camera, as rain is, but slower, larger, and with wobble. The nearest flakes are drawn as large soft discs, as a lens out of focus draws them. RDR2's blizzards in the Grizzlies combine falling snow, blowing snow off the ground, a short visibility and a white-out (observed).

**What reads as real.** Slow flakes of several sizes with a lazy wobble; the near ones large, soft and few; the far ones a grain in the fog; the whole mass sliding with the wind and swirling round obstacles; in a blizzard, the distance gone. **What reads as fake.** Flakes falling as fast as rain; one size; every flake sharp; a blizzard with a clear horizon.

**For Terra:** the blown snow's instanced cards (D345) again, spawned in a box round the camera that wraps as it moves, falling at 1 m/s plus the wind, with a little curl noise; flake size from temperature; near flakes enlarged and blurred by a soft disc texture; and fog density from section 6 doing the rest.

## 8. Snow lying, ice and the cold

**What games do.**
- Lying snow is a layer blended over the material by how much the surface faces up, with noise breaking the edge, often with a height bias so it sits in cracks first; Terra already does this with snow depth from the ground map (D327).
- Trails: Assassin's Creed 3 and Journey raycast onto terrain and changed its mesh; Batman: Arkham Origins rendered what touches the snow from below in an ankle-high orthographic view, accumulated it into a height map, and displaced the snow with relief mapping or tessellation (Barré-Brisebois, GDC 2014).
- Rise of the Tomb Raider deformed snow with a displacement map too (Michels, SIGGRAPH 2015). RDR2's PC release raised the quality of its snow trails (Rockstar via GameSpot).
- Sons of the Forest covers the whole island in winter and freezes its lakes so they can be walked on; a season lasts five days by default (Sons of the Forest wiki).

**What reads as real.** Snow on the tops of branches and needles, not under them; bare dark trunks and the dark undersides of crowns giving contrast; drifts on lee sides; trails, ruts and footprints with blue shadow inside; roofs loaded and eaves bare; a frozen lake flat, matt and white with snow in patches and dark clear ice where wind swept it; frost and rime on the windward tips of grass and twigs; breath as a puff of fog in the cold. **What reads as fake.** Even full white cover; snow under crowns as deep as in the open; trees with snow on every face; a frozen lake still rippling and reflecting.

**For Terra:** keep D327's up-facing snow and add depth by exposure (thinner under crowns, deeper in lee hollows); a frozen state for lakes from the temperature integral (the water shader's ripples stop, the surface becomes ice with snow from the ground map over it); trails as Batman's method, a small camera-centred height map stamped by walkers; breath as a few short-lived cards at the mouth below about 5 C.

## 9. Seasons changing

**What games do.**
- Forza Horizon 4 changes the whole map each week for every player; Playground shot reference in every season, and winter is frost, ice, gritted and wet roads and frozen ponds that open new routes, not only snow (Xbox Wire 2018; GTPlanet interview with Ben Penrose).
- Sons of the Forest moves through four seasons of five days: leaves turn and fall, snow arrives and stays, lakes freeze (Sons of the Forest wiki).
- The common machinery is one global parameter (the date) driving masks in the shaders (leaf colour, leaf loss, snow, wetness), and a per-instance offset so trees, patches and kinds change at different times; Terra already does both, from the leaves map and each plant's seed (D327).
- The Witcher 3's Skellige changes weather quickly, a storm rolling in over a minute or two; it has no seasons (observed).

**What reads as real.** Change that is uneven: one tree bare, its neighbour still yellow; the first snow in the high and open places, lingering in shade; a front arriving as a line across the sky. **What reads as fake.** Everything changing at once; a cut from clear to storm.

**For Terra:** already built in form (D327). For weather, blend every storm value (sun, sky light, fog, wetness, wind, precipitation) from the climate's fields with a front that moves across the world, so a storm arrives from one side over a few minutes and the near and far ground differ while it passes.

## What to build first

In order of how much look each buys for its cost, all on numbers the climate already has: the storm's light and exposure (section 4); fog by visibility (section 6); wetness and puddles (section 2); rain streak layers with occlusion and splashes (section 1); wind amplitude and debris by weather (section 3); falling snow cards (section 7); lightning (section 5); snow depth by exposure, frozen lakes, trails (section 8); the cloud deck (section 4).

## Limits

- Rockstar, CD Projekt, Endnight and Playground have given no talks on their weather; their sections are from footage, store and press material and wikis, and are marked observed.
- The rain visibility numbers come from one study of tropical downpours; mist and the cloud base matter more than the rain itself, and they are not measured here.
- Nothing here was measured in Terra.

## Sources

- Wikipedia, "Lux", table of illuminance examples. https://en.wikipedia.org/wiki/Lux
- Sébastien Lagarde, "Water drop 1 – Observe rainy world", 2012. https://seblagarde.wordpress.com/2012/12/10/observe-rainy-world/
- Sébastien Lagarde, "Water drop 2a – Dynamic rain and its effects", 2012. https://seblagarde.wordpress.com/2012/12/27/water-drop-2a-dynamic-rain-and-its-effects/
- Sébastien Lagarde, "Water drop 2b – Dynamic rain and its effects", 2013. https://seblagarde.wordpress.com/2013/01/03/water-drop-2b-dynamic-rain-and-its-effects/
- Sébastien Lagarde, "Water drop 3a – Physically based wet surfaces", 2013. https://seblagarde.wordpress.com/2013/03/19/water-drop-3a-physically-based-wet-surfaces/
- Sébastien Lagarde, "Water drop 3b – Physically based wet surfaces", 2013. https://seblagarde.wordpress.com/2013/04/14/water-drop-3b-physically-based-wet-surfaces/
- Sébastien Lagarde, "Water drop 4a – Reflecting wet world", 2013. https://seblagarde.wordpress.com/2013/09/08/water-drop-4a-reflecting-wet-world/
- fxguide, "Game environments – Part C: making wet environments". https://www.fxguide.com/?p=43362
- Natalya Tatarchuk, "Artist-Directable Real-Time Rain Rendering in City Environments", SIGGRAPH 2006 course (ATI ToyShop).
- Kshitiz Garg and Shree Nayar, "Photorealistic Rendering of Rain Streaks", SIGGRAPH 2006.
- R. Gunn and G. Kinzer, "The terminal velocity of fall for water droplets in stagnant air", Journal of Meteorology, 1949.
- Bill Rockenbeck, "Blowing from the West: Simulating Wind in 'Ghost of Tsushima'", GDC 2021. https://gdcvault.com/play/1027124/Blowing-from-the-West-Simulating
- Game Developer, "Using vorticles to simulate wind in Ghost of Tsushima". https://gamedeveloper.com/design/using-vorticles-to-simulate-wind-in-i-ghost-of-tsushima-i-
- 80.lv, "Insights into Building VFX for Ghost of Tsushima". https://80.lv/articles/insights-into-building-vfx-for-ghost-of-tsushima/
- Eric Wohllaib, "Procedural Grass in 'Ghost of Tsushima'", GDC 2021 Advanced Graphics Summit. https://gdcvault.com/play/1027214/Advanced-Graphics-Summit-Procedural-Grass
- Jasmin Patry, "Real-Time Samurai Cinema: Lighting, Atmosphere, and Tonemapping in Ghost of Tsushima", SIGGRAPH 2021 Advances. https://www.advances.realtimerendering.com/s2021/jpatry_advances2021/index.html
- Sean Feeley, "Interactive Wind and Vegetation in 'God of War'", GDC 2019. https://gdcvault.com/play/1026036/Interactive-Wind-and-Vegetation-in
- Tiago Sousa, "Vegetation Procedural Animation and Shading in Crysis", GPU Gems 3, ch. 16. https://developer.nvidia.com/gpugems/gpugems3/part-iii-rendering/chapter-16-vegetation-procedural-animation-and-shading-crysis
- Renaldas Zioma, "GPU-Generated Procedural Wind Animations for Trees", GPU Gems 3, ch. 6. https://developer.nvidia.com/gpugems/gpugems3/part-i-geometry/chapter-6-gpu-generated-procedural-wind-animations-trees
- Epic Games, "Pivot Painter Tool 2.0", Unreal Engine documentation (by title).
- Andrew Schneider, "The Real-time Volumetric Cloudscapes of Horizon: Zero Dawn", SIGGRAPH 2015 Advances. https://advances.realtimerendering.com/s2015/index.html
- Andrew Schneider, "Nubis: Authoring Real-Time Volumetric Cloudscapes with the Decima Engine", SIGGRAPH 2017 Advances. https://advances.realtimerendering.com/s2017/index.html
- Fabian Bauer, "Creating the Atmospheric World of Red Dead Redemption 2: A Complete and Integrated Solution", SIGGRAPH 2019 Advances. https://advances.realtimerendering.com/s2019/index.htm
- Bartłomiej Wroński, "Volumetric Fog: Unified Compute Shader-Based Solution to Atmospheric Scattering", SIGGRAPH 2014 Advances. https://advances.realtimerendering.com/s2014/index.html
- Sébastien Hillaire, "Physically Based and Unified Volumetric Rendering in Frostbite", SIGGRAPH 2015 Advances. https://advances.realtimerendering.com/s2015/index.html
- Sébastien Hillaire, "A Scalable and Production Ready Sky and Atmosphere Rendering Technique", EGSR 2020.
- Epic Games, "Volumetric Fog in Unreal Engine". https://dev.epicgames.com/documentation/en-us/unreal-engine/volumetric-fog-in-unreal-engine
- Godot Engine, "Volumetric fog and fog volumes". https://docs.godotengine.org/en/4.3/tutorials/3d/volumetric_fog.html
- Study of 1,174 negative cloud-to-ground flashes (mean 3.67 strokes, 371 ms), Atmosphere, October 2016. https://doaj.org/article/a748140fbe8b4fc582e3a45e3bd36ff2
- Vladimir Rakov and Martin Uman, "Lightning: Physics and Effects", Cambridge University Press, 2003.
- FAA, Aeronautical Information Manual 7-1-17, precipitation intensity by visibility. https://www.angleofattack.com/aim/chapter-7/7-1-17/
- National Weather Service, Aberdeen, blizzard criteria. https://www.weather.gov/abr/blizzardrecap
- Guillermo Montero-Martínez and Fernando García-García, "The influence of rainfall on the extinction coefficient and the meteorological optical range", Atmósfera 39, 2025. https://www.revistascca.unam.mx/atm/index.php/atm/article/view/53491
- J. D. Locatelli and P. V. Hobbs, "Fall speeds and masses of solid precipitation particles", Journal of Geophysical Research, 1974.
- Li and Pomeroy, "Estimates of threshold wind speeds for snow transport using meteorological data", Journal of Applied Meteorology, 1997 (as cited in D345).
- Colin Barré-Brisebois, "Deformable Snow Rendering in Batman: Arkham Origins", GDC 2014. https://gdcvault.com/play/1020177/Deformable-Snow-Rendering-in-Batman ; slides https://www.slideshare.net/colinbb/gdc2014-deformable-snow-rendering-in-batman-arkham-origins
- Michels et al., "Labs R&D: Rendering Techniques in Rise of the Tomb Raider", SIGGRAPH 2015 talk. https://history.siggraph.org/learning/labs-rd-rendering-techniques-in-rise-of-the-tomb-raider
- GameSpot, "Red Dead Redemption 2 PC Graphics Improvements Announced". https://gamespot.com/articles/red-dead-redemption-2-pc-graphics-improvements-ann/1100-6470447/
- Sons of the Forest wiki, "Seasons". https://sonsoftheforest.wiki.gg/wiki/Seasons
- Xbox Wire, "Forza Horizon 4: seasons change everything", 2018. https://news.xbox.com/en-us/2018/08/10/fh4_seasons_change_everything/
- GTPlanet, "Forza Horizon 4 Dev Interview: Recreating UK Weather". https://www.gtplanet.net/?p=82396
- Török and Krzyścin, "The Rendering Features of The Witcher 3: Wild Hunt", SIGGRAPH 2015 talk. https://history.siggraph.org/learning/the-rendering-features-of-the-witcher-3-wild-hunt
