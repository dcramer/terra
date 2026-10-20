# A raymarched breath for the cinematic tier

2026-10-19. The question (a spike, D133): could a raymarched volume, spending what the cinematic tier allows (docs/fire.md, "LOD and budgets"), make the dragon's breath read like a AAA breath? The card breath has sat at 3 to 3.5/10 for many rounds. The developer said "for cinematic shot ... we can burn the budget there a little bit" and "we do it properly, and once". An earlier marched breath gave "a white blob", but that was before the fire rebuild (D415): the brightness order, the ramp's yellow-white hot end, the per-pixel film tonemap mask and the night grade fix.

## Answer

**Yes, it is the better path, but not yet trailer quality. Build it as the cinematic tier's breath (Open for the developers).**

In a blind A/B against main (130d62b3), the harsh reviewer chose the volume in every row where the breath is on, at 90 to 95% confidence. Overall it scored the volume 4.5/10 and main's cards 2/10; the best card round ever scored 5/10, with a different reviewer. Row by row:

| Row | Main | Volume | Confidence |
|---|---|---|---|
| Noon, yard | 2.5 | 4.5 | 90% |
| Dusk, side | 2 | 5 | 95% |
| Dusk, head-on | 1.5 | 5 | 95% |
| Night, head-on | 1.5 | 5 | 95% |
| Night, 1.5 s after it stops | 3 | 3 | tie |

The volume needed two tuning renders and the first round of review. The cards' failures are gone in one step: the salmon shards, the hard cuts along the wall's top, the white stripe, the flat paper discs. What it shows instead is a jet from the jaw, cooling from yellow to orange, into a fireball with a black soot crown.

Its gaps, from the review:
- **Scale and energy.** The jet is a thin, smooth tube and the ball is small.
- **Light.** The wall and ground are lit salmon-pink. That is the jet's light, which the spike did not touch (r33's L.diff and r34's C-light are the candidates).
- **Lighting of the gas.** Nothing in the volume shadows itself, and its smoke neither rises past the wall nor lingers.

A second round showed the limit of tuning by eye. A ball about twice as large, a stream 1.3 times wider and gas half as dense won at noon and from the side. Head-on, though, it filled the frame as a soft yellow blob and lost the soot crown, which was the old "white blob". So the blob comes from thin, hot gas seen from inside, not from the march itself. That round was not landed.

**Cost** (`--bench` offscreen at 1920x1080 on the RTX 3080, 240 frames, dusk):
- **Head-on, the volume filling most of the frame:** the transparent pass goes from 2.22 ms to 4.41 ms. The frame's GPU time goes from 11.19 ms to 13.15 ms, about 2 ms more than the cards.
- **Side view:** the run reported no per-pass GPU times, and the whole GPU frame read 4.3 ms on main against 3.9 ms with the volume, so the volume's cost from the side is too small to measure that way.
- **Not measured:** the integrated GPU, and half resolution.

About 2 ms on the 3080 fits a film's 33 ms frame. It does not fit play's 0.3 ms budget for a fire.

## How shipped games and tools do it

- **EmberGen.** Its viewport ray-marches its simulation grid live, coloured through a temperature ramp. Games mostly use it to bake flipbooks.
- **Unreal.** Heterogeneous Volumes march a sparse volume texture, with a lighting cache and a volumetric shadow map. Niagara Fluids' 3D gas renders through the same path. Epic presents it for cinematics and the path tracer and gives no budget for play.
- **Shipped games.** Guerrilla's Nubis marches clouds through tiling 3D noise with temporal reprojection; no fire talk was found. Sucker Punch's smoke in Second Son was GPU particles with curl noise, drawn as sprites. Froxel fog (Naughty Dog, Bevy) is too coarse for a jet. Hellgate: London shipped a ray-marched 64×64×128 fluid grid (GPU Gems 3, chapter 30).
- **Shadertoy fires** (XT95's Flame, Duke's explosion) are CC BY-NC-SA, so we can't port them. We took only the technique, which is standard: emission from temperature, Beer-Lambert absorption, advected and domain-warped noise.
- **Baked volumes.** OpenVDB and NanoVDB are Apache-2.0. A baked volume plays one fixed shape, which suits neither a jet's changing length nor its aim.

The spike is the analytic version of what Unreal calls a volume material on a proxy box. Its ray, box test, depth-prepass end and jitter follow Bevy's own volumetric fog (`bevy_pbr` volumetric_fog.wesl, MIT/Apache) and the game's earlier rod march. Its fields are split as Houdini's and EmberGen's pyro are: density, temperature and soot. It treats the gas as emitting and absorbing (Fong et al., "Production Volume Rendering", SIGGRAPH 2017 course). No permissive fire-marching source worth porting was found.

## What was built

The spike is commit 6db86f20, in `spike/breath-volume`, deleted after this note. It is a shader built from main's `fire.wesl` plus one edit to `fire.rs`; the game's breath was not changed.
- **Fields.** Each is a function of the point and time, in the jet's frame.
  - The stream is a widening cone. Its noise travels along it with the gas (each parcel's `flow`), warped by more noise, so it boils as it flies. Its line wanders and its tip tears into blobs.
  - The fireball is a set of lumps that roll up off the face. Its temperature is hot inside, with cool folds, and cools upward to a soot cap. Any gas cooler than burning turns to soot.
- **The march.** Up to 96 steps front to back through the jet's hull, ended at the depth prepass, empty space skipped and the first step jittered for TAA. Each step's light is the ramp's colour at its temperature, times 12, against the hearth's 32. Absorption is 9 per metre for flame and 14 for soot, so a thick flame shows its outer layer and its hot core shows only through thin gas.
- **The film mask.** The hull is drawn a second time into the picture's alpha, with the flame share the march found (`FIRE_MASK`). That sends the volume through the per-channel film shoulder, as the hearths' flames go, and is why it reads orange to yellow and not salmon. In the first render, at the hearth's brightness, it was a pale yellow-white blob, the old failure again. At 12 it took colour.

## If it is built

- **Make it a quality preset's choice** (D239, D249): the cinematic preset draws the marched breath and hides the breath's cards; play keeps the cards.
- **The first gains, in order:**
  1. The jet's light colour, orange-yellow not pink.
  2. One shadow tap a step toward the jaw, a cheap self-shadow.
  3. A soot plume rising from the ball past the wall, and lingering.
  4. A turbulent jet that is thicker at mid-reach, without thinning the gas (the second round's lesson).
  5. Noise from a tiling 3D texture rather than hashed value noise, which would also cut the cost.
- **Measure it** on the integrated GPU and at half resolution with a depth-aware upsample. Hold the fire check (`cargo xtask fire`) to the same bounds as the cards.
