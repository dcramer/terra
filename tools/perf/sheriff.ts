// The perf row's decisions (D610), pure: what to bench next, and what a bench's points say.
// A landing is benched once per batch (the landings that came while the last ran, taken
// together at main's tip, as a busy Buildbot builder collapses its pending requests); a
// landing past its band is benched again at the same commit, several runs (a retrigger, as
// Perfherder's sheriffs confirm a suspect), and when that holds its range's good end is benched
// beside it the same way, as Pinpoint measures both sides of a change: only ends that differ,
// past the floor and by a rank test, raise an alert, so the box's level drifting moves both
// ends and raises none. An alert names the scene, the metric, before and after and the range,
// and a regression is bisected (bisect.ts) to its first bad commit, which the alert then names. One perf bench at a time, a rest between two, so the row
// never holds the box with benches back to back (D319, D445).

import { anchor, type Bisect, CAP, found, next as nextStep, type Options, STALE, start, step } from "./bisect.ts";
import { centre, changeLine, FLOORS, type Judged, judge, least, METRICS, type Metric, mannWhitney, type Point, series, type Why } from "./history.ts";

/// A landing's metric past its band, waiting for its retrigger.
export type Suspect = Exclude<Judged, { change: "learning" }> & { scene: string };
/// A suspect being confirmed: the runs of its bad end (the landing, retriggered) and of its
/// good end (the range's last landing in every band, benched beside it) so far.
export type Confirming = Suspect & { bad?: number[]; base?: number[] };

/// Runs a confirm benches at each end, each its own load, so the two sides are samples taken
/// in the same hour of the box: Pinpoint's attempts, compared by a rank test, not one run each.
export const CONFIRM = 5;
/// The p-value at or under which the ends differ (Pinpoint's low threshold): five runs a side
/// pass it only when every bad run is past every good one (2 of 252 splits, 0.008).
export const LOW = 0.01;
/// Rounds of runs a confirm may take: one more when the ends differ by more than the floor
/// but not surely (Pinpoint's "unknown", which adds attempts), then it is let go.
export const ROUNDS = 2;

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
  /// `gone`: its bisect found every step good, the bad end too when benched again beside its
  /// good neighbour, so the change was the box's level while the ends were measured.
  state: "bisecting" | "found" | "narrowed" | "gone" | "open" | "better";
  /// The confirm's runs at each end and its rank test's p-value (`before` and `after` are its
  /// ends' centres).
  runs?: { good: number; bad: number };
  p?: number;
  first?: string;
  left?: string[];
  /// Closed by hand, as Perfherder's sheriffs triage an alert: invalid (not the code: the box,
  /// noise, a false bisect) or accepted (a real change kept on purpose), with why.
  closed?: { as: "invalid" | "accepted"; reason: string; when: string };
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
  /// A suspect landing being confirmed (D610): its bad end `rev` retriggered, then its good end
  /// benched beside it (`phase` good), `round` of ROUNDS.
  retrigger?: { rev: string; good?: string; suspects: Confirming[]; phase?: "bad" | "good"; round?: number };
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
  if (s.retrigger) return s.retrigger.phase === "good" && s.retrigger.good ? { rev: s.retrigger.good, why: "baseline" } : { rev: s.retrigger.rev, why: "retrigger" };
  const landing = tip !== s.benched && touched;
  const bisect = s.bisect && nextStep(s.bisect, now);
  // A step whose good neighbour was just benched again goes next, so the two stay beside each other.
  if (landing && (!bisect || (s.lastWhy !== "landing" && !s.bisect?.refreshed))) return { rev: tip, why: "landing" };
  if (bisect) return { rev: bisect.rev, why: "bisect", end: bisect.end };
  return { wait: tip === s.benched ? `main's tip ${tip.slice(0, 9)} is benched` : `nothing the row reads changed since ${s.benched?.slice(0, 9)}` };
}

/// What a confirm's bench takes: its suspects' scenes, `runs` runs each, every one its own load.
export const confirmOptions = (s: State, runs = CONFIRM): Options => ({ scenes: [...new Set((s.retrigger?.suspects ?? []).map((j) => j.scene))], runs });

const key = (scene: string, metric: Metric) => `${scene} ${metric}`;
const alertId = (scene: string, metric: Metric, bad: string) => `${scene}/${metric}@${bad.slice(0, 9)}`;

const GONE = "no first bad: every step, and its bad end benched again, measured like its good neighbour beside it, so the box's level moved, not the code (close it with `perf.ts alert <id> --invalid`)";

/// An alert in a line: what changed, by how much, where, and what the bisect found.
export function alertLine(a: Alert): string {
  const mark = a.closed || a.state === "gone" ? "·" : a.state === "better" ? "✓" : "✗";
  const where = `in ${a.good.slice(0, 9)}..${a.bad.slice(0, 9)} (${a.commits} commit${a.commits === 1 ? "" : "s"} touching the row)`;
  const runs = a.runs ? `, ${a.runs.good} runs at its good end and ${a.runs.bad} at its bad, p ${a.p?.toFixed(3)}` : "";
  const what = `${changeLine(a.scene, { metric: a.metric, before: a.before, after: a.after, width: a.width, change: "worse" })}${runs}`;
  const tail =
    a.state === "found" ? `; first bad ${a.first?.slice(0, 9)}` : a.state === "gone" ? `; ${GONE}` : a.state === "narrowed" ? `; bisect narrowed it to ${a.left?.length} commits: ${a.left?.map((c) => c.slice(0, 9)).join(" ")}` : a.state === "bisecting" ? "; bisecting" : a.state === "better" ? "; better, not bisected" : "";
  const closed = a.closed ? `; closed ${a.closed.as} ${a.closed.when.slice(0, 16).replace("T", " ")} UTC: ${a.closed.reason}` : "";
  return `${mark} perf ${a.when.slice(0, 16).replace("T", " ")} UTC: ${what}, ${where}${tail}${closed}`;
}

/// A job's points taken in: the new state and the lines it says. `history` is every point
/// before this job's; `commits(good, bad)` the commits after `good` up to `bad` touching what
/// the row reads, oldest first.
export function ingest(s0: State, job: InFlight, points: Point[], history: Point[], commits: (good: string, bad: string) => string[], now: string, cap = CAP, runs = CONFIRM): { state: State; lines: string[] } {
  const s: State = { ...s0, job: undefined, lastEnded: now, lastWhy: job.why, alerts: [...s0.alerts], waiting: [...s0.waiting], since: { ...s0.since } };
  const lines: string[] = [];
  // A scene taken on a busy machine (D185: the box's CPU clock collapsing under its runs) is
  // recorded in the history but judged nowhere, as Perfherder ignores an infra-flagged run.
  for (const p of points.filter((p) => p.busy)) {
    lines.push(`· perf: ${job.rev.slice(0, 9)} ${p.scene} was taken on a busy machine (the bench's note, D185): recorded, not judged, so no alert (D610)`);
  }
  const all = points;
  points = points.filter((p) => !p.busy);
  if (job.why === "bisect") {
    const b = s.bisect;
    if (!b) return { state: s, lines };
    // Every run of every scene it judges: a step of five runs of two scenes is ten samples.
    const scenes = b.scenes ?? [b.scene];
    const samples = points.filter((p) => scenes.includes(p.scene)).flatMap((p) => (p.values[b.metric] === undefined ? [] : [p.values[b.metric] as number]));
    const floor = FLOORS[b.metric].abs ?? 0;
    s.bisect = step(b, job.rev, samples, floor, now, job.end);
    const again = job.end === "good" && b.goodValue !== undefined;
    const what = again
      ? ` (its good neighbour benched again beside the next step, its last measure over ${STALE} min old, as Pinpoint measures neighbours together)`
      : job.end === "bad" && b.badValue !== undefined
        ? " (the bad end again, beside its good neighbour)"
        : job.end
          ? ` (the ${job.end} end)`
          : "";
    const a = anchor(b);
    const judged = !again && (!job.end || (job.end === "bad" && b.badValue !== undefined));
    const verdict = judged ? `: ${s.bisect.lo !== b.lo ? "good" : "bad"}, against ${a.rev.slice(0, 9)}'s ${a.value} (the ends ${b.goodValue} and ${b.badValue})` : "";
    lines.push(samples.length ? `· perf bisect ${b.scene} ${b.metric}: ${job.rev.slice(0, 9)}${what} ran ${samples.join(", ")}${verdict}` : `· perf bisect: ${job.rev.slice(0, 9)} gave no ${scenes.join(", ")} numbers, skipped`);
    const { goodValue, goodWorst, badValue: v } = s.bisect;
    const g = b.metric === "hitches" ? goodWorst : goodValue;
    if (g !== undefined && v !== undefined && v - g <= floor) {
      lines.push(`· perf bisect ${b.scene} ${b.metric} ${b.good.slice(0, 9)}..${b.bad.slice(0, 9)}: its ends measured ${g} and ${v}, no change to look for`);
      s.bisect = s.waiting.shift();
    } else if (!nextStep(s.bisect, Date.parse(now))) lines.push(...finishBisect(s));
    return { state: s, lines };
  }
  if (!points.length) {
    if (!all.length) lines.push(`✗ perf: ${job.id} at ${job.rev.slice(0, 9)} gave no numbers (its log says why); the next landing is benched as usual`);
    else if (job.why !== "landing" && s.retrigger) lines.push(`· perf: ${job.rev.slice(0, 9)}'s ${job.why === "baseline" ? "good end's repeat" : "retrigger"} was all busy, so its suspects are neither confirmed nor raised; the next landing is benched as usual`);
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
      s.retrigger = { rev: job.rev, good: s.good, suspects, phase: "bad", round: 1 };
      lines.push(`· perf: ${job.rev.slice(0, 9)} past its band, retriggered, then its good end ${s.good?.slice(0, 9) ?? "(none yet)"} benched beside it: ${suspects.map((j) => changeLine(j.scene, j)).join("; ")}`);
    } else if (all.length > points.length) {
      // A busy scene was not judged, so this is no landing every band held at.
      lines.push(`✓ perf: ${job.rev.slice(0, 9)} within every band it judged`);
    } else {
      s.good = job.rev;
      lines.push(`✓ perf: ${job.rev.slice(0, 9)} within every band`);
    }
    return { state: s, lines };
  }
  const r = s.retrigger;
  s.retrigger = undefined;
  if (!r) return { state: s, lines };
  const samples = (j: Suspect) => points.filter((p) => p.scene === j.scene).flatMap((p) => (p.values[j.metric] === undefined ? [] : [p.values[j.metric] as number]));
  const none = (j: Suspect, what: string) =>
    `· perf: ${changeLine(j.scene, j)} not confirmed: its ${what} ${all.some((p) => p.scene === j.scene && p.busy) ? "was taken on a busy machine" : "gave no numbers"}, so no alert`;
  const round = r.round ?? 1;
  const rebase = (j: Suspect) => (s.since[key(j.scene, j.metric)] = r.rev);
  if (job.why === "retrigger") {
    // The bad end retriggered: a suspect whose runs, together, are back in its band was a
    // one-off (noise); one that held has its good end benched beside it next.
    const held: Confirming[] = [];
    for (const j of r.suspects) {
      const v = samples(j);
      if (!v.length) {
        lines.push(none(j, "retrigger"));
        continue;
      }
      const c = centre(j.metric, v);
      if (round === 1 && !(j.change === "worse" ? c > j.before + j.width : c < j.before - j.width)) {
        lines.push(`· perf: ${changeLine(j.scene, j)} did not hold on its retrigger (${v.join(", ")}): noise`);
        continue;
      }
      held.push({ ...j, bad: [...(j.bad ?? []), ...v] });
    }
    if (held.length && !r.good) {
      // Pinpoint compares both sides; with no good end there is no other side.
      for (const j of held) rebase(j);
      lines.push(`· perf: ${held.map((j) => changeLine(j.scene, j)).join("; ")} held on its retrigger, but the row has no good end to bench beside it, so no alert; its band re-based at ${r.rev.slice(0, 9)}`);
    } else if (held.length) {
      s.retrigger = { ...r, suspects: held, phase: "good", round };
      lines.push(`· perf: ${held.map((j) => `${j.scene} ${j.metric}`).join(", ")} held on ${r.rev.slice(0, 9)}'s retrigger; its good end ${r.good?.slice(0, 9)} is benched next, beside it`);
      return { state: s, lines };
    }
    s.good = r.rev;
    return { state: s, lines };
  }
  // The good end benched beside the bad: a suspect is confirmed when its ends differ past the
  // floor and surely (Pinpoint's rank test at its low threshold); ends that measure alike now
  // say the box's level moved, not the code, so its band is re-based at that level.
  let worse = false;
  const again: Confirming[] = [];
  const raised: Alert[] = [];
  for (const j of r.suspects) {
    const v = samples(j);
    if (!v.length) {
      lines.push(none(j, "good end's repeat"));
      continue;
    }
    const base = [...(j.base ?? []), ...v];
    const bad = j.bad ?? [];
    const [g, b] = [centre(j.metric, base), centre(j.metric, bad)];
    const p = mannWhitney(base, bad);
    const ends = `its good end ${r.good?.slice(0, 9)} ran ${base.join(", ")} and its bad end ${r.rev.slice(0, 9)} ${bad.join(", ")}, p ${p.toFixed(3)}`;
    const past = j.change === "worse" ? b - g > least(j.metric, g) : g - b > least(j.metric, g);
    if (!past) {
      rebase(j);
      lines.push(`· perf: ${changeLine(j.scene, j)} is the box's level, not the code: ${ends}, alike within its floor, so no alert; its band re-based at ${r.rev.slice(0, 9)}`);
      continue;
    }
    if (p > LOW) {
      if (round < ROUNDS) {
        again.push({ ...j, base, bad });
        lines.push(`· perf: ${changeLine(j.scene, j)} unclear: ${ends}; both ends benched once more`);
      } else lines.push(`· perf: ${changeLine(j.scene, j)} unclear after ${round} rounds: ${ends}, so no alert; the next landing is judged as usual`);
      continue;
    }
    const good = r.good ?? r.rev;
    const range = r.good ? commits(r.good, r.rev) : [];
    const a: Alert = { id: alertId(j.scene, j.metric, r.rev), when: now, scene: j.scene, metric: j.metric, before: g, after: b, width: j.width, runs: { good: base.length, bad: bad.length }, p, good, bad: r.rev, commits: range.length, state: j.change === "worse" ? "open" : "better" };
    rebase(j);
    if (j.change === "worse") {
      worse = true;
      // One commit is its own answer; more are bisected, each step as many runs as an end's.
      if (range.length > 1) s.waiting.push(start(j.scene, j.metric, good, r.rev, range, { goodValue: g, badValue: b, goodWorst: Math.max(...base), goodAt: now, badAt: now }, cap, a.id, { runs }));
    }
    raised.push(a);
  }
  s.alerts.push(...raised);
  if (again.length) s.retrigger = { ...r, suspects: again, phase: "bad", round: round + 1 };
  else if (!worse) s.good = r.rev;
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
    Object.assign(a, f.first ? { state: "found", first: f.first } : f.left.length ? { state: "narrowed", left: f.left } : { state: "gone" });
    lines.push(alertLine(a));
  } else {
    const what = f.first ? `first bad ${f.first.slice(0, 9)}` : f.left.length ? `narrowed to ${f.left.length} commits: ${f.left.map((c) => c.slice(0, 9)).join(" ")}` : GONE;
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

/// An alert closed by hand (`perf.ts alert <id> --invalid|--accept <reason>`), as Perfherder's
/// sheriffs mark one invalid or acknowledged: its bisect, running or waiting, dropped (`job` its
/// step in flight, for the box to cancel), and its series' band re-based at the level since its
/// bad end, unless a later change point already did, so the level it saw raises nothing again.
/// `history` is every point, in the order benched; `id` the alert's id or a prefix naming one.
export function closeAlert(s0: State, id: string, as: "invalid" | "accepted", reason: string, history: Point[], now: string): { state: State; lines: string[]; job?: string } | { error: string } {
  const open = s0.alerts.filter((a) => !a.closed);
  const hits = open.filter((a) => a.id === id).length ? open.filter((a) => a.id === id) : open.filter((a) => a.id.startsWith(id));
  if (hits.length !== 1) {
    const which = open.length ? `open: ${open.map((a) => a.id).join(", ")}` : "none is open";
    return { error: `${hits.length ? "more than one" : "no"} open alert is ${id} (${which}); name one by its id, as \`node tools/perf/perf.ts\` lists them` };
  }
  const a = hits[0];
  if (!reason.trim()) return { error: `closing ${a.id} needs a reason: what shows it ${as === "invalid" ? "is not the code (an A/B's numbers, the box's state)" : "is kept on purpose (the commit and why)"}` };
  let s: State = { ...s0, waiting: s0.waiting.filter((b) => b.alert !== a.id) };
  const lines: string[] = [];
  let job: string | undefined;
  if (s.bisect?.alert === a.id) {
    const got = cancelBisect(s);
    s = got.state;
    job = got.job;
    lines.push(...got.lines);
  }
  const k = key(a.scene, a.metric);
  const at = (rev?: string) => (rev ? history.findIndex((p) => p.rev === rev && p.why !== "bisect") : -1);
  const since = at(s.since[k]) > at(a.bad) ? s.since[k] : a.bad;
  s = { ...s, since: { ...s.since, [k]: since }, alerts: s.alerts.map((x) => (x.id === a.id ? { ...x, state: x.state === "bisecting" ? "open" : x.state, closed: { as, reason: reason.trim(), when: now } } : x)) };
  const closed = s.alerts.find((x) => x.id === a.id) as Alert;
  lines.push(alertLine(closed), `· perf: ${a.scene} ${a.metric}'s band re-based at ${since.slice(0, 9)}, the level since then`);
  return { state: s, lines, job };
}

/// A bisect asked for by hand (`perf.ts bisect`): its ends measured first unless known.
export function askBisect(s: State, scene: string, metric: Metric, good: string, bad: string, range: string[], cap = CAP, options: Options = {}): State {
  const b = start(scene, metric, good, bad, range, {}, cap, undefined, options);
  return s.bisect ? { ...s, waiting: [...s.waiting, b] } : { ...s, bisect: b };
}
