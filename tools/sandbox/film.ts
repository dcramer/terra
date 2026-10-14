#!/usr/bin/env node
// A video of a scripted run, to see how the world looks and moves rather than judging it
// from stills: every second frame of a run saved with --shots, which steps the game a
// sixtieth of a second a frame, put together at 30 frames a second, so it plays at the
// game's own speed however slowly it was drawn.
//
//   node tools/sandbox/film.ts [--frames 300-2700] [--time 15] [--out shots/film.mp4] [-- <sandbox options>]
//
// Without sandbox options it walks the bench's route through the lush forest, the world
// and the walker alone. Needs ffmpeg. Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { mkdirSync, rmSync, statSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { ROOT } from "./client.ts";

const ROUTE = ["-58,-39", "-40,-69", "-6,-80", "30,-74", "59,-54", "77,-22", "79,14"].flatMap((at) => ["--route", at]);

const args = process.argv.slice(2);
const dash = args.indexOf("--");
const own = dash < 0 ? args : args.slice(0, dash);
const arg = (name: string, fallback: string) => {
  const at = own.indexOf(name);
  return at >= 0 ? own[at + 1] : fallback;
};
const hour = arg("--time", "15");
const sandbox = dash < 0 ? ["--experiment", "walk", "--experiment", "lush", "--level", "yard", "--time", hour, ...ROUTE] : args.slice(dash + 1);
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
  console.error(`${run.stdout}${run.stderr}`.split("\n").slice(-20).join("\n"));
  process.exit(1);
}
console.log(`film: ${Math.floor((last - first) / 2) + 1} frames drawn in ${((Date.now() - started) / 1000).toFixed(0)} s`);

const encode = spawnSync(
  "ffmpeg",
  ["-y", "-loglevel", "error", "-framerate", "30", "-pattern_type", "glob", "-i", `${frames}/frame-*.png`, "-c:v", "libx264", "-preset", "slow", "-crf", "21", "-pix_fmt", "yuv420p", "-movflags", "+faststart", out],
  { stdio: "inherit" },
);
if (encode.status !== 0) process.exit(1);
rmSync(frames, { recursive: true, force: true });
console.log(`film: ${out}, ${(statSync(out).size / 1e6).toFixed(1)} MB, ${((last - first) / 60).toFixed(0)} s`);
