#!/usr/bin/env node
// A closed room is lit by its own fires, not the sky (a continuous check,
// tools/continuous/checks.json; D485, D486): the crypt of `--experiment dungeon`
// (assets/data/dungeon_demo.ron) from its first room and a torch-lit corridor, at noon and at
// midnight. Underground and shut off from the sky, each view's mean brightness must be the
// same by day and by night within SAME, and its darkest twentieth near black. Before D485 the
// lush forest's sky volume lit the dungeon whatever its rooms' probes said: the room measured
// 0.039 at noon against 0.008 at night, the corridor 0.168 against 0.150. A forest view from
// the same run is measured too, so the change is seen not to reach outside.
//
//   node tools/sandbox/indoorlight.ts [--out target/check/indoorlight] [--from <dir>]
//
// Builds the release sandbox, then runs it through the GPU scheduler (`cargo xtask gpu still`)
// once at each hour. --from measures a folder a run wrote instead (`noon-00090.png`, ...). A
// line a stage; exits 1 when a view fails. The vantages are the crypt's at seed 7 in the lush
// forest; a new demo seed or site needs them again from the run's log ("Dungeon vantage").
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { look } from "./look.ts";
import { readPng } from "./png.ts";

const ROOT = resolve(import.meta.dirname, "../..");
/// The views, eye and target (east, up, south from the start), each held from its frame and
/// shot at its frame plus HOLD. `inside` views are held to the rule; the forest is reported.
const VIEWS = [
  { name: "room", inside: true, at: "-45.93,-2.4,-69.65,-44.41,-3.0,-65.12" },
  { name: "corridor", inside: true, at: "-39.91,-2.4,-66.63,-36.91,-2.6,-66.62" },
  { name: "forest", inside: false, at: "-46,1.7,-80,-40,1.2,-110" },
];
const STEP = 100;
const HOLD = 90;
const HOURS = [
  ["noon", "12"],
  ["night", "0"],
] as const;
/// How far noon's mean may stand from night's inside, as a share of the brighter.
const SAME = 0.1;
/// The darkest twentieth inside must be under this, on 0..1 sRGB: an unlit corner.
const BLACK = 0.02;

const frame = (k: number) => k * STEP + HOLD;
const still = (dir: string, hour: string, k: number) => join(dir, `${hour}-${String(frame(k)).padStart(5, "0")}.png`);

function run(out: string): boolean {
  const build = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: ROOT, encoding: "utf8" });
  if (build.status !== 0) {
    console.log(`indoorlight: ✗ build\n${build.stderr.split("\n").slice(-20).join("\n")}`);
    return false;
  }
  console.log("indoorlight: built");
  mkdirSync(out, { recursive: true });
  const vantages = VIEWS.flatMap((v, k) => ["--vantage", `${v.at}@${k * STEP}`]);
  const shots = VIEWS.map((_, k) => frame(k)).join(",");
  for (const [hour, time] of HOURS) {
    const sandbox = [resolve(ROOT, "target/release/terra_sandbox"), "--offscreen", "--clean", "--picture", "1920x1080", "--mute", "--experiment", "dungeon", "--time", time, ...vantages, "--shots", `${join(out, hour)}@${shots}`];
    const done = spawnSync("cargo", ["xtask", "gpu", "still", "--", "timeout", "900", ...sandbox], { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT }, maxBuffer: 1 << 28 });
    writeFileSync(join(out, `${hour}.log`), `${done.stdout}${done.stderr}`);
    if (done.status !== 0) {
      console.log(`indoorlight: ✗ the ${hour} run exited ${done.status}, see ${join(out, `${hour}.log`)}`);
      return false;
    }
    console.log(`indoorlight: ran at ${hour}`);
  }
  return true;
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const flag = (name: string) => {
    const i = argv.indexOf(name);
    return i < 0 ? undefined : argv[i + 1];
  };
  const from = flag("--from");
  const out = resolve(from ?? flag("--out") ?? join(ROOT, "target/check/indoorlight"));
  if (!from && !run(out)) process.exit(1);
  let holds = true;
  VIEWS.forEach((view, k) => {
    const files = HOURS.map(([hour]) => still(out, hour, k));
    if (files.some((f) => !existsSync(f))) {
      console.log(`indoorlight: ✗ ${view.name}: no stills in ${out}`);
      holds = false;
      return;
    }
    const [noon, night] = files.map((f) => look(readPng(f)));
    const apart = Math.abs(noon.mean - night.mean) / Math.max(noon.mean, night.mean, 1e-6);
    const told = `mean ${noon.mean.toFixed(3)} at noon, ${night.mean.toFixed(3)} at night (${(apart * 100).toFixed(0)}% apart), darkest twentieth ${noon.p5.toFixed(3)}/${night.p5.toFixed(3)}, below 0.08 ${(noon.dark * 100).toFixed(0)}%/${(night.dark * 100).toFixed(0)}%`;
    if (!view.inside) {
      console.log(`indoorlight: ${view.name}: ${told}`);
      return;
    }
    const ok = apart < SAME && Math.max(noon.p5, night.p5) < BLACK;
    holds &&= ok;
    console.log(`indoorlight: ${ok ? "✓" : "✗"} ${view.name}: ${told} (under ${SAME * 100}% apart, darkest under ${BLACK})`);
  });
  process.exit(holds ? 0 : 1);
}
