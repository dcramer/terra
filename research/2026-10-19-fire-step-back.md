# Fire step back: a professional flipbook in our engine

2026-10-19. The question: after about 30 rounds the developer says "wow fire is absolutely terrible", and earlier "pretty cheap, incomparable to what id expect" against The Witcher 3, ARK, Sons of the Forest and Enshrouded. Is the gap our bakes and emitters, or our renderer? Following AGENTS.md's second-attempt rule, we put a known-good professional flipbook through our own card path at the campfire, measured both, and read how shipped games and professional artists build a campfire. It builds on [How games draw fire](2026-10-19-how-games-draw-fire.md) and does not repeat it.

## Answer

**The renderer and the scene around the flame are the main gap, not the flipbook.**

Swapping our Mantaflow campfire set for a Houdini-made one from Unity Labs moved the campfire's measured colour bands by 2 points or less at dusk and night. Both fail in the same ways. The pro book's own colours are 34-51% yellow with a blue/red ratio (B/R) of 0.03-0.10. After our path the same book is 2-5% yellow with B/R 0.50-0.64.

At night the brightest thing in our frame is the ring of lit stones (sRGB 252,233,222, nearly white), not the flame (235,170,80). In the photographs the flame core is the brightest thing (253,224,133). A shipped campfire keeps that hierarchy, and we invert it.

Our bake is a real but second gap. Its flames are thin glow (median cover 0.16, never over 0.9), where the pro book's are 0.39, with 2.7% of texels over 0.9. The campfire is also missing most of a AAA campfire's layers, the coal bed and glowing logs above all.

Two further findings change how we work:
- **Our own gate rejects the professional flame.** The campfire class fails the pro book's own colours on yellow (34-51% against a maximum of 21) and red (1% against a minimum of 17). So 30 rounds tuned toward a target that pulls flames red.
- **The flipbook is not the lever; brightness is.**
  - We drew the flame two stops brighter (`HEARTH_BRIGHTNESS` 8 to 32; a throwaway edit, reverted).
  - The pro book's brightest 2% at night went from 242,181,107 to 252,215,147, near the photographs' 253,224,133. Ours went from 235,170,80 to 252,206,103.
  - The stones stayed at 252,234,219 either way.
  - The bands moved at most 4 points, because the crop is mostly stones and lit ground.

## The known-good reference

- **Source:** Unity Labs' free VFX image sequences, made in Houdini by Unity Labs Paris and released CC0 (https://unity.com/blog/engine-platform/free-vfx-image-sequences-flipbooks). Its links still resolve. We used `SmallFlame01-temperature`: 64 frames, 16x4 frames of 128x256 on a 2048x1024 sheet, colour and alpha (TGA) plus a separate monochrome temperature EXR (0 to 1.51). It has no motion vectors.
- **Why this one:** JangaFX's promised "CC0 Flipbook Texture Pack" is listed as delayed on its roadmap. Its free CC0 VDBs (Fire Plume, 250 frames; Simple Fire, 101 frames) are volumes, not flipbooks; they are the next test (plan step 4). Unreal's Starter Content may not ship outside Unreal, and the Unity Companion License only covers Unity projects. Neither is usable here.
- **Where it lives:** `~/.cache/terra-review/fire/stepback/pro/`, never committed. CC0 would allow committing it, but nothing needs it in the repository.
- **Conversion:** a scratch script (uncommitted) made our flame map from it:
  - R heat is the temperature EXR, its 99th percentile scaled to 0.97, as `pack.ts` does;
  - G cover is the TGA's alpha;
  - B soot is 0;
  - A distortion is cover times height;
  - each 128x256 frame is centred in a 256 square, 8x8;
  - motion is neutral, with `flow_gain` 2 so the shader crossfades.
- **The run:** the campfire alone, the sheet's camera, noon, dusk and night, ours then theirs in one `cargo xtask gpu still` job (2 min 18 s), measured as the sheet measures (`stage9/cf.ts`, the crop's top two thirds as flame and bottom third as stones).

**Stills (local, not committed):**
- `~/.cache/terra-review/fire/stepback/ours-vs-pro.jpg`: real photographs, then ours and the pro book at noon, dusk and night, then the flame two stops brighter (ours at dusk and night, the pro book at night).
- `~/.cache/terra-review/fire/stepback/{ours-b32,pro-b32}/`: the two-stops-brighter run (49 s).
- `~/.cache/terra-review/fire/stepback/ours-vs-pro-crop.jpg`: the same crops without the photographs.
- `~/.cache/terra-review/fire/stepback/pro/look/all.png`: the pro sheets.
- `~/.cache/terra-review/fire/stepback/{ours,pro-render}/<hour>/`: the frames and measurements.

## The bisect, measured

| | ours | pro book, our path | pro book, own colours | photographs |
|---|---|---|---|---|
| flame band at night: yellow / orange / red % | 1 / 55 / 44 | 2 / 53 / 44 | 34-51 / 46-63 / 1 | class median 9 / 26 / 55 |
| flame band at night: B/R | 0.60 | 0.64 | 0.03-0.10 | 0.03-0.16 |
| brightest 2% of the flame at night, sRGB | 235,170,80 | 242,181,107 | | 253,224,133 |
| brightest 2% of the stones at night, sRGB | 252,233,222 | 252,233,220 | | soot-dark |
| whole crop at dusk and night, bands out | 1 and 1 | 1 and 1 | 4-5 of 15 | |
| cover of lit texels: median, 90th percentile, share over 0.9 | 0.16, 0.27, 0% | 0.39, 0.77, 2.7% | | |
| heat of lit texels: median, 90th percentile | 0.43, 0.80 | 0.14, 0.59 | | |

What each difference is and where it comes from:

1. **Brightness hierarchy (renderer, the largest).** The lit stones out-shine the flame. The campfire round measured them 3-5 stops past Neutral's white from inverse-square light alone, with the flame core only about 2 stops above them; real fires are about 4 stops above what they light at 1 m. Halving or quartering the light keeps them pink (HANDOFF, stage9). A shipped game keys the exposure to the flame and leaves the surroundings darker: a flame about 10^5 cd/m² against a stone of albedo 0.3 at 0.9 m lit to about 50,000 lux, about 5,000 cd/m², is 20 times or 4.3 stops.

   The lever run shows that it takes two changes: four times the flame brings the core to the photographs' colour, but the stones stay white. The flame's level and the near light must move together.
2. **No blue ever in the ramp (renderer).** `ramp.ts` gives B = 0 at every heat; its hottest is 1.16, 1.05, 0, pure yellow. So the only way to white is the tonemapper mixing toward white, which is the salmon (`film.rs` exists to patch that). A photograph's core is 253,224,133: B is about half of R in sRGB, from the sensor's crosstalk and the clip. The pro book's own colours have it; our ramp throws it away.
3. **The flame is mostly additive over a blue-grey world (renderer).** Hot texels have ramp alpha 0 to 0.1, so the flame adds to what is behind it. At night the flame band's B/R is 0.60 though the ramp has no blue. The blue is the lit ground and sky through the flame, plus haze and the grade's `post_saturation` floor. The pro book through the same path reads 0.64, so this is the path, not the book. NVIDIA's Vulcan demo dropped additive for exactly this, because it "saturates color".
4. **Thin cover (our bake).** Our campfire set's cover tops out at 0.27 for 90% of lit texels, so a tongue is a glow, not a body. The tongue and torch sets reach 0.82-0.87.
5. **A cut-edged card layout (emitters).** The pro book's tongues show straight vertical edges in our hearth. The cards stretch a frame to the hearth's width and nothing fades a frame's border. Soft frame edges and depth fade are standard.
6. **Missing layers (emitters, scene).** The logs are black silhouettes (65,42,33 in the lake film). There is no glowing coal bed, the scorched ring is unlit trodden earth, and the flames stand only about as tall as the log stack; in the photographs they reach 1.5 to 3 times it. At night the ground is lit as a flat tan disc about 3 m across, where in the photographs everything past the fuel is near black.

## How shipped games and professionals build a campfire

Studios publish almost nothing about their campfires. A survey found no GDC, SIGGRAPH or Digital Foundry source on fire for any of the five named games:
- **The Witcher 3:** `.w2p` particle files holding a flipbook flame plus a smoke emitter, judging by mods that replace them. NVIDIA FlameWorks was shown at GDC 2014 and not shipped.
- **Red Dead Redemption 2:** particles, per a mod that re-tunes them.
- **ARK:** the Dev Kit ships its campfire's particle system, unpublished; the light has a hue setting and flickers.
- **Sons of the Forest and Enshrouded:** unknown.

What is documented is professional breakdowns, and they agree:

- **Erik Forsström's UE4 campfire** (80.lv, https://80.lv/articles/creating-a-campfire-vfx-in-ue4) has 14 emitters:
  - 6 fire, 4 of them spawned from a skeletal mesh of the logs;
  - 3 embers, 2 smoke, 1 heat distortion and 2 lights.

  It has six shaders, including burning wood (an emissive mask with flicker noise) and a flickering fake-shadow decal on the ground. Particles spawn invisible and fade in fast so nothing pops.
- **Unity's BonFire sample in VFX Graph** (https://github.com/Unity-Technologies/VisualEffectGraph-Samples) has 7 large flame particles, 24 embers and 24 smoke. All are alpha-blended, not additive. The flame is shader-animated (panner, UV distortion, feathered step, a flow map), so few particles can look rich.
- **A Unity and Unreal fire pack** (80.lv, https://80.lv/articles/creating-a-vfx-fire-pack-for-unity-unreal-engine) layers:
  - the main flame and fire licks;
  - burst embers, and many popping GPU embers (its main cost);
  - smoke, glow, light and distortion.

  It adds depth-fade noise and emissive overdrive, and fades the flame by Fresnel so it vanishes seen from above.
- **NVIDIA's Vulcan demo** (GPU Gems, chapter 6) drew 64-frame sprites at quarter resolution, alpha-blended and sorted, with procedural lit smoke and haze in a separate offset buffer.
- **Flipbook practice** (realtimevfx.com, as cited in the survey):
  - 6x6 or 8x8 frames;
  - motion vectors to stretch a short sim, with neutral 128,128 in gaps;
  - premultiplied alpha, so one shader is both alpha and additive;
  - colour from a temperature channel through a ramp.

**A AAA campfire's layers, against ours:**

| Layer | AAA | Terra today |
|---|---|---|
| Core and body flames | 2-6 flipbook or shader-animated emitters, alpha or premultiplied | 3 hearth cards (cards 3-5 drawn nowhere) |
| Licks and tongues off the logs | spawned on the log mesh, own lifetimes | none on the logs |
| Embers and sparks | 2-3 emitters, GPU, velocity-stretched | one spark layer |
| Smoke | lit (six-way or probe), soft, dark | six-way lit, thin |
| Heat haze | its own distortion pass | specified (fire.md), not drawn |
| Light with flicker | 1-2 lights, flicker in step with the emissives | 1 shadowed point light, flicker |
| Coal bed and emissive logs | an emissive char mask with flicker noise | none: black logs |
| Ground | a flickering light or shadow decal, scorch | unlit scorched ring |

## Breaking down a known-good campfire (round 12)

Tags: **measured** is read from a shipped asset file. **Stated** is what the author writes. **Inferred** is ours.
No frame-by-frame capture of the Red Dead Redemption 2, Witcher 3 or Sons of the Forest campfires turned up within the budget. The Unity BonFire sample is the one known-good campfire whose numbers can be read. Its `BonFire.vfx`, `BonfireFlame.shadergraph`, `Feather Step.shadersubgraph` and `BonFire.unity` are at https://github.com/Unity-Technologies/VisualEffectGraph-Samples/tree/master/Assets/Samples/BonFire.

**Unity BonFire, per layer (measured from `BonFire.vfx`):**
- **Flame:** 2 spawned a second, lifetime 2-4 s, capacity 7, so 4-7 cards are alive at once. Alpha-blended (`blendMode: 1`, which is Alpha in VFX Graph; 0 is Additive). Card scale 1 x 1.5, taller than wide. Per-particle alpha comes from noise (frequency 0.25, 3 octaves, roughness 0.5).
- **Flame shader:** UVs pan at (0, 1) a second, with 0.1 of base offset, through a distortion subgraph. `Threshold` 0.07 and `Feather` 0.03 feed a "Feather Step" subgraph. That subgraph is an Add, a Subtract and a Smoothstep, so it reads as `smoothstep(T - F, T + F, x)` (inferred). The vertex deform has exponent 3 and scale 1.55.
- **Flame colour:** one HDR tint, (128, 2, 0.2) linear. There is no texture gradient. R:G:B is 1 : 0.016 : 0.0016, so bright texels clip red and the tonemapper carries them to orange, yellow and white. Thin texels stay deep red (inferred). The "white-hot base" is exposure on dense alpha, not a ramp.
- **Embers:** bursts of 3-10 every 0.1-1.5 s. Capacity 24, lifetime 0.5-2 s. Velocity is 2-5 m/s up and ±0.33 m/s sideways, with turbulence of intensity 5 and drag 1. They are alpha-blended and use the flame shader: threshold 0.07, feather 0.07, pan 1.7 a second.
- **Smoke:** 5 spawned a second, lifetime 4-5.5 s, capacity 24, so about 24 are alive. It rises at 1-1.5 m/s, ±0.1 sideways, with drag 0.2 and a random 0-360° angle. It is lit, not unlit: a normal map at scale 0.1 and smoothness 0.5. It uses an 8x8 flipbook with a soft-particle fade of 1 m. Alpha-blended.
- **Light (measured from `BonFire.unity`):** one point light, "Point Light - Fire". 1290 K. 20,000 lux at 2 m, which is 80,000 cd, about 1.0e6 lm. Shape radius 0.5 m, range 20 m, hard shadows. It sits 0.38 m above the effect's origin and 0.46 m to one side. The object has no flicker component. The scene is daylit: a 100,000 lux sun at 5500 K, so the fire gives a fifth of the sun's lux at 2 m.

**Other breakdowns (stated):**
- Erik Forsström's heat haze fades its refraction from 1.1 to 1 over life. His ground decal is a fire-shadow texture with slight distortion, drawn as moving firelight, not a real light (https://80.lv/articles/creating-a-campfire-vfx-in-ue4).
- Psyros's fire pit is two billboard planes, one for fire and one for smoke, plus a Niagara ember system (https://psyros.artstation.com/projects/GvqKyd).
- Dylan Bennett's UE5 campfire got its flicker from many point lights. He calls that its most expensive part and would cut it for a game (https://dylanbennett6.artstation.com/projects/eRL2RD).
- A reviewed panning-texture campfire took this critique: make the tongues fewer and larger, give the silhouette several triangular peaks inside a cone mask, use a deeper orange in the dark parts, and move faster. Multiplying by a bottom gradient took away a hard line at the base (https://realtimevfx.com/t/campfire-using-panning-textures/20165).
- League VFX artists on flame shapes: yellow or light orange at the centre and red at the edges, white of the erosion map near each shape's centre, close-packed particles of varied size (https://realtimevfx.com/t/about-fire-from-infernal-velkoz/13491).

**Tip erosion, as practitioners do it (stated):**
- Soft erosion is `a = smoothstep(t - f, t + f, mask)`. A hard `step` aliases. With a feather, remap t over [-f, 1 + f], or the mask never fully clears at 0 and 1. Import erosion maps as linear (https://vfxdoc.readthedocs.io/en/latest/shaders/alpha-erosion/).
- t rises over the particle's life, from vertex-colour alpha or a curve. The mask is a 0-1 UV gradient times noise, so the low-value parts go first: the top and the edges (Niels and Torbach, https://realtimevfx.com/t/what-is-your-alpha-opacity-mask-clip-erode-animation-workflow-like/7705). An example edge width is 0.2, as in `smoothstep(.2, .4, x)`. BonFire's is 0.06 (2 x 0.03).
- Equalize the erosion noise, so a threshold of 0.1 removes about 10% of texels and the easing lives in the curve (MikeH, same thread).
- Drive emission with a second smoothstep, offset from the opacity one. The eroding rim then glows and cools at its own rate (Travis, the Velkoz thread; MikeH feeds the inverse of the erosion into emissive).
- Multiplying by the particle's alpha as well as subtracting the threshold gives fade and tearing together (Limeslushie, the Velkoz thread).

**Physics for size and flicker (inferred from fire science):**
- Mean visible flame height, Heskestad: L = 0.235 Q^0.4 - 1.02 D, with Q in kW and D in m (https://arxiv.org/pdf/2008.12209). For a pile 0.5 m across, 30 kW gives 0.41 m and 100 kW gives 0.97 m. The tips flicker above the mean. Our own photographs show 1.5-3 times the pile.
- Puffing frequency of a buoyant flame: f ≈ 1.5 / sqrt(D) Hz, D in m (Cetegen and Ahmed, via https://arxiv.org/pdf/1803.10400). That is 2.1 Hz at 0.5 m and 1.5 Hz at 1 m. So the big light swing is about 2 Hz, with faster small noise on top.
- Firelight colour is 1300-1900 K. Unity lists a candle at 1800 K (https://docs.unity3d.com/ScriptReference/Light-colorTemperature.html), and BonFire uses 1290 K.

**What we take:**
1. **Size:** flame cards 1.5 times taller than wide. The mean flame stands 1-1.5 times the pile's height, and the tips reach about 2 times on the puffs.
2. **Tongues:** 4-7 large body cards alive at once (2 a second, 2-4 s life), plus many small tongues on the logs. Make them fewer and larger than feels right, with a cone-shaped silhouette of several peaks.
3. **Erosion:** `smoothstep(t - f, t + f, gradient * noise)` with f 0.03-0.1. Remap t over [-f, 1 + f] as life goes 0 to 1. Pan the noise up at 1-1.7 UV a second, so the top and the edges tear first.
4. **Emission edge:** a second smoothstep, offset by about 0.05 from the opacity one, so the torn rim glows.
5. **Colour:** one red-dominant HDR tint, about (1, 0.016, 0.0016) times an exposure-scaled intensity. Dense alpha clips to white at the base and thin alpha stays red at the top. Multiply by a bottom-to-top falloff so the upper flame is see-through.
6. **Blend:** alpha or covering for the flame, embers and lit smoke. Additive only for glow.
7. **Embers:** bursts of 3-10 every 0.1-1.5 s, 0.5-2 s life, 2-5 m/s up, turbulence 5, at most about 24 alive.
8. **Smoke:** about 24 alive, 4-5.5 s life, 1-1.5 m/s up, lit by a normal map, soft fade 1 m.
9. **Light:** one shadowed point light about 0.4 m above the coals at 1300-1900 K, with a shape radius about the pile's. Flicker it with a swing of about 2 Hz plus small fast noise, in step with the flame's emission. For moving light on the ground, a flickering ground decal is cheaper than many lights.
10. **Coals and ground:** an emissive coal bed and char mask on the logs, with a firelit ground pool or decal. These hold the bright base when the flame cards are thin.

**Applied in round 12 (measured on the yard, night, 2.5 m and 6 m views):**
- The body at 0.55-0.7 of `tall` hid behind the 0.45 m teepee: only its tips showed. At 0.8, with its foot raised among the logs, it read short; at 0.95 it stands about as high again over them.
- Erosion on the raw cover ate the whole flame: the set's median cover is 0.16. Taken over three times the cover, it tears only the tips.
- Tongues placed inside the pile were hidden by the logs. They must rise from the logs' tops where they stand.
- Our light 0.4 m up, as BonFire's, halved the ground pool: the ring of stones shadows the ground outside it. Kept at 0.75 of the flames, 3 stops by night doubled it (0.058 to 0.12 linear at 0.8 m).
- Blind reviews: 0.8 against the old cone 6/10 to 5/10; 0.95 (tongues 0.26-0.55 of `tall`, white only at the foot, the tops thinner) against 0.8 6/10 to 5/10.

## Plan to AAA, ranked

Each step is checked by the same one-job A/B (ours, the pro book and the photographs at noon, dusk and night), with measurements that judge the flame band and the lit surroundings apart.

1. **Fix the brightness hierarchy (renderer).**
   - The flame's drawn luminance sets the near light, not lumens alone: the fire side of a surface at 1 m sits about 4 stops under the flame core. That means a softer near falloff, from an area light the size of the flames or the stack's lights spread over it, and soot-dark stones.
   - Keep the exposure rule (D415) and the physical lights. This is also the lake troll's ask, done without blowing the stones.
2. **A ramp whose hottest end is white-yellow, and an alpha-weighted blend (renderer).**
   - Give the ramp's top heats blue up to about half of red in sRGB (253,224,133), as a camera records.
   - Raise the hot texels' alpha so the flame covers, not adds over, the blue world: the pro book's 0.03-0.10 B/R must survive our path.
   - When both hold, retire `film.rs`'s mask: it patches a ramp with no blue through a curve that adds blue.
   - Whether the whole camera moves to a per-channel curve is the developers' call (Open, art; fire.md "The tonemapper").
3. **The coal bed, emissive logs and lit ground (scene).**
   - Ember-glow materials on the logs and the bed, flickering with the fire (fire.md "Burnables").
   - A warm light decal on the ring.
   - Flames reaching 1.5-3 times the stack, spawned along the logs, not round its centre.
   - It is the cheapest large gain, and nothing in the flipbook can stand in for it.
4. **Rebake with body (our bake), compared against a known-good sim.**
   - First render JangaFX's CC0 VDBs (Fire Plume, Simple Fire) through our own `bake.py` render path. If they come out good, Mantaflow's settings are the gap. If not, the render path is.
   - The sim must give, per frame:
     - temperature (heat, R);
     - a flame-density cover, opaque in the core, its median over 0.35 and 2-5% over 0.9;
     - soot;
     - exact motion vectors (we have these).
   - Sizes and frame counts:
     - 64 frames at 256 a frame for the hero set;
     - 8 to 16 seconds of sim at 24-30 fps, sampled to 64 frames, the vectors carrying the gaps;
     - a domain of at least 128 voxels across with 2-4x noise upres.
   - Mantaflow stays: free, headless, already wired. Houdini Apprentice is free but non-commercial and watermarks renders, so it is out for shipped textures. We found no free EmberGen licence for commercial use.
5. **Fix the gate before tuning to it (our tool).**
   - Re-derive the campfire class from flame-only crops; today's crops mix the stones in.
   - Judge the flame and the lit surroundings apart.
   - Add a "pro book through our path" row as a standing known-good: a change that makes it worse is wrong whatever the class says.
6. **The emitter stack (emitters).** Bring the campfire to the AAA layer list above:
   - licks spawned on the logs, own lifetimes, fade-in;
   - 2-3 ember emitters;
   - heat haze;
   - soft frame borders and depth fade on every card.

**Keep:**
- the one Fire module and presets (D136);
- the flipbook format (heat, cover, soot, distortion, motion);
- the Mantaflow baker and `pack.ts`;
- six-way smoke;
- the exposure rule;
- physical lights;
- the measuring tool.

**Throw away:**
- the tonemap mask, once steps 1 and 2 hold;
- the campfire set's current bake, for its thin cover;
- the campfire class's current bands;
- tuning a flame's brightness constant to pass a band.

## A cinematic tier

For trailers, close-ups and a hero fire, with about 2-3 ms more:
- **Flipbooks:** 4096-square sheets, 8x8 frames of 512, with motion vectors at full resolution in RG16.
- **Cards:** two to three times as many flames and licks.
- **Smoke:** lit per pixel and self-shadowed.
- **Fog:** the froxel fog lit by the fire's light, its in-scatter softened near the source.
- **The flame as a volume:** a sparse volume (a VDB, Mantaflow's own or JangaFX's CC0) ray-marched for a hero fire seen close. This is Unreal 5's Heterogeneous Volumes path. In real time it is a cinematic budget only.
- **Embers:** motion blur on them.

## Open for the developers

- **Style:** photoreal fire, or the art style's lightly stylised realism (D14)? The photographs and the pro book are photoreal. A stylised fire would be judged against reference game frames instead.
- **The camera's tonemapper:** moving the whole camera to a per-channel curve changes every scene, so the developers see the comparison first.
- **Fire by day:** how much a fire should dominate at noon against night.

## Limits

- One pro book, without motion vectors, converted by us. Its heat is its own temperature render, so its spread (median 0.14) differs from ours (0.43). Its cards are our hearth's, stretched.
- One camera, at 6 m.
- Nothing is public about the five named games' campfires. Their columns are what mods and samples show, not what their studios said.
- The 20x hierarchy estimate is order-of-magnitude, from typical flame luminance, not a measurement of our scene.
