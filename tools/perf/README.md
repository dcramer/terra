# tools/perf

The perf row (D610): the play scenes benched once at each landing batch on main by the render worker, their numbers kept by commit, a change past its noise band retriggered, alerted and bisected to its first bad commit, unattended. As Chromium's perf bots chart every change and Pinpoint bisects a regression ([bisects](https://chromium.googlesource.com/chromium/src/+/HEAD/docs/speed/bisects.md)), and Mozilla's Perfherder alerts on a sustained change and its sheriffs retrigger a suspect job to confirm it ([perf sheriffing](https://firefox-source-docs.mozilla.org/testing/perfdocs/perf-sheriffing.html)). Ideas only; no code is copied.

```bash
node tools/perf/perf.ts                     # the row: last benched, in flight, alerts, its last lines
node tools/perf/perf.ts history [n]         # the history's last rows
node tools/perf/perf.ts main <rev>          # main's numbers at or just before a commit
node tools/perf/perf.ts ab <csv> [--base <rev>]   # your bench's rows against main's side, no bench of main
node tools/perf/perf.ts bisect forest-run hitches <good> <bad>   # a bisect by hand
```

- **The row** is `row.json`: the scenes (lake-play, forest-run, town-dusk), one run each, what they read (a landing touching none of it is not benched), the rest between two of its benches (20 min, so other jobs' benches go between and it never holds the box with benches back to back, D319), the bisect's cap (5 benches) and how often it looks (5 min).
- **Where it runs**: in the render worker's daemon (`tools/render-worker/daemon.ts` calls `worker.ts`'s `tick`). It reads origin/main's tip by `git ls-remote` of the public repository, queues `cargo xtask bench <scenes> --runs 1 --record` at the tip as a bench job of the box's own queue (`by: perf row (…)`, so the planner gives it the box alone, D445), and when the job ends reads the rows its bench appended to `research/benchmarks.csv` (collected into its `out/`). Landings that come while one runs are taken together at the next tip. `TERRA_PERF=0` in the box's `.env` turns it off. Its code is the worker's, so a change here reaches the box with `cargo xtask remote-render --update`.
- **What it keeps**, in `~/terra-render/perf/` on the box: `history.csv` (the bench's own columns after `rev,job,why`), `state.json`, `log.txt` every line it said, `alerts.txt`.
- **The noise band** (`history.ts`): a scene's metric (median, p99, hitches, load) against the window of the 12 points before it on the same card, picture and build, since its last change point: its median, and the wider of a floor (median 3%, p99 5%, load 10%, hitches 2) and three robust deviations (1.4826 times the median absolute deviation). Fewer than 3 points: still learning, nothing alerts.
- **An alert** (`sheriff.ts`): a landing past its band is benched again at the same commit; when the retrigger is past it too, the same way, the alert names the scene, the metric, before and after, and the range from the last landing in every band (`✗ perf … forest-run hitches 0 → 10 (+…), band ±2, in a..b (n commits touching the row)`). A change for the better is said and not bisected.
- **The bisect** (`bisect.ts`): over the range's commits touching what the row reads, oldest first, by halving; a step is bad when its value is nearer the bad end's than the good's; a bench without numbers skips its commit (a neighbour next, as `git bisect skip`); capped at 5 benches, a range left wider named with its commits. Its result goes in the alert's line. Landings and a bisect take turns.
- **An A/B from history**: `perf.ts ab` reads main's side at your base from the history: the newest benched commit at or before it, and whether the commits between touch what the row reads (none: its numbers stand for your base). Bench only your own side (`cargo xtask remote-render --dirty --collect research/benchmarks.csv -- cargo xtask bench <scenes> --runs 1 --record`) and compare its csv.

`perf.test.ts` drives the decisions with a fake bench (a known regression at a known commit); `worker.test.ts` the row on a scratch box with a local repository for origin. Node 22.18 or later, git; no dependencies.
