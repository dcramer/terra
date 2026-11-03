// The perf row's history and its noise band (D610): each scene's numbers by commit, as the
// bench records them (research/benchmarks.csv's columns, `cargo xtask bench --record`), and
// whether a new point is past the noise of the points before it.
//
// As Mozilla's Perfherder judges a change (firefox-source-docs.mozilla.org/testing/perfdocs/
// perf-sheriffing.html, treeherder's perf/alerts.py and analyze.py): against a window of the
// points before it (12, its `back_window`), past a threshold, and only once it holds (a
// retrigger at the same commit, as its sheriffs retrigger a suspect job, rather than waiting
// for 12 points after). Its spread is a robust one (1.4826 times the median absolute
// deviation), so one wild run in the window does not widen the band, as Chromium's anomaly
// finder weighs a step against the segments' spread (catapult dashboard find_change_points.py).
// Ideas only; no code is copied.

/// A metric of a scene's row, worse as it grows.
export type Metric = "median_ms" | "p99_ms" | "hitches" | "load_s";
export const METRICS: Metric[] = ["median_ms", "p99_ms", "hitches", "load_s"];

/// Why a point was benched: a landing on main, a retrigger at a suspect landing (its bad end),
/// the repeat of its range's good end benched beside it (`baseline`, as Pinpoint measures both
/// sides of a change), a bisect's step, or a bisect's end measured by hand (`perf.ts bisect`).
export type Why = "landing" | "retrigger" | "baseline" | "bisect";

/// One scene's numbers at one commit, from one job.
export type Point = {
  rev: string;
  job: string;
  why: Why;
  date: string;
  gpu: string;
  size: string;
  build: string;
  scene: string;
  values: Partial<Record<Metric, number>>;
  /// Taken on a busy machine (the bench's `busy` column, D185): recorded, never judged nor in
  /// a band, as Perfherder ignores a run its infrastructure flagged.
  busy?: boolean;
};

/// The history file's header: ours, then the bench's own row as it records it.
export const HISTORY_HEADER = "rev,job,why,date,commit,gpu,size,build,scene,median_ms,p99_ms,worst_ms,runs,over_pct,hitches,drift_pct,load_s,freezes,busy";
const BENCH_COLUMNS = HISTORY_HEADER.split(",").slice(3);

/// The rows a bench appended to its csv (`--record`): those past the file at its commit.
export function appended(collected: string, committed: string): string[] {
  const before = committed.split("\n").filter(Boolean).length;
  return collected.split("\n").filter(Boolean).slice(Math.max(1, before));
}

/// A bench row (benchmarks.csv's columns) as a point of the history.
export function pointOf(row: string, rev: string, job: string, why: Why): Point | undefined {
  const cells = row.split(",");
  if (cells.length < 10) return undefined;
  const at = (name: string) => cells[BENCH_COLUMNS.indexOf(name)];
  const values: Partial<Record<Metric, number>> = {};
  for (const m of METRICS) {
    const v = at(m);
    if (v !== undefined && v !== "" && Number.isFinite(Number(v))) values[m] = Number(v);
  }
  const point: Point = { rev, job, why, date: at("date"), gpu: at("gpu"), size: at("size"), build: at("build"), scene: at("scene"), values };
  if (at("busy") === "1") point.busy = true;
  return point;
}

/// A history line: our three columns, then the bench's row.
export const historyLine = (row: string, rev: string, job: string, why: Why) => `${rev},${job},${why},${row}`;

/// Every point of a history file.
export function readHistory(text: string): Point[] {
  const points: Point[] = [];
  for (const line of text.split("\n").slice(1).filter(Boolean)) {
    const [rev, job, why, ...rest] = line.split(",");
    const p = pointOf(rest.join(","), rev, job, why as Why);
    if (p) points.push(p);
  }
  return points;
}

/// The least change each metric counts, whatever the spread: Perfherder's 2% is for a mean of
/// many runs; ours is one run a landing, so its floors are wider. Hitches are counted, so theirs
/// is a count.
export type Floor = { pct?: number; abs?: number };
export const FLOORS: Record<Metric, Floor> = {
  median_ms: { pct: 3, abs: 0.2 },
  p99_ms: { pct: 5, abs: 0.5 },
  hitches: { abs: 2 },
  load_s: { pct: 10, abs: 2 },
};
/// Points of the window before a new one, one a job (`perJob`; Perfherder's back window).
export const WINDOW = 12;
/// Fewer points than this and there is no band yet: the row is still learning its noise.
export const LEAST = 3;
/// How many robust deviations a change must pass.
export const DEVIATIONS = 3;

export const median = (xs: number[]) => {
  const s = [...xs].sort((a, b) => a - b);
  const n = s.length;
  return n % 2 ? s[(n - 1) / 2] : (s[n / 2 - 1] + s[n / 2]) / 2;
};

/// A sample's centre: the median of a time (one slow run moves it little), the mean of a count
/// of hitches (they come in bursts in some runs and none in others, so a median hides them).
export const centre = (m: Metric, xs: number[]) => (m === "hitches" ? xs.reduce((a, b) => a + b, 0) / xs.length : median(xs));

/// The least change of a metric at a level: its floor, the larger of its share and its count.
export const least = (m: Metric, at: number) => Math.max(FLOORS[m].abs ?? 0, ((FLOORS[m].pct ?? 0) / 100) * Math.abs(at));

/// Two-sided p-value of the Mann-Whitney U test of two samples, exact (every split of the
/// pooled ranks, ties at their mid-rank) for the few runs the row takes, the normal
/// approximation past a million splits. As Pinpoint compares a change's two sides (catapult
/// dashboard pinpoint/models/compare/compare.py, which also takes Kolmogorov-Smirnov's).
export function mannWhitney(a: number[], b: number[]): number {
  const pooled = [...a, ...b];
  const sorted = [...pooled].sort((x, y) => x - y);
  const rank = (v: number) => {
    const first = sorted.indexOf(v);
    const last = sorted.lastIndexOf(v);
    return (first + last) / 2 + 1;
  };
  const ranks = pooled.map(rank);
  const [n, m] = [a.length, b.length];
  if (!n || !m) return 1;
  const mid = (n * m) / 2;
  const u = (picked: number[]) => picked.reduce((s, r) => s + r, 0) - (n * (n + 1)) / 2;
  const seen = Math.abs(u(ranks.slice(0, n)) - mid);
  let splits = 1;
  for (let i = 0; i < n; i++) splits = (splits * (n + m - i)) / (i + 1);
  if (splits > 1e6) {
    const sd = Math.sqrt((n * m * (n + m + 1)) / 12);
    const z = (seen - 0.5) / sd;
    return Math.min(1, 2 * (1 - normal(z)));
  }
  let as = 0;
  let all = 0;
  const pick: number[] = [];
  const walk = (from: number) => {
    if (pick.length === n) {
      all++;
      if (Math.abs(u(pick) - mid) >= seen - 1e-9) as++;
      return;
    }
    for (let i = from; i <= ranks.length - (n - pick.length); i++) {
      pick.push(ranks[i]);
      walk(i + 1);
      pick.pop();
    }
  };
  walk(0);
  return as / all;
}

/// The standard normal's cumulative distribution (Abramowitz and Stegun 26.2.17).
function normal(z: number): number {
  const t = 1 / (1 + 0.2316419 * Math.abs(z));
  const d = 0.3989422804014327 * Math.exp((-z * z) / 2);
  const p = d * t * (0.31938153 + t * (-0.356563782 + t * (1.781477937 + t * (-1.821255978 + t * 1.330274429))));
  return z >= 0 ? 1 - p : p;
}

/// The noise band of a window: its median, and how far from it a value is still noise, the
/// larger of the metric's floor and three robust deviations of the window.
export function band(values: number[], floor: Floor): { base: number; width: number } | undefined {
  if (values.length < LEAST) return undefined;
  const base = median(values);
  const sigma = 1.4826 * median(values.map((v) => Math.abs(v - base)));
  const width = Math.max(floor.abs ?? 0, ((floor.pct ?? 0) / 100) * Math.abs(base), DEVIATIONS * sigma);
  return { base, width };
}

/// The points one series is judged against: the same scene on the same card, picture and
/// build, from landings and retriggers (a bisect's points and a baseline's are of older
/// commits), none taken on a busy machine, in the order they were benched.
export function series(points: Point[], like: Pick<Point, "scene" | "gpu" | "size" | "build">): Point[] {
  return points.filter((p) => (p.why === "landing" || p.why === "retrigger") && !p.busy && p.scene === like.scene && p.gpu === like.gpu && p.size === like.size && p.build === like.build);
}

/// A series' points as the window counts them: one a job, its runs' median, in the order the
/// jobs were benched. A retrigger's five runs at one commit are one hour of the box, so as five
/// points they would narrow the band below the day's spread and outweigh the landings around
/// them; Perfherder's back window likewise counts pushes, not runs (analyze.py's
/// RevisionDatum, one a push with its runs' values).
export function perJob(points: Point[], metric: Metric): number[] {
  const jobs = new Map<string, number[]>();
  for (const p of points) {
    const v = p.values[metric];
    if (v === undefined) continue;
    const runs = jobs.get(p.job);
    if (runs) runs.push(v);
    else jobs.set(p.job, [v]);
  }
  return [...jobs.values()].map(median);
}

/// What a value says against the window before it.
export type Judged = { metric: Metric; before: number; after: number; width: number; change: "worse" | "better" | "same" } | { metric: Metric; change: "learning" };

/// A value against the series before it: past its band, worse or better. `since` drops the
/// points before the series' last change point (an alert), so the band is the new level's.
export function judge(before: Point[], metric: Metric, after: number, since?: string): Judged {
  let from = 0;
  if (since) {
    const i = before.findIndex((p) => p.rev === since);
    if (i >= 0) from = i;
  }
  const values = perJob(before.slice(from), metric).slice(-WINDOW);
  const b = band(values, FLOORS[metric]);
  if (!b) return { metric, change: "learning" };
  const change = after > b.base + b.width ? "worse" : after < b.base - b.width ? "better" : "same";
  return { metric, before: b.base, after, width: b.width, change };
}

const num = (m: Metric, v: number) => (m === "hitches" ? (Number.isInteger(v) ? `${v}` : v.toFixed(1)) : m === "load_s" ? `${v.toFixed(1)} s` : `${v.toFixed(2)} ms`);

/// A judged change in a line: the scene, the metric, before and after, and the band.
export function changeLine(scene: string, j: Judged): string {
  if (j.change === "learning") return `${scene} ${j.metric}: too few points for a band yet`;
  const pct = j.before ? ` (${j.after >= j.before ? "+" : ""}${Math.round((j.after / j.before - 1) * 100)}%)` : "";
  return `${scene} ${j.metric} ${num(j.metric, j.before)} → ${num(j.metric, j.after)}${pct}, band ±${num(j.metric, j.width)}`;
}
