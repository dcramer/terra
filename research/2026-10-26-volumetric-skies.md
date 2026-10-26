# Volumetric clouds and a real sky: how games and engines draw them

2026-10-26. The question: the developer wants the sky to look real, "volumetric clouds and realism for weather", clouds "in the sky itself", not painted on a backdrop, with "diff shades", rain, wind, sun and moon. What do shipped games and engines do for clouds as volumes, their light and shadows, the sun, moon and stars, and the sky's optical effects, what does each cost, what does Bevy offer, and what is there to port? It builds on [2026-10-18-storms-and-winter.md](2026-10-18-storms-and-winter.md), which covers rain, wet surfaces, wind, lightning, fog and snow; this note does not repeat them. The design that follows is [docs/sky.md](../docs/sky.md).

## Answer

1. **Every shipped real-time cloud system since 2015 is the same method**: a shell of air a few kilometres thick round the planet, ray-marched per pixel through a density made of a *weather map* (where cloud is, how much, what kind) times a *height profile* per cloud kind, eroded by tiling 3D noise; lit by Beer's law toward the sun with a forward-scattering phase function and an approximation of multiple scattering; drawn at reduced resolution and rebuilt over several frames by temporal reprojection. Horizon (Nubis), Frostbite, Unreal's Volumetric Cloud, RDR2 and Microsoft Flight Simulator all do it; they differ in how the weather map is made and how far they let the camera into the cloud.
2. **The shades come from optics, not paint.** Liquid cloud absorbs almost nothing; a base is dark because the light went out the top and sides, an edge toward the sun is bright because cloud scatters strongly forward (the silver lining), a sunlit flank dims at its creases because light entering there has not yet scattered toward the eye (Schneider's "powder" term), and a thick deck's inside is brighter than single scattering says because light bounces many times. Get those four and the clouds read as real.
3. **Clouds are a light, not only a picture.** In every engine that does them well the same clouds shadow the ground (a cloud shadow map over the camera's surroundings), dim and redden the sun, cut shafts in the haze, and are captured into the sky light, so the shade on the ground follows the cloud overhead.
4. **What it costs**: about 2 ms of a PS4 at 1080p (Horizon Zero Dawn), "performant at 1080p without temporal upscaling" on a PS5 for fly-through clouds (Forbidden West); every implementation spends its effort on reprojection and skipping empty air rather than on steps.
5. **Bevy 0.20 has no clouds**, but the author of Bevy's atmosphere has a Nubis-based volumetric cloud implementation in development on top of Bevy's atmosphere (Apache-2.0/MIT, unmerged, active this month): cloud layers from published large-eddy-simulation profiles, a cirrus layer, a time-sliced cloud shadow map after RDR2's, Nubis Cubed's distance field for skipping empty air, multiple scattering derived from Hillaire's, and a path-traced reference to check against. It is the first thing to measure (D196).
6. **The night sky and the sky's optics have closed-form physics**: the moon's phase and brightness (Krisciunas and Schaefer 1991), earthshine, stars by catalogue magnitude, rainbows at 42° and 51° from the point opposite the sun, ice halos at 22°. Each is cheap and testable against a number.

## 1. Clouds as volumes: the method

**Horizon Zero Dawn, Nubis (Schneider and Vos, SIGGRAPH 2015; Schneider, GPU Pro 7, 2016).** A cloud layer between about 1.5 and 4 km. Density is a low-frequency Perlin-Worley 3D texture (128³) shaped by a height gradient per cloud kind (stratus, stratocumulus, cumulus) and a coverage value, then eroded at its edges by a high-frequency Worley texture (32³) distorted by curl noise for wispy, turbulent edges. The *weather texture* gives coverage, precipitation (which darkens and fattens cloud) and kind per place. Light: Beer's law for transmittance toward the sun (six samples in a cone), Henyey-Greenstein phase for forward scattering, and the "powder" term that darkens edges facing the viewer. Drawn at a quarter resolution, each frame updating one pixel in sixteen and reprojecting the rest: about 2 ms on a PS4. High cirrus is a 2D layer above.

**Nubis 2017 (Schneider, SIGGRAPH 2017, "Nubis: authoring real-time volumetric cloudscapes with the Decima engine").** Artists paint the weather map and cloud-kind maps, storm fronts and anvils of cumulonimbus come from the same model.

**Nubis, Evolved (Schneider, SIGGRAPH 2022) and Nubis Cubed (SIGGRAPH 2023), Horizon Forbidden West and its Burning Shores.** Clouds a flying player enters: detailed close up at 1080p without temporal upscaling, superstorms with internal lightning by a "near zero cost" internal light, and in Cubed voxel cloud models with a signed distance field so the march leaps through empty air. The vertical profile per kind is a pair of gradients (top type, bottom type).

**Frostbite (Hillaire, SIGGRAPH 2016 Physically Based Shading course, "Physically based sky, atmosphere and cloud rendering in Frostbite").** Clouds in the same energy-conserving volumetric framework as the atmosphere and fog; multiple scattering by Wrenninge's octaves (Wrenninge, Kulla and Lundqvist, "Oz: the great and volumetric", SIGGRAPH 2013: sum a few scattering terms with extinction and phase anisotropy scaled down each octave); quarter-resolution tracing with temporal reprojection; a cloud shadow map onto the world; clouds captured into the sky's light.

**Unreal Engine's Volumetric Cloud and Sky Atmosphere (Epic documentation; Hillaire, SIGGRAPH 2020 Physically Based Shading course; Hillaire, EGSR 2020, the atmosphere Bevy ports).** A layer given by its bottom altitude and height in kilometres over the Sky Atmosphere's planet; the cloud is a volume material the artist writes (usually a weather map times a height profile times noise); up to two lights (sun and moon); multiple scattering by octaves (contribution, occlusion and eccentricity each 0.5 by default); a cloud shadow map round the camera, shadowing the ground, the atmosphere and the volumetric fog; "cloud ambient occlusion" darkening the sky light under cloud; ground albedo lighting cloud bases from below; tracing scaled by sample counts and a transmittance cut-off for budgets.

**Red Dead Redemption 2 (Bauer, SIGGRAPH 2019 Advances, "Creating the atmospheric world of Red Dead Redemption 2: a complete and integrated solution").** Sky, clouds, fog and the scene's ambient light are one system marched together, so an overcast is a state of the same model; its cloud shadow map is traced a texel in sixteen a frame in Bayer order (as the Bevy branch cites).

**Microsoft Flight Simulator 2020 and 2024 (Asobo).** Volumetric clouds whose weather map is meteoblue's gridded forecast model (some 60 vertical levels), not station reports: the clouds are a picture of a weather model. 2024 adds more cloud variety, cirrus among it (flyawaysimulation.com; forum analyses). The parallel for us is direct: our climate (D315) is the model, and the clouds should be its picture.

**Open-source code we may take (D196).**
- *Bevy, Máté Homolya's `volumetric-clouds-nubis` branch* (github.com/mate-h/bevy, Bevy's licences, MIT or Apache-2.0; an upstream pull request #26253 was opened and closed on 2026-10-20; the feature request is bevyengine/bevy#17895, open). Some 237 commits on Bevy's main, last on 2026-10-22: `VolumetricCloudLayer` (cumulus) and `ThinCloudLayer` (cirrus, after Hogan and Kew 2005) on the atmosphere's entity, moved by an `AtmosphereWind` profile; vertical profiles baked from published large-eddy simulations (DYCOMS-II stratocumulus, BOMEX and RICO trade cumulus, continental cumulus at the ARM Southern Great Plains site) or Nubis's gradients; Perlin-Worley and curl noise; a coarse cloud distance field over 24 km for empty-space skipping (Nubis Cubed); a cloud shadow map, warped, time-sliced one texel per 4x4 tile a frame (RDR2) with temporal filtering; cirrus shadows in its alpha; haze shafts in front of clouds marched per pixel; a path-traced reference with error measured against it. It runs in Bevy's ray-marched atmosphere mode (we use the lookup-texture mode, D212). Its commit messages give costs on an Apple M5 at 1440p: the cloud shadow map 4.35 to 2.40 ms after cutting samples, the distance field under 0.4 ms on the frames that build it, a temporal copy that cost 1 ms removed. Unmerged and unreviewed: an API that may change, on a Bevy newer than ours.
- *Godot*: no volumetric clouds in its core. Sunshine Clouds 2 (Bonkahe, MIT, a compositor effect for Godot 4.5 and later) and Zylann's atmosphere shader (MIT, experimental clouds) are the open implementations; both are smaller and less physical than the Bevy branch.

## 2. Light and shade of a cloud

- **No absorption, only scattering.** Water droplets' single-scattering albedo is close to 1 in visible light; a cloud's grey is light gone elsewhere. Extinction in cumulus is some 0.05 to 0.1 per metre, so a cloud a few hundred metres thick lets almost no sunlight straight through (Hillaire 2016; Nubis).
- **Forward scattering.** The droplets' Mie phase function sends most light within a few degrees of its direction (asymmetry about 0.85), which is the silver lining round a cloud before the sun and the bright fringes of a backlit cumulus. Engines use two Henyey-Greenstein lobes, one forward and one back, blended.
- **Multiple scattering.** Single scattering alone leaves a thick cloud's body near black; real ones are bright inside. Wrenninge's octaves (Frostbite, Unreal, Nubis) or a derived escape term (the Bevy branch) brighten it at almost no cost.
- **Dark bases, bright tops.** A base is lit only by the sky and the ground below: Unreal's ground contribution lights it with the ground's albedo, which is why bases over snow or a lit town are paler and warmer.
- **The sky colour under cloud.** Ground illuminance under cloud follows Kasten and Czeplak (1980) as D418 already does: a full deck leaves about a quarter of clear-sky light, none of it direct. The distribution of light over the sky follows the CIE's standard general skies (CIE S 011/E:2003): fifteen types from overcast (brightest overhead, three times the horizon) to clear, chosen by cloud and the sun's visibility; D490 already uses type 12 for the clear sky.

## 3. Wind and motion

- Clouds move with the wind at their height, which is faster than at the ground: over land the wind grows with height by roughly a seventh power (Hellmann's law), so 1.5 km up it is about twice the surface wind, and turned somewhat to the right in the northern hemisphere (the Ekman spiral). Shear leans towers; cumulus live 10 to 30 minutes, growing and dying, so a scene watched for minutes changes shape, not just position (the Bevy branch animates rising thermals and a life cycle).
- Microsoft Flight Simulator and the Bevy branch both move the weather map with the wind and keep the noise in the cloud's own frame, so shapes drift as a whole.

## 4. Cloud base, kinds and heights

- **Where cloud forms.** A rising parcel saturates at its lifting condensation level, about 125 m for each degree between the air's temperature and its dew point (Espy's rule): humid air makes low bases, dry air high ones. Humidity and temperature, which the climate already has, set the base.
- **Kinds by height (WMO International Cloud Atlas).** Low, below about 2 km: stratus (fog lifted off the ground), stratocumulus, cumulus, and the bases of cumulonimbus, whose tops reach 10 to 16 km and spread into an anvil. Middle, about 2 to 7 km: altocumulus, altostratus, nimbostratus's body. High, 5 to 13 km: cirrus, cirrostratus, cirrocumulus, ice not water. Fog is cloud touching the ground.
- **Kind from the weather.** Fair-weather cumulus in an unstable, sunny afternoon; stratocumulus and stratus under a stable, humid layer; nimbostratus in a warm front's steady rain, with cirrus and cirrostratus thickening ahead of it a day before; cumulonimbus where warm humid air is lifted hard (weather.md's proposed storm cells). A front's cloud sequence is the classic forecast by looking (D78).

## 5. Sun, moon and stars

- **A physically based night sky** (Jensen, Durand, Stark, Premože, Dorsey and Shirley, "A physically-based night sky model", SIGGRAPH 2001): the moon lit by the sun with the Hapke-Lommel-Seeliger law and its opposition surge, earthshine on the dark part, stars from a catalogue by magnitude and colour temperature, airglow and zodiacal light, and the eye's night (scotopic) shift toward blue and loss of colour.
- **The moon's light.** Full moon gives about 0.1 to 0.3 lux; its brightness falls steeply with phase, a quarter moon about a tenth of the full (Krisciunas and Schaefer, "A model of the brightness of moonlight", PASP 103, 1991, which gives the moon's magnitude by phase angle and the sky's brightness by it). The synodic month is 29.53 days; a moon is up by day half its time. A thin crescent shows its dark part by earthshine.
- **Moonlight effects.** A 22° halo round the moon (or sun) in cirrostratus, from hexagonal ice; a small coloured corona in thin water cloud (altocumulus), from diffraction.
- **Stars.** About 6,000 stars to magnitude 6 in the whole sky, some 2,500 above the horizon at once on a dark night; the Milky Way's band only where there is no moon and little haze. Public-domain catalogues (Yale Bright Star Catalogue, Hipparcos) give positions, magnitudes and colours.
- **Games.** Unreal and Frostbite light clouds by the moon as a second directional light; most shipped games draw a fixed moon texture whose phase is a parameter (RDR2 and The Witcher 3 show phases; observed, not documented).

## 6. The sky's optics

- **Rainbow.** A circle 42° round the point opposite the sun, red outside; a fainter secondary at 51° with colours reversed, the sky darker between (Alexander's band); seen only with the sun behind the viewer under about 42° high and rain lit in front (Minnaert, *Light and colour in the outdoors*).
- **Halos and sundogs.** 22° ring and the sundogs either side of a low sun, from ice crystals in cirrus and cirrostratus.
- **Crepuscular rays.** Shafts through gaps in cloud, made by the cloud shadow map read by the haze (Unreal; the Bevy branch marches them per pixel in front of clouds); anticrepuscular rays converging opposite the sun at dusk.
- **Clouds at dusk.** Cloud tops stay sunlit after the ground's sunset and turn pink and orange, then grey from below as the earth's shadow rises (D432's dusk already draws the earth's shadow and the Belt of Venus on the sky; clouds must take it too).
- **Lightning in cloud.** Most flashes light the cloud from inside (storms note); Nubis Evolved lights a storm's inside at almost no cost.

## 7. Seen from far and from high

- Cloud bases are a few kilometres up, so from the ground a cumulus field recedes to a thin band at the horizon on Earth, whose curvature puts a 1.5 km layer's horizon some 140 km away. On a 30 km world (worlds.md) the same layer's horizon is under 10 km: drawn at its true curvature, a cloud layer becomes a near ceiling. Terra's air already draws itself in a frame scaled to the world (D212, D334); clouds need the same treatment, kept at their true heights so peaks pierce them.
- Forbidden West, Microsoft Flight Simulator and Unreal let the camera into and above cloud; it is what costs most (close detail, sorting with the world, fog inside cloud).

## 8. Budget and resolution

| System | Hardware | Cost | How |
|---|---|---|---|
| Horizon Zero Dawn | PS4, 1080p | about 2 ms | quarter resolution, one pixel in 16 a frame, reprojection |
| Horizon Forbidden West | PS5, 1080p | not published | full-detail fly-through "without temporal upscaling" |
| Unreal Volumetric Cloud | scalable | not published | half or quarter resolution traced, reconstructed; sample counts by quality |
| Bevy branch | Apple M5, 1440p | shadow map 2.4 ms after cuts; distance field under 0.4 ms on build frames; view march not given | time-sliced shadow map, distance-field skipping, half-resolution haze with temporal accumulation |

## Limits

- The talks were read through their published abstracts, slides and courses as remembered or summarised; the Horizon Zero Dawn figures are from the talk and the GPU Pro 7 chapter; no frame time was measured here.
- The Bevy branch's quality and cost on our scenes are unknown until measured; its costs above are its author's, on other hardware and resolution.
- No game's moon or rainbow implementation is documented; what they show was observed in footage.

## Sources

- Schneider and Vos, "The real-time volumetric cloudscapes of Horizon Zero Dawn", SIGGRAPH 2015 Advances in Real-Time Rendering (advances.realtimerendering.com/s2015); Schneider, "Real-time volumetric cloudscapes", GPU Pro 7, 2016.
- Schneider, "Nubis: authoring real-time volumetric cloudscapes with the Decima engine", SIGGRAPH 2017 Advances.
- Schneider, "Nubis, evolved", SIGGRAPH 2022 Advances (guerrilla-games.com/read/nubis-evolved); "Nubis, cubed", SIGGRAPH 2023 Advances (guerrilla-games.com/read/nubis-cubed).
- Hillaire, "Physically based sky, atmosphere and cloud rendering in Frostbite", SIGGRAPH 2016 Physically Based Shading course (ea.com/frostbite/news/physically-based-sky-atmosphere-and-cloud-rendering).
- Hillaire, "A scalable and production ready sky and atmosphere rendering technique", EGSR 2020; SIGGRAPH 2020 Physically Based Shading course slides (blog.selfshadow.com/publications/s2020-shading-course).
- Epic Games, "Volumetric cloud component properties" (dev.epicgames.com/documentation/en-us/unreal-engine/volumetric-cloud-component-properties-in-unreal-engine).
- Bauer, "Creating the atmospheric world of Red Dead Redemption 2: a complete and integrated solution", SIGGRAPH 2019 Advances.
- Wrenninge, Kulla and Lundqvist, "Oz: the great and volumetric", SIGGRAPH 2013 Talks.
- Homolya, `volumetric-clouds-nubis` branch, github.com/mate-h/bevy (read 2026-10-26 at 0d852d0a); bevyengine/bevy issue #17895 and pull request #26253.
- Bonkahe, Sunshine Clouds 2 (Godot asset library, MIT); Zylann, Godot atmosphere shader (MIT).
- Microsoft Flight Simulator live weather: flyawaysimulation.com, "MSFS 2020 vs 2024 live weather"; simtuts.com, "MSFS 2024 live weather guide".
- World Meteorological Organization, International Cloud Atlas (cloudatlas.wmo.int).
- CIE S 011/E:2003, Spatial distribution of daylight: CIE standard general sky.
- Kasten and Czeplak, "Solar and terrestrial radiation dependent on the amount and type of cloud", Solar Energy 24, 1980.
- Jensen, Durand, Stark, Premože, Dorsey and Shirley, "A physically-based night sky model", SIGGRAPH 2001.
- Krisciunas and Schaefer, "A model of the brightness of moonlight", PASP 103, 1991.
- Minnaert, *Light and colour in the outdoors*, Springer 1993.
- Hogan and Kew, "A 3D stochastic cloud model for investigating the radiative properties of inhomogeneous cirrus clouds", QJRMS 131, 2005.
