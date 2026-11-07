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
const asked = process.argv.slice(2);
const unknown = asked.filter((name) => !ALL.includes(name));
if (unknown.length) {
  console.error(`✗ front: no check is named ${unknown.join(", ")}; the checks are ${ALL.join(", ")}`);
  process.exit(2);
}
const logs = resolve(ROOT, "target/check");
mkdirSync(logs, { recursive: true });
let failed = 0;
for (const name of asked.length ? asked : ALL) {
  const began = Date.now();
  const ran = spawnSync("node", [resolve(ROOT, `tools/sandbox/${name}.ts`)], { cwd: ROOT, encoding: "utf8" });
  const log = resolve(logs, `front-${name}.log`);
  writeFileSync(log, `${ran.stdout}${ran.stderr}`);
  const took = `${Math.round((Date.now() - began) / 1000)}s`;
  if (ran.status === 0) {
    console.log(`✓ ${name} (${took})`);
    continue;
  }
  failed += 1;
  const why = `${ran.stdout}${ran.stderr}`.split("\n").filter((line) => line.startsWith("✗"));
  console.log(`✗ ${name} (${took})`);
  for (const line of why.length ? why : ["  it ended with no verdict"]) console.log(`  ${line.slice(0, 600)}`);
  console.log(`  full log: target/check/front-${name}.log; what the game said: shots/remote.log`);
}
console.log(failed ? `✗ front: ${failed} of its checks failed` : "✓ front: the title's worlds, Host and Join, and the creator all pass");
process.exit(failed ? 1 : 0);
