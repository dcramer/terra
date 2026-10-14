#!/usr/bin/env node
// What only frames show of building's junk, found on the GPU without a person (D69): run by
// `cargo xtask artifacts --frames [--quality high,low]`. The meshes' own faults (z-fighting,
// holes, gaps under roofs) are `cargo xtask artifacts` without `--frames`, from the meshes;
// detail popping is `cargo xtask lodcheck`. Each check is a scripted run, offscreen, its
// frames saved and compared pixel by pixel:
//
// - **still**: a view held still, the clock stopped (`--detail-sweep 1-1`), for forty frames
//   after TAA has settled. Nothing moves, so any pixel that changes between frames is the
//   renderer's own noise: shadow acne flickering under temporal filtering, a cascade that
//   does not hold still, dither crawling, a distant wall's texture shimmering in its mips.
//   The village from its corner and a street of the town, walls 30 to 300 m off.
// - **glide**: the camera sliding slowly sideways down the town's street (`--glide`). A
//   moving camera changes every pixel a little and smoothly; a pixel that flips for one
//   frame and back (the frames either side agreeing) is shimmer: a cascade's texels
//   swimming, a mip flickering, a seam.
// - **going-up**: a stone house built a step every tenth of a second, the camera still on it
//   (`--going-up`). A flash, a frame unlike both its neighbours, is a section missing or
//   doubled for a frame while it is laid again; a pixel still changing three frames after a
//   piece went in is TAA trailing it (ghosting); the picture's brightness jumping in one
//   frame where nothing went in is a room's light probe popping.
//
// Each check prints its numbers against its limit, and saves its worst frame with what
// changed marked red, in shots/artifacts/<quality>/sheet: so a person sees at a glance what
// failed and where. Fails if any check is over its limit. Under WSL the runs are the Windows
// build on the real GPU. Node 22.18 or later; no dependencies.

import { spawn } from "node:child_process";
import { mkdirSync, readdirSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { ROOT, buildOffscreen, offscreenPath, offscreenRun } from "./client.ts";
import { CHANGED, blur, changed, flicked, grey, jumped, mean, share, trailing } from "./frames.ts";
import { readPng, writePng } from "./png.ts";

const arg = (name: string, fallback: string) => {
  const at = process.argv.indexOf(name);
  return at > 0 ? process.argv[at + 1] : fallback;
};
const qualities = arg("--quality", "high,low").split(",");
const only = arg("--checks", "still,sun,glide,going-up").split(",");

const VILLAGE = ["--experiment", "play", "--level", "yard", "--village", "--time", "17"];
const TOWN = ["--experiment", "play", "--level", "yard", "--radius", "30000", "--town", "40", "--time", "17"];
/// A stone house built a step every tenth of a second, finished by about frame 1790.
const GOING_UP = ["--experiment", "play", "--level", "yard", "--time", "17", "--going-up"];
/// The clock stops on this frame, and frames are compared from the next on: TAA and the eye
/// have long settled by then.
const STOP = 200;
const STILL = [240, 279];

type Check = "still" | "sun" | "glide" | "going-up";
type Run = { name: string; check: Check; args: string[]; frames: [number, number] };
const RUNS: Run[] = [
  {
    name: "village",
    check: "still",
    args: [...VILLAGE, "--vantage", "-85,16,10,-5,0,-50", "--detail-sweep", `1-1@${STOP}-${STOP + 1}`],
    frames: STILL as [number, number],
  },
  {
    name: "street",
    check: "still",
    args: [...TOWN, "--vantage", "-170,12,-170,150,0,-170", "--detail-sweep", `1-1@${STOP}-${STOP + 1}`],
    frames: STILL as [number, number],
  },
  // The clock running, the camera still: the sun moves a twentieth of a degree at a time
  // (D204), which moves a true shadow's edge by under a pixel but flips shadow acne, where
  // a surface shades itself, across whole faces.
  {
    name: "village",
    check: "sun",
    args: [...VILLAGE, "--vantage", "-60,6,-20,-20,2,-45"],
    frames: [240, 480],
  },
  {
    name: "street",
    check: "glide",
    args: [
      ...TOWN, "--glide",
      "--vantage", "-170,12,-176,150,0,-176", "--vantage", "-170,12,-176,150,0,-176@220",
      "--vantage", "-170,12,-164,150,0,-164@460",
    ],
    frames: [240, 380],
  },
  // Its walls going up, from outside; then inside it as its roof closes it in, from the
  // view its log gives (`Going up: … inside it: --vantage …`), the house's frame being the
  // start's.
  {
    name: "walls",
    check: "going-up",
    args: [...GOING_UP],
    frames: [700, 900],
  },
  {
    name: "inside",
    check: "going-up",
    args: [...GOING_UP, "--vantage", "6.2,2.2,-5,2.6,1,-7.6@1500"],
    frames: [1540, 1800],
  },
];

/// Limits, each as a share of the picture, in percent: how much may change between two
/// frames held still; how much may flicker for one frame on a moving camera; how much may
/// flash in one frame, or still be changing three frames after a step; and how far the
/// picture's brightness may jump in one frame.
const LIMITS = { still: 0.3, sun: 0.5, glide: 0.3, flash: 0.3, trail: 0.3, jump: 4 };
type Result = { what: string; frame: number; share: number; mask: Uint8Array; picture: string; limit: number };

/// The frames a run saved, by number.
function frames(dir: string): Map<number, string> {
  const out = new Map<number, string>();
  for (const file of readdirSync(dir)) {
    const m = file.match(/-(\d+)\.png$/);
    if (m) out.set(Number(m[1]), resolve(dir, file));
  }
  return out;
}

function analyse(run: Run, dir: string): Result[] {
  const files = frames(dir);
  const numbers = [...files.keys()].sort((a, b) => a - b);
  const g = numbers.map((n) => grey(readPng(files.get(n)!)));
  const size = g[0].v.length;
  const worst = (what: string, limit: number, each: (i: number) => Uint8Array | undefined): Result => {
    let best: Result = { what, frame: numbers[0], share: -1, mask: new Uint8Array(size), picture: files.get(numbers[0])!, limit };
    let total = 0;
    let counted = 0;
    for (let i = 0; i < g.length; i++) {
      const mask = each(i);
      if (!mask) continue;
      const s = share(mask);
      total += s;
      counted++;
      if (s > best.share) best = { what, frame: numbers[i], share: s, mask, picture: files.get(numbers[i])!, limit };
    }
    console.log(`artifacts: ${run.check} ${run.name}: ${what}: mean ${(total / Math.max(1, counted)).toFixed(3)}% a frame, worst ${best.share.toFixed(3)}% at frame ${best.frame} (limit ${limit}%)`);
    return best;
  };
  if (run.check === "still") {
    return [worst("changed while still", LIMITS.still, (i) => (i > 0 ? changed(g[i - 1], g[i]) : undefined))];
  }
  if (run.check === "sun") {
    return [worst("changed as the sun moved", LIMITS.sun, (i) => (i > 0 ? changed(g[i - 1], g[i]) : undefined))];
  }
  const between = (i: number) => i > 0 && i + 1 < g.length;
  if (run.check === "glide") {
    const b = g.map((x) => blur(x, 1));
    return [worst("flickered for a frame", LIMITS.glide, (i) => (between(i) ? flicked(b[i - 1], b[i], b[i + 1]) : undefined))];
  }
  // Going up: flashes, trails, and the brightness of the whole jumping.
  const d = g.map((x, i) => (i > 0 ? changed(g[i - 1], x) : new Uint8Array(size)));
  const flash = worst("flashed for a frame", LIMITS.flash, (i) => (between(i) ? flicked(g[i - 1], g[i], g[i + 1], CHANGED * 2) : undefined));
  const trail = worst("still changing three frames on", LIMITS.trail, (i) => (i >= 3 ? trailing(d[i - 2], d[i - 1], d[i]) : undefined));
  const means = g.map(mean);
  let jump = { frame: numbers[0], by: 0 };
  for (let i = 1; i < g.length; i++) {
    const by = jumped(means[i - 1], means[i]);
    if (by > jump.by) jump = { frame: numbers[i], by };
  }
  console.log(`artifacts: going-up ${run.name}: brightness jumped at most ${jump.by.toFixed(2)}% in a frame, at frame ${jump.frame} (limit ${LIMITS.jump}%)`);
  const at = numbers.indexOf(jump.frame);
  const jumpedAt: Result = { what: "brightness jumped", frame: jump.frame, share: jump.by, mask: d[at] ?? new Uint8Array(size), picture: files.get(jump.frame)!, limit: LIMITS.jump };
  return [flash, trail, jumpedAt];
}

/// The frame half size with what changed marked red over it.
function marked(result: Result, to: string) {
  const p = readPng(result.picture);
  const width = p.width >> 1;
  const height = p.height >> 1;
  const rgba = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      const i = (y * 2 * p.width + x * 2) * 4;
      const o = (y * width + x) * 4;
      const hot = result.mask[y * width + x];
      rgba[o] = hot ? 255 : p.rgba[i];
      rgba[o + 1] = hot ? 0 : p.rgba[i + 1];
      rgba[o + 2] = hot ? 0 : p.rgba[i + 2];
      rgba[o + 3] = 255;
    }
  }
  writePng(to, { width, height, rgba });
}

const runCommand = (command: string, words: string[]) =>
  new Promise<{ status: number; said: string }>((done) => {
    const child = spawn(command, words, { cwd: ROOT, env: { ...process.env, BEVY_ASSET_ROOT: ROOT } });
    let said = "";
    child.stdout.on("data", (x) => (said += x));
    child.stderr.on("data", (x) => (said += x));
    child.on("close", (status) => done({ status: status ?? 1, said }));
  });

if (!buildOffscreen()) process.exit(1);
let failed = false;
for (const quality of qualities) {
  const out = resolve(ROOT, "shots/artifacts", quality);
  rmSync(out, { recursive: true, force: true });
  mkdirSync(resolve(out, "sheet"), { recursive: true });
  const runs = RUNS.filter((run) => only.includes(run.check));
  // Two runs at a time share the GPU.
  const results: Result[][] = [];
  for (let i = 0; i < runs.length; i += 2) {
    const batch = runs.slice(i, i + 2);
    const done = await Promise.all(
      batch.map((run) => {
        const dir = resolve(out, `${run.check}-${run.name}`);
        mkdirSync(dir, { recursive: true });
        const [command, words] = offscreenRun(`artifacts-${run.check}-${run.name}`, [
          "--clean", "--quality", quality, ...run.args,
          "--shots", `${offscreenPath(dir)}/f@${run.frames[0]}-${run.frames[1]}`,
        ]);
        return runCommand(command, words);
      }),
    );
    batch.forEach((run, k) => {
      const { status, said } = done[k];
      if (status !== 0 || !said.includes("SANDBOX saved")) {
        console.error(said.split("\n").slice(-20).join("\n"));
        process.exit(1);
      }
      if (/thread .* panicked|ERROR/.test(said)) console.log(`artifacts: ${run.check} ${run.name}: the run reported errors:\n${said.split("\n").filter((l) => /panicked|ERROR/.test(l)).slice(0, 5).join("\n")}`);
      results.push(analyse(run, resolve(out, `${run.check}-${run.name}`)));
    });
  }
  console.log(`artifacts: ${quality}:`);
  for (const [r, run] of results.map((r, i) => [r, runs[i]] as const)) {
    for (const result of r) {
      const over = result.share > result.limit;
      failed ||= over;
      const file = resolve(out, "sheet", `${run.check}-${run.name}-${result.what.replace(/\W+/g, "-")}.png`);
      marked(result, file);
      console.log(`  ${over ? "FAIL" : "ok  "}  ${run.check} ${run.name}: ${result.what}: worst ${result.share.toFixed(3)}% at frame ${result.frame} (limit ${result.limit}%)  ${file}`);
    }
  }
}
console.log(failed ? "artifacts: FAIL" : "artifacts: frames clean");
process.exit(failed ? 1 : 0);
