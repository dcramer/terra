#!/usr/bin/env node
// Front's bar in one quiet step (docs/v0-tracks.md; D388): the title screen's worlds in one
// process (title.ts), Host and Join from two title screens (join.ts) and the character
// creator (creator.ts), one after another (a machine runs the game's copies in turn). A line
// a check: its verdict and how long it took; on a failure only the lines that say what failed,
// and the log that holds the rest.
//
//   node tools/sandbox/front.ts [title|join|creator …]
//
// Node 22.18 or later; no dependencies.

import { cpus, loadavg } from "node:os";
import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "../..");
const ALL = ["title", "join", "creator"];

/// Runs the checks named, each the script `<dir>/<name>.ts`, and returns what to say and how
/// many failed: a line a check, and for a failed one the lines of its own that say what
/// failed and where the rest is. Each check's whole output is kept in `logs`.
/// `suite` names the run and says what all its checks passing means (Front's by default;
/// `realms.ts` runs its own through this).
/// A machine whose load is this share of its cores or more is busy (tools/sandbox/load.ts's bar).
const BUSY_LOAD_PCT = 50;

export function run(names: string[], dir: string, logs: string, suite = { name: "front", passed: "the title's worlds, Host and Join, and the creator all pass" }): { said: string[]; failed: number } {
  mkdirSync(logs, { recursive: true });
  const said: string[] = [];
  let failed = 0;
  for (const name of names) {
    const began = Date.now();
    const ran = spawnSync("node", [resolve(dir, `${name}.ts`)], { cwd: ROOT, encoding: "utf8" });
    const all = `${ran.stdout}${ran.stderr}`;
    writeFileSync(resolve(logs, `${suite.name}-${name}.log`), all);
    const took = `${Math.round((Date.now() - began) / 1000)}s`;
    if (ran.status === 0) {
      said.push(`✓ ${name} (${took})`);
      continue;
    }
    failed += 1;
    // A check's own failing lines: its ✗, or a line that says FAIL anywhere (portal.ts's
    // "against trip 1: ...; memory FAIL past 2%", which read as no verdict at all).
    const why = all.split("\n").filter((line) => line.startsWith("✗") || /\bFAIL\b/.test(line)).map((line) => line.trim());
    said.push(`✗ ${name} (${took})`);
    for (const line of why.length ? why : ["it ended with no verdict"]) said.push(`  ${line.slice(0, 600)}`);
    said.push(`  full log: ${resolve(logs, `${suite.name}-${name}.log`)}; what the game said: shots/remote.log`);
    // A check that plays the game by the clock fails on a machine doing other work: said, so
    // its failure is not taken for the game's (D185, D556).
    const load = Math.round((loadavg()[0] / cpus().length) * 100);
    if (load >= BUSY_LOAD_PCT) said.push(`  the machine was busy as it ended (its load ${load}% of its cores): run it again alone when the machine is quiet before taking this for a fault`);
  }
  said.push(failed ? `✗ ${suite.name}: ${failed} of its checks failed` : `✓ ${suite.name}: ${suite.passed}`);
  return { said, failed };
}

// Run as a command, not when a test takes `run` from it.
if (process.argv[1] && resolve(process.argv[1]) === import.meta.filename) {
  const asked = process.argv.slice(2);
  const unknown = asked.filter((name) => !ALL.includes(name));
  if (unknown.length) {
    console.error(`✗ front: no check is named ${unknown.join(", ")}; the checks are ${ALL.join(", ")}`);
    process.exit(2);
  }
  const { said, failed } = run(asked.length ? asked : ALL, import.meta.dirname, resolve(ROOT, "target/check"));
  console.log(said.join("\n"));
  process.exit(failed ? 1 : 0);
}
