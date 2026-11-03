#!/usr/bin/env node
// The perf row from an agent's checkout (D610): what the render worker's row says, and an A/B
// whose main side is read from the row's history rather than benched again.
//
//   node tools/perf/perf.ts                      the row: what it benched, what is in flight, its alerts, its last lines
//   node tools/perf/perf.ts history [n]          the history's last n rows (30)
//   node tools/perf/perf.ts main <rev>           main's side at a commit: the newest benched commit at or before it
//   node tools/perf/perf.ts ab <csv> [--base <rev>]
//                                                your bench's rows (a job's `--collect research/benchmarks.csv`)
//                                                against main's side at their base (default: the merge-base with
//                                                origin/main), each metric inside or past main's noise band
//   node tools/perf/perf.ts bisect <scene> <median_ms|p99_ms|hitches|load_s> <good> <bad>
//                                                a bisect asked by hand, its ends benched first, then by halving
//   node tools/perf/perf.ts bisect --cancel      the running bisect stopped, its step in flight cancelled, the next waiting started
//   node tools/perf/perf.ts alert <id> --invalid <reason>
//                                                an alert closed as not the code (the box, noise; an A/B's numbers
//                                                as the reason), its bisect dropped, its band re-based at the level since
//   node tools/perf/perf.ts alert <id> --accept <reason>
//                                                an alert closed as a real change kept on purpose, the same way
//
// The box answers through its `tools/perf/worker.ts` over ssh, as remote-render's do (TERRA_WORKERS). Node
// 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { loadConfig, onBox, q } from "../render-worker/client.ts";
import { changeLine, judge, METRICS, type Point, pointOf, series } from "./history.ts";

const git = (args: string[]) => spawnSync("git", args, { encoding: "utf8" }).stdout.trim();

function remote(): { host: string; dir: string } | string {
  const cfg = loadConfig(process.cwd());
  if ("error" in cfg) return cfg.error;
  return { host: cfg.workers[0], dir: cfg.dir };
}

function ask(args: string[]): { ok: boolean; out: string } {
  const r = remote();
  if (typeof r === "string") return { ok: false, out: `✗ perf: ${r}` };
  const local = r.host === "local";
  const node = local ? process.execPath : `${r.dir}/node/bin/node`;
  const script = local ? join(import.meta.dirname, "worker.ts") : `${r.dir}/repo/tools/perf/worker.ts`;
  const res = onBox(r, [node, script, "--dir", r.dir, ...args].map(q).join(" "));
  const out = `${res.stdout ?? ""}${res.status === 0 ? "" : res.stderr ?? ""}`.trim();
  return { ok: res.status === 0, out: out || `✗ perf: ssh ${r.host} exited ${res.status}` };
}

type Side = { at?: string; between: number; touched: boolean; window: Point[] };

/// Main's side in a line: where it was benched and whether it stands for the base.
export function sideLine(base: string, s: Side): string {
  if (!s.at) return `✗ perf: the row has benched nothing at or before ${base.slice(0, 9)}; bench main's side too`;
  if (!s.between) return `main's side: the row's numbers at ${base.slice(0, 9)} itself`;
  return `main's side: the row's numbers at ${s.at.slice(0, 9)}, ${s.between} commits before ${base.slice(0, 9)}${s.touched ? ", some touching what it reads: a near side, not the same; bench main's side too when the change you look for is small" : ", none touching what it reads, so they stand for it"}`;
}

/// Your rows against main's window: each metric's line, and whether any is past the band.
export function compare(mine: Point[], window: Point[]): { lines: string[]; worse: boolean } {
  const lines: string[] = [];
  let worse = false;
  for (const p of mine) {
    const before = series(window, p);
    if (!before.length) {
      lines.push(`· ${p.scene}: main's side has no ${p.scene} on ${p.gpu}, ${p.size}, ${p.build}`);
      continue;
    }
    for (const m of METRICS) {
      const v = p.values[m];
      if (v === undefined) continue;
      const j = judge(before, m, v);
      worse ||= j.change === "worse";
      const mark = j.change === "worse" ? "✗" : j.change === "better" ? "✓" : "·";
      lines.push(`${mark} ${changeLine(p.scene, j)}${j.change === "same" ? ", within it" : j.change === "learning" ? "" : `, ${j.change}`}`);
    }
  }
  return { lines, worse };
}

function main(argv: string[]): number {
  const [what, ...rest] = argv;
  if (what === undefined || what === "status" || what === "history" || what === "bisect" || what === "alert") {
    const r = ask(what ? [what, ...rest] : ["status"]);
    console.log(r.out);
    return r.ok ? 0 : 1;
  }
  if (what === "main" || what === "ab") {
    const at = rest.indexOf("--base");
    const base = git(["rev-parse", what === "main" ? rest[0] ?? "HEAD" : at >= 0 ? rest[at + 1] : "HEAD"]);
    const merged = what === "ab" && at < 0 ? git(["merge-base", base, "origin/main"]) : base;
    if (!merged) return console.log("✗ perf: name a commit (git rev-parse found none)"), 2;
    const r = ask(["main", merged]);
    if (!r.ok) return console.log(r.out), 1;
    const side = JSON.parse(r.out.split("\n").at(-1) ?? "{}") as Side;
    console.log(sideLine(merged, side));
    if (what === "main") {
      for (const p of side.window.filter((p) => p.rev === side.at)) console.log(`  ${p.scene}: ${METRICS.map((m) => `${m} ${p.values[m] ?? "-"}`).join(", ")}`);
      return side.at ? 0 : 1;
    }
    const csv = rest.find((a, i) => !a.startsWith("--") && rest[i - 1] !== "--base");
    if (!csv) return console.log("usage: node tools/perf/perf.ts ab <csv> [--base <rev>]"), 2;
    // Your rows: the last one of each scene in the file, as your bench appended them.
    const rows = readFileSync(csv, "utf8").trim().split("\n").slice(1);
    const newest = new Map<string, Point>();
    for (const l of rows) {
      const p = pointOf(l, "", "", "landing");
      if (p) newest.set(p.scene, p);
    }
    if (!side.at) return 1;
    const c = compare([...newest.values()], side.window);
    for (const l of c.lines) console.log(l);
    return c.worse ? 1 : 0;
  }
  console.log(readFileSync(new URL(import.meta.url), "utf8").split("\n").filter((l) => l.startsWith("//   ")).map((l) => l.slice(3)).join("\n"));
  return 2;
}

if (import.meta.main) process.exit(main(process.argv.slice(2)));
