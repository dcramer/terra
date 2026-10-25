#!/usr/bin/env node
// The forest walk's check (D467): one scripted run of `--experiment forest-walk` that walks the
// forest film's path in summer at noon, flips to winter, steps to dusk and night, cycles the
// weather to snowfall and presses feedback, then holds the run to what it said and wrote: each
// change in the log, the snow lying after the flip, the feedback note and picture. With
// --film, every second frame becomes a phone-safe video and three stills.
//
//   node tools/sandbox/forest-walk.ts [--film <out.mp4>]
//
// Needs ffmpeg for --film. Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { ROOT } from "./client.ts";

const args = process.argv.slice(2);
const filmAt = args.indexOf("--film");
const film = filmAt >= 0 ? resolve(args[filmAt + 1]) : undefined;

// The forest film's path (trailer/runs/forest.json), from where the walk starts.
const ROUTE = "-128.6,-88.4;-102,-74;-79.1,-54.4;-59.7,-40.2;-36.5,-21.1;-14.9,-10.1;3.7,4.7;16.8,25;36.7,47.5";
// The timeline, in the game's frames (60 a second) from when the run goes.
const FIRST = 300;
const WINTER = 1100;
const DUSK = 1600;
const NIGHT = 1900;
// The weather key until snowfall comes round: what each press finds is the climate's.
const WEATHERS = [2200, 2260, 2320, 2380, 2440, 2500, 2560];
const FEEDBACK = 2780;
const LAST = 2940;
// Stills: summer on the path, winter at dusk, the end in snowfall.
const STILLS = { summer: 900, dusk: 1800, snowfall: 2900 };

const out = resolve(ROOT, "shots/forest-walk");
const feedback = join(out, "feedback");
const frames = join(out, "frames");
rmSync(out, { recursive: true, force: true });
mkdirSync(feedback, { recursive: true });
mkdirSync(frames, { recursive: true });

const build = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: ROOT, stdio: "inherit" });
if (build.status !== 0) process.exit(1);
console.log("forest-walk: built");

const press = (key: string, frame: number) => ["--press", `${key}@${frame}`];
// One run of the walk, pressing the weather key so many times; what it said, in order.
function walk(weathers: number, filmed: boolean): string[] {
  rmSync(feedback, { recursive: true, force: true });
  rmSync(frames, { recursive: true, force: true });
  mkdirSync(feedback, { recursive: true });
  mkdirSync(frames, { recursive: true });
  const shots = filmed ? `${frames}/frame@${FIRST}-${LAST}/2` : `${frames}/frame@${Object.values(STILLS).join(",")},${LAST}`;
  const sandbox = [
    "--offscreen",
    "--clean",
    "--experiment",
    "forest-walk",
    ...ROUTE.split(";").flatMap((at) => ["--route", at]),
    ...press("F6", WINTER),
    ...press("F7", DUSK),
    ...press("F7", NIGHT),
    ...WEATHERS.slice(0, weathers).flatMap((frame) => press("F8", frame)),
    ...press("p", FEEDBACK),
    "--shots",
    shots,
  ];
  const started = Date.now();
  const run = spawnSync(resolve(ROOT, "target/release/terra_sandbox"), sandbox, {
    cwd: ROOT,
    encoding: "utf8",
    env: { ...process.env, BEVY_ASSET_ROOT: ROOT, TERRA_FEEDBACK_DIR: feedback },
    maxBuffer: 256 * 1024 * 1024,
  });
  const log = `${run.stdout}${run.stderr}`;
  writeFileSync(join(out, "run.log"), log);
  if (run.status !== 0) {
    console.error(log.split("\n").filter((l) => /ERROR|panicked/.test(l)).slice(0, 10).join("\n"));
    console.error(`✗ forest-walk: the run failed; its log is ${join(out, "run.log")}`);
    process.exit(1);
  }
  console.log(`forest-walk: ran${filmed ? " filmed" : ""} in ${((Date.now() - started) / 1000).toFixed(0)} s`);
  const lines = log
    .split("\n")
    .filter((l) => l.includes("forest-walk: "))
    .map((l) => l.slice(l.indexOf("forest-walk: ") + 13).trim());
  writeFileSync(join(out, "said.txt"), `${lines.join("\n")}\n`);
  return lines;
}

// The weather presses that reach snowfall are the climate's: a first run learns how many, and
// a film is made with that many, so it ends in snowfall (the runs are the same frame by frame).
let said = walk(WEATHERS.length, false);
const reached = said.filter((l) => /^\S[^,]*, (today|\d+ days)/.test(l) && !/^(dusk|night|dawn|noon),/.test(l));
const toSnow = reached.findIndex((l) => l.startsWith("snowfall,")) + 1;
console.log(`forest-walk: weathers ${reached.map((l) => l.split(",")[0]).join(" → ")}`);
if (toSnow > 0 && (film || toSnow < WEATHERS.length)) said = walk(toSnow, Boolean(film));
const log = readFileSync(join(out, "run.log"), "utf8");
const failures: string[] = [];
const expect = (what: string, ok: boolean) => {
  if (!ok) failures.push(what);
  console.log(`${ok ? "✓" : "✗"} ${what}`);
};
const at = (pattern: RegExp, from = 0) => said.findIndex((l, i) => i >= from && pattern.test(l));
const summer = at(/^summer 07-20 .* 12:0\d /);
expect("starts in summer, 07-20 at noon", summer >= 0);
const winter = at(/^winter 01-15 /, summer);
expect("the season key flips to winter, 01-15", winter > summer);
const snow = said.slice(winter).map((l) => l.match(/^snow lying (\d+)%/)).find((m) => m && Number(m[1]) >= 30);
expect(`snow lies round the eye in winter (${snow ? snow[1] : "none"}%)`, Boolean(snow));
const dusk = at(/^dusk, /, winter);
expect("the time key steps to dusk", dusk > winter);
const night = at(/^night, 23:00/, dusk);
expect("then to night", night > dusk);
const snowfall = at(/^snowfall, /, night);
expect("the weather key comes round to snowfall", snowfall > night);
const notes = readdirSync(feedback).filter((n) => n.endsWith(".json"));
const pictures = readdirSync(feedback).filter((n) => n.endsWith(".png"));
expect("feedback 1 saved, its note and picture", notes.length === 1 && pictures.length === 1 && notes[0].startsWith("1-"));
if (notes.length === 1) {
  const note = JSON.parse(readFileSync(join(feedback, notes[0]), "utf8"));
  const walked = Math.hypot(note.player.at[0] + 154, note.player.at[2] + 104.4);
  expect(`the note: ${note.moment}, walked ${walked.toFixed(0)} m`, walked > 20 && note.weather === "snowfall" && note.season === "winter");
  expect("the note says how to stand there again", /--date \d+ --time [\d.]+ --at /.test(note.again) && /--vantage /.test(note.again_view));
  const animals = note.animals_within_50_m.map((a: { kind: string; distance_m: number }) => `${a.kind} ${a.distance_m.toFixed(0)} m`);
  console.log(`forest-walk: animals within 50 m at the feedback: ${animals.join(", ") || "none"}`);
}
const met = log.split("\n").filter((l) => /Wildlife: /.test(l)).length;
console.log(`forest-walk: ${met} wildlife lines in the log`);

if (film) {
  mkdirSync(dirname(film), { recursive: true });
  const encode = spawnSync(
    "ffmpeg",
    ["-y", "-loglevel", "error", "-framerate", "30", "-pattern_type", "glob", "-i", `${frames}/frame-*.png`,
      "-an", "-r", "30", "-c:v", "libx264", "-profile:v", "main", "-level", "4.0", "-pix_fmt", "yuv420p", "-preset", "medium",
      "-crf", "27", "-g", "60", "-bf", "2", "-refs", "3", "-vf", "scale=1600:-2", "-movflags", "+faststart", film],
    { stdio: "inherit" },
  );
  expect("the film encoded", encode.status === 0 && existsSync(film));
  if (existsSync(film)) console.log(`forest-walk: ${film}, ${(statSync(film).size / 1e6).toFixed(1)} MB`);
}
// The stills as jpgs, beside the film or in the shots.
const stillsTo = film ? dirname(film) : out;
const all = readdirSync(frames).filter((n) => n.endsWith(".png")).sort();
for (const [name, frame] of Object.entries(STILLS)) {
  const file = all.find((n) => n.includes(String(frame).padStart(5, "0")) || n.endsWith(`-${frame}.png`));
  if (!file) {
    failures.push(`still ${name}: no frame ${frame}`);
    continue;
  }
  spawnSync("ffmpeg", ["-y", "-loglevel", "error", "-i", join(frames, file), "-vf", "scale=1600:-2", "-q:v", "3", join(stillsTo, `${name}.jpg`)]);
}
if (failures.length) {
  console.error(`✗ forest-walk: ${failures.length} failed; ${join(out, "said.txt")} has what the walk said`);
  process.exit(1);
}
console.log("✓ forest-walk");
