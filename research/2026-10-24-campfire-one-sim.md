# Campfire as one whole-fire sim on a few cards: a blind test

2026-10-24. The question: does the campfire read better to our three-judge blind panel if its flame is a few cards playing one high-resolution Mantaflow sim of the whole fire, with the procedural columns, tongues, leads and wisps off? Shipped campfires mostly do it this way. Ours draws about 44 procedural cards, each a torn slice of a small baked flipbook. About fifteen tuning rounds moved our measures toward RDR2, but the panel scored the 10-21 campfire (e04d47af) above today's main.

## Answer

No. As baked here, the one-sim flame scored lowest of the three builds. All three judges gave it 3.5, against 4.06 for main and 4.39 for the 10-21 anchor. All three said the same thing: "a soft orange haze/cloud with no shape". A whole fire rendered as one emissive volume reads as a glowing cone. Main's torn slices at least read as separate tongues. The judges' second and third gaps were the same for every build: near-black stones that the firelight misses, and an even checker of glow on the logs. Those parts were identical in all three builds, and they are what holds every build near 4.

## Method

- **Bake.** We used the existing baker (tools/fire/bake/bake.ts, Blender 5.2 Mantaflow and Cycles on the CPU). It ran locally, because the render worker has no Blender (`tools/blender/run.sh --find` fails there).
  - Fuel came from 14 cylinders along the upper surfaces of `Logs::CAMPFIRE`'s seven logs (two segments a log, the upper one hotter), a faint coal disk and a small smoking source at the apex. The recipe generator is `~/.cache/terra-review/fire/onesim/tools/recipe.ts`.
  - Domain 1.0 x 1.0 x 1.1 m at resolution 128 with noise upres 2, so about 4.3 mm cells (main: 11 mm, noise 2). Frames 512 px over 1.1 m (main: 256 px over 1.4 m), 64 frames with a 16-frame crossfade, 4 samples.
  - The bake took 20 min: sim 451 s, render 706 s. The baker's line: cover 13%, reach 88%, seam 1.03. Flow gain was 2.55, so the motion vectors were worse than none and the set crossfades.
  - Two drafts at half resolution came first. The first had too much fuel and filled the frame as one column. The second was halved.
- **Draw (uncommitted, `ONE_SIM` in fire.wesl).** Two camera-facing upright cards of the set, each at its own moment. Each is the baked frame's size times 1.3, because the sim burned narrower than the model's logs. The second card stands 0.12 m behind the first at 0.92 of its size. Cover is gained 1.7, because the first render read as a thin haze.
  - Every other hearth flame card returns nowhere. The glow inside the stack, the glow card, logs, coals, stones, light, smoke and embers are as on main.
  - The other five fire sets were doubled with a point filter, because the array texture needs one size for every layer.
- **Panel.** Main (49b46148, the same assets as d0cc4051) and the spike were rendered on the local 3080 with `tools/fire/sheet.ts` (frame 330; both sheets 7/12). The anchor's frames were reused from fire/anchors/e04d47af.
  - We followed the fire HANDOFF's method: `trimake.ts` (Fisher-Yates per hour; noon, dusk and night for the three builds, and RDR2's two frames at night), three independent judges on `judge-brief.md`, then `score.ts`.

## Numbers (tri-o1)

| Build | All hours | Judges | Night only |
|---|---|---|---|
| Anchor e04d47af | 4.39 | 4.33 / 4.33 / 4.50 | 4.33 |
| Main | 4.06 | 4.17 / 4.00 / 4.00 | 4.17 |
| One-sim spike | **3.50** | 3.50 / 3.50 / 3.50 | 3.50 |
| RDR2 (night) | 8.58 | spread 0.3 | |

Positions (x - anchor) / (RDR2 - anchor): main -0.08, spike -0.21. The spike's gap to main (0.56) is more than three times the judges' spread (0.17), so it is a real loss and not noise. Its night measures were inside the campfire class (red+orange 85%, yellow 13%, G/R 0.60), so the measures again could not tell what the judges saw.

## Files

Everything is under `~/.cache/terra-review/fire/onesim/`:

- stills: `main/sheet/<hour>/f-00330.png` and `spike2/sheet/<hour>/f-00330.png`;
- the night crop: `desk/campfire-main-vs-onesim-vs-rdr2-night.jpg` (main | spike | RDR2);
- the blind set: `tri-o1/`, with its key in `tri-key-o1.txt`;
- the reviews and scores: `review-o1-j{1,2,3}.md`, `s-o1-j*.txt` and `score-o1.txt`;
- the spike's code: `onesim.patch` (shader, recipe and sidecar);
- the baked set and its contact sheet: `set/`.

## Recommendation

- **Keep main's procedural hearth.** Don't move the campfire to whole-fire sim cards on this evidence.
- **Retry the one-sim only with a sharper sim**, if at all. The haze came from the sim, not the card count. A Mantaflow volume at this resolution, rendered as emission, has no hard tongue edges. Shipped whole-fire flipbooks have them because they come from Houdini or EmberGen, with a flame-mask threshold and high base resolution. A retry would need:
  - base resolution 256 or more with no noise upres (about 1.5-2 h of CPU locally; install Blender on the worker first);
  - a much lower fuel rate, so the flames come out thin;
  - the cover taken through a threshold on the flame mask, so tongue edges come out hard.
- **Before any more flame rounds, work on the stones and the log glow.** Every build's review names them, so a better flame has not moved the score. The stones are near-black on the fire-facing side with no contact light. The logs glow in a regular checker; they need continuous cracked char with the glow in the bed under them.
- **Keep the 10-21 anchor in every panel.** It still beats main, so the last week's flame tuning has not been seen as a gain.
