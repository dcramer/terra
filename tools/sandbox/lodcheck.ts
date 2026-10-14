#!/usr/bin/env node
// Finds what pops as levels of detail change (D69, D230): a crown clipping out, a shadow
// clipping in, a ring of grass, a bush appearing. Each view is held still, with the clock and
// the wind stopped, while every distance at which detail changes is swept from half its own
// to twice it (--detail-sweep), so every switch is crossed in front of a still camera and
// anything that changes between two frames is detail changing. A good transition changes a
// few scattered pixels a frame, which a morphological opening removes; a pop is a patch that
// appears or goes at once, which survives it.
//
//   node tools/sandbox/lodcheck.ts [--views floor,path,clearing,village,town] [--time 17] [--limit 0.5]
//
// Prints each view's worst frames, with the share of the picture that popped and the detail
// scale it popped at (1 is as played: a pop at 1.3 happens 1.3 times as far off as the
// switch's own distance, when the player walks through it), and puts them side by side,
// marked red, in shots/lodcheck/sheet.jpg. Fails if any frame pops more than --limit percent.
// About forty seconds for three views: they are drawn at once and compared on every core.
// Under WSL each view runs on the Windows build, on the real GPU (`offscreenRun`): WSL's own
// draws on the CPU, too slowly to finish. Needs ImageMagick. Node 22.18 or later; no dependencies.

import { spawn, spawnSync } from "node:child_process";
import { availableParallelism } from "node:os";
import { mkdirSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { ROOT, buildOffscreen, imageMagick, offscreenPath, offscreenRun } from "./client.ts";

/// Each view: where the camera stands and what it looks at, east, up and south of the start.
/// The forest's are in the lush forest; `village` is the bench's fifty buildings on the yard
/// from its corner (D213), whose sections change detail as the sweep crosses their distances.
const VIEWS: Record<string, string> = {
  floor: "-56,1.7,-43,-72,1.0,-55",
  path: "-6,1.7,-80,40,2.0,-74",
  clearing: "7,1.8,9,-8,1.2,-7",
  edge: "0,1.8,250,0,5,200",
  village: "-85,16,10,-5,0,-50",
  // Down a street of forty buildings from 25 m up, houses from 30 m to 310 m off, so the
  // sweep crosses each one's far detail handing over to its hull (D278).
  town: "-170,25,-170,150,0,-170",
};
/// What each view draws: the lush forest with the walker alone, or a view's own scene.
const WALK = ["--experiment", "walk", "--experiment", "lush", "--level", "yard"];
const SCENES: Record<string, string[]> = {
  village: ["--experiment", "play", "--level", "yard", "--village"],
  town: ["--experiment", "play", "--level", "yard", "--radius", "30000", "--town", "40"],
};
const SWEEP = { from: 0.5, to: 2.0, first: 300, last: 480 };
/// Frames after the clock stops before comparing, for the wind and anything easing to settle.
const SETTLE = 6;
/// Frames are compared at half size; a pixel counts as changed past this much brightness,
/// and a patch must be this wide to survive.
const WIDTH = 720;
const CHANGED = "4%";
const SPECKLE = "Disk:2";
/// A pixel is black under this much of white in every channel; a frame fails with more than
/// this share of them.
const BLACK = "2%";
const MOST_BLACK = 1;

const arg = (name: string, fallback: string) => {
  const at = process.argv.indexOf(name);
  return at > 0 ? process.argv[at + 1] : fallback;
};
const views = arg("--views", "floor,path,clearing,village,town").split(",");
const hour = arg("--time", "17");
const limit = Number(arg("--limit", "0.5"));
const out = resolve(ROOT, "shots/lodcheck");
rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

const magick = (...words: string[]) => {
  const done = spawnSync(...imageMagick(words), { encoding: "utf8" });
  if (done.status !== 0) throw new Error(`magick ${words.join(" ")}: ${done.stderr}`);
  return done.stdout.trim();
};
/// A command run in the background, its output when it ends.
const run = (command: string, words: string[], env?: NodeJS.ProcessEnv) =>
  new Promise<{ status: number; said: string }>((done) => {
    const child = spawn(command, words, { cwd: ROOT, env: env ?? process.env });
    let said = "";
    child.stdout.on("data", (d) => (said += d));
    child.stderr.on("data", (d) => (said += d));
    child.on("close", (status) => done({ status: status ?? 1, said }));
  });
/// Runs jobs, so many at once.
async function pool<T>(jobs: (() => Promise<T>)[], at_once: number): Promise<T[]> {
  const results: T[] = new Array(jobs.length);
  let next = 0;
  await Promise.all(
    Array.from({ length: Math.min(at_once, jobs.length) }, async () => {
      while (next < jobs.length) {
        const mine = next++;
        results[mine] = await jobs[mine]();
      }
    }),
  );
  return results;
}
const scaleAt = (frame: number) => {
  const t = Math.min(1, Math.max(0, (frame - SWEEP.first) / (SWEEP.last - SWEEP.first)));
  return Math.exp(Math.log(SWEEP.from) * (1 - t) + Math.log(SWEEP.to) * t);
};

if (!buildOffscreen()) process.exit(1);

type Pop = { view: string; frame: number; share: number; scale: number };
const worst: Pop[] = [];
let failed = false;
for (const view of views) if (!VIEWS[view]) throw new Error(`no view ${view}; there are ${Object.keys(VIEWS).join(", ")}`);
// Every view drawn at once, each its own run.
const runs = await Promise.all(
  views.map((view) => {
    const dir = resolve(out, view);
    mkdirSync(dir);
    const [command, words] = offscreenRun(`lodcheck-${view}`, [
      "--clean", ...(SCENES[view] ?? WALK), "--time", hour,
      "--vantage", VIEWS[view], "--detail-sweep", `${SWEEP.from}-${SWEEP.to}@${SWEEP.first}-${SWEEP.last}`,
      "--shots", `${offscreenPath(dir)}/f@${SWEEP.first + SETTLE - 1}-${SWEEP.last}`,
    ]);
    return run(command, words, { ...process.env, BEVY_ASSET_ROOT: ROOT });
  }),
);
views.forEach((view, i) => {
  const { status, said } = runs[i];
  if (status !== 0 || !said.includes("SANDBOX saved")) {
    console.error(said.split("\n").slice(-20).join("\n"));
    process.exit(1);
  }
  if (said.includes("distinct `VisibilityRange`s") || said.includes("distinct detail ranges are in use")) {
    console.log(`lodcheck: ${view}: too many distinct detail ranges; past Bevy's 65,535 they are culled wrongly`);
    failed = true;
  }
});
const file = (view: string, frame: number) => resolve(out, view, `f-${String(frame).padStart(5, "0")}.png`);
const pairs = views.flatMap((view) =>
  Array.from({ length: SWEEP.last - SWEEP.first - SETTLE + 1 }, (_, k) => ({ view, frame: SWEEP.first + SETTLE + k })),
);
const shares = await pool(
  pairs.map(({ view, frame }) => async () => {
    const { said } = await run(...imageMagick([file(view, frame - 1), file(view, frame), "-resize", `${WIDTH}x`, "-compose", "difference", "-composite", "-colorspace", "gray", "-threshold", CHANGED, "-morphology", "Open", SPECKLE, "-format", "%[fx:mean*100]", "info:"]));
    return Number(said.trim());
  }),
  availableParallelism(),
);
for (const view of views) {
  const pops: Pop[] = pairs
    .map((pair, i) => ({ ...pair, share: shares[i], scale: scaleAt(pair.frame) }))
    .filter((pop) => pop.view === view);
  pops.sort((a, b) => b.share - a.share);
  const over = pops.filter((p) => p.share > limit);
  const mean = pops.reduce((sum, p) => sum + p.share, 0) / pops.length;
  console.log(`lodcheck: ${view}: mean ${mean.toFixed(3)}% a frame, worst ${pops[0].share.toFixed(2)}% at scale ${pops[0].scale.toFixed(2)}; ${over.length} frames over ${limit}%`);
  for (const pop of pops.slice(0, 3)) console.log(`lodcheck: ${view}: frame ${pop.frame}: ${pop.share.toFixed(2)}% popped at scale ${pop.scale.toFixed(2)}`);
  if (over.length) failed = true;
  worst.push(...pops.slice(0, 2));
  // Black: nothing lit by day is this dark, not the deepest shade, which the sky lights. A
  // share of it is light gone missing (an unfiltered sky light left every shade black) or
  // NaNs spreading.
  const black = Number(magick(file(view, SWEEP.last), "-resize", `${WIDTH}x`, "-channel", "RGB", "-separate", "-evaluate-sequence", "max", "-threshold", BLACK, "-negate", "-format", "%[fx:mean*100]", "info:"));
  console.log(`lodcheck: ${view}: ${black.toFixed(2)}% of the last frame black`);
  if (black > MOST_BLACK) {
    console.log(`lodcheck: ${view}: FAIL: over ${MOST_BLACK}% black: the shade unlit, or NaNs`);
    failed = true;
  }
}

// The worst frames, each with what popped marked red over it.
const tiles: string[] = [];
for (const pop of worst) {
  const at = (frame: number) => file(pop.view, frame);
  const mask = resolve(out, `mask-${pop.view}-${pop.frame}.png`);
  const mark = resolve(out, `mark-${pop.view}-${pop.frame}.jpg`);
  magick(at(pop.frame - 1), at(pop.frame), "-resize", `${WIDTH}x`, "-compose", "difference", "-composite", "-colorspace", "gray", "-threshold", CHANGED, "-morphology", "Open", SPECKLE, "-morphology", "Dilate", "Disk:3", "-colorspace", "sRGB", "-fill", "red", "-opaque", "white", "-transparent", "black", mask);
  magick(at(pop.frame), "-resize", `${WIDTH}x`, mask, "-composite", mark);
  tiles.push("-label", `${pop.view}, frame ${pop.frame}: ${pop.share.toFixed(2)}% at scale ${pop.scale.toFixed(2)}`, mark);
}
const sheet = resolve(out, "sheet.jpg");
magick("montage", ...tiles, "-tile", "2x", "-geometry", "+4+4", "-pointsize", "16", "-background", "#222", "-fill", "#eee", sheet);
console.log(`lodcheck: ${sheet}`);
console.log(failed ? `lodcheck: FAIL: something pops more than ${limit}% of the picture in one frame` : "lodcheck: pass");
process.exit(failed ? 1 : 0);
