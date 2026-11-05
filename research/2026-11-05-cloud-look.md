# Why the lake's clouds look unrealistic, measured against photographs

Dated 2026-11-05 (clouds-look j1). The developer on the review desk (r-cloud-history): "Those clouds look unrealistic, let's fix that". This note measures how the volumetric clouds (docs/sky.md stage 2, D515) differ from photographs and shipped games, and what the first experiments gave.

## The reference

Kept outside the repository, in `~/.cache/terra-review/refs/clouds/` with `credits.txt` and `sheet-refs.jpg`:

- 9 photographs of fair-weather cumulus from Wikimedia Commons (CC BY-SA, GFDL; authors in the credits): fields to the horizon, heaps over the Rockies, silver edges toward the sun.
- 10 shipped frames: Microsoft Flight Simulator 2020 and 2024, Horizon Forbidden West and Zero Dawn (Steam store screenshots), Unreal Engine 5's volumetric cloud documentation. No RDR2 store frame showed daytime cumulus.

## The measure

`cloudm.py` (scratch, in the handoff's scripts) measures a crop of sky and cloud at 1280 px wide. Cloud is saturation under 0.2 and luma over 0.3. It reports the cloud's luma at the 5th, 50th and 90th percentiles (sRGB), the blue of its darkest quarter ((b − r) / luma, "tint"), its fine detail (the luma's spread less a 2 px blur, inside the cloud away from its edge) and how steep its edges are.

| | p5 | p50 | p90 | tint | fine |
|---|---|---|---|---|---|
| Photographs (9) | 0.60 | 0.78 | 0.90 | 0.11 | 0.013 |
| Games (6) | 0.67 | 0.80 | 0.88 | 0.16 | 0.008 |
| Ours, main (lake, 13 h and 17 h) | 0.59 | **0.96** | **0.98** | **0.05** | **0.008** |

## The gaps, biggest first

1. **The clouds are one flat white.** Half of each cloud is at 0.96, in the tonemapper's shoulder, so nothing in it is modelled. Their shade is not the sky's blue (tint 0.05, photographs 0.11). The cause is the multiple scattering's gain, f / (1 − f). Its escape chance takes the whole column's plane-parallel depth (E2 of a column of optical depth 20 to 30), so p is 0.01 to 0.1 and the gain 10 to 100. A sunlit face then sends up to four times what a white Lambertian surface under the same sun would, which breaks energy conservation. The shaded side, dimmed only by exp(−0.12 τ), is still white too.
2. **No small detail.** Fine detail is 0.008 against 0.013, and the outlines are smooth pillows with no cauliflower billows. The erosion cuts only where the body is under 0.4, and its finest Worley cells are 47 m.
3. **The shapes are dumplings.** The heaps are 1 to 3 km blobs with rounded tops, not domed towers on flat bases. Not addressed yet.

## Experiments (tunables `sky.cloud.x_*`, defaults draw main's clouds)

Over the lake (eye at 84,5,-300 over the start, 1901x1550, temporal on), the 17 h v06 and v09 crops at both hours (`variants.sh` jobs 20261105-102219-0043 and 20261105-105603-8e22):

| Variant | p5 | p50 | p90 | tint | fine |
|---|---|---|---|---|---|
| main | 0.59 | 0.97 | 0.98 | 0.05 | 0.008 |
| Hillaire 2016's octaves alone (gain 1, 3 octaves, g 0.6·0.5^i; round 1, from the shore) | 0.50 | 0.59 | 0.69 | 0.12 | 0.005 |
| octaves gain 3, density ×3, light detail, in-scatter | 0.44 | 0.65 | 0.93 | 0.14 | 0.008 |
| the same, gain 5 | 0.44 | 0.73 | 0.96 | 0.13 | 0.009 |
| gain 3, erosion 0.6, detail ×2 finer | 0.44 | 0.73 | 0.94 | 0.14 | **0.011** |
| gain 3, no in-scatter | 0.45 | 0.79 | 0.97 | 0.12 | 0.010 |

- **The octaves** (Hillaire 2016, after Wrenninge 2013): S = Σ bⁱ p(θ, g cⁱ) e^(−aⁱ τ), with a = b = c = 0.5. They take away the white blowout and show the volume's modelling and the sky's blue in its shade. With a = 0.5 the shade is too dark (p5 0.44 against 0.60), because exponential octaves lose the deep light that real diffusion carries (two-stream transmission falls as 1 / (1 + ¾ (1 − g) τ), not exponentially). The next round tries a = 0.25 (still a ≤ b, so energy is conserved by Hillaire's condition).
- **Erosion 0.6 and detail twice as fine** gave the most detail (0.011) and broke the outlines into puffs.
- **Nubis 2017's in-scattering probability** darkens the edges and bases (p50 0.79 to 0.65 at gain 3).
- **The phase** stays Jendersie and d'Eon's (HG plus Draine, itself a dual lobe fitted to Mie for 20 µm droplets). A classic dual HG would brighten the sides by about 10 times at 90°, which is not what Mie does.

## Rounds 3 to 11 (clouds-look j2)

Same views and crops. Each round's base is the round before's best. The new tunables are `x_floor` (the diffusion's deep light, 1 / (1 + ¾ (1 − g) τ), under the octaves), `x_stretch` (the shape noise changing slower up a heap than across it), `x_taper` (a heap's coverage narrowing toward its own top) and `x_sky` (the sky's light into the clouds).

| Variant | cover | p5 | p50 | p90 | tint | fine |
|---|---|---|---|---|---|---|
| Photographs | 0.48 | 0.60 | 0.78 | 0.90 | 0.11 | 0.013 |
| main | 0.27 | 0.59 | 0.97 | 0.98 | 0.05 | 0.008 |
| c: octaves gain 2, a 0.25, density 3, light detail, in-scatter, erosion 0.6, repeats 2 | 0.20 | 0.52 | 0.76 | 0.91 | 0.09 | 0.008 |
| e3: c with erosion 0.75, stretch 3, taper 0.7, heaped_cover 0.7, floor 2 | 0.19 | 0.53 | 0.83 | 0.96 | 0.07 | 0.009 |
| **k3**: e3 with density 2, gain 1, floor 1.5, sky 3 | 0.19 | 0.55 | 0.80 | 0.95 | **0.11** | 0.007 |
| m3: k3 with erosion 0.9, floor 1, sky 2.5 | 0.15 | 0.55 | 0.73 | 0.93 | 0.12 | 0.008 |

- **Shape**: stretching the shape noise 3 times up a heap and narrowing its coverage toward its own top ended the mushroom caps on stalks. The heaps now stand as domed towers on flat bases. Taper costs cover, which heaped_cover 0.7 gives back.
- **A height-field dome** after Nubis Cubed's dimensional profile (a heap's top as a field over the map, density from how far under it a point is) was tried for four rounds and dropped. Without Nubis 2015's coverage cut, the cover swung between 0.07 and 0.56 and the base became one overcast ceiling.
- **Light**: the shade is lit too little by the sky. With the sky's light ×3 the shaded sides take the photographs' blue (tint 0.11). The sunlit faces still sit at 0.95 against 0.90. The octaves' gain barely moves p90, so it is the single scattering or the tonemapper's shoulder.
- **Cost**: e3 measured GPU 7.87 ms on lake-play at 1901x1550 against main's 7.85 and 7.83, with the clouds pass 0.10 ms in both (job 20261105-161947-64d0-patch). The budget is 7.91.
- **Still open**: cauliflower detail (fine 0.007 to 0.009 against 0.013, and the outlines still read as soft puffs), the cover (0.19 against 0.48, though the photographs choose cloudy skies), and the sunlit p90.

## Rounds 12 and 13, and the defaults (clouds-look j3)

| Variant | cover | p5 | p50 | p90 | tint | fine | mid |
|---|---|---|---|---|---|---|---|
| Photographs | 0.48 | 0.60 | 0.78 | 0.90 | 0.11 | 0.013 | 0.028 |
| Photographs at half their size and back | 0.48 | 0.60 | 0.78 | 0.90 | 0.11 | 0.008 | 0.028 |
| k3 | 0.19 | 0.55 | 0.80 | 0.95 | 0.11 | 0.007 | 0.036 |
| k3, the detail distorted 100 m (after Nubis 2015's curl, by the noise's own Worley channels) | 0.19 | 0.54 | 0.81 | 0.95 | 0.11 | 0.007 | 0.036 |
| the same, 300 m | 0.19 | 0.55 | 0.81 | 0.95 | 0.11 | 0.008 | 0.035 |
| k3, billows (1 − fbm)^0.5, erosion 0.9 | 0.08 | 0.55 | 0.70 | 0.89 | 0.13 | 0.008 | 0.044 |
| k3, the single scattering halved | 0.19 | 0.54 | 0.80 | 0.95 | 0.11 | 0.007 | 0.035 |
| k3, octaves' a 0.5, gain 2, floor 0.75 | 0.19 | 0.51 | 0.70 | 0.94 | 0.14 | 0.009 | 0.040 |
| **b13**: k3 with gain 1.5, floor 0.75 (now the defaults, D626) | 0.19 | 0.54 | 0.77 | 0.94 | 0.12 | 0.007 | 0.036 |

- **The fine detail is the picture's resolution, not the noise's.** The photographs measure 0.008 scaled to half their size and back, and 0.007 at a third: the march draws at half the drawn size and its history at half the shown (D617). At the scale the picture holds (mid, 2 to 8 px), ours already has more than the photographs. Distorting the detail changed nothing; sharper billows ate the cover.
- **The sunlit p90 does not follow the single scattering**: halved, p90 stays 0.95. It moves only with the deep light's gain (0.75 gives 0.94), so the brightest cloud is the many-times-scattered light in the tonemapper's shoulder.
- **The steps** against a new supersampled reference (temporal-default j1's measure, the lake crop at frames 410 and 411, the defaults): 1.04 at 3816x1550 and 0.89 at 1901x1550, under D617's bar of 1.10 (1.07 and 0.82 before); native 0.63 at both.
- **The adversarial judge** (stills of every view, the photographs and the shipped frames): 4.5 out of 10, rejected, though a clear gain on the flat white. Its gaps: caps and stacked lenses on some heaps (v06, v07 at 17 h); bodies smooth inside, no shade between billows; soft outlines with loose specks; horizontal banding on the far, low heaps (v07); far heaps unhazed beside the hazed mountains; the sky overhead empty (cover 0.19).

## Next

The handoff `~/.cache/terra-handoff/clouds-look-j3.md` lists the next steps.
