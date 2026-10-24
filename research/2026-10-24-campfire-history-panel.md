# Campfire history panel: which rounds since 10-21 made it look worse from the camp view

2026-10-24. The question: which landed campfire commits since the 10-21 anchor (e04d47af) made the campfire look worse to the three-judge blind panel? From RDR2's camp view (the `campfire above` sheet row added in cc61dd3a), an earlier panel scored the anchor 4.72, main before cc61dd3a 3.44 and RDR2 8.25. Every round since 10-21 moved the measures toward RDR2, yet the panel's score went down.

## Answer

Two commits cost all of it. The fire climbed to a peak at e9e5b733 (4.75) and then lost 1.08 by main (cc61dd3a, 3.67):

- **688fd141, the campfire band round: −0.75.** All three judges scored it lower (5.3→4.3, 4.3→3.5, 4.8→4.3).
- **cdeae1d5, the campfire short round: −0.42.** All three judges scored it lower (4.3→3.8, 3.8→3.5, 4.3→3.8).

No other step moved every judge the same way, except one rise: cc73f0f1, the hearth mask drawn after the flames, gained 0.42. d0cc4051 (fill, −0.17) and cc61dd3a (stones and feet, +0.17) are inside the noise. Two independent renders of one campfire state scored the same to every judge: d0cc4051 and 5d7b7b4d, whose fire commit touched only buildings. So the worker's render noise is near zero, and the noise is the judges'.

Both drops won their own blind panels when they landed (688fd141 4.6 against 4.2, cdeae1d5 3.33 against 2.83). Those panels used the old low camera, at 16 degrees and 6.2 m. From above, the same changes lose.

## Method

- **Points.** 13 builds in history order. The 12 named in the brief, plus 5d7b7b4d (main before cc61dd3a), taken from the view round's render. Each older commit got cc61dd3a's `campfire above` row as a one-line throwaway patch to `tools/fire/sheet.ts`, made with read-tree/commit-tree and never pushed to main. The anchor's matched view came from the view round's 98a9292e render and was not rendered again.
- **Renders.** The workstation worker rendered `node tools/fire/sheet.ts --time dusk,night` for each point, one job each, 1 to 3 min a job. This machine's GPU was not used. Every crop was checked for the campfire model. d864a313's first dusk had none, the known worker trap, so it was rendered again (`d864a313-nomodel/` keeps the bad one).
- **Panel.** Three judges, each a default-model subagent that wrote scores only. The crop was 480x400+720+430 of f-00420, the same as the view round. Each judge got its own shuffled set: 13 dusk crops and 15 night crops (the 13 builds plus RDR2-1 and RDR2-2), unlabelled. Judge 1 had shifted its labels by one across night-4 to night-8: it scored RDR2 4.0 and d0cc4051 8.5. Asked to recheck those two images, without being told which one was which, it found and fixed the shift. Its corrected file is the one used.
- **Spread** is the max minus the min of the three judges' means. It is mostly a fixed offset: judge 2 scores about 0.5 lower throughout. A step counts as a drop when every judge moves the same way by more than the spread at either end.

## Table

Mean of the three judges, with the spread and each judge's mean (j1/j2/j3). RDR2 is night only.

| Point | What it changed | Night | Dusk | Both |
|---|---|---|---|---|
| e04d47af | 10-21 anchor | 4.33 ±1.5 | 4.17 ±1.0 | 4.25 ±1.25 (4.8/4.5/3.5) |
| 1f1f2e85 | burns along logs, glow in stack | 4.17 ±1.0 | 4.00 ±1.0 | 4.08 ±1.0 (4.3/4.5/3.5) |
| 8ad18901 | no blue, white root to red tip, tongues | 4.33 ±1.0 | 4.00 ±1.0 | 4.17 ±1.0 (4.8/3.8/4.0) |
| cc73f0f1 | hearth mask drawn after the flames | 4.83 ±1.0 | 4.33 ±0.5 | 4.58 ±0.75 (5.0/4.3/4.5) |
| 8081a126 | 12 wisps, torn tips, cooler body | 4.67 ±0.5 | 4.33 ±0.5 | 4.50 ±0.5 (4.8/4.3/4.5) |
| **e9e5b733** | logs glow on visible faces, patchy embers | **5.00 ±1.0** | **4.50 ±1.0** | **4.75 ±1.0 (5.3/4.3/4.8)** |
| **688fd141** | glow in a low band, black char above | 4.17 ±1.0 | 3.83 ±0.5 | **4.00 ±0.75 (4.3/3.5/4.3)** |
| d864a313 | deep orange light | 4.33 ±0.5 | 3.83 ±0.5 | 4.08 ±0.5 (4.3/3.8/4.3) |
| 10499671 | pale ramp, night pool falls off | 4.33 ±0.5 | 3.83 ±0.5 | 4.08 ±0.5 (4.3/3.8/4.3) |
| **cdeae1d5** | low stack 0.24 m, lower flame | 3.83 ±0.5 | 3.50 ±0.0 | **3.67 ±0.25 (3.8/3.5/3.8)** |
| d0cc4051 | fuller pale body, grey stones | 3.67 ±1.0 | 3.33 ±0.5 | 3.50 ±0.75 (3.8/3.0/3.8) |
| 5d7b7b4d | (building fire only) | 3.67 ±1.0 | 3.33 ±0.5 | 3.50 ±0.75 (3.8/3.0/3.8) |
| cc61dd3a | feet glow underneath, stone bounce | 3.83 ±0.5 | 3.50 ±0.0 | 3.67 ±0.25 (3.8/3.5/3.8) |
| RDR2 | 2 frames | 8.33 ±0.25 | - | - |

In this panel the anchor scored 4.25 overall and 4.33 at night, against the view round's 4.72. Main before cc61dd3a (5d7b7b4d) scored 3.50, against 3.44. The ranking holds.

## The drops

- **688fd141 (campfire band round).** The logs glow only in a band low down and where they cross, with plain black char between. The upper logs went from sRGB 179,93,35 to 36,22,16, and the low band from 73,34,19 to 193,101,32. The coal mound is higher, with a hot core showing through the front gap. The stones are sooted only low on their fire-facing faces. From above, the eye mostly sees the logs' upper faces, so the warm crackle along the logs (e9e5b733) becomes black sticks. All the glow then sits at their feet, as the bright yellow grid the view round's judges called a "waffle".
- **cdeae1d5 (campfire short round).** The logs lean in to 0.24 m (the teepee was 0.37 m). The flame is lower and broader: tips went from 1.26 to 0.96 of the ring's radius, with HEARTH_BODY 1.1, tongues 0.5-0.85 and leads 0.85. The coal bed is hotter, and the light's green went to 0.48. From above, the flame becomes a smaller patch on the ring. The bed's yellow chips show between the lower logs, which makes the yellow grid at the feet larger.

## Recommendation

Rework both, aiming at e9e5b733's look on main's later work. Neither reverts cleanly onto main: hearth.ts, coals.ts, fire.wesl, D415 and Campfire.glb conflict, so each is a hand-made reverse change followed by a model regeneration.

1. **Undo 688fd141's log glow first.** It is the larger loss. Give back the warm glow along the logs' upper and visible faces (e9e5b733's hot edges toward the gaps and hotter upper halves). Keep cc61dd3a's feet that glow only underneath, which address the waffle from the other side.
2. **Then undo cdeae1d5's low stack.** Put the stack back to about 0.37 m and the flame heights back to 10499671's values, or to a point between. Do this after step 1, so the yellow bed is no longer the only warm thing in view.
3. **Measure each as its own build** against main and e9e5b733 in one panel from the matched view, with this method: one shuffle per judge, scores only, and a check on any judge whose RDR2 scores land below 6. A change lands only if it beats main beyond the spread with all three judges agreeing.
4. **Judge from the matched view before landing any campfire change.** Both drops passed a low-camera panel.

## Files

Under `~/.cache/terra-review/fire/hist/`:

- Stills in history order: `desk/history-night.jpg` and `desk/history-dusk.jpg`, crops in `crops/<hour>-<sha>.png`, renders in `<sha>/sheet/`.
- Panel: `panel/j<n>/`, keys `panel/key-j<n>.txt`, scores `panel/s-j<n>.txt`, table `panel/score-h1.txt`.
- Scripts in `tools/`: `mk.sh` makes the throwaway commits, `submit.sh` submits the jobs, `gather.sh`, `contact.sh`, `panel.ts`, `score.ts`, `judge-brief.md`.
