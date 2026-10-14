#!/usr/bin/env node
// Finds what pops as levels of detail change (D69, D230): a crown clipping out, a shadow
// clipping in, a ring of grass, a bush appearing. Each view is held still, with the clock and
// the wind stopped, while every distance at which detail changes is swept from half its own
// to twice it (--detail-sweep), so every switch is crossed in front of a still camera and
// anything that changes between two frames is detail changing. A good transition changes a
// few scattered pixels a frame, which a morphological opening removes; a pop is a patch that
// appears or goes at once, which survives it.
//
//   node tools/sandbox/lodcheck.ts [--views floor,path,clearing] [--time 17] [--limit 0.5]
//
// Prints each view's worst frames, with the share of the picture that popped and the detail
// scale it popped at (1 is as played: a pop at 1.3 happens 1.3 times as far off as the
// switch's own distance, when the player walks through it), and puts them side by side,
// marked red, in shots/lodcheck/sheet.jpg. Fails if any frame pops more than --limit percent.
// About fifteen seconds a view. Needs ImageMagick. Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { mkdirSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { ROOT } from "./client.ts";

/// Each view: where the camera stands and what it looks at, east, up and south of the start.
const VIEWS: Record<string, string> = {
  floor: "-56,1.7,-43,-72,1.0,-55",
  path: "-6,1.7,-80,40,2.0,-74",
  clearing: "7,1.8,9,-8,1.2,-7",
  edge: "0,1.8,250,0,5,200",
};
const SWEEP = { from: 0.5, to: 2.0, first: 300, last: 540 };
/// Frames after the clock stops before comparing, for the wind and anything easing to settle.
const SETTLE = 6;
/// Frames are compared at half size; a pixel counts as changed past this much brightness,
/// and a patch must be this wide to survive.
const WIDTH = 960;
const CHANGED = "4%";
const SPECKLE = "Disk:2.5";

const arg = (name: string, fallback: string) => {
  const at = process.argv.indexOf(name);
  return at > 0 ? process.argv[at + 1] : fallback;
};
const views = arg("--views", "floor,path,clearing").split(",");
const hour = arg("--time", "17");
const limit = Number(arg("--limit", "0.5"));
const out = resolve(ROOT, "shots/lodcheck");
rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

const magick = (...words: string[]) => {
  const done = spawnSync("magick", words, { encoding: "utf8" });
  if (done.status !== 0) throw new Error(`magick ${words.join(" ")}: ${done.stderr}`);
  return done.stdout.trim();
};
const scaleAt = (frame: number) => {
  const t = Math.min(1, Math.max(0, (frame - SWEEP.first) / (SWEEP.last - SWEEP.first)));
  return Math.exp(Math.log(SWEEP.from) * (1 - t) + Math.log(SWEEP.to) * t);
};

const build = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: ROOT, stdio: "inherit" });
if (build.status !== 0) process.exit(1);

type Pop = { view: string; frame: number; share: number; scale: number };
const worst: Pop[] = [];
let failed = false;
for (const view of views) {
  const at = VIEWS[view];
  if (!at) throw new Error(`no view ${view}; there are ${Object.keys(VIEWS).join(", ")}`);
  const dir = resolve(out, view);
  mkdirSync(dir);
  const run = spawnSync(
    resolve(ROOT, "target/release/terra_sandbox"),
    [
      "--offscreen", "--clean", "--experiment", "walk", "--experiment", "lush", "--level", "yard", "--time", hour,
      "--vantage", at, "--detail-sweep", `${SWEEP.from}-${SWEEP.to}@${SWEEP.first}-${SWEEP.last}`,
      "--shots", `${dir}/f@${SWEEP.first + SETTLE - 1}-${SWEEP.last}`,
    ],
    { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT } },
  );
  const said = `${run.stdout}${run.stderr}`;
  if (run.status !== 0 || !said.includes("SANDBOX saved")) {
    console.error(said.split("\n").slice(-20).join("\n"));
    process.exit(1);
  }
  if (said.includes("distinct `VisibilityRange`s")) {
    console.log(`lodcheck: ${view}: more distinct detail ranges than Bevy can draw; some are culled wrongly`);
    failed = true;
  }
  const file = (frame: number) => resolve(dir, `f-${String(frame).padStart(5, "0")}.png`);
  const pops: Pop[] = [];
  for (let frame = SWEEP.first + SETTLE; frame <= SWEEP.last; frame++) {
    const share = Number(
      magick(file(frame - 1), file(frame), "-resize", `${WIDTH}x`, "-compose", "difference", "-composite", "-colorspace", "gray", "-threshold", CHANGED, "-morphology", "Open", SPECKLE, "-format", "%[fx:mean*100]", "info:"),
    );
    pops.push({ view, frame, share, scale: scaleAt(frame) });
  }
  pops.sort((a, b) => b.share - a.share);
  const over = pops.filter((p) => p.share > limit);
  const mean = pops.reduce((sum, p) => sum + p.share, 0) / pops.length;
  console.log(`lodcheck: ${view}: mean ${mean.toFixed(3)}% a frame, worst ${pops[0].share.toFixed(2)}% at scale ${pops[0].scale.toFixed(2)}; ${over.length} frames over ${limit}%`);
  for (const pop of pops.slice(0, 3)) console.log(`lodcheck: ${view}: frame ${pop.frame}: ${pop.share.toFixed(2)}% popped at scale ${pop.scale.toFixed(2)}`);
  if (over.length) failed = true;
  worst.push(...pops.slice(0, 2));
}

// The worst frames, each with what popped marked red over it.
const tiles: string[] = [];
for (const pop of worst) {
  const dir = resolve(out, pop.view);
  const file = (frame: number) => resolve(dir, `f-${String(frame).padStart(5, "0")}.png`);
  const mask = resolve(out, `mask-${pop.view}-${pop.frame}.png`);
  const mark = resolve(out, `mark-${pop.view}-${pop.frame}.jpg`);
  magick(file(pop.frame - 1), file(pop.frame), "-resize", `${WIDTH}x`, "-compose", "difference", "-composite", "-colorspace", "gray", "-threshold", CHANGED, "-morphology", "Open", SPECKLE, "-morphology", "Dilate", "Disk:3", "-colorspace", "sRGB", "-fill", "red", "-opaque", "white", "-transparent", "black", mask);
  magick(file(pop.frame), "-resize", `${WIDTH}x`, mask, "-composite", mark);
  tiles.push("-label", `${pop.view}, frame ${pop.frame}: ${pop.share.toFixed(2)}% at scale ${pop.scale.toFixed(2)}`, mark);
}
const sheet = resolve(out, "sheet.jpg");
magick("montage", ...tiles, "-tile", "2x", "-geometry", "+4+4", "-pointsize", "16", "-background", "#222", "-fill", "#eee", sheet);
console.log(`lodcheck: ${sheet}`);
console.log(failed ? `lodcheck: FAIL: something pops more than ${limit}% of the picture in one frame` : "lodcheck: pass");
process.exit(failed ? 1 : 0);
