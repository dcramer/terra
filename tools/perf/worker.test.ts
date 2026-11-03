// The perf row on a scratch box (D610): `tick` queues the row's bench at main's tip in the box's
// own queue, takes a finished job's rows into the history, and a landing batch is benched
// once. Main is a local repository standing for origin; the bench is faked by writing the rows
// a `cargo xtask bench --record` would have appended. No network, no GPU.

import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { box, readJob, writeJob } from "../render-worker/box.ts";
import { compare, sideLine } from "./perf.ts";
import { history, mainSide, type Row, readState, tick } from "./worker.ts";

const git = (cwd: string, ...args: string[]) => spawnSync("git", args, { cwd, encoding: "utf8" }).stdout.trim();
const HEADER = "date,commit,gpu,size,build,scene,median_ms,p99_ms,worst_ms,runs,over_pct,hitches,drift_pct,load_s,freezes";

test("the row queues its bench at main's tip, takes the job's rows in, and benches a batch once", () => {
  const tmp = mkdtempSync(join(tmpdir(), "perf-row-"));
  const origin = join(tmp, "origin");
  mkdirSync(join(origin, "crates"), { recursive: true });
  mkdirSync(join(origin, "research"), { recursive: true });
  git(tmp, "init", "-q", "-b", "main", origin);
  writeFileSync(join(origin, "research/benchmarks.csv"), `${HEADER}\n`);
  const commit = (path: string, text: string) => {
    writeFileSync(join(origin, path), text);
    git(origin, "add", "-A");
    git(origin, "-c", "user.name=t", "-c", "user.email=t@t", "commit", "-q", "-m", path);
    return git(origin, "rev-parse", "HEAD");
  };
  const c1 = commit("crates/a.rs", "1");
  const b = box(join(tmp, "box"));
  git(tmp, "clone", "-q", origin, b.repo);
  const r: Row = { scenes: ["forest-run"], runs: 1, reads: ["crates"], origin, every: 5, rest: 0, cap: 5, minutes: 60 };
  const said: string[] = [];
  const say = (l: string) => said.push(l);
  const now = new Date("2026-11-03T12:00:00Z");

  tick(b, say, now, true, r);
  const s1 = readState(b);
  assert.equal(s1.job?.rev, c1, said.join("\n"));
  assert.equal(s1.job?.why, "landing");
  const job = readJob(b, s1.job.id);
  assert.deepEqual(job?.command, ["cargo", "xtask", "bench", "forest-run", "--runs", "1", "--record"]);
  assert.equal(job?.class, "bench");
  assert.equal(git(b.repo, "rev-parse", `refs/jobs/${s1.job.id}`), c1);

  // While it runs, a docs-only landing and a crates one come in: nothing more is queued.
  commit("notes.md", "x");
  const c3 = commit("crates/a.rs", "3");
  tick(b, say, now, true, r);
  assert.equal(readState(b).job?.id, s1.job.id);

  // The bench ends: its rows appended to the csv come back in its out/.
  const out = join(b.jobs, s1.job.id, "out/research");
  mkdirSync(out, { recursive: true });
  const row = `2026-11-03,${c1.slice(0, 9)},RTX 3080,1920x1080 offscreen,release,forest-run,8.60,40.10,58.00,1,0.40,10,7.0,31.5,0`;
  const stale = `2026-10-30,4fd34bc5,RTX 3080,1920x1080 offscreen,release,forest-run,8.10,12.00,20.00,1,0.40,0,7.0,30.0,0`;
  writeFileSync(join(out, "benchmarks.csv"), `${HEADER}\n${stale}\n${row}\n`);
  if (job) writeJob(b, { ...job, state: "failed", exit: 1, ended: now.toISOString() });
  tick(b, say, now, true, r);
  const points = history(b);
  assert.equal(points.length, 1, "only the row of the job's commit, not the stale one");
  assert.deepEqual(points[0].values, { median_ms: 8.6, p99_ms: 40.1, hitches: 10, load_s: 31.5 });
  const s2 = readState(b);
  assert.equal(s2.benched, c1);
  assert.equal(s2.job?.rev, c3, "the batch that came meanwhile, at its tip, once");

  // Main's side for an A/B at c3 before c3 is benched: c1's numbers, two commits back, which
  // touch what the row reads.
  const side = mainSide(b, c3);
  assert.equal(side.at, c1);
  assert.equal(side.between, 2);
  assert.equal(side.touched, true);
  assert.equal(side.window.length, 1);
  assert.match(readFileSync(join(b.dir, "perf/log.txt"), "utf8"), /queued .* \(landing\)/);
});

test("an A/B reads main's side from the history: each metric inside or past main's band", () => {
  const p = (rev: string, hitches: number, median: number) => ({ rev, job: rev, why: "landing" as const, date: "", gpu: "RTX 3080", size: "1920x1080 offscreen", build: "release", scene: "forest-run", values: { median_ms: median, hitches } });
  const window = [p("a", 0, 8.0), p("b", 1, 8.1), p("c", 0, 7.9), p("d", 0, 8.0)];
  const same = compare([p("", 1, 8.1)], window);
  assert.equal(same.worse, false, same.lines.join("\n"));
  const worse = compare([p("", 9, 8.1)], window);
  assert.equal(worse.worse, true);
  assert.ok(worse.lines.some((l) => l.startsWith("✗ forest-run hitches 0 → 9")), worse.lines.join("\n"));
  assert.match(sideLine("c".repeat(40), { at: "a".repeat(40), between: 3, touched: false, window }), /3 commits before c{9}, none touching what it reads, so they stand for it/);
});
