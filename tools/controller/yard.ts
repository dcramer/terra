#!/usr/bin/env node
// The controller's in-game filmed course (docs/controller.md; D457): the plain course's
// features dug and built in the yard (the sandbox's `controller-yard` experiment: steps
// 0.1-0.5 m down and up, a lump, a mound past the slope limit, a base's stairs, doorway, low
// passage and drop), its route walked by the scripted player at a jog and, beside it, at a
// sprint (shift held), each run filmed through the game's own camera (`--cine` with no keys:
// the HUD hidden, exposure and clock held) and written as a walk record (`--fight-log`, the
// player as a walker). yard-checks.ts measures the heavy rows from the records, not by eye:
// step smoothing (the shown figure's and camera's rise a frame at each step up, and settling),
// feet sliding (the duel's own check) and the yard's own criteria (marks reached, stuck, fallen
// through, the steep mound climbed). The two films side by side, named, in one film. Beside them
// a lake (the play world's, the yard has no water) is swum out into, along the surface, dived
// and surfaced, filmed alone (lake.mp4), and swim.ts measures the swimmer's pose from its record.
// And the bone audit's run on the bare yard (bones.ts, into bones/): how far each pose layer
// turns the player's bones off the clips, standing, jogging and sprinting on the flat.
//
//   cargo xtask controller --film                  run, film and check, then the scorecard
//   node tools/controller/yard.ts [--out <dir>]    the run, film and checks alone
//   node tools/controller/yard.ts --check <dir>    only the checks, on records already made
//
// Into ~/.cache/terra-review/controller-yard/ by default: yard.mp4, checks.txt, checks.json
// (the numbers the scorecard reads), and each run's walk-<pace>.jsonl and log; a clean checkout's
// checks are kept by its commit too, for nightly to bank (tools/scorecard/films.ts). Exits 1 when a
// check fails or a run did not finish; a run that did not finish (silent for 8 min, or killed:
// ../sandbox/streamed.ts, whose newest line passes on every 30 s on stderr) leaves its own rows
// not measured, and the others are judged on the records they wrote. Under WSL the runs are the
// Windows build, on the real GPU; start it as
// `cargo xtask gpu film -- cargo xtask controller --film` (D319). Needs ffmpeg. Node 22.18 or
// later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { ROOT, buildOffscreen, offscreenPath, offscreenRun } from "../sandbox/client.ts";
import { read } from "../sandbox/duel/checks.ts";
import { streamed } from "../sandbox/streamed.ts";
import { CONTROLLER_FILMS, filmedThrough, keep, stamp } from "../scorecard/films.ts";
import { pose } from "./swim.ts";
import { report as bonesReport, run as bonesRun } from "./bones.ts";
import { type Check, type Walk, check } from "./yard-checks.ts";

/// The frames run and filmed (every second one kept, for 30 a second): the route's start to
/// past its end at a jog, the turns and the stops' sweep.
const FROM = 20;
const TO = 2660;
const ROLLS = 10;
const PICTURE = "960x540";
/// Once the route is done (a jog's by frame ~960, a sprint's ~640), the body turns where it
/// stands, as a player tapping a way to face does: toward back, then toward right, a tap of
/// three frames each (to 1.5 m/s at accel 30), so it goes about 10 cm at decel 15 while it
/// turns some 35 deg (the facing turns only while a way is held; turn.foot's plants, a turn
/// of 20 deg within 0.2 m). Six frames went 0.54 m at decel 15 (D528), a stop, not a turn.
/// The wait before is not stuck: the route is done once its last mark is reached.
const TURNS = ["--press", "s@1300-1303", "--press", "d@1600-1603"];
/// Then the stop's phase sweep (S0): four runs from standing, out and back along the camera's
/// way, released after 60, 71, 82 and 93 frames (a quarter of a jog's stride apart), so each
/// stop brakes at another phase of the stride; and a run at a full sprint, released after
/// 1.5 s (shift held: the jog's one sprint; the sprint's every run is one). Each run then
/// stands about 1.5 s, past a late settle step. A braking stop's settle depends on the foot
/// the release finds (Plan v2: ~0.3 or ~0.6 s), so one release proves nothing.
const SWEEP = [
  ...["--press", "w@1750-1810", "--press", "s@1900-1971", "--press", "w@2060-2142", "--press", "s@2230-2323"],
  ...["--press", "w@2420-2510"],
];
const SPRINTED = ["--press", "shift@2420-2510"];
type Run = { name: string; args: string[]; to: number };
const YARD = ["--clean", "--experiment", "controller-yard"];
/// The paces walked: a jog (the route's own), and a sprint, shift held throughout.
const PACES: Run[] = [
  { name: "jog", args: [...YARD, ...TURNS, ...SWEEP, ...SPRINTED], to: TO },
  { name: "sprint", args: [...YARD, "--press", `shift@1-${TO + 10}`, ...TURNS, ...SWEEP], to: TO },
];
/// The lake: walked in off the shore until it swims (the frames to 540), slowed to tread water,
/// dived (down held), swum under, then up to the surface (space held) and on along it.
const LAKE: Run = {
  name: "lake",
  args: [
    ...["--clean", "--experiment", "play", "--radius", "30000", "--site", "lake@-15.34,1.68", "--date", "06-10", "--time", "12", "--steady"],
    ...["--press", "w@60-540", "--press", "down@600-630", "--press", "w@640-880", "--press", "space@900-1120", "--press", "w@1000-1200"],
  ],
  to: 1300,
};

const args = process.argv.slice(2);
if (args.includes("--help") || args.includes("-h")) {
  console.log("yard.ts [--out <dir>] | --check <dir>");
  process.exit(0);
}
const opt = (name: string) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
const out = resolve(opt("--check") ?? opt("--out") ?? join(homedir(), ".cache/terra-review/controller-yard"));
const started = Date.now();
const took = () => `${((Date.now() - started) / 1000).toFixed(0)} s`;

/// The checkout as built; kept for nightly only if it is as clean once the checks are written.
const filmed = stamp();
if (!args.includes("--check")) {
  mkdirSync(out, { recursive: true });
  if (!buildOffscreen()) process.exit(1);
  console.log(`yard: built (${took()})`);
  const work = resolve(ROOT, "shots/controller-yard");
  rmSync(work, { recursive: true, force: true });
  // Beside them the bone audit's run on the bare yard (bones.ts), its own record in bones/.
  const runs = await Promise.all([...[...PACES, LAKE].map((pace) => film(pace, work)), bonesRun(join(out, "bones")).then((r) => ({ name: "bones", ...r }))]);
  for (const each of runs.filter((r) => !r.ok)) console.log(`yard: ✗ ${each.name} did not finish; its log: ${each.log}`);
  console.log(`yard: ran ${runs.map((r) => `${r.name}${r.ok ? "" : " (did not finish)"}`).join(", ")} (${took()})`);
  const done = (runs: Run[]) => runs.every((pace) => finished(pace.name));
  const films: [Run[], string][] = [[PACES, "yard.mp4"], [[LAKE], "lake.mp4"]];
  for (const [runs, name] of films.filter(([runs]) => done(runs))) {
    const mp4 = sideBySide(work, runs, name);
    console.log(`yard: film ${mp4}, ${(statSync(mp4).size / 1e6).toFixed(1)} MB (${took()})`);
  }
  rmSync(work, { recursive: true, force: true });
}
const passed = report();
// A clean checkout's whole film is kept by its commit, for nightly to bank its rows (tools/scorecard/films.ts).
const whole = [...PACES, LAKE].every((pace) => finished(pace.name)) && finished("bones/bones");
if (!args.includes("--check") && whole) console.log(`yard: ${keep(out, filmedThrough(filmed, stamp()), CONTROLLER_FILMS)}`);
process.exit(passed && whole ? 0 : 1);

/// Whether a run went to its end: its log, written as it ran, says it saved its frames.
function finished(name: string): boolean {
  const log = join(out, `${name}.log`);
  return existsSync(log) && readFileSync(log, "utf8").includes("SANDBOX saved");
}

/// The checks on each pace's record and log, a line each, kept as checks.txt and checks.json.
/// A run that did not finish (stopped, or killed part way) leaves its own rows not measured,
/// a line saying so with no number, and the others are judged on the records they wrote.
function report(): boolean {
  const unmeasured = (name: string, run: string): Check => ({ name, ok: false, line: `✗ not measured: the ${run} run did not finish (its log ${join(out, `${run}.log`)})`, value: NaN, faults: 1 });
  const stopped = PACES.find((pace) => !finished(pace.name));
  const walks: Walk[] = PACES.map(({ name }) => {
    if (stopped) return { name, frames: [], route: [], ground: [] };
    const log = readFileSync(join(out, `${name}.log`), "utf8");
    const route = [...log.matchAll(/Route: (.*?)\.?\r?$/gm)].map((m) => m[1]);
    const ground = (/Course: ground every metre east from the start: ([-\d. ]+)/.exec(log)?.[1] ?? "").trim().split(" ").map(Number);
    return { name, frames: read(join(out, `walk-${name}.jsonl`)), route, ground };
  });
  // A stopped run's rows by their names alone (the checks of no frames), each not measured.
  const yard = stopped ? check(walks).map((each) => unmeasured(each.name, stopped.name)) : check(walks);
  const swim = finished(LAKE.name) ? pose({ name: LAKE.name, frames: read(join(out, `walk-${LAKE.name}.jsonl`)), route: [], ground: [] }) : unmeasured("swim pose", LAKE.name);
  const lines = [...yard, swim, ...bonesReport(join(out, "bones"), finished("bones/bones"))];
  const said = lines.map((each) => `${each.name}: ${each.line}`);
  for (const line of said) console.log(`yard: ${line}`);
  writeFileSync(join(out, "checks.txt"), said.join("\n") + "\n");
  writeFileSync(join(out, "checks.json"), JSON.stringify(lines, null, 1) + "\n");
  return lines.every((each) => each.ok);
}

/// One pace's run to its end, its frames in work/<pace>, its record and log in out.
async function film(pace: Run, work: string): Promise<{ name: string; ok: boolean; log: string }> {
  const frames = join(work, pace.name);
  mkdirSync(frames, { recursive: true });
  const shot = join(work, `${pace.name}.json`);
  writeFileSync(shot, JSON.stringify({ about: `the game's camera, at a ${pace.name}`, seconds: (pace.to - ROLLS) / 60 + 1, keys: [], look: { shutter: 90 } }, null, 2));
  const [command, words] = offscreenRun(`yard-${pace.name}`, [
    ...pace.args,
    ...["--fight-log", offscreenPath(join(out, `walk-${pace.name}.jsonl`)), "--picture", PICTURE],
    ...["--cine", `${offscreenPath(shot)}@${ROLLS}`, "--shots", `${offscreenPath(frames)}/f@${FROM}-${pace.to}/2`],
  ]);
  const log = join(out, `${pace.name}.log`);
  const said = await streamed({ tag: "yard", name: pace.name, command, words, log, cwd: ROOT, env: { ...process.env, BEVY_ASSET_ROOT: ROOT } });
  return { name: pace.name, ok: said.includes("SANDBOX saved"), log };
}

/// Runs' films side by side at 30 a second, each named in its corner.
function sideBySide(work: string, PACES: Run[], name: string): string {
  const font = spawnSync("fc-match", ["-f", "%{file}", "DejaVu Sans:bold"], { encoding: "utf8" }).stdout.trim();
  const inputs = PACES.flatMap((pace) => ["-framerate", "30", "-pattern_type", "glob", "-i", `${join(work, pace.name)}/f-*.png`]);
  const named = PACES.map((pace, i) => `[${i}]drawtext=fontfile=${font}:text='${pace.name}':x=14:y=12:fontsize=22:fontcolor=white:borderw=2:bordercolor=black@0.7[c${i}]`);
  const graph = [...named, `${PACES.map((_, i) => `[c${i}]`).join("")}${PACES.length > 1 ? `hstack=inputs=${PACES.length}` : "null"}[v]`].join(";");
  const mp4 = join(out, name);
  const made = spawnSync("ffmpeg", ["-y", "-loglevel", "error", ...inputs, "-filter_complex", graph, "-map", "[v]", "-an", "-c:v", "libx264", "-preset", "slow", "-crf", "24", "-pix_fmt", "yuv420p", "-movflags", "+faststart", mp4], { encoding: "utf8" });
  if (made.status !== 0 || !existsSync(mp4)) throw new Error(`ffmpeg failed: ${made.stderr.slice(0, 400)}`);
  return mp4;
}
