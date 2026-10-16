# Colour balance, measured against a grey card (2026-10-15)

**Question.** The developers, about log walls: "you can adjust the look. i dont want them to be literally grey but yeah they're a bit warm, maybe everything is?" Is the whole picture warm, by how much, and where does it come from: the sun's colour (D212), the sky's light, the grade (D244, D286, D302), the tone mapping, or the materials?

**Answer.** Everything was warm, and mostly from the grade. At noon in the open a matte 18% grey came out pink, red over blue 1.33 to 1.52 in linear light, and five sixths of that was the grade's constant warmth of 0.02 in CIE x, added under the forest's canopy (D286) but applied everywhere. Moved in x alone, off the daylight locus, it tints pink rather than amber, and it turned the sky-lit shade lilac. The sun's key added a little; the sky, the atmosphere and the tone mapping (Khronos PBR Neutral, hue-preserving on greys) nothing to speak of. Fixed by D369; the wood's own tints toned down under D262. The open question, whether places should carry a warm grade as the reference games do, is D370.

## How it was measured

A throwaway remote method set three matte cubes (linear albedo 0.04, 0.18 and 0.6, roughness 1) and an 18% sphere on the ground, and toggled the grade's warmth, the sun's key colour and the saturation, one at a time. Shot on the RTX 3080 (Windows build), mean colour over each lit face averaged in linear light, sRGB shown 0 to 255; R/B and R/G are linear. The code was not kept (D249).

### Noon, the yard (sun 60° up, nothing round to bounce colour)

| What | 18% grey, sunlit face | R/B | 60% grey | 18% sphere, lit |
|---|---|---|---|---|
| As it was | 142,126,124 | 1.33 | 241,221,221 | 190,171,174 |
| Grade's warmth off | 131,128,128 | 1.05 | 238,235,237 | 178,174,179 |
| Sun's key white, grade on | 141,127,128 | 1.23 | 241,224,227 | 190,173,179 |
| Both off | 129,129,131 | 0.97 | 235,234,239 | 175,175,183 |
| Both off and saturation 1 | 129,128,131 | 0.96 | 234,234,240 | 175,175,184 |

The 4% black cube's face away from the sun, lit by the sky alone: 6,6,18 as it was, a purple; 6,14,25 with the grade's warmth off, a sky blue.

### Noon, the village's street

| What | 18% grey | R/B | 60% grey | sphere |
|---|---|---|---|---|
| As it was | 100,86,84 | 1.44 | 179,163,163 | 132,117,120 |
| Grade's warmth off | 91,88,87 | 1.10 | 168,166,168 | 122,119,123 |
| Both off | 91,90,91 | 0.99 | 167,168,172 | 121,121,128 |

The street's faces facing it pick up its orange ground's bounce (the trodden ground measured 119,71,36, red over blue 10), which is why the 18% face stays a little warm with everything off.

### 18:00, the yard (sun 6° up)

| What | 18% grey, sunlit | R/B | 60% grey |
|---|---|---|---|
| As it was | 168,111,94 | 3.50 | 238,182,171 |
| Grade's warmth off | 153,114,98 | 2.64 | 237,194,182 |
| Both off | 149,119,103 | 2.23 | 237,203,189 |

At a low sun the atmosphere's own reddening (Bevy's transmittance on the sun) gives the 18% grey a red over blue of 2.2, about a 3,500 K light against a daylight balance, which is right for a golden hour. The grade's 0.032 then took it to 3.5, salmon-pink, as its x-only shift leans magenta.

## Causes

| Cause | Share of the noon cast | Verdict |
|---|---|---|
| The grade's day warmth, `temperature` 0.02 in CIE x, `tint` 0 | about 85% | A bug in the open: a white balance moved off the daylight locus, so pink, not amber, and a lilac shade |
| The sun's key, (1.0, 0.98, 0.95) above the air (D212) | about 15% | Kept: real sunlight is a touch warm of D65 |
| Atmosphere's transmittance on the sun | none at noon, most of the evening's warmth | Physical, kept |
| Sky light and the sky's picture (IBL) | slightly cool, as it should be | Kept |
| Tone mapping (Khronos PBR Neutral) and saturation 0.82 | none on greys | Kept |
| Materials' tints | the logs' and timber's warm tints on already brown scans; the village's ground | Wood toned down (D262); ground left to its owners (D370) |

## What changed

**The grade (D369).** The day's 0.02 warmth now applies only under a canopy measured closed over the eye (`OpenOverhead`, which only the lush forest measures), where D286 tuned it and where it offsets the leaves' green light, as a camera balancing it would; the low sun's warmth (D244) moves the white point along the daylight locus (tint at -1.02 times temperature, from D65 to D75), so it reads amber. At noon in the village's street the 18% grey went from 82,69,67 (R/B 1.52) to 74,71,70 (1.11, the ground's bounce), and the 60% grey in the shade at 16:30 from lilac 157,151,189 to sky blue 142,154,191.

The looks check (D356) on the same views, before and after, on the Windows GPU:

| Place | Holding | Red/green (ref) | Lilac | Other |
|---|---|---|---|---|
| Lush forest | 14 to 14 of 18 | 1.074 to 1.074 (1.095) | 0% | unchanged, as its views are under the canopy |
| Town | 11 to 14 | 1.100 to 0.991 (1.002) | 2.0% to 0.1% | shade red/green 1.797 to 1.248 (1.128) |
| Tundra | 15 to 16 | 1.131 to 1.039 (1.047) | 2.1% to 0.3% | |
| Lake | 9 to 9 | 0.938 to 0.805 (1.028) | 0.1% to 0% | its shore's teal water now a debug tint (0.2% to 3.6%); far under sky 0.29 to 0.43 (0.45) |

**The wood (D262).** The scans themselves are grey-brown (mean colour of the colour maps: `log_bark` 105,93,80, `log_wood` 122,112,100, `rough_wood` 109,101,90, `shakes` 89,81,73). The kits then multiplied them by warm tints in sRGB: pine logs (1.0, 0.92, 0.8), 0.6 on blue once in linear light, and hewn timber (1.0, 0.86, 0.7), 0.45 on blue, so a log's bark drew about 105,84,62. Now pine logs (0.98, 0.94, 0.88), hewn timber (0.98, 0.92, 0.83), their sawn ends a little greyer, shakes (0.96, 0.96, 0.95). A log's age (D262) still takes it from the tint toward the tint's mean, so an old log is its scans' own grey-brown and a young one browner. Lit by the 16:30 sun the log gable went from 117,78,50 (R/B 5.6) to 109,82,56 (4.0); still warm in a low sun, as real wood is.

**Stone and slate.** Fieldstone's scan (Poly Haven's `stone_wall`, mean 165,148,125) is a warm sandstone of its own; its tint (0.93, 0.91, 0.88) warmed it further, and is now an even 0.91, keeping the scan's colour. Slate's scan (`grey_roof_tiles_02`) is neutral grey (122,124,123) and the kit tints it blue-grey (0.6, 0.64, 0.72), as real roofing slate is; kept. The stone plinth in shade read 64,57,60 before and 58,58,63 after.

## Recommended next (not done here)

- **The village's ground** is the warmest large thing in view: trodden dirt at 119 to 136, 71 to 87, 36 to 54 lit at noon, red over blue 6 to 11 in linear light. Real packed earth is a greyer brown. The world's ground belongs to the world work (D370).
- **The lake** measures cooler than Far Cry 5's valleys with the grade neutral. Its sky and teal water, not a grade, are what to change, if the developers keep D370 as recommended.
- **Logs in shade are near black**: a log gable in the noon shade measured 13,12,13 after (21,11,9 before). Worth measuring against a real log wall's shade: the logs' low reflectance (0.1) and dark grooves may take too much light.

Stills: `shots/colour-balance.jpg` in the session that made them (the village's street, a log house and the grey reference, noon and 16:30, before and after).
