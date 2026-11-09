#!/usr/bin/env node
// Which systems a scene's frames go to, with no display (D577, D71): a Tracy capture of the
// game run offscreen as the bench runs it, summarised a system a line, costliest first.
// `cargo xtask profile --tracy` wants a window; this is the same capture on a machine with
// none. The Tracy tools are those `cargo xtask profile --tracy` fetches (run it once).
//
//   node tools/perf/trace.ts [--frames 2700] [--top 25] [--match TEXT] -- <the game's arguments>
//   node tools/perf/trace.ts -- --experiment play --radius 30000 --seed 3 --date 07-10 --time 13 \
//        --route 0,-90 --route 90,-90 --route 90,0          # the bench's far-walk-first
//
// The capture runs from the program's start, so a system's calls include the loading screen's
// frames, when the world is empty: its mean a call reads lower than in play, the more so the
// shorter the run. Compare systems with each other, and a change against the same run before
// it. The zones are kept in target/trace/zones.csv. Node 22.18 or later; no dependencies.

import { execFileSync, spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join } from "node:path";

const all = process.argv.slice(2);
const split = all.indexOf("--");
if (split < 0) {
  console.error("✗ trace: give the game's arguments after `--`, as a bench scene has them (xtask/src/scenes.rs)");
  process.exit(1);
}
const [own, game] = [all.slice(0, split), all.slice(split + 1)];
const option = (name: string, otherwise: string) => (own.includes(name) ? own[own.indexOf(name) + 1] : otherwise);
const FRAMES = option("--frames", "2700");
const TOP = Number(option("--top", "25"));
const MATCH = option("--match", "");

const tools = join(homedir(), ".cache/terra/tools");
const folder = existsSync(tools) ? readdirSync(tools).filter((name) => name.startsWith("tracy-")).sort().pop() : undefined;
const tool = (name: string) => join(tools, folder ?? "", name);
if (!folder || !existsSync(tool("tracy-capture"))) {
  console.error("✗ trace: the Tracy tools are not fetched: run `cargo xtask profile --tracy` once (it fetches them into ~/.cache/terra/tools, D577), then this again");
  process.exit(1);
}
const build = spawnSync("cargo", ["build", "--release", "-p", "terra_sandbox", "--features", "tracy"], { encoding: "utf8" });
if (build.status !== 0) {
  console.error(`✗ trace: the Tracy build failed:\n${build.stderr.trim().split("\n").slice(-12).join("\n")}`);
  process.exit(1);
}
mkdirSync("target/trace", { recursive: true });
const [trace, zones] = ["target/trace/run.tracy", "target/trace/zones.csv"];
rmSync(trace, { force: true });
// The capture waits for the game, and ends when the game does.
const capture = spawn(tool("tracy-capture"), ["-o", trace, "-f"], { stdio: "ignore" });
const done = new Promise<void>((resolve) => capture.on("exit", () => resolve()));
const ran = spawnSync("target/release/terra_sandbox", [...game, "--offscreen", "--bench", "--settle", "60", "--bench-frames", FRAMES, "--mute"], { encoding: "utf8", maxBuffer: 1 << 28 });
if (ran.status !== 0) {
  capture.kill();
  console.error(`✗ trace: the game ended with ${ran.status ?? ran.signal}:\n${(ran.stderr || ran.stdout).trim().split("\n").slice(-8).join("\n")}`);
  process.exit(1);
}
await Promise.race([done, new Promise((resolve) => setTimeout(resolve, 20_000))]);
capture.kill();
writeFileSync(zones, execFileSync(tool("tracy-csvexport"), [trace], { maxBuffer: 1 << 28 }));

type Zone = { name: string; total: number; calls: number };
const rows: Zone[] = readFileSync(zones, "utf8")
  .split("\n")
  .slice(1)
  .map((line) => {
    // name,src_file,src_line,total_ns,total_perc,counts,...: a name may hold commas, so from the end.
    const cells = line.split(",");
    const calls = Number(cells[cells.length - 5]);
    const total = Number(cells[cells.length - 7]);
    return { name: cells.slice(0, cells.length - 9).join(","), total, calls };
  })
  .filter((zone) => Number.isFinite(zone.total) && zone.calls > 0);
// The frames run: the main schedule's calls.
const frames = rows.find((zone) => zone.name === "schedule{name=Main}")?.calls ?? Number(FRAMES);
const systems = rows
  .filter((zone) => zone.name.startsWith("system{name=") && zone.calls >= frames / 4 && zone.name.includes(MATCH))
  .map((zone) => ({ name: zone.name.slice(13, -2), frame: zone.total / frames / 1e6, calls: zone.calls }))
  .sort((a, b) => b.frame - a.frame);
console.log(`trace: ${frames} frames (the loading screen's among them); a system's milliseconds a frame, and its calls:`);
for (const system of systems.slice(0, TOP)) console.log(`  ${system.frame.toFixed(2).padStart(6)}  x${String(system.calls).padEnd(6)} ${system.name.slice(0, 110)}`);
console.log(`✓ trace: ${systems.length} systems run most frames; every zone in ${zones}`);
