# The lush forest beside Sons of the Forest: light, ground, leaves, trees

2026-10-14. Question, from the developers after the latest lush forest walk: the forest is "slightly wrong elements wise, and fidelity wise": tree detail a bit low, leaves not great, the ground brown most of the time when they want more grass, and the whole still bright where Sons of the Forest (SotF) feels shaded inside the forest even by day. What exactly differs, measured against SotF's own pictures, and what do we change?

## Answer

Our forest is **about two stops too bright, and three stops too bright in its shadows**. It looks up into **lit, lime crowns at eye height** where SotF looks up into **dark trunks and crown undersides**. Its ground shows **sunlit orange litter** where SotF's is **dark olive grass and ferns**. Colour matters too: SotF's frame is warm olive (red above green, little blue) and ours is cool green-cyan. Leaf and bark fidelity are real gaps, but they come after light and composition.

Every one of the 20 SotF frames has more near-black than any of our 25 frames, and a darker 5th percentile. 15 of the 20 are darker on average than our darkest frame.

## Reference set

All official, from the Steam store (`store.steampowered.com/api/appdetails?appids=1326470` and `242760`): store screenshots, and stills taken every 2 s from the store's four SotF trailers (HLS streams, ffmpeg, 1920×1080). Only summer, daytime forest interiors were kept; snow, caves, beach, night and menus were left out.

- **SotF, 20 frames.** Store screenshots 2, 3, 5 and 8 (0-based order of the API). From the Multiplayer Trailer (t0), stills at 2, 8, 9, 12, 15, 21 and 25 × 2 s; from Trailer 3 (t1), 2, 21, 22, 25 and 44; from Trailer 2 (t2), 7, 35, 37 and 40.
- **The Forest, 6 frames.** Store screenshots 1, 3, 5, 6, 12 and 14.
- **Ours, 25 frames.** Vantage shots 13-path, 13-floor, 13-clearing, 17-floor and 17-path, plus `film.mp4` and `pines.mp4` every 4 s (10 each).

The files are kept outside the repository (licence): the session scratchpad holds `sotf/ref/`, with the URLs in `sotf/steam-api/` and `sotf/trailer-frames/urls.txt`.

## Method

The same script ran on every frame (Node, decoding through ImageMagick at 1920×1080). The centre-bottom box, x 36–64% and y 50–100%, is masked out of every frame, which removes our placeholder player. SotF's first-person hands sit mostly at the lower right and were left in.

- **Luminance:** Y′ = 0.2126R′ + 0.7152G′ + 0.0722B′ on the sRGB-encoded values. The shares above 0.85 and below 0.08 use the same scale.
- **Colour:** HSL saturation; chroma (max − min); mean RGB, with B/G and R/G.
- **Classes by hue, per third of the frame:**
  - *sky*: top 60% only, bright and blue or bright and neutral;
  - *green*: hue 55–170°;
  - *brown*: hue 8–55°;
  - lower third split into *orange* 8–38°, *olive* 38–65° and *green* 65–170°;
  - *lit litter*: orange, saturation above 0.25 and Y′ above 0.25.
- **Local contrast:** the SD of Y′ in 32 px blocks, raw (`blockSD`) and divided by the block's mean (`relSD`).

The hue classes are proxies. "Brown in the middle third" is trunks plus olive shade, not trunks alone.

## Numbers (means; the per-frame table is at the end)

| | SotF (20) | The Forest (6) | Ours (25) | Ours ÷ SotF, in linear light |
|---|---|---|---|---|
| Mean Y′ | **0.148** (0.074–0.273) | 0.236 | **0.287** (0.184–0.338) | 3.5× (1.8 stops) |
| 5th / 50th / 95th percentile | 0.016 / 0.106 / 0.407 | 0.038 / 0.186 / 0.590 | 0.099 / 0.227 / 0.687 | 8× / 3.8× / 3.1× (3.0 / 1.9 / 1.6 stops) |
| Share above 0.85 | 0.3% | 0.1% | 1.9% | |
| Share below 0.08 | **46%** (min 17%) | 22% | **2.4%** (max 11%) | |
| Top third: mean Y′, green share, below 0.08 | 0.17, 14%, 42% | 0.30, 45%, 12% | 0.37, **60%**, 0.6% | |
| Middle third, brown (trunks and olive shade) | 49% | 26% | 6.5% | |
| Lower third: mean Y′, 5th percentile | 0.106, 0.011 | 0.133, 0.024 | 0.213, 0.088 | |
| Lower third: green / olive / orange by hue | 11 / 35 / 23% | 27 / 24 / 26% | 43 / 25 / 18% | |
| Lower third, lit orange litter | 3.4% | 3.5% | **9.0%** (film frames up to 34%) | |
| Lower third below 0.08 | 59% | 43% | 5.1% | |
| Mean RGB | 0.161, 0.149, 0.102 | 0.230, 0.242, 0.189 | 0.257, 0.302, 0.225 | |
| R/G, B/G | **1.10, 0.64** | 0.98, 0.75 | **0.85, 0.75** | |
| HSL saturation; chroma | 0.47; 0.071 | 0.29; 0.077 | 0.28; 0.109 | |
| blockSD; relSD | 0.053; **0.43** | 0.067; 0.35 | 0.072; **0.25** | |

How to read it:

- **Exposure.** SotF's forest does not expose its shade back up to mid-grey. Its median sits two stops under ours.
- **Shadow depth.** This is the largest gap. SotF's dark half is near black; ours bottoms out near sRGB 25. We carry 3 stops more fill at the 5th percentile, and about 1.1 stops of that comes on top of the exposure difference.
- **Highlights.** Our flecks are not too bright for the frame (p95 differs least). What makes the frame bright is everything else.
- **Relative contrast.** SotF has 1.7 times our relSD: its picture is dark ground broken by bright flecks and sky gaps, and ours is evenly mid-tone.
- **Colour.** SotF's greens read olive, with red above green, and its shade is warm umber. Ours are green with blue in them. HSL saturation looks higher in SotF only because HSL inflates dark pixels. In chroma, ours is more colourful.
- **Ground.** By hue, SotF's lower third is **not** greener than ours. What reads as grass there is dark olive blades and fronds with no bare surface between them. What reads as "brown" in ours is bright orange litter in sun patches (D275's Dry Decay Leaves) and a lower third twice as bright.
- **What the camera sees.** Our top third is 60% lit leaves, because the oak crowns start low (D247). SotF's is 14%: tall straight conifers, crowns far overhead, trunks in rows down the middle third, and sky in slits.

## Seen, not measured (side-by-side views)

- **Trees.** SotF has many tall, thin trunks per view, with deep vertical furrows lit in patches, ivy climbing many boles, and small dead branches. Ours shows few, thick, smooth-reading trunks, with big crowns filling the view.
- **Leaves.** SotF's leaves are small, dense and dark, with glints of specular on the sunlit ones, and backlit ones glowing only toward the sun. Ours read as large, flat, evenly lit leaves of one size and one lime-to-mid green. Viewed at 1280 px the crown is a texture of round leaves, not a mass with depth.
- **Ground.** SotF has ankle- to knee-high grass and ferns nearly everywhere off the paths, with moss and small plants in it, and dark soil showing only in gaps. Ours has clean orange litter between plants, and the sun patches on it are the brightest large areas in the lower frame.
- **Air.** Sun shafts in haze in about a quarter of SotF's frames. The haze lifts the middle distance (5th percentile 0.029 in the middle third against 0.011 in the lower). Ours is lifted everywhere (0.108 in the middle), so the haze has no dark to read against.

## What SotF is built with

P is primary (Endnight's own notes and the game's menu); C is community or inference.

- **Unity HDRP** (C, PCGH's tech test). The menu backs this up (P, listed by PCGamesN): its options include Contact Shadows, Micro Shadowing, Ambient Occlusion, Fog Quality, Clouds, Screen Space Reflection, Parallax Distance, Colour Grade, Bloom, Film Grain and Chromatic Aberration, which are the names of HDRP's own overrides. Micro shadows, for example, darken normal-map and AO detail in direct light.
- **Volumetrics.** Raymarched volumetric fog and clouds at low resolution (C, PCGH). Endnight's notes mention "volumetric light from sun" (Patch 12), local fog hemispheres, low-lying fog (Patch 15), and fog laid out over the mountains (v1.0) (P, Steam announcements).
- **Grading.** A selectable colour-grade "Style", meaning LUT looks, including "Found Footage" (P, Patch 15). The warm, dark look is graded, not only lit.
- **Materials.** Photogrammetry and parallax mapping on many textures (C, PCGH). Screen-space reflections (C).
- **Vegetation.** Mesh LODs, then billboards for trees and bushes, with stippled (dithered) fades (P, Patch 14 and Patch 15); seasonal materials on trees (P). Endnight has not said what it authors trees with or how it places grass (see [forest-lod-and-mood.md](forest-lod-and-mood.md)). For The Forest's Unity 5 update: subsurface scattering on plant shaders, bendable plants and new billboards (C, Wccftech).
- **Not found.** Nothing public on SotF's GI. A dynamic day with seasons argues against baked lightmaps (C).

## Recommendations, ranked by effect per cost

Targets are SotF's means, eased toward The Forest where copying SotF exactly would be a horror grade. All the decisions named below are the Agent's, so they can be changed with a reason and a log line, except where it says the developers decide.

1. **Stop opening the eye so far under the canopy.** `scene.rs`: `ADAPT_BRIGHTER` from 3.25 to about 1.25, or lower the metered target (`METERED_AS_SET`) under trees.
   - Target: mean Y′ 0.15–0.20, median 0.10–0.14, p95 0.40–0.55.
   - This amends D270, which widened the eye from 2.5 to 3.25 stops, and D216's metered target. D270 widened the eye so that flecks would go bright. Recommendation 2 keeps the flecks bright by taking light out of the shade instead.
2. **Deepen the shade, not the sun.**
   - Targets: p5 0.015–0.035 (now 0.099); below 0.08, 25–45% of the frame (now 2.4%); top third below 0.08, 15–40% (now 0.6%); relSD 0.35 or more (now 0.25).
   - Levers, measured one at a time:
     - `daylight.rs` `SKY_LIGHT_GAIN` 1.6 toward 1.0 under the canopy (D216's 1.4×);
     - `sky_occlusion.rs` `LEAF_GLOW` about halved, `SHADED_GROUND` 0.2 to about 0.1;
     - `foliage_fragment.wesl` `BOUNCE` (0.55, 0.6, 0.4) to about 0.3 of that, `FLOOR_LIGHT` 0.4 to about 0.2, so crown undersides go dark as SotF's do (D264);
     - stronger SSAO in foliage.
   - D216 forbade extra shadow contrast because it once crushed the shade. Crushing to near black over about 40% of the frame is now the measured target, so say so when amending it, and do it with light before grading.
3. **Warm the grade to olive and take out blue.** `daylight.rs` grade temperature +0.08 to +0.12 by day under trees (it is 0.035 only near sunset). Warm the haze (`DistanceFog` (0.5, 0.56, 0.55) and the volumetric ambient (0.55, 0.62, 0.5)) toward an olive grey with blue under green by a third.
   - Target: R/G 0.95–1.05, B/G 0.62–0.70.
   - This fits D279 (green haze, no teal), and goes further. Industry practice is a LUT, as SotF's Styles are; check whether pinned Bevy's tone mapping takes a custom LUT before building one.
4. **Make the ground green and dark, not orange and lit** (amends D246 and D275).
   - In `lush/floor.rs` `growing`, the floor under a closed canopy goes from grass 0.06 + 0.3 × tufts to about 0.35–0.5, made of shade grasses and sedges. Add carpets of the kinds already grown (bilberry, wood sorrel, herbs, ferns), so the floor off the paths is 60–80% plant cover.
   - Darken and desaturate the litter layer's albedo toward umber in `ground.wesl` and `cover/litter.rs`, about 30% darker.
   - Target, lower third: lit orange litter under 4% (now 9%, up to 34%), mean Y′ 0.10–0.14, olive plus green 45% or more, bare litter showing mainly on paths and at tree feet.
5. **Raise the canopy and put trunks in view** (the "elements"). The frame needs to look up into trunks, not crowns: top third green under 25%, middle third brown or trunk 30% or more.
   - Fewer low, wide oak crowns in the first 20 m: D247's young-oak sub-canopy of wide, low crowns at 8 to 12 m is the main cause.
   - Longer clear boles (oak crown base from a third of its height to half; pines with 50–60% clear).
   - More, thinner, taller stems per view.
   - **Decided by the developers (D301): a tall conifer stand.** Which forest the lush forest is (an oak wood with a closed low canopy, or a tall, open conifer stand as SotF's) is a design choice. Recommend a SotF-like conifer stand with oaks as the exception, and mark it Open until they choose. Grass then follows from D246's own rule, since a high, open canopy lets light reach the floor.
6. **Leaves: smaller, darker, glinting** (`textures.ts cards` recipes, D269; `foliage_fragment.wesl`).
   - Twice the leaves per card at half the size.
   - Oak albedo about 20% darker and more olive.
   - Keep a little sheen on upper faces lit by the sun (roughness about 0.5), with the underside rule of D264 unchanged.
   - Self-shadowing inside a crown (`LAYERS` 0.06) kept, so a crown reads as masses.
   - Judge by a harsh review against these frames. Compression in SotF's frames rules out a fine-detail number.
7. **Trunks with more relief and life.** The bark already has relief parallax (`lush.rs`, 4 steps). Deepen the depth scale, add a micro-shadow term (darkening the normal map's detail in direct sun, as HDRP's micro shadows do), and put ivy and moss bands on some boles, as SotF has (D264).
8. **Air.** Once the shade is dark, the existing haze and shafts (D224, `VolumetricFog`) will read. Then add low-lying fog in hollows (SotF Patch 15). Target: the middle third's 5th percentile 2 to 3 times the lower third's (SotF 0.029 against 0.011; ours 0.108 against 0.088).
9. **Make this measurement a tool** (D71). It will be run after every change above: a `tools/` script printing these columns for a sheet against `sotf/ref`'s means.

Do 1 to 3 together and measure. They are constants, and they decide whether 4 to 8 look right.

## Limits

- **Marketing frames.** SotF's frames are graded, chosen from trailers with a horror tone, and compressed; time of day is unknown; the hands are left in. The Forest's six are brighter, and are given as the milder bound.
- **Compression.** Fine detail cannot be compared fairly across JPEG, video and PNG; only coarse statistics are used.
- **Proxies.** The hue classes are proxies, not segmentation.
- **Not seen.** No SotF frame is from a broadleaf wood, so the leaf comparisons are against its shrubs and ferns.
- **Nothing run.** Nothing here was run in the game. Each target needs the walk's film and the vantage sheet measured again after the change.

## Sources

- Steam store data, SotF and The Forest (screenshots and trailers): <https://store.steampowered.com/app/1326470/>, <https://store.steampowered.com/app/242760/>
- SotF Steam announcements (P): Patch 12 <https://store.steampowered.com/news/app/1326470/view/5220291886491263176>, Patch 14 <https://store.steampowered.com/news/app/1326470/view/5302486387999527997>, Patch 15 <https://store.steampowered.com/news/app/1326470/view/5395937983662520952>, v1.0 <https://store.steampowered.com/news/app/1326470/view/5728086065182647154>
- PC Games Hardware, tech tests (C): <https://www.pcgameshardware.de/Sons-of-the-Forest-Spiel-72418/Specials/Test-Steam-Anforderungen-Benchmark-Release-1414211/>, <https://www.pcgameshardware.de/Sons-of-the-Forest-Spiel-72418/Specials/Test-Steam-Anforderungen-Benchmark-Release-1414211/2/>
- PCGamesN, the settings menu (C): <https://www.pcgamesn.com/sons-of-the-forest/best-settings>
- Wccftech, The Forest's Unity 5 update (C): <https://wccftech.com/forest-updated-unity-5-engine-brings-graphical-gameplay-updates/amp/>
- Unity HDRP fog: <https://docs.unity3d.com/Packages/hdrp/manual/Override-Fog.html>
- Earlier notes: [aaa-forest-look.md](aaa-forest-look.md), [forest-lod-and-mood.md](forest-lod-and-mood.md), [far-forest.md](far-forest.md)

## Per-frame measurements

Shares are in percent. `sat` is HSL saturation. `lowLitterLit` is the lit orange litter in the lower third, and `lowDark` the share below 0.08 there.

| Frame | meanY | p5 | p50 | p95 | hi85 | lo08 | sat | bOverG | rOverG | skyTop | greenTop | brownMid | lowGreen | lowOlive | lowOrange | lowLitterLit | lowDark | blockSD | relSD |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| forest-01 | 0.228 | 0.025 | 0.165 | 0.680 | 0.0 | 20.6 | 0.285 | 0.777 | 0.926 | 5.5 | 40.7 | 11.7 | 42.1 | 20.2 | 15.2 | 0.8 | 45.8 | 0.076 | 0.393 |
| forest-03 | 0.289 | 0.039 | 0.168 | 0.822 | 0.3 | 16.3 | 0.271 | 0.830 | 0.915 | 3.5 | 39.1 | 17.4 | 53.5 | 16.1 | 17.4 | 0.9 | 42.1 | 0.091 | 0.406 |
| forest-05 | 0.268 | 0.044 | 0.219 | 0.623 | 0.0 | 13.7 | 0.216 | 0.764 | 0.883 | 1.9 | 67.0 | 5.1 | 24.3 | 45.8 | 16.0 | 4.9 | 45.5 | 0.078 | 0.317 |
| forest-06 | 0.085 | 0.019 | 0.075 | 0.178 | 0.0 | 55.1 | 0.397 | 0.604 | 1.062 | 0.0 | 46.2 | 46.4 | 16.1 | 28.0 | 41.3 | 0.1 | 70.1 | 0.032 | 0.393 |
| forest-12 | 0.381 | 0.069 | 0.354 | 0.772 | 0.0 | 6.2 | 0.166 | 0.845 | 0.901 | 10.8 | 41.3 | 6.2 | 20.1 | 10.3 | 7.5 | 4.7 | 11.0 | 0.077 | 0.256 |
| forest-14 | 0.164 | 0.035 | 0.135 | 0.462 | 0.0 | 21.6 | 0.403 | 0.649 | 1.206 | 3.1 | 36.5 | 70.1 | 7.4 | 21.5 | 58.8 | 9.6 | 44.9 | 0.047 | 0.315 |
| sotf-02 | 0.087 | 0.006 | 0.066 | 0.250 | 0.0 | 61.2 | 0.726 | 0.363 | 1.243 | 0.2 | 5.4 | 67.8 | 4.4 | 38.0 | 22.8 | 1.4 | 68.5 | 0.037 | 0.454 |
| sotf-03 | 0.087 | 0.007 | 0.060 | 0.211 | 0.3 | 65.4 | 0.561 | 0.657 | 1.021 | 3.2 | 23.6 | 38.1 | 17.2 | 39.0 | 2.5 | 0.0 | 80.2 | 0.046 | 0.515 |
| sotf-05 | 0.095 | 0.008 | 0.068 | 0.271 | 0.0 | 56.6 | 0.588 | 0.437 | 1.287 | 0.1 | 2.0 | 84.6 | 1.8 | 68.8 | 22.3 | 2.5 | 25.6 | 0.048 | 0.547 |
| sotf-08 | 0.075 | 0.006 | 0.041 | 0.263 | 0.1 | 73.2 | 0.624 | 0.526 | 1.220 | 0.2 | 10.6 | 53.6 | 6.5 | 17.6 | 12.3 | 0.2 | 91.2 | 0.042 | 0.602 |
| sotf-t0-002 | 0.140 | 0.014 | 0.073 | 0.483 | 0.1 | 53.4 | 0.516 | 0.632 | 1.088 | 0.2 | 14.1 | 64.7 | 6.5 | 20.8 | 21.2 | 3.1 | 63.5 | 0.058 | 0.409 |
| sotf-t0-008 | 0.199 | 0.032 | 0.136 | 0.539 | 0.0 | 29.4 | 0.468 | 0.593 | 1.132 | 0.9 | 9.8 | 54.6 | 9.0 | 32.8 | 51.2 | 20.2 | 21.9 | 0.097 | 0.530 |
| sotf-t0-009 | 0.196 | 0.031 | 0.140 | 0.517 | 0.0 | 26.9 | 0.432 | 0.641 | 1.159 | 0.8 | 5.1 | 74.0 | 7.7 | 43.4 | 40.1 | 11.9 | 28.3 | 0.088 | 0.482 |
| sotf-t0-012 | 0.177 | 0.027 | 0.111 | 0.539 | 0.3 | 29.9 | 0.298 | 0.820 | 1.107 | 8.0 | 1.4 | 92.6 | 0.1 | 10.1 | 69.6 | 0.2 | 68.0 | 0.035 | 0.234 |
| sotf-t0-015 | 0.273 | 0.025 | 0.272 | 0.579 | 0.4 | 24.9 | 0.303 | 0.783 | 1.083 | 0.7 | 18.2 | 49.2 | 0.3 | 27.1 | 54.3 | 2.0 | 62.0 | 0.062 | 0.297 |
| sotf-t0-021 | 0.246 | 0.040 | 0.238 | 0.476 | 0.0 | 16.9 | 0.254 | 0.781 | 1.077 | 0.0 | 3.3 | 41.4 | 0.3 | 46.9 | 43.6 | 5.0 | 46.7 | 0.052 | 0.245 |
| sotf-t0-025 | 0.183 | 0.023 | 0.154 | 0.416 | 0.3 | 22.2 | 0.316 | 0.749 | 1.097 | 1.4 | 3.6 | 76.6 | 0.4 | 44.1 | 35.3 | 1.2 | 65.5 | 0.038 | 0.227 |
| sotf-t1-002 | 0.074 | 0.004 | 0.053 | 0.217 | 0.0 | 71.1 | 0.556 | 0.554 | 1.051 | 1.7 | 10.6 | 29.8 | 23.2 | 14.5 | 1.3 | 0.1 | 87.4 | 0.044 | 0.554 |
| sotf-t1-021 | 0.080 | 0.006 | 0.053 | 0.283 | 0.0 | 68.4 | 0.592 | 0.506 | 1.039 | 0.1 | 31.2 | 15.9 | 19.0 | 9.7 | 2.4 | 0.0 | 90.8 | 0.038 | 0.423 |
| sotf-t1-022 | 0.080 | 0.007 | 0.056 | 0.193 | 0.1 | 68.8 | 0.556 | 0.657 | 1.041 | 3.6 | 21.7 | 39.2 | 13.2 | 34.0 | 2.4 | 0.0 | 88.3 | 0.040 | 0.472 |
| sotf-t1-025 | 0.108 | 0.007 | 0.057 | 0.367 | 0.0 | 58.6 | 0.602 | 0.465 | 1.139 | 0.0 | 18.8 | 41.3 | 2.8 | 54.8 | 22.7 | 5.8 | 34.8 | 0.061 | 0.605 |
| sotf-t1-044 | 0.175 | 0.011 | 0.119 | 0.454 | 0.0 | 39.0 | 0.551 | 0.478 | 1.152 | 0.0 | 2.3 | 48.6 | 2.5 | 71.3 | 21.4 | 9.6 | 12.5 | 0.055 | 0.424 |
| sotf-t2-007 | 0.124 | 0.008 | 0.088 | 0.376 | 0.0 | 46.5 | 0.514 | 0.506 | 1.069 | 0.0 | 37.3 | 43.3 | 15.6 | 30.9 | 12.9 | 0.8 | 61.9 | 0.058 | 0.513 |
| sotf-t2-035 | 0.246 | 0.012 | 0.103 | 0.837 | 4.0 | 44.0 | 0.265 | 1.027 | 0.918 | 12.1 | 11.7 | 7.9 | 45.1 | 3.9 | 0.2 | 0.0 | 86.7 | 0.049 | 0.279 |
| sotf-t2-037 | 0.132 | 0.030 | 0.105 | 0.330 | 0.1 | 33.6 | 0.249 | 0.775 | 1.005 | 0.8 | 27.1 | 24.1 | 19.1 | 51.7 | 11.1 | 2.1 | 43.9 | 0.041 | 0.312 |
| sotf-t2-040 | 0.182 | 0.019 | 0.127 | 0.540 | 1.0 | 31.1 | 0.367 | 0.795 | 0.976 | 4.2 | 27.9 | 27.0 | 27.6 | 33.3 | 12.3 | 1.8 | 57.5 | 0.063 | 0.401 |
| film-01 | 0.291 | 0.142 | 0.241 | 0.594 | 0.7 | 0.0 | 0.253 | 0.741 | 0.877 | 3.0 | 70.5 | 7.4 | 8.1 | 61.9 | 29.4 | 8.9 | 0.1 | 0.068 | 0.232 |
| film-02 | 0.338 | 0.110 | 0.235 | 0.885 | 6.0 | 0.5 | 0.293 | 0.774 | 0.944 | 10.8 | 45.0 | 11.2 | 19.1 | 41.1 | 36.4 | 23.9 | 1.5 | 0.079 | 0.243 |
| film-03 | 0.332 | 0.066 | 0.257 | 0.860 | 5.3 | 10.6 | 0.361 | 0.647 | 0.956 | 7.8 | 69.0 | 13.0 | 29.2 | 27.4 | 42.8 | 32.0 | 19.9 | 0.088 | 0.301 |
| film-04 | 0.324 | 0.077 | 0.259 | 0.742 | 3.0 | 5.5 | 0.297 | 0.685 | 0.913 | 3.7 | 74.0 | 4.3 | 38.2 | 19.6 | 40.8 | 33.9 | 15.4 | 0.091 | 0.303 |
| film-05 | 0.184 | 0.090 | 0.164 | 0.332 | 0.1 | 2.6 | 0.340 | 0.736 | 0.607 | 0.3 | 69.6 | 0.6 | 77.0 | 15.1 | 2.3 | 0.0 | 1.7 | 0.044 | 0.225 |
| film-06 | 0.287 | 0.091 | 0.277 | 0.580 | 0.5 | 2.5 | 0.307 | 0.653 | 0.729 | 0.8 | 91.8 | 0.0 | 79.9 | 0.9 | 0.1 | 0.0 | 8.2 | 0.073 | 0.253 |
| film-07 | 0.245 | 0.112 | 0.212 | 0.521 | 0.2 | 0.6 | 0.168 | 0.933 | 0.799 | 1.1 | 43.5 | 0.4 | 17.1 | 14.5 | 8.6 | 0.4 | 2.1 | 0.047 | 0.184 |
| film-08 | 0.284 | 0.104 | 0.231 | 0.609 | 0.4 | 0.8 | 0.187 | 0.862 | 0.822 | 1.7 | 57.8 | 0.5 | 38.4 | 28.5 | 5.3 | 2.1 | 2.8 | 0.063 | 0.217 |
| film-09 | 0.259 | 0.119 | 0.189 | 0.689 | 2.0 | 0.2 | 0.192 | 0.867 | 0.813 | 3.3 | 46.8 | 0.9 | 31.6 | 21.8 | 1.4 | 0.4 | 0.6 | 0.053 | 0.190 |
| film-10 | 0.271 | 0.097 | 0.227 | 0.617 | 0.6 | 1.3 | 0.208 | 0.889 | 0.784 | 2.0 | 44.1 | 0.4 | 28.0 | 29.4 | 2.9 | 0.1 | 4.6 | 0.062 | 0.218 |
| pines-01 | 0.278 | 0.107 | 0.207 | 0.839 | 4.8 | 0.0 | 0.292 | 0.755 | 0.830 | 11.4 | 54.2 | 4.5 | 59.3 | 20.0 | 12.4 | 6.6 | 0.1 | 0.059 | 0.203 |
| pines-02 | 0.308 | 0.115 | 0.237 | 0.802 | 3.7 | 0.1 | 0.288 | 0.748 | 0.870 | 5.3 | 58.5 | 6.6 | 36.1 | 33.3 | 28.7 | 16.8 | 0.2 | 0.067 | 0.221 |
| pines-03 | 0.244 | 0.125 | 0.210 | 0.526 | 1.1 | 0.0 | 0.265 | 0.780 | 0.890 | 2.1 | 32.7 | 32.2 | 53.1 | 30.5 | 12.7 | 2.5 | 0.0 | 0.049 | 0.191 |
| pines-04 | 0.298 | 0.104 | 0.207 | 0.814 | 4.3 | 0.8 | 0.294 | 0.720 | 0.878 | 8.2 | 57.6 | 9.9 | 72.5 | 16.5 | 9.0 | 7.7 | 2.3 | 0.063 | 0.215 |
| pines-05 | 0.291 | 0.132 | 0.244 | 0.678 | 2.0 | 0.0 | 0.292 | 0.700 | 0.706 | 2.3 | 81.8 | 0.5 | 74.4 | 12.3 | 1.5 | 1.3 | 0.1 | 0.077 | 0.259 |
| pines-06 | 0.306 | 0.092 | 0.223 | 0.741 | 1.6 | 2.1 | 0.251 | 0.793 | 0.917 | 2.0 | 55.0 | 4.7 | 17.3 | 34.3 | 41.8 | 21.2 | 4.5 | 0.093 | 0.322 |
| pines-07 | 0.304 | 0.095 | 0.226 | 0.756 | 1.6 | 1.8 | 0.239 | 0.819 | 0.821 | 5.6 | 50.0 | 3.6 | 44.7 | 23.0 | 15.2 | 0.3 | 6.3 | 0.069 | 0.238 |
| pines-08 | 0.294 | 0.088 | 0.216 | 0.723 | 2.2 | 3.7 | 0.304 | 0.745 | 0.899 | 1.5 | 53.4 | 8.5 | 16.7 | 39.6 | 35.4 | 20.0 | 9.9 | 0.102 | 0.369 |
| pines-09 | 0.265 | 0.097 | 0.192 | 0.709 | 2.3 | 0.7 | 0.236 | 0.781 | 0.821 | 2.5 | 54.7 | 2.6 | 43.4 | 18.9 | 5.3 | 3.0 | 2.1 | 0.068 | 0.237 |
| pines-10 | 0.293 | 0.112 | 0.238 | 0.628 | 0.3 | 0.3 | 0.206 | 0.877 | 0.801 | 1.4 | 55.6 | 0.5 | 44.2 | 11.1 | 4.1 | 2.1 | 1.0 | 0.063 | 0.216 |
| v-13-clearing | 0.293 | 0.087 | 0.232 | 0.725 | 1.6 | 3.1 | 0.331 | 0.686 | 0.773 | 4.3 | 72.7 | 2.9 | 76.5 | 14.8 | 5.0 | 4.9 | 3.7 | 0.080 | 0.301 |
| v-13-floor | 0.295 | 0.090 | 0.218 | 0.739 | 2.5 | 2.5 | 0.315 | 0.640 | 0.828 | 1.8 | 76.6 | 2.2 | 70.5 | 19.1 | 6.5 | 5.8 | 2.8 | 0.097 | 0.341 |
| v-13-path | 0.271 | 0.071 | 0.208 | 0.658 | 0.4 | 8.1 | 0.319 | 0.666 | 0.869 | 0.1 | 75.8 | 8.0 | 44.2 | 22.5 | 32.5 | 29.6 | 10.3 | 0.089 | 0.353 |
| v-17-floor | 0.306 | 0.090 | 0.255 | 0.710 | 0.9 | 2.9 | 0.346 | 0.559 | 1.007 | 0.4 | 55.7 | 22.5 | 41.7 | 38.8 | 19.1 | 0.0 | 6.4 | 0.077 | 0.261 |
| v-17-path | 0.306 | 0.066 | 0.274 | 0.686 | 0.5 | 10.2 | 0.293 | 0.643 | 1.007 | 0.3 | 62.7 | 15.8 | 23.1 | 30.5 | 44.7 | 1.2 | 21.4 | 0.073 | 0.258 |
| **SotF (20)** | 0.148 | 0.016 | 0.106 | 0.407 | 0.3 | 46.1 | 0.467 | 0.637 | 1.095 | 1.9 | 14.3 | 48.7 | 11.1 | 34.6 | 23.1 | 3.4 | 59.3 | 0.053 | 0.426 |
| **The Forest (6)** | 0.236 | 0.038 | 0.186 | 0.590 | 0.1 | 22.3 | 0.290 | 0.745 | 0.982 | 4.1 | 45.1 | 26.2 | 27.3 | 23.7 | 26.0 | 3.5 | 43.2 | 0.067 | 0.347 |
| **Ours (25)** | 0.287 | 0.099 | 0.227 | 0.687 | 1.9 | 2.4 | 0.275 | 0.748 | 0.846 | 3.3 | 60.4 | 6.5 | 43.4 | 25.0 | 17.8 | 9.0 | 5.1 | 0.072 | 0.254 |
