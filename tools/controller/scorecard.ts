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
//   cargo xtask controller --film             also the in-game filmed course's rows (yard.ts: the GPU,
//                                             minutes; `cargo xtask gpu film -- cargo xtask controller --film`)
//   cargo xtask controller --raise "<why>"    write the rows that got worse, or are new, with why
//   cargo xtask controller --lower            write the rows that got better (nightly's, D414)
//
// The heavy criteria: the mover's bench runs with --bench (release, one core, medians); the
// in-game filmed course with --film (yard.ts writes checks.json beside its film). What cargo printed
// goes to target/controller/scorecard.log.
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { bakeFile } from "../characters/markers.ts";
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

/// The clip files of `files` whose baked foot-lock markers (D455, tools/characters/markers.ts)
/// are missing or not what the bake makes now: a re-export that drops them leaves every body's
/// feet unlocked. A clip that skates is the bake's own fault, not counted here.
export function staleMarkers(files: string[]): string[] {
  return files.filter((file) => {
    const { count, same } = bakeFile(resolve(ROOT, file), false);
    return !count || !same;
  });
}

function main() {
  const { values: opt } = parseArgs({
    options: {
      json: { type: "string" },
      raise: { type: "string" },
      lower: { type: "boolean", default: false },
      bench: { type: "boolean", default: false },
      film: { type: "boolean", default: false },
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

  // The game's clip files keep their baked foot locks.
  for (const c of criteria) {
    const check = c.check as { by?: string; files?: string[] } | null;
    if (check?.by !== "markers" || !check.files) continue;
    const stale = staleMarkers(check.files);
    values.set(c.id, stale.length);
    if (stale.length) console.log(`  markers stale or missing (run node tools/characters/markers.ts on them): ${stale.join(", ")}`);
  }

  // The bench: the mover's cost a body a tick, in release on one core (D185's CPU half).
  if (opt.bench) {
    const bench = ["test", "-q", "--release", "-p", "terra_walker", "--lib", "--", "--ignored", "--exact", BENCH, "--nocapture", "--test-threads=1"];
    const b = spawnSync("cargo", bench, { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
    const bout = (b.stdout ?? "") + (b.stderr ?? "");
    logged += `\n$ cargo ${bench.join(" ")}\n${bout}`;
    for (const [id, v] of courseValues(bout)) values.set(id, v);
    for (const m of bout.matchAll(/^bench\t(.*)$/gm)) console.log(`  ${m[1].replaceAll("\t", ": ")}`);
  }
  // The in-game filmed course: its checks' numbers, a row each.
  if (opt.film) {
    const out = join(homedir(), ".cache/terra-review/controller-yard");
    const json = join(out, "checks.json");
    rmSync(json, { force: true });
    const y = spawnSync(process.execPath, [resolve(import.meta.dirname, "yard.ts"), "--out", out], { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
    logged += `\n$ node tools/controller/yard.ts --out ${out}\n${(y.stdout ?? "") + (y.stderr ?? "")}`;
    for (const m of (y.stdout ?? "").matchAll(/^yard: (.*)$/gm)) console.log(`  ${m[1]}`);
    const checks: { name: string; value: number }[] = existsSync(json) ? JSON.parse(readFileSync(json, "utf8")) : [];
    for (const c of criteria) {
      const check = c.check as { by?: string; name?: string } | null;
      const found = check?.by === "film" ? checks.find((each) => each.name === check.name) : undefined;
      if (found && Number.isFinite(found.value)) values.set(c.id, round(found.value));
    }
  }
  writeFileSync(log, logged);

  const ran = (c: Criterion) => ({ bench: opt.bench, film: opt.film })[(c.check as { by?: string } | null)?.by ?? ""] === true;
  const results = criteria.map((c) => (c.measurable && c.cost === "heavy" && !ran(c) ? { id: c.id, status: "skipped" as Status } : judge(c, values.get(c.id), held.get(c.id))));
  finish({
    name: "controller",
    criteria,
    results,
    held,
    baseline: BASELINE,
    baselineName: "tools/controller/baseline.tsv",
    log: "target/controller/scorecard.log",
    heavy: "run with --bench, the filmed course with --film",
    heavyRun: false,
    json: opt.json,
    raise: opt.raise,
    lower: opt.lower,
  });
}

if (import.meta.main) main();
