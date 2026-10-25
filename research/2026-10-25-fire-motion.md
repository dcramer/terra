# How real campfires move, measured against ours

2026-10-25, fire round g12. Question: four flame changes in a row (g10-g11) tied or lost to main in blind panels that judge our motion strips against RDR2 stills, so nothing has measured how real fire moves. By what measured numbers does main's campfire move unlike filmed campfires, and which gap is largest?

## Answer

The three gaps furthest outside the filmed clips' range, ranked by how far out they are:

1. **Our fire flickers too slowly.** The flame's area changes with a median frequency of 0.83 Hz at dusk and night. The five real clips run 1.83-3.83 Hz, so ours is 2.2 times below the slowest of them. Ours has no peak in 0.5-8 Hz. Two real clips peak at 1.9-2.2 Hz, the puffing a fire of this size has.
2. **It pumps too much.** The flame's area varies with an sd/mean of 0.25 at night and 0.36 at dusk. The real clips give 0.09-0.19, so ours is 1.3-1.9 times over the top of their range. The whole flame swells and shrinks together. In ours the low and high flame's flicker correlate best at no lag (r 0.24, 0.38). In four of the five real clips the correlation at no lag is about zero (-0.11 to 0.12): their changes travel up the flame instead of happening everywhere at once.
3. **Its top is too red.** In the top third of H, red is 57% at night and 55% at dusk. The real clips give 3-47%, 8-10 points under ours. Pale in the top third is inside their range (ours 1%, real 0-47%).

The outline's roughness is inside the real range in every band, and so is the number of tongues. Nothing measured says the rim needs more tearing: g10-g11's lever was not a gap, and the panels agreed.

## Clips

The clips are kept outside the repository, in `~/.cache/terra-review/review/anchors/fire-motion/src`, with `LICENCES.tsv`. All are from Wikimedia Commons, real fire, 30 fps, and the first 10 s of each is measured.

| Clip | View | Licence |
|---|---|---|
| Campfire Video 1 (Virginia State Parks) | standing, night | CC BY 2.0 |
| Campfire video 2 (Virginia State Parks) | standing, night | CC BY 2.0 |
| Campfire at night, Claytor Lake State Park (Virginia State Parks) | standing, dusk, stone ring | CC BY 2.0 |
| Fire.ogv (Cary Bass) | close, night | public domain |
| Feuer.webm (Brützlschnätz) | close, night | CC BY-SA 4.0 |

Ours is main 066c233c's campfire. It was filmed fresh on the worker in the matched view (the fire sheet's campfire from above with a 6 s drift, as in clip6.ts), at dusk 18.4 and night 22, with flame mattes (`fire/g12/tools/clipm.ts`, the job `remote/20261025-143601-88f0`).

## Table

Measured with `node tools/fire/motion.ts <clip> [--matte <matte>]`. The flame is measure.ts's flame colour brighter than half the clip's 99th-percentile flame luma, and inside the matte for ours. Each clip is scaled so the flame is 160 px tall. H is the flame's height.

| Measure | main dusk | main night | real min | real max | ours |
|---|---|---|---|---|---|
| flicker median Hz | 0.83 | 0.83 | 1.83 | 3.83 | **below** |
| flicker dominant Hz (0.5-8) | 0.50 | 0.50 | 0.48 | 2.16 | edge (no peak) |
| area sd/mean | 0.36 | 0.25 | 0.09 | 0.19 | **above** |
| height sd/mean | 0.14 | 0.11 | 0.08 | 0.14 | in |
| low-high flicker r at no lag | 0.38 | 0.24 | -0.11 | 0.58 | in (4 of 5 real ≤ 0.12) |
| tongues above 0.5 H | 3.4 | 2.3 | 1.6 | 3.6 | in |
| tongues above 0.75 H | 1.9 | 1.7 | 1.2 | 2.2 | in |
| tongue centroid rise median, H/s | 0.03 | 0.03 | 0.00 | 0.17 | in (low) |
| tongue centroid rise p90, H/s | 0.92 | 0.77 | 0.39 | 2.66 | in |
| roughness whole | 33.1 | 24.8 | 18.7 | 32.2 | in (dusk at top) |
| roughness low / mid / top | 24.7 / 17.1 / 14.6 | 17.6 / 13.6 / 10.5 | 17.7 / 7.9 / 8.6 | 23.9 / 17.3 / 19.5 | in |
| red % in the top third | 55 | 57 | 3 | 47 | **above** |
| pale % low / mid / top | 7 / 12 / 1 | 3 / 10 / 1 | 1 / 0 / 0 | 24 / 55 / 47 | in |

## Limits

- **Tongue lifetime is not measured yet.** Every real clip gives one frame (0.03 s). At 30 fps a filmed tongue changes shape more than it overlaps itself, so the overlap tracker breaks every frame. Ours gives 0.07 s. The number is printed, but nothing is concluded from it.
- **Time-of-flight rise speed is weak.** The lag of best correlation between the flicker at 0.4-0.55 H and at 0.75-0.9 H gives correlations of only 0.1-0.2 in the real clips, so the speeds in H/s it prints are not trusted. Only the correlation at no lag is used, in gap 2.
- **Block matching was tried first and failed.** Matching the flame's luma from frame to frame, the textbook optical flow, gave a median of 0 for every clip, real and ours. A filmed flame is clipped white inside, and its outline changes shape faster than it moves, so the matches scatter both ways.
- **Ours loses its dim red rim to the cut.** One rule measures both: half the 99th-percentile luma. That rule drops ours' dim red tongue tips, which the matte calls flame, and the logs' glowing texture under the see-through flame counts as flame. Gap 3 is measured on what passes the cut, so the real red share of our top is likely higher still.
- **Five clips, two of them close-ups.** Claytor Lake (dusk) is the only clip with an in-phase low-high correlation (0.58). Its camera may be pumping its exposure.

## Next

The target for the next fire rounds, in the handoff, is to raise the flicker median into 1.8-3.8 Hz and bring the area sd/mean under 0.19. Both point at the field's time evolution, the `evolve` rate and its noise's time octaves, rather than its shape. Cooling the top third's red under 47% comes next. Each round measures its film with motion.ts before any panel.
