# Real-time fire rendering: colour, shape, smoke, haze and light

2026-10-16. Question: what makes real-time fire read as real, in shipped games and in physics, and what numbers should our procedural flame, smoke and light cards (`assets/shaders/fire.wesl`, `fire_smoke.wesl`, `crates/terra_world/src/fire.rs`) use? It follows [2026-10-15-dragons-and-fire.md](2026-10-15-dragons-and-fire.md), which covers spread and caps.

Tags: **[P]** primary (paper, engine source or docs), **[S]** secondary (forums, practitioners), **[I]** worked out here (the colour tables come from integrating Planck's law against the CIE 1931 2° observer, Wyman, Sloan and Shirley's 2013 fit, and running the result through Khronos PBR Neutral's published formula). Nothing was measured in our game.

## Answer

- **Fire's real colour is a deep red-orange, and it looks yellow only because it overexposes.** Soot in a wood flame glows at about 1100 to 1500 K, whose chromaticity in linear sRGB is (1, 0.07 to 0.18, 0). The yellow and white people see in footage come from the camera's or the eye's per-channel shoulder clipping red first.
- **Our tonemapper preserves hue, so it cannot make that shift, and it turns bright orange to salmon.** Khronos PBR Neutral blends towards white along a straight line in chromaticity. Colour scientists call this "salmon fire". Run through it at our daylight exposure, a 1400 K flame comes out sRGB (252, 150, 120). The fire shader has to make the hue shift itself.
- **By day a real fire is dim against the sun.** A flame's body is about 500 to 2000 nits and its core about 5000. Diffuse white in 30 klx of sun is 9550 nits. Our 3600 nits peak is plausible. Our firelight, though, is about 50 times brighter than the flames that make it.
- **Shape comes from where fuel is and from intermittency.** The flame is solid to about half its mean height, then breaks into detached flamelets up to about 1.5 times it, all puffing at 1.5/√D Hz. Flamelets stay hot as they rise and grow sparser. They do not cool smoothly to dark red. Their edges are crisp.
- **Shipped games mostly use flipbooks** (simulated in EmberGen or Houdini, with motion vectors). Procedural noise erosion is the stylised and mobile route. With no textures, erosion can be pushed towards realism by subtracting a scrolling, domain-warped noise from a gradient with a tight threshold.

## 1. Colour and luminance

- **[P] Temperatures.** McCaffrey's buoyant flames are a continuous region at about 900 °C (1170 K), an intermittent region and then the plume. Visible tips sit at about 320 to 550 °C time-averaged (NBSIR 79-1910, https://nvlpubs.nist.gov/nistpubs/Legacy/IR/nbsir79-1910.pdf; Babrauskas, https://doctorfire.com/temperatures-in-flames-and-fires/). Large fires and room fires after flashover run 1100 to 1250 °C. A tip's average is low because the flame is there only part of the time. The flamelets themselves are near 1000 to 1200 K.
- **[I] What soot emits.** Soot emits as a blackbody times λ^-1.39 (Hottel and Broughton). The table normalises colour to its largest channel, and nits assume an emissivity of 0.8, which is a 2 m thick flame at κ ≈ 0.8 /m.

| T (K) | heat | linear sRGB | nits | × sunlit white | at EV 10.8, our shoulder (below) → sRGB |
|---|---|---|---|---|---|
| 1000 | 0.0 | 1, 0.042, 0 | 2 | 0.0002 | 12, 0, 0 |
| 1100 | 0.17 | 1, 0.068, 0 | 16 | 0.002 | 47, 5, 0 |
| 1200 | 0.33 | 1, 0.094, 0 | 99 | 0.01 | 108, 29, 0 |
| 1300 | 0.5 | 1, 0.122, 0 | 464 | 0.05 | 188, 76, 4 |
| 1400 | 0.67 | 1, 0.151, 0 | 1760 | 0.18 | 237, 144, 39 |
| 1500 | 0.83 | 1, 0.180, 0 | 5620 | 0.59 | 240, 215, 125 |
| 1600 | 1.0 | 1, 0.209, 0 | 15600 | 1.6 | 240, 239, 215 |

- **[I] A check against a candle.** A 1800 K soot flame at emissivity 0.1 is about 12,000 nits. That matches the measured candle flame of 10,000 to 20,000 cd/m² (https://amper.ped.muni.cz/light/luminance/candle/).
- **[P] Why ours goes salmon.** Bevy's own notes say PBR Neutral "suffers from significant Abney shifting". Above a peak channel of 0.76 it scales hue-preservingly, then mixes towards white by `1 − 1/(0.15·(peak − newPeak) + 1)` (https://github.com/KhronosGroup/ToneMapping/tree/main/PBR_Neutral). Per-channel curves (ACES, film) shift bright red towards orange and yellow. Bevy's docs say of ACES: "Bright greens and reds turn orange." Graders call the straight-line path to white "salmon fire" and fix it by blending in some per-channel hue shift (https://github.com/aurelienpierreeng/ansel/discussions/873, https://discuss.pixls.us/t/agx-after-tone-mapping/54892/3).
- **[I] What ours does now.** Our ramp's orange (1, 0.36, 0.05) at an exposed peak of 1.7, which is 3600 nits at EV 10.8, gives (249, 167, 95), peach. At EV 9 (dusk) it gives about (254, 205, 175), pink. Our yellow and white stops are 3000 to 4500 K chromaticities, which no wood flame has.
- **[P] Engines let emissive ignore exposure.** HDRP's Emission node has an *Exposure Weight* from 0 to 1, so emissive can be authored relative to the current exposure (https://docs.unity3d.com/Packages/com.unity.shadergraph@14.0/manual/Emission-Node.html). Practitioners avoid pure additive blending, push emissive and lower alpha, and keep only the hottest core white (https://realtimevfx.com/t/looking-for-some-tips-for-creating-realistic-fire-effects-in-real-time-vfx/28143, https://www.anisopteragames.com/hdr-effects/).
- **[I] The recommended colour model.**
  - **Temperature from heat:** T = 1000 K + 600 K × heat. Colour is the table's soot chromaticity, interpolated, plus 0.02·R in blue. Luminance is the table's nits (interpolate log nits) times ε = 1 − exp(−0.8·thickness).
  - **The shoulder, in the fire shader:** take x = colour × nits × view.exposure, apply `s = 1 − exp(−x)` per channel, then add white-hot lift with `s += (1 − s)·w`, where `w = 0.7·saturate((Y·exposure − 1.5)/6)`. Emit `s / view.exposure × alpha`.
  - **What it gives:** below PBR Neutral's knee, so its desaturation stays small. By day, orange flames with a yellow core. At night (EV 6), a 1300 K body reads (240, 235, 202) with orange edges, as photographs at night do.
  - **Targets for a daylight building fire:** tips 0.002 to 0.01 of sunlit white, body 0.05 to 0.2, core up to 0.6.

## 2. Shape and motion

- **[S] Shipped practice.**
  - Back 4 Blood made about 90% of its fire flipbooks in EmberGen, each with a motion-vector texture so few frames blend smoothly (https://realtimevfx.com/t/justin-cherry-realtime-vfx-reels-2021-back-4-blood/18495). Motion vectors at half the sheet's resolution are enough.
  - The layout is a big rolling flipbook in the middle and smaller, faster ones at the edges, each card pivoted at its foot.
  - Star Wars Battlefront II's fire splits a sheet into smoke alpha and emissive fire.
  - RiME (Trümpler, Unreal Fest 2018) and League of Legends draw fire from a shape mask distorted by two scrolling noises. LoL's artists stress subtracting the erosion, not multiplying it (https://www.unrealengine.com/events/unreal-fest-europe-2018/stylized-vfx-in-rime, https://realtimevfx.com/t/shannon-berke-lol-fx-knowledge-share/1133/168). That is the stylised route.
  - No fire talk was found for RDR2, Horizon, Ghost of Tsushima, Battlefield or Far Cry 5.
- **[P] Flame height.** Heskestad's `L = 0.235·Q^0.4 − 1.02·D`, and the flame puffs at `f ≈ 1.5/√D` Hz (Cetegen and Ahmed 1993, https://arxiv.org/pdf/1803.10400). At 500 kW/m² **[I]**:

| fuel width D | Q | mean height L | L/D | puffing | gas speed, intermittent region (1.9·Q^0.2) |
|---|---|---|---|---|---|
| 0.5 m | 98 kW | 0.96 m | 1.9 | 2.1 Hz | 4.8 m/s |
| 1 m | 390 kW | 1.5 m | 1.5 | 1.5 Hz | 6.3 m/s |
| 2 m | 1.6 MW | 2.4 m | 1.2 | 1.1 Hz | 8.3 m/s |
| 6 m (a roof) | 14 MW | 4.6 m | 0.8 | 0.6 Hz | 12.8 m/s |

- **[P] Intermittency.** McCaffrey's continuous region ends near 0.08·Q^0.4 and his intermittent region near 0.2·Q^0.4. For a 1 m fire that is about 0.55·L and 1.4·L.
- **[I] A procedural flame card, from the above.**
  - **Size and pivot:** card height 1.5·L, width ≈ D, pivoted at the foot. Coverage is about 1 below 0.5·L, 50% at L and 0 by 1.4·L.
  - **Breathing:** the whole blaze's height swings ±20 to 25% at 1.5/√D Hz, phased per blaze, so it breathes and sheds puffs.
  - **Noise:** gradient or simplex noise, not value noise, whose lattice reads as blobs. Use 4 or 5 octaves of turbulence (Σ|n|, Perlin's 1985 fire) on near cards and 2 far. The largest features are about D/2 across, which is 2 cycles a card. Vertical frequency is 0.6 times horizontal, stretching further towards the tip.
  - **Domain warp:** a third noise at half frequency moves u by 0.15 to 0.25 of the card's width, more with height.
  - **Scroll:** 2 to 3 m/s upward in world units (puff speed ≈ L·f, 0.2 to 0.4 of the gas speed). A second octave at 1.6 times the speed and 2.3 times the frequency.
  - **Erosion:** `margin = n + g(y)·mask(x) − θ(y)`, alpha `= saturate(margin/0.1)`, with θ rising from 0.35 at the foot to 0.75 at the top. A softness of 0.08 to 0.12 keeps edges crisp. Ours is `smoothstep(0.25, 1.05, edge)`, which reads as glowing blobs.
  - **Temperature from depth inside the flame:** `T = 1100 + 400·saturate(margin/0.4)`, minus at most 150 K up the height. Thick tongues are then yellow at heart and thin wisps orange-red, and the top breaks into detached flamelets, not a red fade. For a big fire, dark soot pockets where the noise is low.
- **[I] Card budget.** Gather burning cells into fronts. Each 2 to 4 m of front gets one large card (D by 1.5·L) and 2 to 4 smaller, faster flamelet cards at its edges and top. Aim for no more than 4 to 6 cards over a pixel. Ours puts 9 cards in every 1 m blaze, so a burning wall is many small alike tongues.

## 3. Soft particles and overdraw

- **[P] Depth fade** (Lorach, NVIDIA 2007): `fade = saturate((scene_depth − card_depth)/d)` in linear depth, optionally with a contrast curve (https://developer.download.nvidia.com/whitepapers/2007/SDK10/SoftParticles_hi.pdf, https://vfxdoc.readthedocs.io/en/latest/shaders/fading/).
  - **[I] Our fade distances:** d of 0.25 to 0.4 m for flames (they hug walls, so a 1 m fade would erase them), 1 to 2 m for smoke, and a near-camera fade from 0.5 to 2 m.
  - **What Bevy has:** our camera already has a depth prepass, which TAA requires, and Bevy 0.20 has `bevy_pbr::prepass::utils::prepass_depth`, which `mist.wesl` and `lake.wesl` already use.
- **[P] Overdraw.** Draw smoke into a half- or quarter-resolution buffer with depth-aware upsampling (Cantlay, GPU Gems 3 ch. 23, https://developer.nvidia.com/gpugems/gpugems3/part-iv-image-effects/chapter-23-high-speed-screen-particles). Fewer, larger cards fitted to the shape.
- **[S] TAA smears cards that write no motion vectors.** Unreal gives particle materials a "Responsive AA" flag for this. Ours discard in the prepass, so the cards take the background's motion.

## 4. Smoke from a burning building

- **[P] Optics.** Smoke from flaming pine has a single-scattering albedo of 0.32 and a mass extinction of 8.9 m²/g. Smouldering smoke scatters far more (Chen et al. 2006; Singh et al. 2016 found 0.46 to 0.74 across woods, https://acp.copernicus.org/articles/16/13491/2016/).
- **[S] Firefighters on its colour.** Colour follows ventilation and age more than fuel. Fast, dense black smoke comes from near the seat of the fire. Smoke pales as it is filtered and diluted, and natural materials run tan to brown (https://www.fireengineering.com/firefighter-training/the-art-of-reading-smoke-understanding-the-color/).
- **[I] For us.**
  - **Colour:** linear albedo 0.03 to 0.05 (near-black, a little brown) for 0 to 2·L above the flames, rising to 0.15 to 0.3 grey-tan, about (0.25, 0.23, 0.20), at 5 to 10·L. Ours is 0.07 → 0.24.
  - **Opacity:** about 0.8 to 1.0 in the dense column, where black smoke hides what is behind it, falling to 0.2 to 0.4 as it spreads. Ours is at most 0.55.
  - **Where it starts:** at 0.8 to 1.2·L, partly in front of the flamelets. Dark smoke wrapping flames is the strongest cue a building is burning.
  - **Speed and spread:** rising at 3 to 8 m/s near a building fire, falling as z^-1/3 (McCaffrey's plume), widening at a half-angle of about 6 to 8° (plume radius ≈ 0.12·z).
- **[P] Lit, and self-shadowed cheaply.** Unity's six-way lighting bakes light from six directions into two textures (https://unity.com/blog/engine-platform/realistic-smoke-with-6-way-lighting-in-vfx-graph). Frostbite voxelises particles into its froxel volume (Hillaire, SIGGRAPH 2015). **[I] Ours** uses N = to_eye, so a puff lights like a flat disc.
  - **Normals:** a hemisphere normal from uv, `N = normalize(right·x + up·y + to_eye·sqrt(1 − r²))`.
  - **Self-shadow:** darken by density on the side away from the sun, by `mix(1, 0.4, density)`.
  - **Under-glow:** keep the orange glow from below. Run it through the flames' shoulder, as it is fire's light.

## 5. Heat haze

- **[S] How games do it.** Sample the opaque scene's colour at screen UV + offset, where the offset is signed scrolling noise times a mask (Unity's particle Distortion, Godot's examples, https://docs.unity.cn/Manual/shader-StandardParticleShaders.html). Practice keeps it under 0.01 of the UV.
- **[I] For us.** An offset of 0.002 to 0.005 of the screen width (4 to 10 px at 1080p), scaled by 1/distance and faded out past about 40 m. It fills a region from 0.5·L to 2·L above the flames, scrolling up at 1 to 2 m/s, at a noise period of 0.2 to 0.4 m. `ScreenSpaceTransmission` is already on our camera, and `view_transmission_texture` is the copy to sample.

## 6. Firelight

- **[S] Shipped flicker.** Quake and every Valve engine step light styles at 10 Hz, from a = 0 through m = 100% to z = 200%. "Flicker A" (`mmnmmommommnonmmonqnmmo`) wavers 100 to 133% (https://developer.valvesoftware.com/wiki/Lightstyle). Practice is smooth noise rather than random steps, a seed for each light, and moving the light as well as changing its intensity.
- **[I] Intensity.** A Lambertian flame emits Φ ≈ π × mean luminance × visible area. A 1 m blaze (about 3 m² at about 1500 nits) gives about 15,000 lm. A candle's 12.6 lm from 80 W, scaled to 1400 K soot, gives 4,000 to 20,000 lm for 400 kW. Ours, `LUMENS_A_METRE = 700_000`, is about 50 times the first estimate and 35 to 170 times the second. Physically, by day firelight barely shows past a few metres, and it dominates at dusk.
- **[I] Flicker.** Use 2 or 3 octaves of smooth noise centred on 1.5/√D Hz, extending to about 4 Hz. Amplitude is ±15 to 25% for one blaze and ±5 to 10% for a cluster of many, shrinking as 1/√(number of blazes). Bob the light 0.1 to 0.3 m so the shadows move. Ours is fixed sines at 1.4 and 3.7 Hz, ±22%, at any size.
- **[I] Colour.** 1800 to 2000 K: linear (1, 0.22 to 0.27, about 0.005), sRGB about (255, 130 to 140, 0 to 18). Ours, `srgb(1, 0.55, 0.22)`, is right.
- **[I] How many lights.** One per cluster, as now, with 1 or 2 shadowed nearest the camera.

## 7. Reference colours

- No source publishes colours sampled from footage of burning timber buildings. The last column of §1's table is what a camera's per-channel response gives for each temperature, our stand-in.
- A reference sheet of daylight and night building-fire frames, measured with `tools/sandbox/look.ts`, would check it before tuning. That follows the environment skill's practice.

## Limits

- GDC Vault talks were not read. No fire breakdown was found for RDR2, Horizon, Ghost of Tsushima, Battlefield or Far Cry 5.
- The soot emissivity, the flames' absorption coefficient (κ) and the lumens per watt are textbook values and scaling, not measurements of a wood fire.
- The shoulder and noise numbers are starting points to tune against a reference.
- Nothing was run in the game.
