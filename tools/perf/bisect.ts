// The perf row's bisect (D610): from an alert's range to its first bad commit, by halving,
// each step one bench of the row at the commit chosen. As Chromium's Pinpoint bisects a
// regression from a point on the perf dashboard to a change (chromium.googlesource.com/
// chromium/src/+/HEAD/docs/speed/bisects.md), each step judged against its good neighbour, as
// Pinpoint's job_state.py compares adjacent changes (catapult dashboard/pinpoint/models), the
// ends measured first when the history lacks them. Pinpoint schedules every change's attempts
// together on one bot, so neighbours are measured in the same stretch of its level; our steps
// are one bench and a rest each, hours apart, so a neighbour measured over an hour before is
// benched again beside the step (STALE). Few runs a step, not Pinpoint's 10 to 160, so a step is
// judged on the alerted metric alone, and capped at a few benches, a range left wider named as
// such. Ideas only; no code is copied.

import { centre, type Metric } from "./history.ts";

/// A bisect of one scene's metric over `commits`, oldest first: the commits after `good` up to
/// and including `bad` that touch what the row reads. `lo` is the index of the newest known
/// good (-1: `good` itself), `hi` of the oldest known bad. The ends' values: from the alert, or
/// measured first when started by hand. `skipped`: commits whose bench gave no numbers.
export type Bisect = {
  scene: string;
  metric: Metric;
  good: string;
  bad: string;
  commits: string[];
  lo: number;
  hi: number;
  /// The ends' centres, and the good end's worst run (for an intermittent metric), and when
  /// each was measured.
  goodValue?: number;
  badValue?: number;
  goodWorst?: number;
  goodAt?: string;
  badAt?: string;
  /// The newest good measured, a step judged good or its good side benched again: each step is
  /// judged by its offset from this, its neighbour, as Pinpoint compares adjacent changes.
  near?: Near;
  /// The good side was just benched again for the step next, which then is never put off.
  refreshed?: boolean;
  /// Benches of the good side again (STALE), not counted in `benches` or its cap.
  refreshes?: number;
  /// The bad end benched again beside its good neighbour, at the bisect's end.
  badAgain?: boolean;
  benches: number;
  cap: number;
  skipped: string[];
  /// The alert it names its result in.
  alert?: string;
  /// For a flaky metric, as Pinpoint repeats each change: the scenes benched and judged
  /// together (default the one), the runs a step (each its own load), and a commit applied
  /// onto every step that lacks it (Pinpoint's patch: a revert of another cause in the range).
  scenes?: string[];
  runs?: number;
  patch?: string;
};

/// The benches a bisect may take, its ends' included when measured: 4 halve 16 commits to one.
export const CAP = 5;

/// Minutes a measurement of the good side stands for the box's level: a step benched later than
/// this after its neighbour benches that neighbour again first, beside it. The box's level
/// drifted about 4% over 2026-11-03, and a bisect's steps, one bench and a rest each, take hours.
export const STALE = 60;

/// The newest good measured: its commit, centre, worst run and when.
export type Near = { rev: string; value: number; worst: number; at?: string };

export type Options = Pick<Bisect, "scenes" | "runs" | "patch">;

export function start(scene: string, metric: Metric, good: string, bad: string, commits: string[], ends: { goodValue?: number; badValue?: number; goodWorst?: number; goodAt?: string; badAt?: string } = {}, cap = CAP, alert?: string, options: Options = {}): Bisect {
  return { scene, metric, good, bad, commits, lo: -1, hi: commits.length - 1, ...ends, benches: 0, cap, skipped: [], alert, ...options };
}

/// The good side a step is judged against: the newest good measured, else the good end.
export const anchor = (b: Bisect): Near => b.near ?? { rev: b.good, value: b.goodValue ?? 0, worst: b.goodWorst ?? b.goodValue ?? 0, at: b.goodAt };

const minutes = (from?: string, to?: string | number) => (from === undefined || to === undefined ? Infinity : Math.abs((typeof to === "number" ? to : Date.parse(to)) - Date.parse(from)) / 60_000);

/// Whether a step's runs are the bad side's. Hitches come in bursts in some runs and not others
/// (one run in three, 2026-11-03), so a centre hides them: a step is bad when any of its runs
/// hitched past its good neighbour's worst by the metric's floor. Any other metric: its offset
/// from its good neighbour nearer the ends' difference than nothing, the ends' difference
/// measured in one hour (the confirm's) and the offset in another, so the box's level drifting
/// between the two moves neither, as Pinpoint compares each change with its neighbour measured
/// beside it rather than with a value from hours before.
export function isBad(b: Bisect, samples: number[], floor: number): boolean {
  const a = anchor(b);
  if (b.metric === "hitches") return Math.max(...samples) > a.worst + floor;
  const offset = centre(b.metric, samples) - a.value;
  const delta = (b.badValue ?? 0) - (b.goodValue ?? 0);
  return Math.abs(offset - delta) < Math.abs(offset);
}

/// The commit a bisect benches next and why, or nothing when it is done or out of benches: the
/// good end, then the bad, when not known; then the middle of what is left (skipped commits
/// stepped over, toward the bad end first, as `git bisect skip` tries a neighbour); when nothing
/// is left but the bad end itself, measured more than STALE from its good neighbour, the bad end
/// again beside it, so drift alone never names it. Before any judged step (`now` given), its
/// good neighbour again (`end` good) when that was measured more than STALE ago.
export function next(b: Bisect, now?: number): { rev: string; end?: "good" | "bad" } | undefined {
  if (b.goodValue === undefined) return b.benches >= b.cap ? undefined : { rev: b.good, end: "good" };
  if (b.badValue === undefined) return b.benches >= b.cap ? undefined : { rev: b.bad, end: "bad" };
  const open = b.commits.slice(b.lo + 1, b.hi).map((c, i) => ({ c, i: b.lo + 1 + i })).filter(({ c }) => !b.skipped.includes(c));
  const a = anchor(b);
  let pick: { rev: string; end?: "bad" } | undefined;
  if (open.length && b.benches < b.cap) {
    const mid = Math.floor((b.lo + b.hi) / 2);
    pick = { rev: open.reduce((best, o) => (Math.abs(o.i - mid) < Math.abs(best.i - mid) ? o : best)).c };
  } else if (!open.length && b.hi === b.commits.length - 1 && !b.badAgain && minutes(b.badAt, a.at) > STALE) pick = { rev: b.bad, end: "bad" };
  if (!pick) return undefined;
  if (now !== undefined && !b.refreshed && minutes(a.at, now) > STALE) return { rev: a.rev, end: "good" };
  return pick;
}

/// A bisect with a bench's runs at `rev`, the metric's value of each (none: the bench gave no
/// numbers, so the commit is skipped), judged by `isBad`; `floor` the metric's least change,
/// `at` when it was benched, `end` the end `next` named. The good side benched again (`end`
/// good once the good end is known) becomes the step's neighbour and costs no bench of the cap.
export function step(b: Bisect, rev: string, samples: number[] | number | undefined, floor = 0, at?: string, end?: "good" | "bad"): Bisect {
  const runs = samples === undefined ? [] : typeof samples === "number" ? [samples] : samples;
  const again = end === "good" && b.goodValue !== undefined;
  if (again) {
    if (!runs.length) return { ...b, refreshed: true, refreshes: (b.refreshes ?? 0) + 1 }; // judged against the older one
    return { ...b, near: { rev, value: centre(b.metric, runs), worst: Math.max(...runs), at }, refreshed: true, refreshes: (b.refreshes ?? 0) + 1 };
  }
  const s: Bisect = { ...b, benches: b.benches + 1, skipped: [...b.skipped], refreshed: false };
  if (!runs.length) {
    if (rev === b.good || rev === b.bad) s.cap = s.benches; // an end without numbers ends it
    else s.skipped.push(rev);
    return s;
  }
  if (rev === b.good && s.goodValue === undefined) return { ...s, goodValue: centre(b.metric, runs), goodWorst: Math.max(...runs), goodAt: at };
  if (rev === b.bad && s.badValue === undefined) return { ...s, badValue: b.metric === "hitches" ? Math.max(...runs) : centre(b.metric, runs), badAt: at };
  const i = b.commits.indexOf(rev);
  if (i < 0 || s.goodValue === undefined || s.badValue === undefined) return s;
  if (rev === b.bad) Object.assign(s, { badAt: at, badAgain: true });
  if (isBad(s, runs, floor)) return { ...s, hi: i };
  return { ...s, lo: i, near: { rev, value: centre(b.metric, runs), worst: Math.max(...runs), at } };
}

/// What a bisect found: the first bad commit when one is left, or the commits it narrowed to;
/// none when even its bad end, benched again, measured like its good neighbour.
export function found(b: Bisect): { first?: string; left: string[] } {
  // A skipped commit stays in what is left: it may be the one, as `git bisect` says.
  const left = b.commits.slice(b.lo + 1, b.hi + 1);
  return { first: left.length === 1 ? left[0] : undefined, left };
}

/// Whether the ends, once measured, differ at all: a bad end no worse than the good leaves
/// nothing to look for.
export const endsDiffer = (b: Bisect, width: number) => b.goodValue === undefined || b.badValue === undefined || b.badValue - b.goodValue > width;
