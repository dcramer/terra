#!/usr/bin/env node
// The character controller's scorecard (D457, tools/controller/README.md): every criterion of
// criteria.json measured, a line each, and held to baseline.tsv by the shared ratchet
// (tools/scorecard, D452's).
//
//   cargo xtask controller                    the cheap criteria: the plain-Rust course
//                                             (terra_motion::course) driving the mover, seconds
//   cargo xtask controller --json FILE        the results also as JSON
//   cargo xtask controller --raise "<why>"    write the rows that got worse, or are new, with why
//   cargo xtask controller --lower            write the rows that got better (nightly's, D414)
//
// The heavy criteria (the in-game filmed course, stage 1b; the mover's bench) are not run
// yet: each says what it needs. What cargo printed goes to target/controller/scorecard.log.
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { type Criterion, finish, type Held, judge, readBaseline, round, type Status } from "../scorecard/ratchet.ts";

const ROOT = resolve(import.meta.dirname, "../..");
export const CRITERIA = resolve(import.meta.dirname, "criteria.json");
export const BASELINE = resolve(import.meta.dirname, "baseline.tsv");

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
  writeFileSync(log, `scorecard.ts ${process.argv.slice(2).join(" ")}\n\n$ cargo ${args.join(" ")}\n${out}`);
  const values = courseValues(out);

  const results = criteria.map((c) => (c.measurable && c.cost === "heavy" ? { id: c.id, status: "skipped" as Status } : judge(c, values.get(c.id), held.get(c.id))));
  finish({
    name: "controller",
    criteria,
    results,
    held,
    baseline: BASELINE,
    baselineName: "tools/controller/baseline.tsv",
    log: "target/controller/scorecard.log",
    heavy: "the in-game filmed course, stage 1b",
    heavyRun: false,
    json: opt.json,
    raise: opt.raise,
    lower: opt.lower,
  });
}

if (import.meta.main) main();
