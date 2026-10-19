# How games draw fire: four surveys combined

2026-10-19. The question: twelve rounds of tuning left Terra's fire "cartoonish", "salmon" and "pastel", so how do shipped games make theirs read as real, and what should one fire module for every fire in the game take from them? Four surveys looked at The Witcher 3 and Cyberpunk 2077; ARK, Unreal and Niagara; the AAA talks (Frostbite, Decima, RDR2, Doom, God of War, Monster Hunter World, Back 4 Blood, Game of Thrones); and the survival games (Enshrouded, Sons of the Forest, Valheim). This note is their findings together, with the sources. It builds on [2026-10-16-real-time-fire-rendering.md](2026-10-16-real-time-fire-rendering.md) and [2026-10-18-dragon-breath.md](2026-10-18-dragon-breath.md), whose measured real fires (`refs/fire-real`, MEASURED.md) are the yardstick. The design that follows from it is [docs/fire.md](../docs/fire.md) (D415).

## Answer

None of these games draws fire better than we do with a cleverer simulation. Each draws ordinary flipbook sprites. What makes them look real is a handful of rules. Every one of those rules is one we break:

1. **Heat, cover and smoke are separate channels.**
   - Every pipeline stores the flame's emission and its opacity apart, and colours the heat through one tuned ramp. The fire is drawn alpha-blended or premultiplied, so deep red and soot can darken what is behind them.
   - Ours keys emission and opacity from one luma channel of filmed footage. A dim texel is therefore also a thin one. That one choice is the root of the lime, khaki and pastel rounds.
2. **The tonemapper makes the yellow.**
   - The Witcher 3 and Doom 2016 tonemap with Hable's per-channel filmic curve. It clips red first, then green, so a bright orange goes yellow and then white, as a camera records flame.
   - Ours is KhronosPbrNeutral (`crates/terra_world/src/scene.rs:332`). Past its knee it blends a colour toward a white of the same peak, so bright orange goes peach. That matches "salmon" and "pastel" word for word.
3. **Fire's brightness follows exposure, but only part of the way.**
   - The Witcher scales fire about 15x by day and about 5x at night. Unreal and Unity blend between physical and exposure-compensated. Frostbite is fully physical.
   - Ours is fully compensated (drawn "on the screen's scale"). So a dawn fire is no stronger than a noon fire, and it reads as pasted on.
4. **The light sells it.**
   - Real fire lights cast shadows, flicker, move a few millimetres to a decimetre so their shadows swim, and scatter in the volumetric fog.
   - One flicker drives the light, the glowing embers and the char.
   - Ours casts no shadows, bobs only vertically and is not in the fog.
5. **Smoke is lit, dark and alpha-blended.** It is ideally six-way lightmapped, backlit by its own fire, and casts shadows. Smoke is half the picture in the references. Our breath has none.
6. **A fire is a composite.**
   - A breath is a hot rod, flame and ember sprites along it, soot, and scorch with lingering fire where it hits (Monster Hunter World).
   - A building fire is many small fires on its fuel.
   - None of these is one smooth volume or one card.
7. **Layers run on their own clocks:** fuel glow, flame, smoke, sparks, a flare and the light. Heat haze goes in a distortion buffer.

## 1. The flame: channels, source, ramp and blending

- **Two flipbooks, one for alpha and one for emission** (Star Wars Battlefront II, from a frame capture). One alpha holds the smoke's transparency and the other the emissive fire mask. The two RGB triplets hold six directional lightmaps. One LUT colours both the smoke and the fire. Source: [Schreibt, Battlefront II layered explosion](https://simonschreibt.de/gat/battlefront-ii-layered-explosion/).
- **R alpha, G emissive, B distortion** (Back 4 Blood).
  - About 90% of its fire was EmberGen and the rest bought footage. Every flipbook has motion vectors.
  - The flame is unlit translucent. "Particle color multiplied onto itself" keeps a dimming flame saturated rather than grey.
  - Source: [Cherry, RealtimeVFX](https://realtimevfx.com/t/justin-cherry-realtime-vfx-reels-2021-back-4-blood/18495).
- **R dense smoke, G flame, B thin smoke**, each animated on its own (Monster Hunter World's "RGBcommon" shader). Source: [Famitsu on CEDEC 2018](https://www.famitsu.com/news/201808/25162914.html).
- **One ramp for the whole game** (Skull and Bones).
  - A greyscale heat render from Houdini is sampled into a 128x16 BC7 ramp. Its RGB is the emission and its alpha the premultiply amount, so the hottest part adds light and the cool part occludes, in one draw.
  - Artists retune the ramp once for every flipbook.
  - They tried a physical temperature channel and found the artist ramp simpler and better.
  - Source: [Mederic, RealtimeVFX](https://realtimevfx.com/t/smoke-lighting-and-texture-re-usability-in-skull-bones/5339).
- **Alpha, not additive.** "Additive washes out darker reds and oranges" ([RealtimeVFX thread](https://realtimevfx.com/t/looking-for-some-tips-for-creating-realistic-fire-effects-in-real-time-vfx/28143)).
  - Premultiplied blending (`ONE, ONE_MINUS_SRC_ALPHA`) runs from pure additive at alpha 0 to pure over at 1. A particle can therefore turn from fire into smoke over its life, with no sorting fight between the two.
  - Niagara Fluids colours fire the same way: temperature through a blackbody or curve, the curve's alpha the opacity, and smoke its own density curve ([Epic](https://dev.epicgames.com/documentation/unreal-engine/niagara-fluids-reference-in-unreal-engine)).
- **Footage versus simulation.** Filmed fire on black is added as light. It has no opacity and no separate heat of its own, and it cannot be relit.
  - Games use it far off and additively, and use simulations with separate channels close up.
  - Film does the same: Game of Thrones broke photographed fire apart by luminance and saturation and re-simulated on it ([VFX Voice](https://vfxvoice.com/from-fire-to-ice-season-7s-game-of-thrones-dragons/)). A fire's luminance never sets its own opacity.
- **Several scales, faster than a simulation.** A big slow rolling body sits in the middle, with smaller, faster licks at the edges. Use fewer sprites with more variety. Playback rate follows velocity, and cards lean between world up and their velocity (RealtimeVFX thread above).
- **Motion-vector frame blending**, so few frames stay sharp in slow motion. SideFX Labs recommends vectors at half the colour resolution, unfiltered ([Flipbook Textures ROP](https://www.sidefx.com/docs/houdini/nodes/out/labs--flipbook_textures-1.0.html)). `tools/textures/flames.ts` already does this; it is not the gap.

## 2. Tonemapping and exposure

- **The Witcher 3** tonemaps with Hable's Uncharted 2 curve per channel, with a white point of 11.2. Average luminance is clamped to artist bounds, and two curves are blended at dusk. Source: [Astralis, reverse engineering The Witcher 3](https://astralcode.blogspot.com/2017/09/reverse-engineering-rendering-of.html).
- **Doom 2016** also uses Uncharted 2. Its red cast comes from the grade, not the fire's colours. Source: [Courrèges](https://www.adriancourreges.com/blog/2016/09/09/doom-2016-graphics-study/).
- **Frostbite** lights particles with the same lights, bloom and filmic tonemap as everything else. In Battlefield 3 they had been "too dark or too bright or the wrong color" against the world.
  - Emissive is authored in physical units and pre-exposed. A flame then dominates at night and is weak at noon, which matches MEASURED observation 10.
  - Source: [Lagarde and de Rousiers, Moving Frostbite to PBR §4.10](https://seblagarde.wordpress.com/wp-content/uploads/2015/07/course_notes_moving_frostbite_to_pbr_v32.pdf).
- **Exposure set per time of day** (The Witcher 3).
  - Each emitter names an environment colour group (`ECG_FX_Fire`, `ECG_FX_FireLight`, `ECG_FX_Smoke`). Each weather has a 24-hour curve for each group.
  - The 4.0 data sets `fxFire` near white at 14.8-15.2 from 08:17 to 17:23, 6.3 at midnight and 4.5 at 02:40. `fxFireLight` is orange at night.
  - The flame's colour is in its texture. Only its scale follows the hour.
  - Source: [next-gen env diff](https://github.com/ElementaryLewis/Witcher-3-Next-Gen-Changelog/blob/main/CR2W/env/dlc/bob/data/environment/definitions/sunny/sunny_bob_v7.env.patch).
- **A blend, not a switch.** Unreal's `EyeAdaptationInverse` and its lights' inverse-exposure blend, Unity HDRP's Exposure Weight and Enshrouded's per-draw `exposureCorrectionFactor` all sit between physical (0) and fully compensated (1). Sources: [RealtimeVFX on EyeAdaptationInverse](https://realtimevfx.com/t/breaking-down-eyeadaptation-and-eyeadaptationinverse-shader-nodes-in-unreal-engine/28374) and [Niagara light renderer](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/NiagaraLightRendererProperties).
- **What Bevy 0.20 has** (read from `bevy_render-0.20.0/src/view/mod.rs`):
  - Reinhard and AcesFitted are per-channel. AcesFitted is "dramatic hue shifting", the nearest Bevy has to Hable's yellowing.
  - AgX: "little to no hue shifting".
  - TonyMcMapface: hue preserved but for a Bezold-Brücke shift.
  - BlenderFilmic: "suffers from hue shifting".
  - SomewhatBoringDisplayTransform and KhronosPbrNeutral.
  - **Bevy has no Hable curve.** Godot's Filmic tonemapper is Hable's, under MIT, so a Hable curve would be ported from Godot (D196) through our tonemapping shader patch (`shader_patch`, D302).

## 3. Firelight

| Game | Flicker | Jitter | Shadows | Fog | Source |
|---|---|---|---|---|---|
| The Witcher 3 | strength 0.3-0.4 over 0.3-0.4 s | 0.005 (likely metres) | the torch yes, shadow fade from 3-8 m | n/a | [REDkit, torch diff](https://github.com/ElementaryLewis/Witcher-3-Next-Gen-Changelog/blob/main/CR2W/w2ent/items/usable/torch/torch.w2ent.patch) |
| Valheim | ±10%, product of three sines (1, 0.564, 0.758) at 10 rad/s | 0.1 m on three axes | within 20 m, light to 40 m, eased not cut | n/a | `LightFlicker.cs`, `LightLod.cs` (decompiled) |
| Sons of the Forest | one master flicker linked to the light and to the glowing material's emission | n/a | yes (HDRP) | yes | `LightFlickerBase.cs` (decompiled) |
| Enshrouded | n/a | n/a | "Point Light Shadows" for torches and candles | point lights and VFX-spawned emissive fog volumes feed the froxels; GI takes up to four point-light bounces | [Krause, GPC 2025](https://static.graphicsprogrammingconference.com/public/2025/talks/the-fog-is-lifting-volumetric-rendering-enshrouded/Krause-the-fog-is-lifting-volumetric-rendering-enshrouded.pdf); [Kolesik, GPC 2024](https://www.youtube.com/watch?v=57F1ezwH7Mk) |
| RDR2 | n/a | n/a | sampled from the light cluster with their shadow maps | particle fog volumes for fires and smoke-filled rooms | [Bauer, SIGGRAPH 2019](https://advances.realtimerendering.com/s2019/slides_public_release.pptx) |
| Frostbite | n/a | n/a | n/a | particles voxelised into the froxel V-buffer, which carries emissive | [Hillaire, SIGGRAPH 2015](https://advances.realtimerendering.com/s2015/Frostbite%20PB%20and%20unified%20volumetrics.pptx) |

- **Why the jitter matters:** moving the light moves every shadow it casts, which is the strongest cue for a live flame.
- **Why the link matters:** when the light, the logs and the embers pulse apart, the fire reads as separate effects.
- **ARK's lesson:** players switched the torch light off for frame rate, and the cosy look went with it. Its fire reads as fire through its light, not its flame.
- **In Bevy 0.20:**
  - The volumetric fog scatters point lights that carry `VolumetricLight` and samples their shadow maps (`bevy_pbr-0.20.0/src/volumetric_fog/volumetric_fog.wesl`).
  - `FogVolume` has no emission term.
  - `PointLight` has `radius` (soft shadows from the flame's size) and `PointLightTexture` (a cookie), but no shadow strength to fade.

## 4. Smoke

- **Six-way lightmaps** (Battlefront II, Unity, Unreal): six renders under white directional light, packed into two RGBA maps.
  - Unity's layout: map 0 is right, top and back, with transparency in its alpha; map 1 is left, bottom and front, with emission in its alpha.
  - At runtime the light's direction weights them, which gives backlit rims and inner self-shadow on a flat sprite.
  - Baked in Houdini (Labs MDC), EmberGen ("six point") or Blender (Unity's `unity_6way.py`).
  - Source: [Unity, 6-way lighting](https://unity.com/blog/engine-platform/realistic-smoke-with-6-way-lighting-in-vfx-graph).
- **Decoupled particle lighting** (Doom): each particle is lit as if in world space by the clustered lights and shadows, into a lighting atlas, then drawn as lighting times albedo. Source: [idTech 666, SIGGRAPH 2016](https://www.advances.realtimerendering.com/s2016/Siggraph2016_idTech6.pdf).
- **Backlight**: Cyberpunk 2077 has a backlighting factor on its particles, so smoke lit from behind by its own fire glows at its base (SIGGRAPH 2021). Houdini Labs exports an "emissive scatter" pass for the glow on the smoke's underside.
- **Shadows**: Frostbite voxelises particles into an extinction volume, so smoke shadows itself, the ground and the fog. Valheim's smoke is lit and casts into the sun's shadow.
- **Stable lighting**: Monster Hunter World's "Temporal Shadow" eases a particle's lighting over about 2 s, so dappled sun does not flicker it.

## 5. Composite fires

- **Breath** (Monster Hunter World, Zeno Jiva).
  - A TubeLight rod with a core, "so the tube doesn't look hollow", surrounded by flame and ember particles.
  - A ray cast sets the range and finds the ground, where it spawns blasts and scorch marks. Lingering fire is placed only where the breath hits land, not water.
  - Elden Ring's breath reads the same in play: overlapping flipbook flames in a cone, dark smoke breaking off its edges, a strong light on the ground, burning patches left behind.
  - Unreal flame bursts are a ribbon jet with panning noise and refraction, plus EmberGen burst flipbooks, smoke and sparks ([80.lv, Anchikov](https://80.lv/articles/creating-a-flame-burst-effect-using-unreal-engine-5)).
- **Explosion** (Back 4 Blood's molotov): about three explosion textures and two or three fire textures, embers, and a decal whose emissive radiates from the impact. Lingering flames are soft L-shaped camera-facing meshes scrolling up off the ground.
- **Structure fire** (Valheim, Sons of the Forest).
  - Ballistic cinders spawn fire sites where they land, each with a 1 m radius and a generation count.
  - Burnables swap fresh, alight and burnt materials.
  - The Witcher's burning villages are many small effects. MEASURED's building fires leave the windows and climb a storey above the roof.
- **Volumes** (Unreal Heterogeneous Volumes, Niagara Fluids) are experimental and for cinematics in Epic's own docs. In games they are baked to flipbooks. The shipped raymarchers (Horizon's Nubis clouds, CS2's smoke) draw large, slow media, not fast flame ([Epic](https://dev.epicgames.com/documentation/en-us/unreal-engine/heterogeneous-volumes-in-unreal-engine)).

## 6. Layers, haze and LOD

- **A timeline of tracks** (The Witcher 3). An effect's emissive fuel, flames, smoke, velocity-stretched sparks, flare sprite and light each start, stop and hide on their own. LOD is by birth rate and auto-hide at 20-50 m, with prewarm and a near-camera fade.
- **Heat haze in a distortion buffer.**
  - Doom writes RG offsets (B blur or occlusion) into a quarter-resolution buffer and applies it in the tonemap pass.
  - Cyberpunk flags the tiles touched and resolves only those: under 1 ms at 2160p on Xbox One X.
  - Keep it subtle: one noise-panned quad over a group of flames, never many rising refracting sprites.
- **Wind** (God of War): GPU particles sample the fluid wind volume, scaled by a per-particle influence. Artists wanted separate ambient and gust influences, because level wind blew effects off screen ([Feeley, SIGGRAPH 2019](https://advances.realtimerendering.com/s2019/Feeley-Advances_in_RTR-Interactive_Wind_in_God_of_War-web.pptx)).
- **Budgets**: Monster Hunter World fades alpha by angle, culls at 30 m and spreads its collision checks over frames. A UE5 campfire breakdown found its point lights the costliest part.

## 7. Baking tools

The developer has since ruled out paid tools (docs/fire.md, The baker). The table stands as what each would give.

| Tool | Cost (2026, check the store) | Exports | Drivable by an agent | Notes |
|---|---|---|---|---|
| Houdini Indie (Pyro, SideFX Labs) | about $299 a year, or $449 for two; for under $100K revenue; three seats a studio ([Shade](https://shade.inc/blog/sidefx-houdini-pricing-2026-fx-core-indie-license-costs), [SideFX](https://www.sidefx.com/buy/)) | Labs Flipbook Textures: final colour, separate alphas, non-emissive colour, motion vectors (R8G8 or R16G16), MDC six-way, normals, depth | yes, headless through `hython` | The AAA standard. Skull and Bones' heat render came from it. |
| EmberGen (JangaFX) | indie (under $1M) reported at $300 the first year, then $150-180 a year maintenance ([Digital Production](https://digitalproduction.com/2026/03/20/jangafx-makes-its-layoff-assistance-programme-permanent/), [JangaFX](https://jangafx.com/software/pricing)) | emission, alpha, motion vectors, normals, depth, "six point" lighting, VDB | no: no documented command line, and obfuscated project files ([forum](https://forums.jangafx.com/t/ember-project-file-format/841)) | Real-time and fast to iterate by hand. About 90% of Back 4 Blood's fire. |
| Blender Mantaflow plus Unity's `unity_6way.py` | free | anything we render; six-way through the add-on | yes, headless (`tools/blender`) | Slow, coarse sims. D121 allows Python inside Blender. |
| `tools/textures/flames.ts` | free | keyed footage: premultiplied red and green, a derived soot, cover from luma | yes | No heat or cover of its own, so it is the root cause above. Fine for far, additive fire. |

## 8. What Terra should port, ranked

1. **Change the tonemapper, chosen by measurement.**
   - Render the fire sheet through AcesFitted, AgX, TonyMcMapface, BlenderFilmic and a Hable port. Keep the one that brings back yellow cores without hurting the scenes' own look references.
   - Why first: no fire tuning can undo a curve that turns orange peach. It is one line to try.
2. **Split heat from cover; one ramp with a premultiply column; draw premultiplied.**
   - Why: it removes the lime, khaki and pastel failures at their root. It needs flipbooks with real channels, which means a baker (item 7).
3. **Light rules.** Shadowed near lights with a soft radius, three-axis jitter, `VolumetricLight` in the fog, and one flicker linked to the embers' and char's glow. Why: it is cheap, the light exists already, and it matters most at dusk.
4. **Partial exposure compensation.** One weight between physical and screen scale, so a dawn fire dominates and a noon one does not. Why: a one-number experiment, measurable against observation 10.
5. **Six-way-lit, backlit, dark smoke that shadows.** Why: half the picture in every reference.
6. **Composite breath and explosion** (Monster Hunter World's way), and **structure fire as sites on fuel** with fresh, alight and charred materials. Why: twelve rounds on one analytic volume have not produced turbulence. The sprites carry it.
7. **A baker with real channels.**
   - Houdini Indie would be the industry's choice, since an agent can drive it headless. But the developer rules out paying ("i do want to be able to beat it without having to pay for fire library").
   - So: Blender's Mantaflow, rendered pass by pass in Cycles. It is free, headless and already in our tools. Our own offline sim is the fallback.
   - Why: the channel split needs real heat and density renders, and six-way smoke needs a volume renderer.
8. **Heat haze buffer, sparks, flare, wind influence, LOD by birth rate and distance.**

Not worth porting: a grid fire simulation at run time (CDPR showed FlameWorks in 2014 and shipped sprites); raymarched sparse-volume fire for gameplay; Sons of the Forest's camera-centred fire volume (its purpose is unknown).

## Limits

- CD Projekt, Endnight, Iron Gate, Wildcard and FromSoftware have given no fire talks. Their sections come from shipped data, decompiled scripts, mods, frame captures and observation, and are marked so in the four surveys.
- Particle counts and millisecond budgets of shipped fires are mostly unpublished.
- The tool prices are from third-party reports in 2026. Check the store before buying.
- Nothing here was measured in Terra. The tonemapper and exposure claims are predictions to test (docs/fire.md, stage 1).
