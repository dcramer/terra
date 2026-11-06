#!/usr/bin/env node
// The figure's animation scorecard (D631, tools/anim/README.md): every criterion of
// criteria.json measured by the animation course (terra_walker figure::course, the man's figure
// posed headless with no GPU and driven through every transition it makes), a line each, held
// to baseline.tsv by the shared ratchet (tools/scorecard, D452's), and the failing row to work
// on next.
//
//   cargo xtask anim                    the course (release, seconds once built)
//   cargo xtask anim --json FILE        the results also as JSON
//   cargo xtask anim --raise "<why>"    write the rows that got worse with why (new rows: their
//                                       first number)
//   cargo xtask anim --lower            write the rows that got better (nightly's, D414)
//   cargo xtask anim --first ID,...     write held rows again as first numbers
//
// What cargo printed (each transition's `anim` line: its clips, worst joint and when) goes to
// target/anim/scorecard.log. Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { parseArgs } from "node:util";
import { type Criterion, finish, type Held, judge, miss, readBaseline, type Result, round } from "../scorecard/ratchet.ts";

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

/// How many bars a row is out: its miss over its bar's size (a bar of 0 counts its miss alone).
function barsOut(c: Criterion, value: number): number {
  const m = miss(value, c.bar);
  const size = c.bar && "at_most" in c.bar ? c.bar.at_most : 0;
  return size > 0 ? m / size : m;
}

/// The failing rows in the order a job takes them (the task list): most noticed first, then the
/// most bars out.
export function ranked(criteria: Criterion[], results: Result[]): { c: Criterion; out: number }[] {
  return criteria
    .map((c, i) => ({ c, r: results[i] }))
    .filter(({ r }) => r.value !== undefined && (r.miss ?? 0) > 0)
    .map(({ c, r }) => ({ c, out: round(barsOut(c, r.value!)) }))
    .sort((a, b) => b.c.noticed - a.c.noticed || b.out - a.out);
}

/// The course's line (`anim`, its name, its family, ...) whose number is a row's worst, for the
/// `next` line: a row `<measure>.<family>` (start.root is the start's glide, seam.loops the
/// seams' pop); none for a row of no transition's measure (stop.rest, stop.settle).
export function worstLine(out: string, id: string): string | undefined {
  const [kind, family] = { "start.root": ["glide", "start"], "seam.loops": ["pop", "seam"] }[id] ?? id.split(".");
  if (!["pop", "slide", "glide"].includes(kind)) return undefined;
  const lines = [...out.matchAll(/^anim\t(.*)$/gm)].map((m) => m[1]).filter((l) => l.split("\t")[1] === family);
  const num = (l: string) => Number(new RegExp(`\\b${kind} (-?[\\d.]+)`).exec(l)?.[1] ?? NaN);
  return lines.sort((a, b) => num(b) - num(a))[0]?.replaceAll("\t", "  ");
}

function main() {
  const { values: opt } = parseArgs({
    options: { json: { type: "string" }, raise: { type: "string" }, lower: { type: "boolean", default: false }, first: { type: "string" } },
  });
  if (opt.raise !== undefined && !opt.raise.trim()) throw new Error("--raise needs its why");
  const file: { course: { crate: string; test: string }; criteria: Criterion[] } = JSON.parse(readFileSync(CRITERIA, "utf8"));
  const { criteria } = file;
  const held = existsSync(BASELINE) ? readBaseline(readFileSync(BASELINE, "utf8")) : new Map<string, Held>();
  const logDir = resolve(ROOT, "target/anim");
  mkdirSync(logDir, { recursive: true });
  const log = join(logDir, "scorecard.log");

  // The course: one ignored test, in release (the figure's systems in debug take minutes).
  const args = ["test", "-q", "--release", "-p", file.course.crate, "--lib", "--", "--ignored", "--exact", file.course.test, "--nocapture"];
  const r = spawnSync("cargo", args, { cwd: ROOT, encoding: "utf8", maxBuffer: 1 << 28 });
  const out = (r.stdout ?? "") + (r.stderr ?? "");
  writeFileSync(log, `scorecard.ts ${process.argv.slice(2).join(" ")}\n\n$ cargo ${args.join(" ")}\n${out}`);
  const values = courseValues(out);
  if (r.status !== 0) console.log(`✗ the animation course failed (cargo exit ${r.status}): read target/anim/scorecard.log from the end`);

  const results = criteria.map((c) => judge(c, values.get(c.id), held.get(c.id)));
  finish({
    name: "anim",
    criteria,
    results,
    held,
    baseline: BASELINE,
    baselineName: "tools/anim/baseline.tsv",
    log: "target/anim/scorecard.log",
    heavy: "none",
    heavyRun: true,
    json: opt.json,
    raise: opt.raise,
    lower: opt.lower,
    first: opt.first?.split(",").filter(Boolean),
  });
  const next = ranked(criteria, results)[0];
  if (next) {
    console.log(`next: ${next.c.id} (noticed ${next.c.noticed}, ${next.out} bars out), the skill's loop (.agents/skills/anim)`);
    const worst = worstLine(out, next.c.id);
    if (worst) console.log(`  worst: ${worst}`);
  }
}

if (import.meta.main) main();
