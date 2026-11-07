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

import { spawnSync } from "node:child_process";
import { mkdirSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "../..");
const ALL = ["title", "join", "creator"];

/// Runs the checks named, each the script `<dir>/<name>.ts`, and returns what to say and how
/// many failed: a line a check, and for a failed one the lines of its own that say what
/// failed and where the rest is. Each check's whole output is kept in `logs`.
export function run(names: string[], dir: string, logs: string): { said: string[]; failed: number } {
  mkdirSync(logs, { recursive: true });
  const said: string[] = [];
  let failed = 0;
  for (const name of names) {
    const began = Date.now();
    const ran = spawnSync("node", [resolve(dir, `${name}.ts`)], { cwd: ROOT, encoding: "utf8" });
    const all = `${ran.stdout}${ran.stderr}`;
    writeFileSync(resolve(logs, `front-${name}.log`), all);
    const took = `${Math.round((Date.now() - began) / 1000)}s`;
    if (ran.status === 0) {
      said.push(`✓ ${name} (${took})`);
      continue;
    }
    failed += 1;
    const why = all.split("\n").filter((line) => line.startsWith("✗"));
    said.push(`✗ ${name} (${took})`);
    for (const line of why.length ? why : ["it ended with no verdict"]) said.push(`  ${line.slice(0, 600)}`);
    said.push(`  full log: ${resolve(logs, `front-${name}.log`)}; what the game said: shots/remote.log`);
  }
  said.push(failed ? `✗ front: ${failed} of its checks failed` : "✓ front: the title's worlds, Host and Join, and the creator all pass");
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
