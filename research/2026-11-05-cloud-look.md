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

## Next

The handoff `~/.cache/terra-handoff/clouds-look-j1.md` lists the next steps.
