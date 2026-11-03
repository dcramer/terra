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
  goodValue?: number;
  badValue?: number;
  benches: number;
  cap: number;
  skipped: string[];
  /// The alert it names its result in.
  alert?: string;
};

/// The benches a bisect may take, its ends' included when measured: 4 halve 16 commits to one.
export const CAP = 5;

export function start(scene: string, metric: Metric, good: string, bad: string, commits: string[], ends: { goodValue?: number; badValue?: number } = {}, cap = CAP, alert?: string): Bisect {
  return { scene, metric, good, bad, commits, lo: -1, hi: commits.length - 1, ...ends, benches: 0, cap, skipped: [], alert };
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

/// A bisect with a bench's value at `rev` (none: the bench gave no numbers, so the commit is
/// skipped). A value nearer the bad end than the good is bad.
export function step(b: Bisect, rev: string, value: number | undefined): Bisect {
  const s = { ...b, benches: b.benches + 1, skipped: [...b.skipped] };
  if (value === undefined) {
    if (rev === b.good || rev === b.bad) s.cap = s.benches; // an end without numbers ends it
    else s.skipped.push(rev);
    return s;
  }
  if (rev === b.good && s.goodValue === undefined) return { ...s, goodValue: value };
  if (rev === b.bad && s.badValue === undefined) return { ...s, badValue: value };
  const i = b.commits.indexOf(rev);
  if (i < 0 || s.goodValue === undefined || s.badValue === undefined) return s;
  const bad = Math.abs(value - s.badValue) < Math.abs(value - s.goodValue);
  return bad ? { ...s, hi: i } : { ...s, lo: i };
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
