#!/usr/bin/env node
// The yard's duel, filmed and checked (docs/movement.md's Look row): the player driven by
// the fight script (`--duel duels/yard.ron`) against the sparring partner, run four times
// side by side, each filmed from one camera by `--cine` (the HUD hidden, the exposure and
// clock held): from the side, from behind the player's right shoulder and raised, from
// above, and the game's own camera behind the player. Each run writes its fight record
// (`--fight-log`); the four must be the same fight, and the checks in duel/checks.ts read
// the first, a line each, ✗ on a failure (D388). Then a contact sheet of each step's
// wind-up, contact and recovery from the side and three-quarter cameras (duel/moments.ts),
// and the four films in a 2x2 grid with the step's name, with a copy under 15 MB for the
// review desk.
//
//   cargo xtask duel [--out <dir>]          run, film, sheet and check
//   cargo xtask duel --check <fight.jsonl>…  only the checks, on records already made
//
// Into ~/.cache/terra-review/duel/ by default: duel.mp4, duel-desk.mp4, sheet.jpg,
// checks.txt (and checks.json, its numbers), and each camera's fight-<camera>.jsonl and log.
// Exits 1 when a check fails.
// Under WSL the runs are the Windows build, on the real GPU (client.ts `offscreenRun`);
// start it as `cargo xtask gpu film -- cargo xtask duel` (D319). Needs ffmpeg and
// ImageMagick. Node 22.18 or later; no dependencies.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { ROOT, buildOffscreen, imageMagick, offscreenPath, offscreenRun } from "./client.ts";
import { type Check, type Frame, check, framed, read, readMoves, same, say, share } from "./duel/checks.ts";
import { COLUMNS, moments } from "./duel/moments.ts";
import { staggerJump } from "./duel/shield.ts";

const MOVES = resolve(ROOT, "assets/data/moves/sword.shielded.ron");
/// The player's share of the frame's height the game's camera is held to (D180).
const SHARE: number[] = JSON.parse(readFileSync(resolve(ROOT, "tools/combat/refs/enshrouded.json"), "utf8")).targets.camera_player_share;
const SCRIPT = "duels/yard.ron";
/// The frames filmed, every second one kept for 30 a second: from before the draw to past
/// the script's end (frame 1274 in the first run).
const FROM = 20;
const TO = 1330;
/// The shot rolls a little before, so the exposure it holds is the yard's, not the load's.
const ROLLS = 10;
const PICTURE = "960x540";
const HOUR = "16";
const DESK_MB = 15;
const FRAMED_M = 1.5;

/// Where the cameras stand and look, metres east, up and south of the start, as the fight
/// record gives places: the fight moves within a few metres of CENTRE (the `framed` check
/// holds it there, within FRAMED_M), the fighters facing along north-west to south-east at
/// first. A camera with no keys is the game's own.
type Camera = { name: string; about: string; keys: { t: number; at: number[]; look: number[]; lens: number }[] };
const CENTRE = [7.2, 0.8, 13.9];
/// A camera standing so far east, up and south of CENTRE, looking at it or so far from it.
const from = (offset: number[], lens: number, look = [0, 0, 0]) => [
  { t: 0, at: offset.map((v, i) => CENTRE[i] + v), look: look.map((v, i) => CENTRE[i] + v), lens },
];
const CAMERAS: Camera[] = [
  { name: "side", about: "from the south-west, square to the fighters' line", keys: from([-5.7, 0.5, 5.7], 35) },
  { name: "three-quarter", about: "behind the player's right shoulder, raised", keys: from([4.6, 1.6, 1.9], 32, [-0.2, 0, -0.5]) },
  { name: "above", about: "high over the yard, looking down", keys: from([1.5, 9.0, 2.7], 28, [0, -0.6, 0]) },
  { name: "player", about: "the game's camera behind the player", keys: [] },
];

const args = process.argv.slice(2);
if (args.includes("--help") || args.includes("-h")) {
  console.log("duel.ts [--out <dir>] | --check <fight.jsonl>…");
  process.exit(0);
}
const opt = (name: string) => (args.includes(name) ? args[args.indexOf(name) + 1] : undefined);
const out = resolve(opt("--out") ?? join(homedir(), ".cache/terra-review/duel"));
const started = Date.now();
const took = () => `${((Date.now() - started) / 1000).toFixed(0)} s`;

if (args.includes("--check")) {
  const files = args.slice(args.indexOf("--check") + 1).filter((each) => !each.startsWith("--"));
  const records = files.map((file) => ({ name: file, frames: read(file) }));
  process.exit(report(records, undefined) ? 0 : 1);
}

mkdirSync(out, { recursive: true });
if (!buildOffscreen()) process.exit(1);
console.log(`duel: built (${took()})`);
const work = resolve(ROOT, "shots/duel");
rmSync(work, { recursive: true, force: true });
const filmed = await Promise.all(CAMERAS.map((camera) => film(camera)));
const failed = filmed.filter((each) => !each.ok);
if (failed.length) {
  for (const each of failed) console.log(`duel: ✗ ${each.name} did not finish; its log: ${each.log}`);
  process.exit(1);
}
console.log(`duel: filmed ${CAMERAS.map((c) => c.name).join(", ")} (${took()})`);
const records = CAMERAS.map((camera) => ({ name: camera.name, frames: read(record(camera.name)) }));
const ok = report(records, join(out, "checks.txt"));
const sheet = contactSheet(records[0].frames);
console.log(`duel: sheet ${sheet} (${took()})`);
const [mp4, desk] = grid(records[0].frames);
console.log(`duel: film ${mp4}, ${mb(mp4)} MB; for the desk ${desk}, ${mb(desk)} MB (${took()})`);
rmSync(work, { recursive: true, force: true });
process.exit(ok ? 0 : 1);

function record(name: string) {
  return join(out, `fight-${name}.jsonl`);
}

/// The checks, a line each, kept in `keep` too; whether all held.
function report(records: { name: string; frames: Frame[] }[], keep: string | undefined): boolean {
  const lines: Check[] = [];
  if (records.length > 1) lines.push(same(records));
  lines.push(...check(records[0].frames, existsSync(MOVES) ? readMoves(MOVES) : undefined));
  const jump = staggerJump(records[0].frames);
  if (jump) lines.push(jump);
  lines.push(framed(records[0].frames, CENTRE, FRAMED_M));
  // The game's own camera, the player's run (or a record named for it).
  const player = records.find((each) => each.name === "player" || each.name.endsWith("player.jsonl"));
  if (player) lines.push(share(player.frames, SHARE));
  const said = lines.map(say);
  for (const line of said) console.log(`duel: ${line}`);
  if (keep) writeFileSync(keep, said.join("\n") + "\n");
  // The same as numbers beside it, for the combat scorecard's `--film` (tools/combat).
  if (keep) writeFileSync(keep.replace(/\.txt$/, ".json"), JSON.stringify(lines, null, 1) + "\n");
  return lines.every((each) => each.ok);
}

/// One camera's run to its end, its frames in work/<camera>, its record and log in out.
async function film(camera: Camera): Promise<{ name: string; ok: boolean; log: string }> {
  const frames = join(work, camera.name);
  mkdirSync(frames, { recursive: true });
  const shot = join(work, `${camera.name}.json`);
  const look = { shutter: 90 };
  writeFileSync(shot, JSON.stringify({ about: camera.about, seconds: (TO - ROLLS) / 60 + 1, keys: camera.keys, look }, null, 2));
  const [command, words] = offscreenRun(`duel-${camera.name}`, [
    ...["--clean", "--experiment", "play", "--level", "yard", "--time", HOUR, "--face", "0,-10"],
    ...["--duel", SCRIPT, "--fight-log", offscreenPath(record(camera.name)), "--picture", PICTURE],
    ...["--cine", `${offscreenPath(shot)}@${ROLLS}`, "--shots", `${offscreenPath(frames)}/f@${FROM}-${TO}/2`],
  ]);
  const log = join(out, `${camera.name}.log`);
  const said = await run(command, words);
  writeFileSync(log, `${command} ${words.join(" ")}\n\n${said}`);
  return { name: camera.name, ok: said.includes("SANDBOX saved"), log };
}

/// A run to its end; tried again while Windows will not replace a program still closing.
async function run(command: string, words: string[]): Promise<string> {
  for (let attempt = 0; ; attempt++) {
    const said = await new Promise<string>((done, fail) => {
      const child = spawn(command, words, { cwd: ROOT, env: { ...process.env, BEVY_ASSET_ROOT: ROOT} });
      const chunks: Buffer[] = [];
      child.stdout.on("data", (chunk) => chunks.push(chunk));
      child.stderr.on("data", (chunk) => chunks.push(chunk));
      child.on("error", fail);
      child.on("close", () => done(Buffer.concat(chunks).toString("utf8")));
    });
    if (attempt >= 30 || !said.includes("Could not copy the program")) return said;
    await new Promise((done) => setTimeout(done, 2000));
  }
}

/// The saved frame nearest `n`: every second one from FROM.
function kept(n: number) {
  return Math.min(TO - ((TO - FROM) % 2), Math.max(FROM, FROM + 2 * Math.round((n - FROM) / 2)));
}
function still(camera: string, n: number) {
  return join(work, camera, `f-${String(kept(n)).padStart(5, "0")}.png`);
}

/// A row a step: its wind-up, contact and recovery from the side, then the three-quarter.
function contactSheet(frames: Frame[]): string {
  const file = join(out, "sheet.jpg");
  const tiles = moments(frames).flatMap((row) =>
    ["side", "three-quarter"].flatMap((camera) =>
      row.frames.map((n, i) => ["-label", `${row.step}: ${row.subject ? COLUMNS[i] : `+${i} s`}, ${camera}, frame ${kept(n)}`, still(camera, n)]),
    ),
  );
  const words = ["montage", ...["-font", fontFile("DejaVu Sans"), "-pointsize", "13", "-fill", "#eee", "-background", "#1b1b1b"], ...tiles.flat(), ...["-tile", "6x", "-geometry", "320x180+3+3", "-quality", "88", file]];
  const made = spawnSync(...imageMagick(words), { encoding: "utf8" });
  if (made.status !== 0) throw new Error(`the contact sheet failed: ${made.stderr.slice(0, 400)}`);
  return file;
}

/// The four films in a 2x2 grid at 30 a second, each camera named in its corner and the
/// script's step along the foot; and the desk's copy (the review-desk skill's line).
function grid(frames: Frame[]): [string, string] {
  const font = fontFile("DejaVu Sans:bold");
  const text = (words: string) => `fontfile=${font}:text='${words.replace(/[':]/g, " ")}'`;
  const inputs = CAMERAS.flatMap((camera) => ["-framerate", "30", "-pattern_type", "glob", "-i", `${join(work, camera.name)}/f-*.png`]);
  const named = CAMERAS.map((camera, i) => `[${i}]drawtext=${text(camera.name)}:x=14:y=12:fontsize=22:fontcolor=white:borderw=2:bordercolor=black@0.7[c${i}]`);
  const steps = frames.filter((frame) => frame.step !== null && frame.frame <= TO);
  const captions = steps.map((frame, i) => {
    const [a, b] = [(frame.frame - FROM) / 60, ((steps[i + 1]?.frame ?? TO) - FROM) / 60];
    return `drawtext=${text(frame.step!)}:x=(w-text_w)/2:y=h-60:fontsize=34:fontcolor=white:borderw=3:bordercolor=black@0.7:enable='between(t,${Math.max(0, a).toFixed(3)},${b.toFixed(3)})'`;
  });
  const graph = [...named, `[c0][c1][c2][c3]xstack=inputs=4:layout=0_0|w0_0|0_h0|w0_h0${captions.length ? `,${captions.join(",")}` : ""}[v]`].join(";");
  const mp4 = join(out, "duel.mp4");
  const desk = join(out, "duel-desk.mp4");
  ffmpeg([...inputs, "-filter_complex", graph, "-map", "[v]", "-an", "-c:v", "libx264", "-preset", "slow", "-crf", "18", "-pix_fmt", "yuv420p", "-movflags", "+faststart", mp4]);
  for (const crf of [28, 32, 36]) {
    ffmpeg(["-i", mp4, "-an", "-c:v", "libx264", "-crf", String(crf), "-preset", "slow", "-vf", "scale=1600:-2", "-movflags", "+faststart", desk]);
    if (statSync(desk).size < DESK_MB * 1e6) break;
  }
  if (statSync(desk).size >= DESK_MB * 1e6) console.log(`duel: ✗ the desk's copy is ${mb(desk)} MB, over ${DESK_MB}`);
  return [mp4, desk];
}

function fontFile(name: string) {
  return spawnSync("fc-match", ["-f", "%{file}", name], { encoding: "utf8" }).stdout.trim();
}

function ffmpeg(words: string[]) {
  const made = spawnSync("ffmpeg", ["-y", "-loglevel", "error", ...words], { encoding: "utf8" });
  if (made.status !== 0) throw new Error(`ffmpeg failed: ${made.stderr.slice(0, 400)}`);
}

function mb(file: string) {
  return (statSync(file).size / 1e6).toFixed(1);
}
