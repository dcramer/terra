# Why the forest walk's summer night was black

2026-10-25. The question: the overnight looks review scored the forest walk's summer night 1.8 of 10, its path view a mean of 0.8 of 255. What makes it black, against the night frames of Sons of the Forest and The Witcher 3, and what lever fixes it? It informs D310 and D302.

## Measured

`node tools/sandbox/look.ts measure`, sRGB values. Terra's views are the forest walk's path and crowns vantages (`--experiment forest-walk --time 23`, summer; `--date 01-15` winter), main 5ad39590, rendered on the worker. The anchors are crops of `~/.cache/terra-review/review/anchors/forest-walk` (kept out of the repository).

| View | Mean | p5 | Under 0.08 | Red/green | Blue/green | Contrast |
|---|---|---|---|---|---|---|
| SotF night 1 | 0.057 | 0.002 | 76% | 0.69 | 1.33 | 0.29 |
| SotF night 2 | 0.156 | 0.018 | 36% | 0.82 | 1.19 | 0.22 |
| Witcher 3 moonlit | 0.193 | 0.067 | 10% | 0.37 | 1.33 | 0.06 |
| SotF night reference (`looks/sotf-forest-night.json`, 10 frames) | 0.075 | 0.006 | 68% | 0.73 | 1.29 | 0.34 |
| Terra summer night, path | 0.000 | 0.000 | 100% | 0.52 | 1.50 | 0.04 |
| Terra summer night, crowns | 0.003 | 0.000 | 100% | 0.70 | 2.56 | 0.15 |
| Terra winter night, path | 0.037 | 0.000 | 86% | 0.88 | 1.62 | 0.57 |
| Terra summer noon, path | 0.170 | 0.034 | 34% | 0.93 | 0.52 | 0.48 |

The hue is already the moon's (blue over green 1.5, the sky 2.6); the light is what is missing.

## Cause

- The moon is up: at 23:00 it stands 40 degrees up (D310), and the open grass at the path's edge takes it. Under the crowns it reaches only in flecks.
- The sky's light under a closed canopy is the irradiance volume's (D270): the share of sky seen, near nought, and the leaves' glow, a quarter of the sky's light. Deep night's ambient is 30 (fire round 25 cut it to a third, so the yard's field beyond a campfire's reach stays dark).
- The exposure is the night key, EV 9, on the small planet everywhere; only a far-seeing world's open land opens by night (1.45 stops). Under the trees nothing opens it, so the forest stood five stops under the open land with a key tuned for the open yard round a campfire.
- Not the fog: the night's haze is drawn after the exposure at 0.15 of the day's, so it stays dark too.

## Levers tried (summer night, path / crowns mean)

| Lever | Path | Crowns |
|---|---|---|
| None (main) | 0.000 | 0.003 |
| Eye opens 1.5 stops under the crowns by night, haze with it | 0.003 | 0.009 |
| 2.5 stops | 0.013 | 0.026 |
| 3.5 stops | 0.053 | 0.079 |
| 2.5 stops and the hidden share of sky lit by the sky's blue by night (0.5 and 1.0 of it) | 0.014-0.015 | 0.026 |
| 3 stops and the same | 0.028-0.030 | 0.046-0.047 |

The blue floor in the volume changed nothing measurable: by night the ambient is far under the moon, so lifting the hidden share does not show. The exposure is the lever: a dark-adapted eye, as Unreal's auto-exposure falls to its minimum EV in a dark scene. At 3.5 stops the far forest stands as trunks against the moonlit haze, as Sons of the Forest's does; the near floor is still darker than its.

## Answer

Open the eye by night under a closed canopy, 3.5 stops at full night, keyed by how open the sky over the eye is (`daylight::WOODED_NIGHT_EV`), the haze raised with it. Open land keeps the key, so the yard's campfire view is unchanged. Next, if the near floor must lift: a true histogram auto-exposure (Bevy's `AutoExposure`), which would also close the eye round a fire.
