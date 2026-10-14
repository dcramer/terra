#!/usr/bin/env node
// Finds what pops in or out, or flickers for a frame, in a stretch of a scripted run (D69):
// level-of-detail switches, shadows that jump, things that vanish. It saves consecutive
// frames with --shots, then compares each with the ones either side, small and blurred so
// that the camera's own smooth motion cancels out:
//
// - a pop changes a frame from the one before but not into the one after;
// - a flicker changes a frame from both neighbours, which agree with each other.
//
// Each frame gets the share of the picture that popped and that flickered; the worst are put
// side by side with where it happened marked, in <out>/pops.jpg.
//
//   node tools/sandbox/pops.ts [--frames 1200-1320] [--out shots/pops] [-- <sandbox options>]
//
// Without sandbox options it walks the bench's route through the lush forest at 17:00. A
// planted grey patch 20 pixels square in one frame scores about 1%; a smooth walk scores
// under 0.2%. Needs ImageMagick (cargo xtask doctor). Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { mkdirSync, readdirSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { ROOT } from "./client.ts";

const WALK = ["--experiment", "walk", "--experiment", "lush", "--level", "yard", "--rolling", "--time", "17", "--route", "-58,-39", "--route", "-40,-69", "--route", "-6,-80", "--route", "30,-74", "--route", "59,-54"];
/// Frames are compared this small, blurred this much, and a pixel counts as changed past
/// this much of full brightness.
const SMALL = "240x135";
const BLUR = "0x1.5";
const CHANGED = "8%";
/// Frames above this share count as having popped or flickered.
const EVENT = 0.5;

const args = process.argv.slice(2);
const dash = args.indexOf("--");
const own = dash < 0 ? args : args.slice(0, dash);
const sandbox = dash < 0 ? WALK : args.slice(dash + 1);
const arg = (name: string, fallback: string) => {
  const at = own.indexOf(name);
  return at >= 0 ? own[at + 1] : fallback;
};
const [first, last] = arg("--frames", "1200-1320").split("-").map(Number);
const out = resolve(ROOT, arg("--out", "shots/pops"));
rmSync(out, { recursive: true, force: true });
mkdirSync(resolve(out, "small"), { recursive: true });

const magick = (...words: string[]) => {
  const done = spawnSync("magick", words, { encoding: "utf8" });
  if (done.status !== 0) throw new Error(`magick ${words.join(" ")}: ${done.stderr}`);
  return done.stdout.trim();
};

const build = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: ROOT, stdio: "inherit" });
if (build.status !== 0) process.exit(1);
const run = spawnSync(resolve(ROOT, "target/release/terra_sandbox"), ["--offscreen", "--clean", ...sandbox, "--shots", `${out}/frame@${first}-${last}`], {
  cwd: ROOT,
  encoding: "utf8",
  env: { ...process.env, BEVY_ASSET_ROOT: ROOT },
});
if (run.status !== 0 || !`${run.stdout}`.includes("SANDBOX saved")) {
  console.error(`${run.stdout}${run.stderr}`.split("\n").slice(-20).join("\n"));
  process.exit(1);
}

const name = (frame: number) => `frame-${String(frame).padStart(5, "0")}.png`;
for (const file of readdirSync(out).filter((f) => f.endsWith(".png"))) {
  magick(resolve(out, file), "-resize", SMALL, "-colorspace", "gray", "-blur", BLUR, resolve(out, "small", file));
}
const small = (frame: number) => resolve(out, "small", name(frame));
const difference = (a: number, b: number, to: string) => magick(small(a), small(b), "-compose", "difference", "-composite", to);
const share = (...words: string[]) => Number(magick(...words, "-threshold", CHANGED, "-format", "%[fx:mean*100]", "info:"));

type Score = { frame: number; pop: number; flicker: number };
const scores: Score[] = [];
const [d1, d2, d3] = ["d1", "d2", "d3"].map((d) => resolve(out, `${d}.png`));
for (let frame = first + 1; frame < last; frame++) {
  difference(frame, frame - 1, d1);
  difference(frame + 1, frame, d2);
  difference(frame + 1, frame - 1, d3);
  const pop = share(d1, d2, "-compose", "minus_src", "-composite");
  const flicker = share(d1, d2, "-compose", "darken", "-composite", d3, "-compose", "minus_src", "-composite");
  scores.push({ frame, pop, flicker });
}

const worst = [...scores].sort((a, b) => Math.max(b.pop, b.flicker) - Math.max(a.pop, a.flicker)).slice(0, 6);
const events = scores.filter((s) => Math.max(s.pop, s.flicker) > EVENT);
const median = (values: number[]) => [...values].sort((a, b) => a - b)[Math.floor(values.length / 2)];
console.log(`pops: frames ${first}-${last}: median pop ${median(scores.map((s) => s.pop)).toFixed(2)}%, flicker ${median(scores.map((s) => s.flicker)).toFixed(2)}%; ${events.length} frames over ${EVENT}%`);
for (const s of worst) console.log(`pops: frame ${s.frame}: pop ${s.pop.toFixed(2)}%, flicker ${s.flicker.toFixed(2)}%`);

// The worst frames, each beside where it changed, marked in red over the frame before.
const tiles: string[] = [];
for (const s of worst) {
  const mark = resolve(out, `mark-${s.frame}.png`);
  difference(s.frame, s.frame - 1, d1);
  magick(resolve(out, name(s.frame)), "-resize", "640x360", "(", d1, "-resize", "640x360", "-threshold", CHANGED, "-colorspace", "sRGB", "-type", "TrueColorAlpha", "-fill", "red", "-opaque", "white", "-transparent", "black", ")", "-compose", "over", "-composite", mark);
  tiles.push("-label", `frame ${s.frame}: pop ${s.pop.toFixed(2)}%, flicker ${s.flicker.toFixed(2)}%`, mark);
}
const sheet = resolve(out, "pops.jpg");
magick("montage", ...tiles, "-tile", "3x", "-geometry", "+4+4", "-pointsize", "16", "-background", "#222", "-fill", "#eee", sheet);
console.log(`pops: ${sheet}`);
