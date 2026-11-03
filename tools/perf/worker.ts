// The perf row on the render worker (D610): the daemon calls `tick` every few minutes. It looks
// at origin/main (a `git ls-remote` of the public repository, the box holding no credentials),
// queues one bench of the row's scenes at main's tip once per landing batch (sheriff.ts `plan`)
// as a job of the box's own queue, so the planner gives it the box alone (D319, D445), and when
// it ends takes its numbers into the history and says what changed (`ingest`). Its folder,
// `<box>/perf/`: `history.csv` every point, `state.json` the row's state, `log.txt` every line it
// said, `alerts.txt` its alerts. `node worker.ts --dir <box> …` answers the client (perf.ts). TERRA_PERF=0 in
// the box's .env turns it off.

import { spawnSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { type Box, box, boxEnv, git, outDir, readJob, wake, writeJob } from "../render-worker/box.ts";
import { type Job, jobId } from "../render-worker/jobs.ts";
import { appended, HISTORY_HEADER, historyLine, METRICS, type Metric, type Point, pointOf, readHistory, series, WINDOW } from "./history.ts";
import { alertLine, askBisect, empty, ingest, plan, type State } from "./sheriff.ts";

export type Row = { scenes: string[]; runs: number; reads: string[]; origin: string; every: number; rest: number; cap: number; minutes: number };
export const row = (): Row => JSON.parse(readFileSync(join(import.meta.dirname, "row.json"), "utf8"));

const dir = (b: Box) => join(b.dir, "perf");
const file = (b: Box, name: string) => join(dir(b), name);

export function readState(b: Box): State {
  const f = file(b, "state.json");
  return existsSync(f) ? { ...empty(), ...JSON.parse(readFileSync(f, "utf8")) } : empty();
}

function saveState(b: Box, s: State): void {
  mkdirSync(dir(b), { recursive: true });
  writeFileSync(file(b, "state.json"), `${JSON.stringify(s, null, 1)}\n`);
  writeFileSync(file(b, "alerts.txt"), s.alerts.map(alertLine).join("\n") + (s.alerts.length ? "\n" : ""));
}

export function history(b: Box): Point[] {
  const f = file(b, "history.csv");
  return existsSync(f) ? readHistory(readFileSync(f, "utf8")) : [];
}

/// The commits after `good` up to `bad` on main's first-parent line that touch what the row
/// reads, oldest first.
export function commitsBetween(repo: string, good: string, bad: string, reads: string[]): string[] {
  const r = git(repo, ["rev-list", "--first-parent", "--reverse", `${good}..${bad}`, "--", ...reads]);
  return r.status === 0 ? r.stdout.split("\n").filter(Boolean) : [];
}

/// origin/main's tip, its objects fetched into the box's clone when it lacks them.
function tipOfMain(b: Box, origin: string): string | { error: string } {
  const ls = spawnSync("git", ["ls-remote", origin, "refs/heads/main"], { cwd: b.repo, encoding: "utf8", timeout: 30_000 });
  const tip = ls.status === 0 ? ls.stdout.split(/\s/)[0] : "";
  if (!/^[0-9a-f]{40}$/.test(tip)) return { error: `git ls-remote ${origin} failed: ${(ls.stderr ?? "").trim().split("\n").at(-1) || ls.error?.message || "no main"}` };
  if (git(b.repo, ["cat-file", "-e", `${tip}^{commit}`]).status !== 0) {
    const f = spawnSync("git", ["fetch", "-q", origin, "+refs/heads/main:refs/perf/main"], { cwd: b.repo, encoding: "utf8", timeout: 120_000 });
    if (f.status !== 0) return { error: `git fetch ${origin} failed: ${(f.stderr ?? "").trim().split("\n").at(-1)}` };
  }
  return tip;
}

/// The bench job of the row at `rev`, queued in the box's own queue under refs/jobs/<id>.
function queueBench(b: Box, r: Row, rev: string, why: string, now: Date): Job | { error: string } {
  const id = jobId(now, Math.random().toString(16).slice(2, 6).padEnd(4, "0"));
  const ref = git(b.repo, ["update-ref", `refs/jobs/${id}`, rev]);
  if (ref.status !== 0) return { error: `git update-ref refs/jobs/${id} ${rev.slice(0, 9)}: ${ref.stderr.trim()}` };
  const job: Job = {
    id,
    kind: "run",
    rev,
    command: ["cargo", "xtask", "bench", ...r.scenes, "--runs", String(r.runs), "--record"],
    collect: ["research/benchmarks.csv"],
    cook: true,
    minutes: r.minutes,
    class: "bench",
    by: `perf row (${why})`,
    queued: now.toISOString(),
    state: "queued",
  };
  writeJob(b, job);
  wake(b);
  return job;
}

/// The rows a finished job of the row recorded: those its bench appended to the csv.
function rowsOf(b: Box, j: Job): string[] {
  const f = join(outDir(b, j.id), "research/benchmarks.csv");
  if (!existsSync(f)) return [];
  // Only rows of this commit (the bench names it short, `+` when its tree was dirty).
  const ours = (l: string) => {
    const c = (l.split(",")[1] ?? "").replace("+", "");
    return c.length >= 7 && j.rev.startsWith(c);
  };
  return appended(readFileSync(f, "utf8"), git(b.repo, ["show", `${j.rev}:research/benchmarks.csv`]).stdout).filter(ours);
}

let lastTick = 0;
let lastError = "";

/// One pass of the row: the job in flight taken in once it ended, then the next bench queued
/// when one is due. Every `every` minutes; `say` is the daemon's log.
export function tick(b: Box, say: (line: string) => void, now = new Date(), force = false, r = row()): void {
  if (boxEnv(b).TERRA_PERF === "0") return;
  if (!force && now.getTime() - lastTick < r.every * 60_000) return;
  lastTick = now.getTime();
  let s = readState(b);
  const note = (lines: string[]) => {
    if (!lines.length) return;
    mkdirSync(dir(b), { recursive: true });
    appendFileSync(file(b, "log.txt"), lines.map((l) => `${now.toISOString().slice(0, 16)} ${l}\n`).join(""));
    for (const l of lines) say(l);
  };
  if (s.job) {
    const j = readJob(b, s.job.id);
    if (j && j.state !== "queued" && j.state !== "running") {
      const why = s.job.why;
      const rows = rowsOf(b, j);
      const points = rows.map((l) => pointOf(l, j.rev, j.id, why)).filter((p): p is Point => !!p);
      const before = history(b);
      mkdirSync(dir(b), { recursive: true });
      if (!existsSync(file(b, "history.csv"))) writeFileSync(file(b, "history.csv"), `${HISTORY_HEADER}\n`);
      appendFileSync(file(b, "history.csv"), rows.map((l) => `${historyLine(l, j.rev, j.id, why)}\n`).join(""));
      const got = ingest(s, s.job, points, before, (g, bad) => commitsBetween(b.repo, g, bad, r.reads), now.toISOString(), r.cap);
      s = got.state;
      note([`perf: ${j.id} ${j.state} at ${j.rev.slice(0, 9)}, ${points.length} scenes`, ...got.lines]);
      saveState(b, s);
    } else if (!j) {
      note([`perf: its job ${s.job.id} is gone; the row goes on`]);
      s = { ...s, job: undefined };
      saveState(b, s);
    }
  }
  const tip = tipOfMain(b, r.origin);
  if (typeof tip !== "string") {
    if (tip.error !== lastError) note([`✗ perf: ${tip.error}; the row waits for main`]);
    lastError = tip.error;
    return;
  }
  lastError = "";
  const touched = !s.benched || git(b.repo, ["diff", "--quiet", s.benched, tip, "--", ...r.reads]).status !== 0;
  const p = plan(s, tip, touched, now.getTime(), r.rest);
  if ("wait" in p) return;
  const job = queueBench(b, r, p.rev, p.why, now);
  if ("error" in job) return note([`✗ perf: ${job.error}`]);
  s = { ...s, job: { id: job.id, rev: p.rev, why: p.why, end: p.end } };
  saveState(b, s);
  note([`perf: queued ${job.id}, the row at ${p.rev.slice(0, 9)} (${p.why}${p.end ? `, the bisect's ${p.end} end` : ""})`]);
}

/// Main's side at `rev`: the newest commit at or before it on main's first-parent line that the
/// row benched, how many commits lie between and whether any touch what the row reads (none:
/// its numbers stand for `rev`), and each scene's window up to it.
export function mainSide(b: Box, rev: string): { at?: string; between: number; touched: boolean; window: Point[] } {
  const r = row();
  const points = history(b).filter((p) => p.why !== "bisect");
  const benched = new Set(points.map((p) => p.rev));
  const line = git(b.repo, ["rev-list", "--first-parent", "-n", "2000", rev]).stdout.split("\n").filter(Boolean);
  const i = line.findIndex((c) => benched.has(c));
  if (i < 0) return { between: 0, touched: true, window: [] };
  const at = line[i];
  const touched = i > 0 && git(b.repo, ["diff", "--quiet", at, rev, "--", ...r.reads]).status !== 0;
  const upTo = points.slice(0, points.findLastIndex((p) => p.rev === at) + 1);
  const window = r.scenes.flatMap((scene) => {
    const last = upTo.findLast((p) => p.scene === scene);
    return last ? series(upTo, last).slice(-WINDOW) : [];
  });
  return { at, between: i, touched, window };
}

/// `node worker.ts --dir <box> …`: what the client's perf.ts asks.
export function perfCommand(b: Box, args: string[]): number {
  const [what, ...rest] = args;
  const s = readState(b);
  if (what === "status" || what === undefined) {
    const r = row();
    const tail = existsSync(file(b, "log.txt")) ? readFileSync(file(b, "log.txt"), "utf8").trim().split("\n").slice(-Number(rest[0] ?? 12)) : [];
    const job = s.job && readJob(b, s.job.id);
    console.log(`perf row: ${r.scenes.join(", ")}, ${r.runs} run each, at each landing batch on main${boxEnv(b).TERRA_PERF === "0" ? " (off: TERRA_PERF=0)" : ""}`);
    console.log(`  last benched ${s.benched?.slice(0, 9) ?? "nothing"}, last in every band ${s.good?.slice(0, 9) ?? "none yet"}; ${s.job ? `in flight ${s.job.id} (${s.job.why}, ${job?.state ?? "gone"})` : "nothing in flight"}`);
    if (s.bisect) console.log(`  bisecting ${s.bisect.scene} ${s.bisect.metric} ${s.bisect.good.slice(0, 9)}..${s.bisect.bad.slice(0, 9)}: ${s.bisect.hi - s.bisect.lo} commits left, ${s.bisect.benches} of ${s.bisect.cap} benches`);
    if (s.waiting.length) console.log(`  ${s.waiting.length} bisects waiting`);
    console.log(s.alerts.length ? `alerts:\n${s.alerts.map((a) => `  ${alertLine(a)}`).join("\n")}` : "alerts: none");
    if (tail.length) console.log(`log (${file(b, "log.txt")}):\n${tail.map((l) => `  ${l}`).join("\n")}`);
    return 0;
  }
  if (what === "history") {
    const f = file(b, "history.csv");
    console.log(existsSync(f) ? readFileSync(f, "utf8").trim().split("\n").slice(-Number(rest[0] ?? 30)).join("\n") : "no history yet");
    return 0;
  }
  if (what === "main" && rest[0]) {
    const rev = git(b.repo, ["rev-parse", "-q", "--verify", `${rest[0]}^{commit}`]).stdout.trim();
    if (!rev) return console.log(`✗ perf: the box does not have ${rest[0]}; push it with a job first`), 1;
    console.log(JSON.stringify(mainSide(b, rev)));
    return 0;
  }
  if (what === "bisect" && rest.length === 4 && (METRICS as string[]).includes(rest[1])) {
    const [scene, metric, goodArg, badArg] = rest;
    const tipped = tipOfMain(b, row().origin);
    if (typeof tipped !== "string") return console.log(`✗ perf: ${tipped.error}`), 1;
    const [good, bad] = [goodArg, badArg].map((c) => git(b.repo, ["rev-parse", "-q", "--verify", `${c}^{commit}`]).stdout.trim());
    if (!good || !bad) return console.log(`✗ perf: the box lacks ${good ? badArg : goodArg}; it fetches main, so name commits on main`), 1;
    const range = commitsBetween(b.repo, good, bad, row().reads);
    if (!range.length) return console.log(`✗ perf: no commit in ${goodArg}..${badArg} touches what the row reads (row.json reads)`), 1;
    saveState(b, askBisect(s, scene, metric as Metric, good, bad, range, row().cap));
    console.log(`✓ perf: bisect of ${scene} ${metric} over ${range.length} commits queued${s.bisect ? " behind the one running" : ""}; its ends are benched first; \`node tools/perf/perf.ts\` follows it`);
    return 0;
  }
  console.log("usage: node tools/perf/worker.ts --dir DIR [status [lines] | history [lines] | main <rev> | bisect <scene> <median_ms|p99_ms|hitches|load_s> <good> <bad>]");
  return 2;
}

// On the box, as the client (perf.ts) runs it over ssh: `node worker.ts --dir <box> <command…>`.
if (import.meta.main) {
  const argv = process.argv.slice(2);
  const at = argv.indexOf("--dir");
  const b = box(at >= 0 ? argv[at + 1] : join(import.meta.dirname, "../../.."));
  const code = perfCommand(b, at >= 0 ? [...argv.slice(0, at), ...argv.slice(at + 2)] : argv);
  process.stdout.write("", () => process.exit(code));
}
