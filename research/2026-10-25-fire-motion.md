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

## Tongues (fire round g18)

The reviewers' next fault: our tongues pump up and down instead of leaning, splitting and shedding. `tools/fire/tongues.ts` now measures those four on the same clips. A tongue is a peak of the flame above half its height that stands at least 0.1 H over the notch joining it to a taller one. The flame is first closed and its holes filled, then swept as a component tree. A free blob counts as a tongue by size alone. Tongues are tracked as storm-cell trackers (TITAN) track cells: each track's next footprint is forecast from its mean step, then matched by overlap. A forecast that two pieces fill is a split, and two forecasts filled by one piece are a merger. A shed is a free blob that comes off above the attached tongue it left, or a whole tongue that comes free. A split or shed counts only if its branch never joins back. Each rate is per tongue a second. Lean is the angle from vertical of the line from a tongue's foot to its tip.

**Check (known-good).** Each real clip was slowed to a third by showing each frame three times. The slowed clips read 2.9-3.2 times the lifetime and 0.30-0.35 of each rate, and lean is unchanged. `tongues.test.ts` holds this on masks whose lifetimes, sheds and splits are known. On the real clips each measure's spread is stated in the table, as most over least: lifetime 3.0, sheds 2.3, splits 2.0, lean 3.2. The three standing-view campfires agree more closely on splits (0.99-1.09).

Ours is main 92e3c712, filmed fresh in the g12 view (job `remote/20261025-161821-629c`; its fire code is g15's L3b, and the g12 measures match L3b's to two decimals). The outputs are in `fire/g18/motion/`.

| Measure | main dusk | main night | real min | real max | spread | standing-view 3 | ours |
|---|---|---|---|---|---|---|---|
| lifetime mean, s | 0.14 | 0.07 | 0.04 | 0.12 | 3.0x | 0.05-0.12 | dusk **above** |
| sheds per tongue a second | 0.92 | 0.67 | 0.75 | 1.70 | 2.3x | 0.75-1.70 | night **below** |
| splits per tongue a second | 1.73 | 2.40 | 0.99 | 1.97 | 2.0x | 0.99-1.09 | night **above**; 1.6-2.2x the standing view |
| lean median, deg | 9.5 | 8.7 | 7.6 | 24.5 | 3.2x | 9.6-24.5 | in, low (under the standing view) |
| (merges per tongue a second) | 3.86 | 4.50 | 3.74 | 6.32 | | 3.74-4.06 | in |

Splits are furthest from the real range. Ours forks about twice as often as the standing-view campfires do, and most branches later merge into another tongue. It sheds least and leans least, and at dusk its tongues live longest. These numbers describe lobes that rise and fall past each other in place. Real tongues fork less often, and their tips come off and rise away.

**The splits bar for our view (fire round g21): 1.2 or fewer, on both dusk and night.** Our film is the campfire from above, a camp's eye standing a few metres off. Per clip, splits read 1.06 and 1.09 (Virginia State Parks 1 and 2), 0.99 (Claytor Lake), 1.19 (Feuer) and 1.97 (Fire.ogv). Feuer is a close view, but its fire is built like ours: 1.6 tongues above half height and 12 born a second, against ours' 2.0 and 10.5 and the standing views' 1.6-3.0 and 12-19. Fire.ogv is a close view of a larger, busier fire, with 3.6 tongues above half height and 38 born a second. Crowded tongues fork into each other, so its rate belongs to a different fire, not to a different camera. Four clips of a fire built like ours, from both distances, read 0.99-1.19, so the bar is their top. Taking the whole range, 1.97, would pass main (1.98/1.87 over 20 s) and hold nothing. The same four clips give lifetime 0.04-0.12 s, sheds 0.75-1.70 and lean 7.6-24.5 deg, so those targets stand. Those clips give 120-190 tracks over their 10 s, and our 20 s films 230-280, so both sides carry counting noise: a rate of ours over 20 s is good to about ±0.2 (g20).

## Limits

- **Tongue lifetime is near the frame rate.** Real tongues live 0.04-0.12 s on average, one to four frames at 30 fps, so the mean is used, not the median. Shed blobs last 1-2 frames before they fade under the cut, so their rise speed (printed) is not trusted.
- **Time-of-flight rise speed is weak.** The lag of best correlation between the flicker at 0.4-0.55 H and at 0.75-0.9 H gives correlations of only 0.1-0.2 in the real clips, so the speeds in H/s it prints are not trusted. Only the correlation at no lag is used, in gap 2.
- **Block matching was tried first and failed.** Matching the flame's luma from frame to frame, the textbook optical flow, gave a median of 0 for every clip, real and ours. A filmed flame is clipped white inside, and its outline changes shape faster than it moves, so the matches scatter both ways.
- **Ours loses its dim red rim to the cut.** One rule measures both: half the 99th-percentile luma. That rule drops ours' dim red tongue tips, which the matte calls flame, and the logs' glowing texture under the see-through flame counts as flame. Gap 3 is measured on what passes the cut, so the real red share of our top is likely higher still.
- **A 6 s film is too short for the tongue rates (fire round g20).** It gives 60-100 tracks, and a build's rates moved by up to 40% between its 6 s and 20 s films. Filmed for 20 s (230-280 tracks, `fire/g20/`), main, g19's A and g19's D read splits 1.98/1.87, 1.83/1.63 and 1.58/1.36 (dusk/night). Their 6 s films had read 1.73/2.40, 1.32/1.00 and 1.21/1.24. D's sheds read 1.15/1.29 over 20 s against 0.87/0.80 over 6 s, which reverses g19's ranking of its builds. Film ours for 20 s and pass `--seconds 20`, since motion.ts reads 10 s unless told. Even over 20 s, the lifetime mean still moves between 0.08 and 0.18 s from one third of a film to the next. Area sd/mean grows with the film's length (main at dusk: 0.18 over 6 s, 0.20 over 10 s, 0.24 over 20 s), so compare it with main's on films of the same length. The real clips are 10 s.
- **Five clips, two of them close-ups.** Claytor Lake (dusk) is the only clip with an in-phase low-high correlation (0.58). Its camera may be pumping its exposure.

## Next

The target for the next fire rounds, in the handoff, is to raise the flicker median into 1.8-3.8 Hz and bring the area sd/mean under 0.19. Both point at the field's time evolution, the `evolve` rate and its noise's time octaves, rather than its shape. Cooling the top third's red under 47% comes next. Each round measures its film with motion.ts before any panel.
