# Review

Blind visual judging by fresh subagent judges without position bias (D472): each judge sees fixed anchors and ONE build, scores it 0-10, and builds are compared by their judges' means. Node 22.18 or later; no dependencies.

```bash
node tools/review/judge.ts --brief <brief.md> --set <set> --anchors <anchors> --judges <n> --out <panel> [--builds a,b] [--seed n]
node tools/review/judge.ts --score <panel> [--compare a:b,a+a2:b]       # every build against the first by default
node tools/review/judge.ts --brief ... --set ... --out <panel> --judges <n> --builds b --add   # more judges, same panel
node tools/review/judge.ts ... --choice a:b                               # forced choice, side by side (measured, not for verdicts)
```

- **A set** is `<set>/<build>/<view>.png|jpg`, the same views in every build (e.g. `dusk.png`, `night.png`). Enter one build twice under two names for an A/A check.
- **Anchors** are `<anchors>/anchors.json`, `[{ "file", "score", "what" }]` with the files beside it, shown first at their fixed scores. Images of other games stay out of the repository: keep sets and anchors in `~/.cache/terra-review/review/` (the fire set: `anchors/fire`, RDR2 at 9 and an early prototype at 2).
- **A brief** says what the images show and what to score (`briefs/`); the tool adds the files, the anchors and the output rule.
- **Running it.** The tool spawns nothing. It writes `<panel>/jNN/prompt.md` per judge, lists them in `<panel>/judges.txt` and keeps who saw what in `<panel>/key.json`. Run one fresh default-model subagent per judge with the prompt `Read <panel>/jNN/prompt.md and follow it exactly.` (several at once is fine; each takes ~15 s); each writes `jNN/scores.txt`. Then `--score`, which writes `score.txt` and names any judge whose file is missing or malformed, to run again.
- **Faults.** An absolute judge then writes `jNN/faults.txt`, its three worst faults an image (`<image> <fault>`), after its scores and apart from them, so the statistics never read it. `--score` gathers them into `<panel>/faults.txt`, by build and view, each line with its judge and that judge's score of the view: read it to learn why a panel went against a change, before tuning again.
- **Reading a score.** `M6 - main: +0.29 [+0.08, +0.50] p 0.009 -> M6 better; sd 0.30, n 16/8 shows 0.42, 0.25 needs 23 a build`: the difference, its 95% Welch interval and p (a permutation p beside it), the verdict (the interval clear of nought), the judges' spread, the least difference this panel shows at 80% power, and the judges a 0.25 difference needs. Fix the number of judges before scoring; adding judges until a difference shows inflates false wins.
- **stats.ts** holds the statistics (Welch's t, permutation, Wilson, binomial, judges needed), each tested at known values in `judge.test.ts`.
- **Scale.** Absolute scores are compressed: a difference within-judge rankings call 1.2 points is about 0.25 here (D472).
