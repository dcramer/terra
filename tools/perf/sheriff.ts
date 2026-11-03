// The perf row's decisions (D610), pure: what to bench next, and what a bench's points say.
// A landing is benched once per batch (the landings that came while the last ran, taken
// together at main's tip, as a busy Buildbot builder collapses its pending requests); a
// landing past its band is benched again at the same commit (a retrigger, as Perfherder's
// sheriffs confirm a suspect); a confirmed change raises an alert naming the scene, the metric,
// before and after and the range, and a regression is bisected (bisect.ts) to its first bad
// commit, which the alert then names. One perf bench at a time, a rest between two, so the row
// never holds the box with benches back to back (D319, D445).

import { type Bisect, CAP, found, next as nextStep, type Options, start, step } from "./bisect.ts";
import { changeLine, FLOORS, type Judged, judge, METRICS, type Metric, type Point, series, type Why } from "./history.ts";

/// A landing's metric past its band, waiting for its retrigger.
export type Suspect = Exclude<Judged, { change: "learning" }> & { scene: string };

export type Alert = {
  id: string;
  when: string;
  scene: string;
  metric: Metric;
  before: number;
  after: number;
  width: number;
  /// The range it came in: the last landing in its band, and the one past it.
  good: string;
  bad: string;
  /// Commits in the range touching what the row reads.
  commits: number;
  state: "bisecting" | "found" | "narrowed" | "open" | "better";
  first?: string;
  left?: string[];
};

/// The perf job in flight: its id, commit and why; a bisect's end, when it measures one.
export type InFlight = { id: string; rev: string; why: Why; end?: "good" | "bad" };

export type State = {
  /// The last landing benched, and the last whose every metric was in its band.
  benched?: string;
  good?: string;
  job?: InFlight;
  lastEnded?: string;
  lastWhy?: Why;
  retrigger?: { rev: string; good?: string; suspects: Suspect[] };
  bisect?: Bisect;
  /// Bisects waiting their turn, the worst first.
  waiting: Bisect[];
  alerts: Alert[];
  /// Each series' last change point (`<scene> <metric>`): its band starts there.
  since: Record<string, string>;
};

export const empty = (): State => ({ waiting: [], alerts: [], since: {} });

/// Minutes the row rests after its bench before the next, so other jobs' benches go between.
export const REST = 20;

/// What to bench next: a retrigger first, then a landing batch or a bisect's step, taking
/// turns when both wait; nothing while a perf bench runs or rests.
export function plan(s: State, tip: string, touched: boolean, now: number, rest = REST): { rev: string; why: Why; end?: "good" | "bad" } | { wait: string } {
  if (s.job) return { wait: `${s.job.id} benches ${s.job.rev.slice(0, 9)} (${s.job.why})` };
  if (s.lastEnded && now - Date.parse(s.lastEnded) < rest * 60_000) return { wait: `resting ${rest} min after its last bench` };
  if (s.retrigger) return { rev: s.retrigger.rev, why: "retrigger" };
  const landing = tip !== s.benched && touched;
  const bisect = s.bisect && nextStep(s.bisect);
  if (landing && (!bisect || s.lastWhy !== "landing")) return { rev: tip, why: "landing" };
  if (bisect) return { rev: bisect.rev, why: "bisect", end: bisect.end };
  return { wait: tip === s.benched ? `main's tip ${tip.slice(0, 9)} is benched` : `nothing the row reads changed since ${s.benched?.slice(0, 9)}` };
}

const key = (scene: string, metric: Metric) => `${scene} ${metric}`;
const alertId = (scene: string, metric: Metric, bad: string) => `${scene}/${metric}@${bad.slice(0, 9)}`;

/// An alert in a line: what changed, by how much, where, and what the bisect found.
export function alertLine(a: Alert): string {
  const mark = a.state === "better" ? "✓" : "✗";
  const where = `in ${a.good.slice(0, 9)}..${a.bad.slice(0, 9)} (${a.commits} commit${a.commits === 1 ? "" : "s"} touching the row)`;
  const what = changeLine(a.scene, { metric: a.metric, before: a.before, after: a.after, width: a.width, change: "worse" });
  const tail =
    a.state === "found" ? `; first bad ${a.first?.slice(0, 9)}` : a.state === "narrowed" ? `; bisect narrowed it to ${a.left?.length} commits: ${a.left?.map((c) => c.slice(0, 9)).join(" ")}` : a.state === "bisecting" ? "; bisecting" : a.state === "better" ? "; better, not bisected" : "";
  return `${mark} perf ${a.when.slice(0, 16).replace("T", " ")} UTC: ${what}, ${where}${tail}`;
}

/// A job's points taken in: the new state and the lines it says. `history` is every point
/// before this job's; `commits(good, bad)` the commits after `good` up to `bad` touching what
/// the row reads, oldest first.
export function ingest(s0: State, job: InFlight, points: Point[], history: Point[], commits: (good: string, bad: string) => string[], now: string, cap = CAP): { state: State; lines: string[] } {
  const s: State = { ...s0, job: undefined, lastEnded: now, lastWhy: job.why, alerts: [...s0.alerts], waiting: [...s0.waiting], since: { ...s0.since } };
  const lines: string[] = [];
  if (job.why === "bisect") {
    const b = s.bisect;
    if (!b) return { state: s, lines };
    // Every run of every scene it judges: a step of five runs of two scenes is ten samples.
    const scenes = b.scenes ?? [b.scene];
    const samples = points.filter((p) => scenes.includes(p.scene)).flatMap((p) => (p.values[b.metric] === undefined ? [] : [p.values[b.metric] as number]));
    const floor = FLOORS[b.metric].abs ?? 0;
    s.bisect = step(b, job.rev, samples, floor);
    lines.push(samples.length ? `· perf bisect ${b.scene} ${b.metric}: ${job.rev.slice(0, 9)}${job.end ? ` (the ${job.end} end)` : ""} ran ${samples.join(", ")}` : `· perf bisect: ${job.rev.slice(0, 9)} gave no ${scenes.join(", ")} numbers, skipped`);
    const { goodValue, goodWorst, badValue: v } = s.bisect;
    const g = b.metric === "hitches" ? goodWorst : goodValue;
    if (g !== undefined && v !== undefined && v - g <= floor) {
      lines.push(`· perf bisect ${b.scene} ${b.metric} ${b.good.slice(0, 9)}..${b.bad.slice(0, 9)}: its ends measured ${g} and ${v}, no change to look for`);
      s.bisect = s.waiting.shift();
    } else if (!nextStep(s.bisect)) lines.push(...finishBisect(s));
    return { state: s, lines };
  }
  if (!points.length) {
    lines.push(`✗ perf: ${job.id} at ${job.rev.slice(0, 9)} gave no numbers (its log says why); the next landing is benched as usual`);
    if (job.why === "landing") s.benched = job.rev;
    else s.retrigger = undefined;
    return { state: s, lines };
  }
  if (job.why === "landing") {
    s.benched = job.rev;
    const suspects: Suspect[] = [];
    for (const p of points) {
      const before = series(history, p);
      for (const m of METRICS) {
        const v = p.values[m];
        if (v === undefined) continue;
        const j = judge(before, m, v, s.since[key(p.scene, m)]);
        if (j.change === "worse" || j.change === "better") suspects.push({ ...j, scene: p.scene });
      }
    }
    if (suspects.length) {
      s.retrigger = { rev: job.rev, good: s.good, suspects };
      lines.push(`· perf: ${job.rev.slice(0, 9)} past its band, retriggered: ${suspects.map((j) => changeLine(j.scene, j)).join("; ")}`);
    } else {
      s.good = job.rev;
      lines.push(`✓ perf: ${job.rev.slice(0, 9)} within every band`);
    }
    return { state: s, lines };
  }
  // A retrigger: each suspect confirmed when this run too is past its band the same way.
  const r = s.retrigger;
  s.retrigger = undefined;
  if (!r) return { state: s, lines };
  let worse = false;
  const raised: Alert[] = [];
  for (const j of r.suspects) {
    const v = points.find((p) => p.scene === j.scene)?.values[j.metric];
    const held = v !== undefined && (j.change === "worse" ? v > j.before + j.width : v < j.before - j.width);
    if (!held) {
      lines.push(`· perf: ${changeLine(j.scene, j)} did not hold on its retrigger (${v ?? "no number"}): noise`);
      continue;
    }
    const good = r.good ?? r.rev;
    const range = r.good ? commits(r.good, r.rev) : [];
    const a: Alert = { id: alertId(j.scene, j.metric, r.rev), when: now, scene: j.scene, metric: j.metric, before: j.before, after: (j.after + v) / 2, width: j.width, good, bad: r.rev, commits: range.length, state: j.change === "worse" ? "open" : "better" };
    s.since[key(j.scene, j.metric)] = r.rev;
    if (j.change === "worse") {
      worse = true;
      // One commit is its own answer; more are bisected.
      if (range.length > 1) s.waiting.push(start(j.scene, j.metric, good, r.rev, range, { goodValue: a.before, badValue: a.after }, cap, a.id));
    }
    raised.push(a);
  }
  s.alerts.push(...raised);
  if (!worse) s.good = r.rev;
  // The worst change bisected first, by its share of its own band.
  s.waiting.sort((x, y) => share(y) - share(x));
  if (!s.bisect && s.waiting.length) s.bisect = s.waiting.shift();
  for (const a of raised) {
    if (s.bisect?.alert === a.id) a.state = "bisecting";
    else if (a.state === "open" && a.commits === 1) Object.assign(a, { state: "found", first: commitsOrBad(commits, a) });
    lines.push(alertLine(a));
  }
  return { state: s, lines };
}

const share = (b: Bisect) => ((b.badValue ?? 0) - (b.goodValue ?? 0)) / Math.max(1e-6, Math.abs(b.goodValue ?? 1));
const commitsOrBad = (commits: (g: string, b: string) => string[], a: Alert) => commits(a.good, a.bad)[0] ?? a.bad;

/// A bisect done or out of benches: its alert names what it found, the next waiting starts.
function finishBisect(s: State): string[] {
  const b = s.bisect;
  if (!b) return [];
  const f = found(b);
  const lines: string[] = [];
  const a = s.alerts.find((a) => a.id === b.alert);
  if (a) {
    Object.assign(a, f.first ? { state: "found", first: f.first } : { state: "narrowed", left: f.left });
    lines.push(alertLine(a));
  } else {
    const what = f.first ? `first bad ${f.first.slice(0, 9)}` : `narrowed to ${f.left.length} commits: ${f.left.map((c) => c.slice(0, 9)).join(" ")}`;
    lines.push(`${f.first ? "✗" : "·"} perf bisect ${b.scene} ${b.metric} ${b.good.slice(0, 9)}..${b.bad.slice(0, 9)}: ${b.goodValue} → ${b.badValue}, ${what} (${b.benches} benches)`);
  }
  s.bisect = s.waiting.shift();
  return lines;
}

/// The running bisect stopped by hand (`perf.ts bisect --cancel`, as Pinpoint's jobs are
/// cancelled): dropped, its alert left open, the next waiting started; `job` is its step in
/// flight, for the box to cancel, which the row then forgets so it plans again at once.
export function cancelBisect(s0: State): { state: State; lines: string[]; job?: string } {
  const b = s0.bisect;
  if (!b) return { state: s0, lines: [] };
  const s: State = { ...s0, alerts: s0.alerts.map((a) => (a.id === b.alert && a.state === "bisecting" ? { ...a, state: "open" } : a)), waiting: [...s0.waiting] };
  const job = s.job?.why === "bisect" ? s.job.id : undefined;
  if (job) s.job = undefined;
  s.bisect = s.waiting.shift();
  const lines = [`· perf bisect ${b.scene} ${b.metric} ${b.good.slice(0, 9)}..${b.bad.slice(0, 9)} cancelled by hand after ${b.benches} benches${job ? `, its step ${job} cancelled` : ""}${s.bisect ? `; next, ${s.bisect.scene} ${s.bisect.metric}` : ""}`];
  return { state: s, lines, job };
}

/// A bisect asked for by hand (`perf.ts bisect`): its ends measured first unless known.
export function askBisect(s: State, scene: string, metric: Metric, good: string, bad: string, range: string[], cap = CAP, options: Options = {}): State {
  const b = start(scene, metric, good, bad, range, {}, cap, undefined, options);
  return s.bisect ? { ...s, waiting: [...s.waiting, b] } : { ...s, bisect: b };
}
