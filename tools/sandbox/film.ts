#!/usr/bin/env node
// A video of a scripted run, to see how the world looks and moves rather than judging it
// from stills: every second frame of a run saved with --shots, which steps the game a
// sixtieth of a second a frame, put together at 30 frames a second, so it plays at the
// game's own speed however slowly it was drawn.
//
//   node tools/sandbox/film.ts [--place lush|lake] [--frames 300-2700] [--time 15] [--date 10-15] [--out shots/film.mp4] [-- <sandbox options>]
//
// Without sandbox options it walks the bench's route through the lush forest, the world
// and the walker alone; with --place, that place's own route (places.ts). Needs ffmpeg. Node 22.18
// or later; no dependencies.

import { spawnSync } from "node:child_process";
import { mkdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { ROOT } from "./client.ts";
import { place } from "./places.ts";

const ROUTE = ["-58,-39", "-40,-69", "-6,-80", "30,-74", "59,-54", "77,-22", "79,14"].flatMap((at) => ["--route", at]);
// Halfway, a look up into the canopy and back down, as the walk bench does (D185).
const LOOK = ["--press", "up@1500-1545", "--press", "down@1650-1695"];

const args = process.argv.slice(2);
const dash = args.indexOf("--");
const own = dash < 0 ? args : args.slice(0, dash);
const arg = (name: string, fallback: string) => {
  const at = own.indexOf(name);
  return at >= 0 ? own[at + 1] : fallback;
};
const hour = arg("--time", "15");
// A day of the year to film, as the sandbox's --date takes it (docs/climate.md).
const date = arg("--date", "");
const dated = date ? ["--date", date] : [];
// A place's own route (places.ts, D326), walked from where it starts.
const placed = own.includes("--place") ? place(arg("--place", "lush")) : undefined;
const walked = placed?.route
  ? [...placed.args, "--time", hour, ...dated, "--at", placed.route.at, "--face", placed.route.face, ...placed.route.through.split(";").flatMap((at) => ["--route", at])]
  : undefined;
const sandbox =
  dash >= 0 ? args.slice(dash + 1) : (walked ?? ["--experiment", "walk", "--experiment", "lush", "--level", "yard", "--time", hour, ...dated, ...ROUTE, ...LOOK]);
const [first, last] = arg("--frames", "300-2700").split("-").map(Number);
const out = resolve(ROOT, arg("--out", "shots/film.mp4"));
const frames = resolve(dirname(out), "film-frames");
rmSync(frames, { recursive: true, force: true });
mkdirSync(frames, { recursive: true });

const build = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: ROOT, stdio: "inherit" });
if (build.status !== 0) process.exit(1);
const started = Date.now();
const run = spawnSync(resolve(ROOT, "target/release/terra_sandbox"), ["--offscreen", "--clean", ...sandbox, "--shots", `${frames}/frame@${first}-${last}/2`], {
  cwd: ROOT,
  encoding: "utf8",
  env: { ...process.env, BEVY_ASSET_ROOT: ROOT },
  maxBuffer: 64 * 1024 * 1024,
});
if (run.status !== 0 || !`${run.stdout}`.includes("SANDBOX saved")) {
  // The whole log beside the film, and its errors here: a run that dies early, as one out of
  // the GPU's memory does, ends with nothing but its shutdown's warnings.
  const log = `${run.stdout}${run.stderr}`;
  writeFileSync(`${out}.log`, log);
  const errors = log.split("\n").filter((l) => /ERROR|panicked/.test(l));
  console.error([...new Set(errors)].slice(0, 10).join("\n"));
  console.error(log.split("\n").slice(-20).join("\n"));
  console.error(`film: the run failed; its log is ${out}.log`);
  process.exit(1);
}
console.log(`film: ${Math.floor((last - first) / 2) + 1} frames drawn in ${((Date.now() - started) / 1000).toFixed(0)} s`);
// How far a route's walker got, so a film whose walker stalled or gave up a point says so.
const route = `${run.stdout}${run.stderr}`.split("\n").filter((l) => l.includes("Route: ")).map((l) => l.slice(l.indexOf("Route: ") + 7));
if (route.length) console.log(`film: route ${route.join(" ")}`);
// How far a time-lapse's settlement got as it ran (D381), so one whose builders idled says so.
for (const line of `${run.stdout}${run.stderr}`.split("\n").filter((l) => l.includes("Time-lapse at "))) {
  console.log(`film: ${line.slice(line.indexOf("Time-lapse at "))}`);
}

const encode = spawnSync(
  "ffmpeg",
  ["-y", "-loglevel", "error", "-framerate", "30", "-pattern_type", "glob", "-i", `${frames}/frame-*.png`, "-c:v", "libx264", "-preset", "slow", "-crf", "21", "-pix_fmt", "yuv420p", "-movflags", "+faststart", out],
  { stdio: "inherit" },
);
if (encode.status !== 0) process.exit(1);
rmSync(frames, { recursive: true, force: true });
console.log(`film: ${out}, ${(statSync(out).size / 1e6).toFixed(1)} MB, ${((last - first) / 60).toFixed(0)} s`);
