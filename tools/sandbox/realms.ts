#!/usr/bin/env node
// The Realms track's checks in one quiet step (docs/v0-tracks.md, Realms; D388, D627): the
// portal's five round trips with nothing left behind and every crossing within 3 s
// (portal.ts), a world's gate from shut to the realm beyond and back across a restart
// (gate.ts), and a joined game handed between two realms of a host (crossing.ts), one after
// another (a machine runs the game's copies in turn). A line a check with its time; on a
// failure only the lines that say what failed, the rest in target/check/realms-<check>.log.
//
//   node tools/sandbox/realms.ts [portal] [gate] [crossing]
//
// Node 22.18 or later; no dependencies. About twenty minutes for all three.

import { resolve } from "node:path";
import { run } from "./front.ts";

const ROOT = resolve(import.meta.dirname, "../..");
const ALL = ["portal", "gate", "crossing"];

const asked = process.argv.slice(2);
const unknown = asked.filter((name) => !ALL.includes(name));
if (unknown.length) {
  console.error(`✗ realms: no check is named ${unknown.join(", ")}; the checks are ${ALL.join(", ")}`);
  process.exit(2);
}
const { said, failed } = run(asked.length ? asked : ALL, import.meta.dirname, resolve(ROOT, "target/check"), { name: "realms", passed: "the portal's round trips, a world's gate and a crossing between a host's realms all pass" });
console.log(said.join("\n"));
process.exit(failed ? 1 : 0);
