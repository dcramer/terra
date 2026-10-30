#!/usr/bin/env node
// The bone audit (D584): how far each layer that poses a body after its clips turns each bone
// of the player's body off what its clips gave, worst and mean, by motion. One scripted run on
// the bare yard (flat ground, `--level yard`): standing, jogging and sprinting with the hammer
// in hand as play starts, then the same with the hands empty (the bar's empty slot 6), its
// every frame written by the game (`--bone-audit`, terra_walker::bone_audit: each layer's
// bones moved between the pose writers, in degrees and centimetres). A line a layer on the
// terminal (its worst bone, and the bone it bends most on average, with the motion of each),
// the full table by motion, layer and bone in bones.txt beside the record. The controller's
// rows `bones.*` are its checks (criteria.json), run with the filmed course too (yard.ts).
//
//   node tools/controller/bones.ts [--out <dir>]   run, then judge
//   node tools/controller/bones.ts --check <dir>   judge the record already there
//
// Into ~/.cache/terra-review/bones/ by default: bones.jsonl (the record), bones.log (the run),
// bones.txt (the table), checks.json and stills from the game's camera (jog-hammer.png and
// the others, a frame each). Under WSL the run is the Windows build on the real GPU; start it as
// `cargo xtask gpu still -- node tools/controller/bones.ts` (D319). Exits 1 when a check fails
// or the run did not do what it was scripted to (a motion off its speed, or not on the flat).
// Node 22.18 or later; no dependencies.

import { existsSync, mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { ROOT, buildOffscreen, offscreenPath, offscreenRun } from "../sandbox/client.ts";
import { streamed } from "../sandbox/streamed.ts";
import type { Check } from "./yard-checks.ts";

/// A frame of the record: the bones each layer moved, `[degrees, centimetres]` by bone name.
export type Line = { frame: number; speed: number; up: number; layers: Record<string, Record<string, [number, number]>> };
/// A motion: the frames it is judged over (its start's blend past) and the speeds it holds.
export type Motion = { name: string; from: number; to: number; speed: [number, number] };
/// What a layer did to a bone over a motion: its worst turn, its mean turn over every frame of
/// the motion (a frame it was left alone counting 0), its worst travel and its share of frames.
export type Bone = { worst: number; mean: number; cm: number; share: number };
export type Table = Map<string, Map<string, Map<string, Bone>>>;

/// The script: frames counted as `--press` counts them, 60 a second. Standing from the
/// arrival's settling, a jog (w) and on into a sprint (shift as well), standing again, the
/// empty slot picked, and the same again with empty hands. Each motion is judged from 1.5 s
/// after it starts, past the blend into it.
export const MOTIONS: Motion[] = [
  { name: "idle, hammer", from: 90, to: 300, speed: [0, 0.2] },
  { name: "jog, hammer", from: 390, to: 780, speed: [2.5, 5.5] },
  { name: "sprint, hammer", from: 870, to: 1140, speed: [5.5, 9.5] },
  { name: "idle, empty", from: 1380, to: 1590, speed: [0, 0.2] },
  { name: "jog, empty", from: 1680, to: 2070, speed: [2.5, 5.5] },
  { name: "sprint, empty", from: 2160, to: 2430, speed: [5.5, 9.5] },
];
const PRESSES = ["w@300-1140", "shift@780-1140", "6@1260", "w@1590-2430", "shift@2070-2430"];
/// Stills from the game's camera: a frame of each moving motion and the end, which ends the run.
const STILLS: [string, number][] = [
  ["jog-hammer", 600],
  ["sprint-hammer", 1000],
  ["jog-empty", 1900],
  ["sprint-empty", 2300],
  ["end", 2440],
];
/// The run's words but its records: the bare yard, the start facing west, the presses.
export const RUN = [
  ...["--clean", "--experiment", "play", "--level", "yard", "--no-lair", "--time", "12"],
  ...["--at", "0,0", "--face", "-40,0", "--steady", "--picture", "960x540"],
  ...PRESSES.flatMap((press) => ["--press", press]),
];
/// The most the body's distance from the planet's centre may change over a motion: on the flat.
const FLAT_M = 0.02;

/// The checks the controller's rows read (criteria.json `bones.*`), with their bars: on flat
/// ground the foot lock bends a leg a few degrees at most (D584), and the per-pose fix-ups
/// D584 retires (the carried forearm turned, the arms kept off the hips) are to move nothing.
export const CHECKS: { name: string; layer: string; of: "worst" | "mean"; most: number }[] = [
  { name: "bones feet worst", layer: "feet", of: "worst", most: 5 },
  { name: "bones feet mean", layer: "feet", of: "mean", most: 2 },
  { name: "bones carry_hand mean", layer: "carry_hand", of: "mean", most: 0 },
  { name: "bones arms mean", layer: "arms", of: "mean", most: 0 },
];

const round1 = (x: number) => Math.round(x * 10) / 10;

/// Each motion's layers' bones, from the record's frames inside it.
export function tally(lines: Line[], motions = MOTIONS): Table {
  const table: Table = new Map();
  for (const motion of motions) {
    const frames = lines.filter((l) => l.frame >= motion.from && l.frame < motion.to);
    const layers = new Map<string, Map<string, Bone>>();
    for (const line of frames) {
      for (const [layer, bones] of Object.entries(line.layers)) {
        if (!layers.has(layer)) layers.set(layer, new Map());
        const of = layers.get(layer)!;
        for (const [bone, [deg, cm]] of Object.entries(bones)) {
          const b = of.get(bone) ?? { worst: 0, mean: 0, cm: 0, share: 0 };
          b.worst = Math.max(b.worst, deg);
          b.mean += deg / frames.length;
          b.cm = Math.max(b.cm, cm);
          if (deg >= 0.5 || cm >= 0.2) b.share += 1 / frames.length;
          of.set(bone, b);
        }
      }
    }
    table.set(motion.name, layers);
  }
  return table;
}

/// Why the run's motions are not what they were scripted to be, if they are not: too few
/// frames, a speed out of the motion's, or the ground not flat under it.
export function offScript(lines: Line[], motions = MOTIONS): string[] {
  const whys: string[] = [];
  for (const motion of motions) {
    const frames = lines.filter((l) => l.frame >= motion.from && l.frame < motion.to);
    if (frames.length < (motion.to - motion.from) * 0.9) {
      whys.push(`${motion.name}: ${frames.length} frames recorded of ${motion.to - motion.from}`);
      continue;
    }
    const speed = frames.reduce((s, l) => s + l.speed, 0) / frames.length;
    if (speed < motion.speed[0] || speed > motion.speed[1]) whys.push(`${motion.name}: ${speed.toFixed(2)} m/s, not ${motion.speed[0]}-${motion.speed[1]}`);
    const ups = frames.map((l) => l.up);
    const rise = Math.max(...ups) - Math.min(...ups);
    if (rise > FLAT_M) whys.push(`${motion.name}: the ground rose ${(rise * 100).toFixed(1)} cm, not flat`);
  }
  return whys;
}

/// A layer's worst bone and the bone it bends most on average, over every motion.
export function layerWorst(table: Table, layer: string): { worst: [string, string, number]; mean: [string, string, number]; cm: [string, string, number] } {
  const out = { worst: ["", "", 0] as [string, string, number], mean: ["", "", 0] as [string, string, number], cm: ["", "", 0] as [string, string, number] };
  for (const [motion, layers] of table) {
    for (const [bone, b] of layers.get(layer) ?? []) {
      if (b.worst > out.worst[2]) out.worst = [bone, motion, b.worst];
      if (b.mean > out.mean[2]) out.mean = [bone, motion, b.mean];
      if (b.cm > out.cm[2]) out.cm = [bone, motion, b.cm];
    }
  }
  return out;
}

/// The layers as the game names them, in its order (terra_walker::bone_audit::LAYERS).
export const LAYERS = ["carry_arm", "fitted", "carry_hand", "grip", "feet", "arms", "shield_arm", "gaze"];

/// A line a layer: its worst bone and degrees, the bone it bends most on average, and the
/// farthest it moves one, each with its motion.
export function summary(table: Table): string[] {
  return LAYERS.map((layer) => {
    const { worst, mean, cm } = layerWorst(table, layer);
    if (worst[2] < 0.5 && cm[2] < 0.2) return `${layer}: still`;
    const moved = cm[2] >= 0.2 ? `; moves ${cm[0]} ${round1(cm[2])} cm (${cm[1]})` : "";
    return `${layer}: worst ${worst[0]} ${round1(worst[2])}° (${worst[1]}); mean ${mean[0]} ${round1(mean[2])}° (${mean[1]})${moved}`;
  });
}

/// The full table: a row a motion, layer and bone, the worst turn first.
export function fullTable(table: Table): string {
  const rows = ["motion\tlayer\tbone\tworst deg\tmean deg\tframes moved\tworst cm"];
  for (const [motion, layers] of table) {
    for (const layer of LAYERS) {
      const bones = [...(layers.get(layer) ?? [])].sort((a, b) => b[1].worst - a[1].worst || b[1].cm - a[1].cm);
      for (const [bone, b] of bones) rows.push([motion, layer, bone, round1(b.worst), round1(b.mean), `${Math.round(b.share * 100)}%`, round1(b.cm)].join("\t"));
    }
  }
  return rows.join("\n") + "\n";
}

/// The rows' checks from a record; off the script, each not measured, saying why.
export function judge(lines: Line[]): Check[] {
  const whys = offScript(lines);
  const table = tally(lines);
  return CHECKS.map(({ name, layer, of, most }) => {
    if (whys.length) return { name, ok: false, line: `✗ not measured: the run was not as scripted (${whys.join("; ")})`, value: NaN, faults: 1 };
    const found = layerWorst(table, layer)[of];
    const value = round1(found[2]);
    const ok = value <= most;
    const where = value > 0 ? ` (${found[0]}, ${found[1]})` : "";
    return { name, ok, line: `${ok ? "✓" : "✗"} ${value}°${where}, bar ${most}°`, value, faults: ok ? 0 : 1 };
  });
}

export const read = (file: string): Line[] =>
  readFileSync(file, "utf8")
    .split("\n")
    .filter((l) => l.trim())
    .map((l) => JSON.parse(l) as Line);

/// The run to its end, its record, log and stills in `out`. Whether it saved its stills.
export async function run(out: string): Promise<{ ok: boolean; log: string }> {
  mkdirSync(out, { recursive: true });
  const stills = join(out, "frames");
  mkdirSync(stills, { recursive: true });
  const [command, words] = offscreenRun("bones", [
    ...RUN,
    ...["--bone-audit", offscreenPath(join(out, "bones.jsonl"))],
    ...["--shots", `${offscreenPath(stills)}/f@${STILLS.map(([, f]) => f).join(",")}`],
  ]);
  const log = join(out, "bones.log");
  const said = await streamed({ tag: "bones", name: "bones", command, words, log, cwd: ROOT, env: { ...process.env, BEVY_ASSET_ROOT: ROOT } });
  for (const [name, frame] of STILLS) {
    const still = join(stills, `f-${String(frame).padStart(5, "0")}.png`);
    if (existsSync(still)) renameSync(still, join(out, `${name}.png`));
  }
  return { ok: said.includes("SANDBOX saved"), log };
}

/// The checks of the record in `out`, the table written beside it; not measured if the run
/// left no record.
export function report(out: string, ran = true): Check[] {
  const file = join(out, "bones.jsonl");
  if (!ran || !existsSync(file)) {
    return CHECKS.map(({ name }) => ({ name, ok: false, line: `✗ not measured: the bone audit's run did not finish (its log ${join(out, "bones.log")})`, value: NaN, faults: 1 }));
  }
  const lines = read(file);
  const table = tally(lines);
  writeFileSync(join(out, "bones.txt"), fullTable(table));
  return judge(lines);
}

async function main() {
  const args = process.argv.slice(2);
  if (args.includes("--help") || args.includes("-h")) {
    console.log("bones.ts [--out <dir>] | --check <dir>");
    return;
  }
  const opt = (name: string) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
  const out = resolve(opt("--check") ?? opt("--out") ?? join(homedir(), ".cache/terra-review/bones"));
  let ran = true;
  if (!args.includes("--check")) {
    if (!buildOffscreen()) process.exit(1);
    console.log("bones: built");
    const done = await run(out);
    ran = done.ok;
    if (!ran) console.log(`bones: ✗ the run did not finish; its log: ${done.log}`);
  }
  const checks = report(out, ran);
  if (existsSync(join(out, "bones.jsonl"))) for (const line of summary(tally(read(join(out, "bones.jsonl"))))) console.log(`bones: ${line}`);
  for (const c of checks) console.log(`bones: ${c.name}: ${c.line}`);
  writeFileSync(join(out, "checks.json"), JSON.stringify(checks, null, 1) + "\n");
  console.log(`bones: the table by motion, layer and bone: ${join(out, "bones.txt")}`);
  process.exit(checks.every((c) => c.ok) ? 0 : 1);
}

if (import.meta.main) await main();
