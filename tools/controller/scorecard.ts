#!/usr/bin/env node
// The character controller's scorecard (D457, tools/controller/README.md): every criterion of
// criteria.json measured, a line each, and held to baseline.tsv by the shared ratchet
// (tools/scorecard, D452's).
//
//   cargo xtask controller                    the cheap criteria: the plain-Rust course
//                                             (terra_motion::course) driving the mover, seconds
//   cargo xtask controller --json FILE        the results also as JSON
//   cargo xtask controller --bench            also the heavy cost rows: the mover's release bench
//                                             (terra_walker's movement::bench, D185's CPU half)
//   cargo xtask controller --raise "<why>"    write the rows that got worse, or are new, with why
//   cargo xtask controller --lower            write the rows that got better (nightly's, D414)
//
// The heavy criteria: the mover's bench runs with --bench (release, one core, medians); the
// in-game filmed course (stage 1b) is not run yet and says what it needs. What cargo printed
// goes to target/controller/scorecard.log.
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { type Criterion, finish, type Held, judge, readBaseline, round, type Status } from "../scorecard/ratchet.ts";

const ROOT = resolve(import.meta.dirname, "../..");
export const CRITERIA = resolve(import.meta.dirname, "criteria.json");
export const BASELINE = resolve(import.meta.dirname, "baseline.tsv");
/// The mover's bench: one body walked on the course and the planet's ground at 2 and 30 km.
const BENCH = "movement::bench::the_mover_bench_times_a_body_walking";

/// The course's numbers from the test's output: `scorecard`, an id and a number, tab-separated.
export function courseValues(out: string): Map<string, number> {
  const values = new Map<string, number>();
  for (const m of out.matchAll(/^scorecard\t(\S+)\t(\S+)$/gm)) {
    const value = Number(m[2]);
    if (Number.isFinite(value)) values.set(m[1], round(value));
  }
  return values;
}

function main() {
  const { values: opt } = parseArgs({
    options: {
      json: { type: "string" },
      raise: { type: "string" },
      lower: { type: "boolean", default: false },
      bench: { type: "boolean", default: false },
    },
  });
  if (opt.raise !== undefined && !opt.raise.trim()) throw new Error("--raise needs its why");
  const file: { course: { crate: string; test: string }; criteria: Criterion[] } = JSON.parse(readFileSync(CRITERIA, "utf8"));
  const criteria = file.criteria;
  const held = existsSync(BASELINE) ? readBaseline(readFileSync(BASELINE, "utf8")) : new Map<string, Held>();
  const logDir = resolve(ROOT, "target/controller");
  mkdirSync(logDir, { recursive: true });
  const log = join(logDir, "scorecard.log");

  // The course: one test drives the mover through every case and prints a number a criterion.
  const args = ["test", "-q", "-p", file.course.crate, "--lib", "--", "--exact", file.course.test, "--nocapture"];
  const r = spawnSync("cargo", args, { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
  const out = (r.stdout ?? "") + (r.stderr ?? "");
  const values = courseValues(out);
  let logged = `scorecard.ts ${process.argv.slice(2).join(" ")}\n\n$ cargo ${args.join(" ")}\n${out}`;

  // The bench: the mover's cost a body a tick, in release on one core (D185's CPU half).
  if (opt.bench) {
    const bench = ["test", "-q", "--release", "-p", "terra_walker", "--lib", "--", "--ignored", "--exact", BENCH, "--nocapture", "--test-threads=1"];
    const b = spawnSync("cargo", bench, { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
    const bout = (b.stdout ?? "") + (b.stderr ?? "");
    logged += `\n$ cargo ${bench.join(" ")}\n${bout}`;
    for (const [id, v] of courseValues(bout)) values.set(id, v);
    for (const m of bout.matchAll(/^bench\t(.*)$/gm)) console.log(`  ${m[1].replaceAll("\t", ": ")}`);
  }
  writeFileSync(log, logged);

  const results = criteria.map((c) => (c.measurable && c.cost === "heavy" && !(opt.bench && (c.check as { by?: string } | null)?.by === "bench") ? { id: c.id, status: "skipped" as Status } : judge(c, values.get(c.id), held.get(c.id))));
  finish({
    name: "controller",
    criteria,
    results,
    held,
    baseline: BASELINE,
    baselineName: "tools/controller/baseline.tsv",
    log: "target/controller/scorecard.log",
    heavy: "run with --bench; the in-game filmed course is stage 1b",
    heavyRun: false,
    json: opt.json,
    raise: opt.raise,
    lower: opt.lower,
  });
}

if (import.meta.main) main();
