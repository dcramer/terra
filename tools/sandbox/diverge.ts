#!/usr/bin/env node
// Finds where two runs of the game part ways: the first frame whose state differs, and
// which values. Riot and Factorio found that only the first difference matters; everything
// after follows from it.
//
//   node tools/sandbox/diverge.ts [--from FRAME] A.jsonl B.jsonl
//   node tools/sandbox/diverge.ts --twice [--here] [--from FRAME] -- <sandbox options for a scripted run>
//
// A trace is what the sandbox writes with --trace: one line of JSON per frame. --twice runs
// the same scripted run twice and compares them, which says whether the game does the same
// thing given the same start and the same keys: on the Windows GPU from WSL unless --here.
// --from skips frames before it, such as a start-up that is known to differ, to see what
// comes after.
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { ROOT, onWsl } from "./client.ts";

type Frame = { frame: number; [key: string]: unknown };

const readAll = (path: string): Frame[] =>
  readFileSync(path, "utf8")
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line));

/// Every leaf of `a` and `b` that differs, by its path, such as `creatures[2].at[0]`.
function differences(a: unknown, b: unknown, path = ""): [string, unknown, unknown][] {
  if (typeof a === "object" && a !== null && typeof b === "object" && b !== null) {
    const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
    return [...keys].flatMap((key) => {
      const inner = Array.isArray(a) ? `${path}[${key}]` : path ? `${path}.${key}` : key;
      return differences((a as any)[key], (b as any)[key], inner);
    });
  }
  return Object.is(a, b) ? [] : [[path, a, b]];
}

/// The first frame where the two differ, if any, how many frames differ, and the last.
export function compare(a: Frame[], b: Frame[]) {
  const length = Math.min(a.length, b.length);
  let first: { frame: number; index: number; found: [string, unknown, unknown][] } | undefined;
  let differing = 0;
  let last = 0;
  for (let i = 0; i < length; i++) {
    const found = differences(a[i], b[i]);
    if (!found.length) continue;
    first ??= { frame: a[i].frame, index: i, found };
    differing += 1;
    last = a[i].frame;
  }
  if (!first && a.length !== b.length) first = { frame: a[length - 1]?.frame ?? 0, index: length, found: [["frames", a.length, b.length]] };
  return first && { ...first, differing, last, of: length };
}

function report(a: Frame[], b: Frame[]) {
  const parted = compare(a, b);
  if (!parted) {
    console.log(`the same: ${a.length} frames`);
    return 0;
  }
  console.log(`parted on frame ${parted.frame}, the same for ${parted.index} frames before; ${parted.found.length} values differ:`);
  for (const [path, x, y] of parted.found.slice(0, 30)) console.log(`  ${path}: ${JSON.stringify(x)}  vs  ${JSON.stringify(y)}`);
  if (parted.found.length > 30) console.log(`  … and ${parted.found.length - 30} more`);
  const after = parted.last === parted.frame ? "and on no other" : `the last on frame ${parted.last}`;
  console.log(`${parted.differing} of ${parted.of} frames differ, ${after}${parted.last < a[parted.of - 1].frame ? "; the runs agree again after it" : ""}`);
  return 1;
}

/// Runs a scripted run, writing its trace to `trace`, and waits for it to end.
function run(sandbox: string[], trace: string, here: boolean) {
  const windows = onWsl && !here;
  // A Windows run is given the path through WSL's share, so the trace lands here.
  const path = windows ? spawnSync("wslpath", ["-w", trace], { encoding: "utf8" }).stdout.trim() : trace;
  const args = [...sandbox, "--trace", path];
  const [command, rest] = windows
    ? ["cargo", ["xtask", "win", "-p", "terra_sandbox", "--copy", "diverge", "--", ...args]]
    : ["cargo", ["run", "-q", "-p", "terra_sandbox", "--", ...args]];
  const done = spawnSync(command, rest as string[], { cwd: ROOT, stdio: ["ignore", "ignore", "inherit"] });
  if (done.status !== 0) throw new Error(`the run failed: ${command} ${(rest as string[]).join(" ")}`);
}

const given = process.argv.slice(2);
const fromAt = given.indexOf("--from");
const from = fromAt >= 0 && fromAt < (given.indexOf("--") >>> 0) ? Number(given[fromAt + 1]) : 0;
const args = fromAt >= 0 && fromAt < (given.indexOf("--") >>> 0) ? [...given.slice(0, fromAt), ...given.slice(fromAt + 2)] : given;
const read = (path: string): Frame[] => readAll(path).filter((frame) => frame.frame >= from);
if (args[0] === "--twice") {
  const end = args.indexOf("--");
  const here = args.slice(0, end < 0 ? args.length : end).includes("--here");
  const sandbox = end < 0 ? [] : args.slice(end + 1);
  const folder = resolve(ROOT, "shots");
  mkdirSync(folder, { recursive: true });
  const [first, second] = [resolve(folder, "trace-1.jsonl"), resolve(folder, "trace-2.jsonl")];
  for (const trace of [first, second]) {
    console.log(`running ${trace}…`);
    run(sandbox, trace, here);
  }
  process.exit(report(read(first), read(second)));
} else if (args.length === 2) {
  process.exit(report(read(args[0]), read(args[1])));
} else {
  console.log("node tools/sandbox/diverge.ts A.jsonl B.jsonl\nnode tools/sandbox/diverge.ts --twice [--here] -- <sandbox options for a scripted run>");
  process.exit(args.length ? 1 : 0);
}
