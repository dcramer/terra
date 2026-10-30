#!/usr/bin/env node
// The player with the pose layers switched (D584, `--pose-layers`, terra_walker::pose): the
// bone audit's run (bones.ts: the bare yard, standing, jogging and sprinting with the hammer,
// then with empty hands) played once a column, each under its own switch, filmed from one
// camera tracking the player from ahead on their left, and laid side by side: every layer on
// (as shipped), every pose layer off (the clip as it is) and only D584's three (feet, gaze,
// grip). Each column's run writes its own bone audit, so a line a column says which layers
// moved a bone: a layer switched off reads still.
//
//   node tools/controller/layers.ts [--out <dir>]   run, film, stills
//   node tools/controller/layers.ts --check <dir>   read the audits already there
//
// Into ~/.cache/terra-review/layers-off/ by default: layers.mp4 (the film, 30 a second, the
// review desk's ffmpeg line), a still a moment (idle-hammer.png, jog-hammer.png,
// sprint-hammer.png, jog-empty.png) and per column its audit (<column>/bones.jsonl, its table
// bones.txt) and log. Under WSL the runs are the Windows build on the real GPU; start it as
// `cargo xtask gpu film -- node tools/controller/layers.ts` (D319). Needs ffmpeg. Node 22.18
// or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { ROOT, buildOffscreen, offscreenPath, offscreenRun } from "../sandbox/client.ts";
import { streamed } from "../sandbox/streamed.ts";
import { RUN, fullTable, offScript, read, summary, tally } from "./bones.ts";

/// The columns: a name for the film and the switch's words.
export const COLUMNS: { name: string; label: string; switch: string }[] = [
  { name: "shipped", label: "every layer (as shipped)", switch: "all" },
  { name: "clip", label: "no layer (the clip)", switch: "none" },
  { name: "d584", label: "feet, gaze, grip (D584)", switch: "only=feet,gaze,grip" },
];
/// The frames filmed, every second one for 30 a second, from the camera's roll to the run's end.
const ROLLS = 60;
const LAST = 2440;
/// The moments a still is taken of, each column's frame side by side.
export const MOMENTS: [string, number][] = [
  ["idle-hammer", 200],
  ["jog-hammer", 600],
  ["sprint-hammer", 1000],
  ["jog-empty", 1900],
];
const PICTURE = "640x720";
/// The camera: tracking the player's feet, 4.3 m from them 60 degrees off ahead on their left
/// (they face west, their left is south), at 1.2 m, looking at their chest; no motion blur, so
/// a frame shows the pose.
const SHOT = {
  about: "the player from ahead on their left, tracked",
  seconds: (LAST - ROLLS) / 60 + 1,
  rig: { follow: true },
  keys: [{ t: 0, at: [-2.15, 1.2, 3.72], look: "player" }],
  look: { shutter: 0 },
};

const args = process.argv.slice(2);
if (args.includes("--help") || args.includes("-h")) {
  console.log("layers.ts [--out <dir>] | --check <dir>");
  process.exit(0);
}
const opt = (name: string) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
const out = resolve(opt("--check") ?? opt("--out") ?? join(homedir(), ".cache/terra-review/layers-off"));
const work = resolve(ROOT, "shots/layers-off");

/// One column's run to its end, its frames in work/<name>, its audit and log in out/<name>.
async function film(column: (typeof COLUMNS)[number]): Promise<boolean> {
  const frames = join(work, column.name);
  const own = join(out, column.name);
  mkdirSync(frames, { recursive: true });
  mkdirSync(own, { recursive: true });
  const shot = join(work, `${column.name}.json`);
  writeFileSync(shot, JSON.stringify(SHOT, null, 2));
  const [command, words] = offscreenRun(`layers-${column.name}`, [
    ...RUN,
    ...["--pose-layers", column.switch, "--picture", PICTURE],
    ...["--bone-audit", offscreenPath(join(own, "bones.jsonl"))],
    ...["--cine", `${offscreenPath(shot)}@${ROLLS}`, "--shots", `${offscreenPath(frames)}/f@${ROLLS}-${LAST}/2`],
  ]);
  const log = join(own, "run.log");
  const said = await streamed({ tag: "layers", name: column.name, command, words, log, cwd: ROOT, env: { ...process.env, BEVY_ASSET_ROOT: ROOT } });
  const ok = said.includes("SANDBOX saved");
  if (!ok) console.log(`layers: ✗ ${column.name} did not finish; its log: ${log}`);
  return ok;
}

const font = () => spawnSync("fc-match", ["-f", "%{file}", "DejaVu Sans:bold"], { encoding: "utf8" }).stdout.trim();

/// The columns' inputs side by side, each named in its corner, then `filter`, and into `file`
/// with `then`.
function sideBySide(inputs: string[], filter: string, then: string[], file: string) {
  const named = COLUMNS.map((c, i) => `[${i}]drawtext=fontfile=${font()}:text='${c.label}':x=12:y=10:fontsize=24:fontcolor=white:borderw=2:bordercolor=black@0.7[c${i}]`);
  const graph = [...named, `${COLUMNS.map((_, i) => `[c${i}]`).join("")}hstack=inputs=${COLUMNS.length}${filter}[v]`].join(";");
  const made = spawnSync("ffmpeg", ["-y", "-loglevel", "error", ...inputs, "-filter_complex", graph, "-map", "[v]", ...then, file], { encoding: "utf8" });
  if (made.status !== 0 || !existsSync(file)) throw new Error(`ffmpeg failed on ${file}: ${made.stderr.slice(0, 400)}`);
}

const frame = (column: string, at: number) => join(work, column, `f-${String(at).padStart(5, "0")}.png`);

/// The film and the stills from the columns' frames in work, which are then thrown away.
function compose() {
  const mp4 = join(out, "layers.mp4");
  // The review desk's line: H.264, crf 28, 1600 wide, no sound.
  sideBySide(
    COLUMNS.flatMap((c) => ["-framerate", "30", "-pattern_type", "glob", "-i", `${join(work, c.name)}/f-*.png`]),
    ",scale=1600:-2",
    ["-an", "-c:v", "libx264", "-crf", "28", "-preset", "slow", "-pix_fmt", "yuv420p", "-movflags", "+faststart"],
    mp4,
  );
  console.log(`layers: film ${mp4}, ${(statSync(mp4).size / 1e6).toFixed(1)} MB`);
  for (const [name, at] of MOMENTS) {
    if (!COLUMNS.every((c) => existsSync(frame(c.name, at)))) continue;
    sideBySide(COLUMNS.flatMap((c) => ["-i", frame(c.name, at)]), "", ["-frames:v", "1"], join(out, `${name}.png`));
  }
  console.log(`layers: stills ${MOMENTS.map(([name]) => join(out, `${name}.png`)).join(", ")}`);
  rmSync(work, { recursive: true, force: true });
}

if (!args.includes("--check")) {
  mkdirSync(out, { recursive: true });
  if (!buildOffscreen()) process.exit(1);
  console.log("layers: built");
  rmSync(work, { recursive: true, force: true });
  const ran = await Promise.all(COLUMNS.map(film));
  if (ran.every(Boolean)) compose();
} else if (COLUMNS.every((c) => existsSync(frame(c.name, LAST)))) {
  // A run's frames left by a composition that failed: composed again.
  compose();
}
// Each column's audit: the layers that moved a bone under its switch.
for (const column of COLUMNS) {
  const file = join(out, column.name, "bones.jsonl");
  if (!existsSync(file)) {
    console.log(`layers: ${column.name}: ✗ no audit (its log ${join(out, column.name, "run.log")})`);
    continue;
  }
  const lines = read(file);
  const table = tally(lines);
  writeFileSync(join(out, column.name, "bones.txt"), fullTable(table));
  for (const why of offScript(lines)) console.log(`layers: ${column.name}: ✗ not as scripted: ${why}`);
  const moved = summary(table).filter((line) => !line.endsWith(": still"));
  console.log(`layers: ${column.name} (--pose-layers ${column.switch}): ${moved.length ? "" : "every layer still"}`);
  for (const line of moved) console.log(`layers:   ${line}`);
}
const log = (c: (typeof COLUMNS)[number]) => join(out, c.name, "run.log");
process.exit(COLUMNS.every((c) => existsSync(log(c)) && readFileSync(log(c), "utf8").includes("SANDBOX saved")) ? 0 : 1);
