// The perf row's bisect (D610): from an alert's range to its first bad commit, by halving,
// each step one bench of the row at the commit chosen. As Chromium's Pinpoint bisects a
// regression from a point on the perf dashboard to a change (chromium.googlesource.com/
// chromium/src/+/HEAD/docs/speed/bisects.md), each step judged by which end the value is
// nearer, the ends measured first when the history lacks them; with one run a step, not
// Pinpoint's many, so a step is judged on the alerted metric alone, and capped at a few
// benches, a range left wider named as such. Ideas only; no code is copied.

import type { Metric } from "./history.ts";

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
  /// The ends' means, and the good end's worst run (for an intermittent metric).
  goodValue?: number;
  badValue?: number;
  goodWorst?: number;
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

export type Options = Pick<Bisect, "scenes" | "runs" | "patch">;

export function start(scene: string, metric: Metric, good: string, bad: string, commits: string[], ends: { goodValue?: number; badValue?: number; goodWorst?: number } = {}, cap = CAP, alert?: string, options: Options = {}): Bisect {
  return { scene, metric, good, bad, commits, lo: -1, hi: commits.length - 1, ...ends, benches: 0, cap, skipped: [], alert, ...options };
}

const mean = (xs: number[]) => xs.reduce((a, b) => a + b, 0) / xs.length;

/// Whether a step's runs are the bad end's. Hitches come in bursts in some runs and not others
/// (one run in three, 2026-11-03), so a mean hides them: a step is bad when any of its runs
/// hitched past the good end's worst by the metric's floor, which the good end never did. Any
/// other metric: its mean nearer the bad end's than the good's.
export function isBad(b: Bisect, samples: number[], floor: number): boolean {
  if (b.metric === "hitches") return Math.max(...samples) > (b.goodWorst ?? b.goodValue ?? 0) + floor;
  return Math.abs(mean(samples) - (b.badValue ?? 0)) < Math.abs(mean(samples) - (b.goodValue ?? 0));
}

/// The commit a bisect benches next and why, or nothing when it is done or out of benches: the
/// good end, then the bad, when not known; then the middle of what is left (skipped commits
/// stepped over, toward the bad end first, as `git bisect skip` tries a neighbour).
export function next(b: Bisect): { rev: string; end?: "good" | "bad" } | undefined {
  if (b.benches >= b.cap) return undefined;
  if (b.goodValue === undefined) return { rev: b.good, end: "good" };
  if (b.badValue === undefined) return { rev: b.bad, end: "bad" };
  const open = b.commits.slice(b.lo + 1, b.hi).map((c, i) => ({ c, i: b.lo + 1 + i })).filter(({ c }) => !b.skipped.includes(c));
  if (!open.length) return undefined;
  const mid = Math.floor((b.lo + b.hi) / 2);
  const pick = open.reduce((best, o) => (Math.abs(o.i - mid) < Math.abs(best.i - mid) ? o : best));
  return { rev: pick.c };
}

/// A bisect with a bench's runs at `rev`, the metric's value of each (none: the bench gave no
/// numbers, so the commit is skipped), judged by `isBad`; `floor` the metric's least change.
export function step(b: Bisect, rev: string, samples: number[] | number | undefined, floor = 0): Bisect {
  const s = { ...b, benches: b.benches + 1, skipped: [...b.skipped] };
  const runs = samples === undefined ? [] : typeof samples === "number" ? [samples] : samples;
  if (!runs.length) {
    if (rev === b.good || rev === b.bad) s.cap = s.benches; // an end without numbers ends it
    else s.skipped.push(rev);
    return s;
  }
  if (rev === b.good && s.goodValue === undefined) return { ...s, goodValue: mean(runs), goodWorst: Math.max(...runs) };
  if (rev === b.bad && s.badValue === undefined) return { ...s, badValue: b.metric === "hitches" ? Math.max(...runs) : mean(runs) };
  const i = b.commits.indexOf(rev);
  if (i < 0 || s.goodValue === undefined || s.badValue === undefined) return s;
  return isBad(s, runs, floor) ? { ...s, hi: i } : { ...s, lo: i };
}

/// What a bisect found: the first bad commit when one is left, or the commits it narrowed to.
export function found(b: Bisect): { first?: string; left: string[] } {
  // A skipped commit stays in what is left: it may be the one, as `git bisect` says.
  const left = b.commits.slice(b.lo + 1, b.hi + 1);
  return { first: left.length === 1 ? left[0] : undefined, left };
}

/// Whether the ends, once measured, differ at all: a bad end no worse than the good leaves
/// nothing to look for.
export const endsDiffer = (b: Bisect, width: number) => b.goodValue === undefined || b.badValue === undefined || b.badValue - b.goodValue > width;
