// The perf row's noise band, alerts and bisect (D610), driven by a fake bench: a function of the
// commit giving each scene's numbers, a known regression at a known commit, so every answer is
// known another way. `node --test tools/perf`.

import assert from "node:assert/strict";
import { test } from "node:test";
import { type Bisect, found, next, start, step } from "./bisect.ts";
import { appended, band, FLOORS, historyLine, judge, mannWhitney, perJob, type Point, pointOf, readHistory, type Why } from "./history.ts";
import { alertLine, askBisect, CONFIRM, cancelBisect, closeAlert, empty, ingest, plan, type State } from "./sheriff.ts";

const commits = Array.from({ length: 30 }, (_, i) => `c${String(i).padStart(2, "0")}${"0".repeat(38)}`);
const SCENES = ["lake-play", "forest-run", "town-dusk"];

/// The fake bench: steady numbers with a small wobble by commit and run, and forest-run's
/// hitches up from about 0 to 10 from commit `bad` on.
function fakeBench(bad: number) {
  let run = 0;
  return (rev: string, why: Why): Point[] => {
    const i = commits.indexOf(rev);
    run++;
    const wobble = ((i * 7 + run * 3) % 5) / 100; // within 0 to 4%
    return SCENES.map((scene) => ({
      rev,
      job: `j${run}`,
      why,
      date: "2026-11-03",
      gpu: "RTX 3080",
      size: "1920x1080 offscreen",
      build: "release",
      scene,
      values: { median_ms: 8 * (1 + wobble / 2), p99_ms: 12 * (1 + wobble), hitches: scene === "forest-run" && i >= bad ? 9 + (run % 3) : run % 2, load_s: 30 * (1 + wobble) },
    }));
  };
}

/// The row driven to quiet: each step plans, benches with the fake and takes its points in.
function drive(s: State, tip: string, bench: ReturnType<typeof fakeBench>, history: Point[], lines: string[], most = 30): State {
  for (let n = 0; n < most; n++) {
    const p = plan(s, tip, true, Date.parse("2026-11-03T12:00:00Z"), 0);
    if ("wait" in p) return s;
    const job = { id: `job${n}`, rev: p.rev, why: p.why, end: p.end };
    // A confirm's end and a bisect's step of a flaky metric are several runs, each its own.
    const runs = p.why === "landing" ? 1 : p.why === "bisect" ? (s.bisect?.runs ?? 1) : CONFIRM;
    const points = Array.from({ length: runs }, () => bench(p.rev, p.why)).flat();
    const r = ingest({ ...s, job }, job, points, history, (g, b) => commits.slice(commits.indexOf(g) + 1, commits.indexOf(b) + 1), "2026-11-03T12:00:00Z");
    history.push(...points);
    lines.push(...r.lines);
    s = r.state;
  }
  return s;
}

test("the band is the window's median and the wider of its floor and three robust deviations", () => {
  assert.equal(band([8, 8.1], FLOORS.median_ms), undefined, "too few points: still learning");
  const tight = band([8, 8, 8, 8], FLOORS.median_ms);
  assert.deepEqual(tight, { base: 8, width: 0.24 }, "no spread: the floor, 3% of 8 ms");
  const wide = band([8, 9, 10, 8.5, 9.5], FLOORS.median_ms);
  assert.ok(wide && Math.abs(wide.width - 3 * 1.4826 * 0.5) < 1e-9, "a spread of 0.5 ms: three robust deviations");
  // One wild run in the window moves neither the median nor the band much.
  const wild = band([8, 8, 8, 8, 30], FLOORS.median_ms);
  assert.deepEqual(wild, { base: 8, width: 0.24 });
  assert.deepEqual(band([0, 1, 0, 1, 0], FLOORS.hitches), { base: 0, width: 2 }, "hitches: a count of two at least");
});

test("a value is judged against the series since its last change point", () => {
  const p = (rev: string, v: number): Point => ({ rev, job: rev, why: "landing", date: "", gpu: "g", size: "s", build: "release", scene: "x", values: { median_ms: v } });
  const before = [p("a", 8), p("b", 8), p("c", 8), p("d", 10), p("e", 10), p("f", 10)];
  assert.equal(judge(before.slice(0, 3), "median_ms", 10).change, "worse", "against the old level, 10 is worse");
  assert.equal(judge(before, "median_ms", 10, "d").change, "same", "against the level since d, it is not");
  assert.equal(judge(before, "median_ms", 7, "d").change, "better");
});

test("the window counts a job once, its runs' median, so a retrigger's tight runs do not narrow the band", () => {
  const p = (job: string, v: number, why: Why = "landing"): Point => ({ rev: job, job, why, date: "", gpu: "g", size: "s", build: "release", scene: "x", values: { median_ms: v } });
  // Five landings over the day spread ±0.6 ms, then a retrigger's five runs in one hour, alike.
  const day = [p("a", 8.0), p("b", 8.6), p("c", 7.6), p("d", 8.4), p("e", 7.8)];
  const retrigger = [8.0, 8.01, 7.99, 8.0, 8.02].map((v) => p("r", v, "retrigger"));
  assert.deepEqual(perJob([...day, ...retrigger], "median_ms"), [8.0, 8.6, 7.6, 8.4, 7.8, 8.0]);
  // As one point: deviations 0, .6, .4, .4, .2, 0, their median .3, three robust deviations
  // 3 x 1.4826 x .3 = 1.33 ms, so 8.5 ms is inside. As five, the median deviation is 0 and
  // the band falls to its 3% floor (0.24 ms): the day's spread lost, 8.5 ms an alert.
  const j = judge([...day, ...retrigger], "median_ms", 8.5);
  assert.equal(j.change, "same");
  assert.ok(j.change === "same" && Math.abs(j.width - 3 * 1.4826 * 0.3) < 1e-9, JSON.stringify(j));
});

test("the bench's appended rows become history and read back", () => {
  const committed = "date,commit,gpu\n2026-10-30,abc,RTX\n";
  const row = "2026-11-03,8daf9da4,NVIDIA GeForce RTX 3080,1920x1080 offscreen,release,forest-run,8.63,40.21,58.00,1,0.42,10,7.0,31.5,0";
  const rows = appended(`${committed}${row}\n`, committed);
  assert.deepEqual(rows, [row]);
  const point = pointOf(row, "8daf9da4cc", "j1", "landing");
  assert.deepEqual(point?.values, { median_ms: 8.63, p99_ms: 40.21, hitches: 10, load_s: 31.5 });
  const back = readHistory(`header\n${historyLine(row, "8daf9da4cc", "j1", "landing")}\n`);
  assert.deepEqual(back, [point]);
});

test("landings in their band raise nothing; a step past it is retriggered, confirmed and alerted with its range", () => {
  const bench = fakeBench(13);
  const history: Point[] = [];
  const lines: string[] = [];
  let s = empty();
  // Five landings in batches, all before the regression: learning, then quiet.
  for (const tip of [commits[1], commits[3], commits[5], commits[8], commits[10]]) s = drive(s, tip, bench, history, lines);
  assert.equal(s.alerts.length, 0, lines.join("\n"));
  assert.equal(s.good, commits[10]);
  // The next batch lands c11..c20 together: c13 brought the hitches.
  s = drive(s, commits[20], bench, history, lines, 1);
  assert.equal(s.retrigger?.rev, commits[20], "past its band: retriggered at the same commit");
  s = drive(s, commits[20], bench, history, lines, 1);
  assert.deepEqual(plan(s, commits[20], true, Date.parse("2026-11-03T12:00:00Z"), 0), { rev: commits[10], why: "baseline" }, "held: its good end benched beside it");
  assert.equal(s.alerts.length, 0, "no alert on the bad end's runs alone");
  s = drive(s, commits[20], bench, history, lines, 1);
  const alert = s.alerts.find((a) => a.scene === "forest-run" && a.metric === "hitches");
  assert.ok(alert, lines.join("\n"));
  assert.equal(s.alerts.length, 1, "only forest-run's hitches changed");
  assert.equal(alert.good, commits[10]);
  assert.equal(alert.bad, commits[20]);
  assert.equal(alert.commits, 10);
  assert.ok(alert.after >= 9 && alert.before <= 1);
  const line = alertLine(alert);
  assert.deepEqual(alert.runs, { good: CONFIRM, bad: CONFIRM });
  assert.ok((alert.p ?? 1) <= 0.01, "every bad run past every good one");
  assert.match(line, /forest-run hitches [\d.]+ → (9|10)(\.\d)? .*5 runs at its good end and 5 at its bad, p 0\.008, in c10000000\.\.c20000000 \(10 commits touching the row\); bisecting/);
});

test("the rank test's p-value: exact for a few runs, ties at their mid-rank", () => {
  const close = (a: number, b: number) => Math.abs(a - b) < 1e-9;
  assert.ok(close(mannWhitney([1, 2, 3, 4, 5], [6, 7, 8, 9, 10]), 2 / 252), "apart: the two most extreme of 252 splits");
  assert.ok(close(mannWhitney([1, 2, 3, 4, 6], [5, 7, 8, 9, 10]), 4 / 252), "one overlap");
  assert.equal(mannWhitney([0, 0, 0, 0, 0], [0, 0, 0, 0, 0]), 1, "all ties: nothing told apart");
  assert.ok(mannWhitney([9.3, 9.7, 9.5, 9.6, 9.4], [9.6, 9.5, 9.7, 9.4, 9.8]) > 0.2, "two samples of one level");
  const many = (from: number) => Array.from({ length: 12 }, (_, i) => from + i);
  assert.ok(mannWhitney(many(0), many(100)) < 1e-4, "past a million splits, the normal approximation");
});

test("a level shift of the box alone, both ends moved together, is no alert, and the band re-bases at it", () => {
  const history: Point[] = [];
  const lines: string[] = [];
  const steady = fakeBench(99);
  let s = empty();
  for (const tip of commits.slice(1, 6)) s = drive(s, tip, steady, history, lines);
  assert.equal(s.good, commits[5]);
  // From now on every lake-play run is 5% slower, whatever the commit: the box drifted.
  const drifted = (rev: string, why: Why) => steady(rev, why).map((p) => (p.scene === "lake-play" ? { ...p, values: { ...p.values, median_ms: (p.values.median_ms ?? 0) * 1.05 } } : p));
  lines.length = 0;
  s = drive(s, commits[6], drifted, history, lines);
  assert.equal(s.alerts.length, 0, lines.join("\n"));
  assert.ok(history.some((p) => p.why === "baseline" && p.rev === commits[5]), "its good end benched beside it");
  assert.ok(lines.some((l) => /lake-play median_ms .* the box's level, not the code: its good end c05000000 ran .* so no alert; its band re-based at c06000000/.test(l)), lines.join("\n"));
  assert.equal(s.since["lake-play median_ms"], commits[6]);
  assert.equal(s.good, commits[6], "the code was fine: a good end");
  // The landings after are judged at the box's new level: no retrigger, no alert.
  lines.length = 0;
  for (const tip of commits.slice(7, 10)) s = drive(s, tip, drifted, history, lines);
  assert.ok(!lines.some((l) => /retriggered/.test(l)), lines.join("\n"));
  assert.equal(s.good, commits[9]);
});

test("ends that differ past the floor but not surely are benched once more, then let go", () => {
  const at = (rev: string, why: Why, values: number[]): Point[] =>
    values.map((v, i) => ({ rev, job: `${rev.slice(0, 3)}-${why}-${i}`, why, date: "", gpu: "g", size: "s", build: "release", scene: "lake-play", values: { median_ms: v } }));
  const history = [...at(commits[1], "landing", [9.3]), ...at(commits[2], "landing", [9.28]), ...at(commits[3], "landing", [9.32])];
  let s: State = { ...empty(), benched: commits[3], good: commits[3] };
  const take = (rev: string, why: Why, values: number[]) => {
    const job = { id: why, rev, why };
    const r = ingest({ ...s, job }, job, at(rev, why, values), history, () => [rev], "2026-11-03T12:00:00Z");
    history.push(...at(rev, why, values));
    s = r.state;
    return r.lines;
  };
  take(commits[4], "landing", [9.9]);
  assert.equal(s.retrigger?.phase, "bad");
  take(commits[4], "retrigger", [9.9, 9.4, 9.85, 9.8, 9.75]);
  assert.equal(s.retrigger?.phase, "good");
  // Medians 9.45 and 9.8 (+3.7%), past the 3% floor, but runs overlap: unclear, once more.
  const unclear = take(commits[3], "baseline", [9.3, 9.45, 9.85, 9.4, 9.5]);
  assert.ok(unclear.some((l) => /unclear: .*both ends benched once more/.test(l)), unclear.join("\n"));
  assert.deepEqual(plan(s, commits[4], true, Date.parse("2026-11-03T13:00:00Z"), 0), { rev: commits[4], why: "retrigger" });
  take(commits[4], "retrigger", [9.9, 9.45, 9.85, 9.8, 9.75]);
  const last = take(commits[3], "baseline", [9.3, 9.5, 9.85, 9.4, 9.45]);
  assert.ok(last.some((l) => /unclear after 2 rounds: .* so no alert/.test(l)), last.join("\n"));
  assert.equal(s.alerts.length, 0);
  assert.equal(s.retrigger, undefined, "let go");
});

test("a one-off spike that does not hold on its retrigger is noise", () => {
  const history: Point[] = [];
  const lines: string[] = [];
  const steady = fakeBench(99);
  let s = empty();
  for (const tip of commits.slice(1, 6)) s = drive(s, tip, steady, history, lines);
  let spiked = false;
  const spiky = (rev: string, why: Why) =>
    steady(rev, why).map((p) => {
      if (spiked || p.scene !== "town-dusk") return p;
      spiked = true;
      return { ...p, values: { ...p.values, p99_ms: 40 } };
    });
  s = drive(s, commits[6], spiky, history, lines);
  assert.equal(s.alerts.length, 0, lines.join("\n"));
  assert.ok(lines.some((l) => /town-dusk p99_ms .* did not hold on its retrigger/.test(l)));
  assert.equal(s.good, commits[6]);
});

test("a row's busy column marks its point; a row without one is quiet", () => {
  const row = "2026-11-03,4688fd3d7,NVIDIA GeForce RTX 4090,1920x1080 offscreen,release,forest-run,8.88,12.01,57.10,20,0.70,20,6.0,31.5,0";
  assert.equal(pointOf(`${row},1`, "4688fd3d7", "j1", "landing")?.busy, true);
  assert.equal(pointOf(`${row},0`, "4688fd3d7", "j1", "landing")?.busy, undefined);
  assert.equal(pointOf(row, "4688fd3d7", "j1", "landing")?.busy, undefined, "a row from before the column");
  const back = readHistory(`header\n${historyLine(`${row},1`, "4688fd3d7", "j1", "landing")}\n`);
  assert.equal(back[0]?.busy, true);
});

test("a scene taken on a busy machine is recorded but raises nothing, on a landing or a retrigger", () => {
  const history: Point[] = [];
  const lines: string[] = [];
  const steady = fakeBench(99);
  let s = empty();
  for (const tip of commits.slice(1, 6)) s = drive(s, tip, steady, history, lines);
  const good = s.good;
  // forest-run's 20 hitches on a busy box (the CPU clock collapsing), its MISSED an upper bound.
  const busy = (rev: string, why: Why) => steady(rev, why).map((p) => (p.scene === "forest-run" ? { ...p, busy: true, values: { ...p.values, hitches: 20 } } : p));
  lines.length = 0;
  s = drive(s, commits[6], busy, history, lines);
  assert.equal(s.alerts.length, 0, lines.join("\n"));
  assert.equal(s.retrigger, undefined, "nothing to retrigger: the busy scene was not judged");
  assert.ok(lines.some((l) => /c06000000 forest-run was taken on a busy machine .*recorded, not judged, so no alert/.test(l)), lines.join("\n"));
  assert.ok(lines.some((l) => /c06000000 within every band it judged/.test(l)), lines.join("\n"));
  assert.equal(s.benched, commits[6]);
  assert.equal(s.good, good, "a landing with a scene unjudged is no good end of a range");
  assert.ok(history.some((p) => p.rev === commits[6] && p.busy), "recorded in the history");
  // A suspect whose retrigger came out busy is neither confirmed nor alerted.
  const spikeThenBusy = (rev: string, why: Why) =>
    steady(rev, why).map((p) => (p.scene === "town-dusk" ? { ...p, busy: why === "retrigger" || undefined, values: { ...p.values, p99_ms: 40 } } : p));
  lines.length = 0;
  s = drive(s, commits[7], spikeThenBusy, history, lines);
  assert.equal(s.alerts.length, 0, lines.join("\n"));
  assert.ok(lines.some((l) => /town-dusk p99_ms .* not confirmed: its retrigger was taken on a busy machine, so no alert/.test(l)), lines.join("\n"));
});

test("the bisect halves the range: its choice of commits and the first bad commit", () => {
  const range = commits.slice(11, 21); // c11..c20, the bad c13
  let b: Bisect = start("forest-run", "hitches", commits[10], commits[20], range, { goodValue: 0.5, badValue: 10 });
  const chosen: string[] = [];
  const value = (rev: string) => (commits.indexOf(rev) >= 13 ? 10 : 0);
  for (let n = 0; n < 10; n++) {
    const pick = next(b);
    if (!pick) break;
    chosen.push(pick.rev.slice(0, 3));
    b = step(b, pick.rev, value(pick.rev));
  }
  // Indices 0..9 of c11..c20: the middle of (-1, 9) is 4 (c15, bad), of (-1, 4) is 1 (c12,
  // good), of (1, 4) is 2 (c13, bad), and c13 is left alone.
  assert.deepEqual(chosen, ["c15", "c12", "c13"]);
  assert.equal(found(b).first, commits[13]);
  assert.equal(b.benches, 3);
});

test("a bisect measures its ends first when started by hand, skips a commit with no numbers, and stops at its cap", () => {
  const range = commits.slice(1, 17); // c01..c16, the bad c09
  let b = start("forest-run", "hitches", commits[0], commits[16], range, {}, 4);
  assert.deepEqual(next(b), { rev: commits[0], end: "good" });
  b = step(b, commits[0], 0);
  assert.deepEqual(next(b), { rev: commits[16], end: "bad" });
  b = step(b, commits[16], 10);
  const mid = next(b);
  assert.equal(mid?.rev, commits[8], "the middle of c01..c16");
  b = step(b, commits[8], undefined);
  assert.ok(b.skipped.includes(commits[8]));
  const neighbour = next(b);
  assert.ok(neighbour && [commits[7], commits[9]].includes(neighbour.rev), "a neighbour of the skipped commit");
  b = step(b, neighbour.rev, commits.indexOf(neighbour.rev) >= 9 ? 10 : 0);
  assert.equal(next(b), undefined, "four benches: its cap");
  const f = found(b);
  assert.equal(f.first, undefined);
  assert.ok(f.left.includes(commits[9]) && f.left.length > 1, "narrowed, the bad commit among those left");
});

test("a bisect cancelled by hand drops its step in flight and starts the next waiting", () => {
  const range = commits.slice(1, 17);
  const first = askBisect(empty(), "forest-run", "hitches", commits[0], commits[16], range);
  const both = askBisect(first, "lake-play", "p99_ms", commits[0], commits[16], range);
  const s: State = { ...both, job: { id: "20261103-100250-daf6", rev: commits[0], why: "bisect", end: "good" } };
  const got = cancelBisect(s);
  assert.equal(got.job, "20261103-100250-daf6", "its step in flight, for the box to cancel");
  assert.equal(got.state.job, undefined);
  assert.equal(got.state.bisect?.scene, "lake-play", "the next waiting starts");
  assert.equal(got.state.waiting.length, 0);
  assert.match(got.lines[0], /forest-run hitches .* cancelled by hand/);
  const landing: State = { ...first, job: { id: "20261103-104001-20ba", rev: commits[16], why: "landing" } };
  const kept = cancelBisect(landing);
  assert.equal(kept.job, undefined, "a landing's bench in flight is the row's, not the bisect's");
  assert.equal(kept.state.job?.id, "20261103-104001-20ba");
  assert.equal(kept.state.bisect, undefined);
  assert.deepEqual(cancelBisect(empty()).lines, [], "nothing to cancel");
});

test("the whole row on the fake bench bisects the alert to its first bad commit and names it", () => {
  const bench = fakeBench(13);
  const history: Point[] = [];
  const lines: string[] = [];
  let s = empty();
  for (const tip of [commits[1], commits[3], commits[5], commits[8], commits[10], commits[20]]) s = drive(s, tip, bench, history, lines);
  const alert = s.alerts.find((a) => a.scene === "forest-run");
  assert.equal(alert?.state, "found", lines.join("\n"));
  assert.equal(alert?.first, commits[13]);
  assert.ok(lines.at(-1)?.includes("first bad c13000000"), lines.at(-1));
  assert.equal(s.bisect, undefined);
});

test("an alert closed as invalid drops its bisect, re-bases its band and raises nothing again", () => {
  const bench = fakeBench(13);
  const history: Point[] = [];
  const lines: string[] = [];
  let s = empty();
  for (const tip of [commits[1], commits[3], commits[5], commits[8], commits[10]]) s = drive(s, tip, bench, history, lines);
  s = drive(s, commits[20], bench, history, lines, 3);
  const a = s.alerts.find((a) => a.scene === "forest-run");
  assert.equal(a?.state, "bisecting", lines.join("\n"));
  const inFlight: State = { ...s, job: { id: "jb", rev: commits[15], why: "bisect" } };
  assert.match((closeAlert(inFlight, "forest-run/hitches", "invalid", " ", history, "2026-11-03T16:00:00Z") as { error: string }).error, /needs a reason/);
  assert.match((closeAlert(inFlight, "town-dusk", "invalid", "x", history, "2026-11-03T16:00:00Z") as { error: string }).error, /no open alert is town-dusk \(open: forest-run\/hitches@c20000000\)/);
  const got = closeAlert(inFlight, "forest-run/hitches", "invalid", "A/B: 9.71 ms at c10, 9.63 ms at c13", history, "2026-11-03T16:00:00Z");
  assert.ok(!("error" in got));
  assert.equal(got.job, "jb", "its bisect's step in flight, for the box to cancel");
  assert.equal(got.state.bisect, undefined);
  assert.equal(got.state.job, undefined);
  const closed = got.state.alerts.find((x) => x.id === a.id);
  assert.deepEqual(closed?.closed, { as: "invalid", reason: "A/B: 9.71 ms at c10, 9.63 ms at c13", when: "2026-11-03T16:00:00Z" });
  assert.match(alertLine(closed as NonNullable<typeof closed>), /^· perf .*; closed invalid 2026-11-03 16:00 UTC: A\/B: 9\.71 ms/);
  assert.equal(got.state.since["forest-run hitches"], commits[20], "the band from the level its bad end saw");
  assert.match((closeAlert(got.state, a.id, "accepted", "x", history, "2026-11-03T16:00:00Z") as { error: string }).error, /none is open/, "closed once");
  // The level stays where the alert saw it: the next landings are in their band.
  s = got.state;
  lines.length = 0;
  for (const tip of [commits[22], commits[25], commits[28]]) s = drive(s, tip, bench, history, lines);
  assert.equal(s.alerts.length, 1, lines.join("\n"));
  assert.equal(s.good, commits[28], lines.join("\n"));
});

test("a flaky metric's bisect judges every run: a step is bad when one run hitched past the good end's worst", () => {
  // Bad from c07 on: about 8 hitches in one run of three, else 0 or 1 (2026-11-03's forest-run).
  let run = 0;
  const runs = (rev: string) => Array.from({ length: 10 }, () => (commits.indexOf(rev) >= 7 && ++run % 3 === 0 ? 8 : run++ % 2));
  const range = commits.slice(1, 13);
  let b = start("forest-run", "hitches", commits[0], commits[12], range, {}, 7, undefined, { scenes: ["forest-run", "lake-play"], runs: 5 });
  for (let n = 0; n < 7; n++) {
    const pick = next(b);
    if (!pick) break;
    b = step(b, pick.rev, runs(pick.rev), FLOORS.hitches.abs);
  }
  assert.equal(b.goodWorst, 1);
  assert.equal(found(b).first, commits[7], JSON.stringify(b));
  // A mean would have called a bad step good: 8 in one run of ten is a mean under the floor.
  assert.ok(8 / 10 < (FLOORS.hitches.abs ?? 0));
});

test("a regression in a batch of one commit names it without a bisect", () => {
  const bench = fakeBench(11);
  const history: Point[] = [];
  const lines: string[] = [];
  let s = empty();
  for (const tip of [commits[1], commits[3], commits[5], commits[8], commits[10], commits[11]]) s = drive(s, tip, bench, history, lines);
  const alert = s.alerts.find((a) => a.scene === "forest-run");
  assert.equal(alert?.state, "found", lines.join("\n"));
  assert.equal(alert?.first, commits[11]);
  assert.equal(s.bisect, undefined);
  assert.equal(history.filter((p) => p.why === "bisect").length, 0, "no bench spent on it");
});

test("the plan rests after a bench, takes a batch at the tip once, and interleaves landings with a bisect", () => {
  const now = Date.parse("2026-11-03T12:00:00Z");
  const s = { ...empty(), benched: commits[3], lastEnded: "2026-11-03T11:50:00Z" };
  assert.ok("wait" in plan(s, commits[9], true, now, 20), "resting");
  assert.deepEqual(plan(s, commits[9], true, now, 5), { rev: commits[9], why: "landing" }, "the batch's tip, once");
  assert.ok("wait" in plan({ ...s, benched: commits[9] }, commits[9], true, now, 5), "benched already");
  assert.ok("wait" in plan(s, commits[9], false, now, 5), "nothing it reads changed");
  const bisecting = { ...s, bisect: start("forest-run", "hitches", commits[0], commits[3], commits.slice(1, 4), { goodValue: 0, badValue: 9 }) };
  assert.equal((plan({ ...bisecting, lastWhy: "landing" }, commits[9], true, now, 5) as { why: string }).why, "bisect", "after a landing, the bisect's turn");
  assert.equal((plan({ ...bisecting, lastWhy: "bisect" }, commits[9], true, now, 5) as { why: string }).why, "landing", "after a bisect step, the landing's");
});
