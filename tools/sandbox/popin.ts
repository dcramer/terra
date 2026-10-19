#!/usr/bin/env node
// Finds things that appear whole inside the picture between two frames of a film or a still
// sequence (docs/scene-rules.md, Appearance; the lake film's "animals pop into existence"):
// an animal put out in view, a prop or a crown switched in. It reads frames already made,
// so it runs on any film, still sequence or archived round, without rendering (D69).
//
// Each frame is made small, grey and blurred. A pixel is new when it is brighter (or darker)
// by more than CHANGED than anything within REACH of it in each of the two frames before: what
// moved, the camera's own motion among it, and what a moving edge uncovers, which is like what
// was beside it, are found near; an animal put out whole is not. It popped in when it is still
// there the next two frames: something as bright within REACH of it in each, so a flicker or
// a glint of a frame does not count. New pixels next to each other make a patch; a patch of
// at least LEAST pixels that does not touch the picture's edge (what comes in from an edge
// walks in) is a pop-in. A frame where most of the picture changed (a cut, a flash) is passed
// over, and so is a patch of flame-coloured pixels, as flames lick out new each frame by
// design. A fade in over 0.5 s changes no frame by CHANGED.
//
//   node tools/sandbox/popin.ts <film.mp4 | frames directory> [--out <dir>] [--least 12]
//
// Prints a line a pop-in with its time, place and size, marks each in <out>/popin-<frame>.jpg
// (the frame before and the frame, the patch boxed), and exits 1 if there were any. <out> is
// beside the film by default (`popin/`). Needs ffmpeg. Node 22.18 or later; no dependencies.

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";

/// Frames are compared at this size, blurred by a box this many pixels each side, twice.
const [W, H] = [480, 270];
const BLUR = 1;
/// A pixel changed past this much of 255.
const CHANGED = 24;
/// How far a thing may move between frames and be found again, pixels at that size (a
/// twentieth of the picture's height at 1080p).
const REACH = 5;
/// A frame where more than this share of the picture changed is a cut or a flash.
const CUT = 0.25;
/// Rows darker than this all film long are letterbox.
const BAR = 6;
/// A patch with flame-coloured pixels (bright, orange to yellow) round it, this many to each of
/// its own, is fire.
const FIRE = 0.3;
/// The largest pop-ins are marked in pictures, at most this many.
const MARKS = 12;

const args = process.argv.slice(2);
const arg = (name: string, fallback: string) => {
  const at = args.indexOf(name);
  return at >= 0 ? args[at + 1] : fallback;
};
const input = args.find((a, i) => !a.startsWith("--") && !(i > 0 && args[i - 1].startsWith("--")));
if (!input || !existsSync(input)) {
  console.error("popin: give a film or a directory of frames");
  process.exit(2);
}
const LEAST = Number(arg("--least", "12"));
const out = resolve(arg("--out", join(dirname(resolve(input)), "popin")));
rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

// The frames, decoded by ffmpeg: a film as it is, a directory as its images in name order.
const isDir = statSync(input).isDirectory();
let source: string[];
let fps = 30;
if (isDir) {
  const frames = readdirSync(input).filter((f) => /\.(png|jpe?g)$/i.test(f)).sort();
  const list = join(out, "frames.txt");
  writeFileSync(list, frames.map((f) => `file '${resolve(input, f)}'\nduration ${1 / fps}`).join("\n"));
  source = ["-f", "concat", "-safe", "0", "-i", list];
} else {
  const probe = spawnSync("ffprobe", ["-v", "error", "-select_streams", "v", "-show_entries", "stream=r_frame_rate", "-of", "csv=p=0", input], { encoding: "utf8" });
  const [n, d] = probe.stdout.trim().split("/").map(Number);
  if (n && d) fps = n / d;
  source = ["-i", input];
}

const size = W * H;
const blur = (frame: Uint8Array): Float32Array => {
  let a = Float32Array.from(frame);
  for (let pass = 0; pass < 2; pass++) {
    const b = new Float32Array(size);
    for (let y = 0; y < H; y++) {
      let sum = 0;
      for (let x = -BLUR; x <= BLUR; x++) sum += a[y * W + Math.min(W - 1, Math.max(0, x))];
      for (let x = 0; x < W; x++) {
        b[y * W + x] = sum / (2 * BLUR + 1);
        sum += a[y * W + Math.min(W - 1, x + BLUR + 1)] - a[y * W + Math.max(0, x - BLUR)];
      }
    }
    const c = new Float32Array(size);
    for (let x = 0; x < W; x++) {
      let sum = 0;
      for (let y = -BLUR; y <= BLUR; y++) sum += b[Math.min(H - 1, Math.max(0, y)) * W + x];
      for (let y = 0; y < H; y++) {
        c[y * W + x] = sum / (2 * BLUR + 1);
        sum += b[Math.min(H - 1, y + BLUR + 1) * W + x] - b[Math.max(0, y - BLUR) * W + x];
      }
    }
    a = c;
  }
  return a;
};

type Patch = { frame: number; x0: number; y0: number; x1: number; y1: number; area: number; peak: number };
const patches: Patch[] = [];
const rowMax = new Float32Array(H);
let cuts = 0;
let flames = 0;

/// The least or most of a frame within REACH of each pixel, a square, in two passes.
function spread(a: Float32Array, most: boolean): Float32Array {
  const pick = most ? Math.max : Math.min;
  const b = new Float32Array(size);
  for (let y = 0; y < H; y++)
    for (let x = 0; x < W; x++) {
      let v = a[y * W + x];
      for (let d = Math.max(0, x - REACH); d <= Math.min(W - 1, x + REACH); d++) v = pick(v, a[y * W + d]);
      b[y * W + x] = v;
    }
  const c = new Float32Array(size);
  for (let x = 0; x < W; x++)
    for (let y = 0; y < H; y++) {
      let v = b[y * W + x];
      for (let d = Math.max(0, y - REACH); d <= Math.min(H - 1, y + REACH); d++) v = pick(v, b[d * W + x]);
      c[y * W + x] = v;
    }
  return c;
}

type Frame = { at: Float32Array; lo: Float32Array; hi: Float32Array; fire: Uint8Array };

/// The patches that popped in at frame `index`, the middle of five.
function find(index: number, [b2, b1, now, n1, n2]: Frame[]) {
  const popped = new Uint8Array(size);
  let changed = 0;
  const half = CHANGED / 2;
  for (let i = 0; i < size; i++) {
    const v = now.at[i];
    if (Math.abs(v - b1.at[i]) > CHANGED) changed++;
    const bright = v > b1.hi[i] + CHANGED && v > b2.hi[i] + CHANGED && n1.hi[i] >= v - half && n2.hi[i] >= v - half;
    const dark = v < b1.lo[i] - CHANGED && v < b2.lo[i] - CHANGED && n1.lo[i] <= v + half && n2.lo[i] <= v + half;
    if (bright || dark) popped[i] = 1;
  }
  if (changed > size * CUT) {
    cuts++;
    return;
  }
  // Patches of new pixels, 8-connected.
  const stack: number[] = [];
  for (let start = 0; start < size; start++) {
    if (popped[start] !== 1) continue;
    let [x0, y0, x1, y1, area, peak, burning] = [W, H, 0, 0, 0, 0, 0];
    popped[start] = 2;
    stack.push(start);
    while (stack.length) {
      const i = stack.pop()!;
      const [x, y] = [i % W, Math.floor(i / W)];
      [x0, y0, x1, y1] = [Math.min(x0, x), Math.min(y0, y), Math.max(x1, x), Math.max(y1, y)];
      area++;
      peak = Math.max(peak, Math.abs(now.at[i] - b1.at[i]));
      for (let dy = -1; dy <= 1; dy++)
        for (let dx = -1; dx <= 1; dx++) {
          const [nx, ny] = [x + dx, y + dy];
          if (nx < 0 || ny < 0 || nx >= W || ny >= H) continue;
          const j = ny * W + nx;
          if (popped[j] === 1) {
            popped[j] = 2;
            stack.push(j);
          }
        }
    }
    // Flames lick out by design, a new tongue each frame, and their smoke with them: fire's
    // own look (docs/fire.md). A patch by flames, in this frame or the one before, is theirs.
    for (let y = Math.max(0, y0 - REACH); y <= Math.min(H - 1, y1 + REACH); y++)
      for (let x = Math.max(0, x0 - REACH); x <= Math.min(W - 1, x1 + REACH); x++) burning += now.fire[y * W + x] | b1.fire[y * W + x];
    if (burning > area * FIRE) {
      flames++;
      continue;
    }
    if (area >= LEAST) patches.push({ frame: index, x0, y0, x1, y1, area, peak });
  }
}

// Streams the frames through a window of four.
const decoder = spawn("ffmpeg", ["-v", "error", ...source, "-vf", `scale=${W}:${H},format=rgb24`, "-f", "rawvideo", "-"], { stdio: ["ignore", "pipe", "inherit"] });
const window: Frame[] = [];
let pending = Buffer.alloc(0);
let frames = 0;
decoder.stdout.on("data", (chunk: Buffer) => {
  pending = Buffer.concat([pending, chunk]);
  while (pending.length >= size * 3) {
    const rgb = pending.subarray(0, size * 3);
    const frame = new Uint8Array(size);
    const fire = new Uint8Array(size);
    for (let i = 0; i < size; i++) {
      const [r, g, b] = [rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]];
      frame[i] = Math.round(0.299 * r + 0.587 * g + 0.114 * b);
      fire[i] = Number(r >= 150 && r >= g && g >= b * 1.2 && r - b >= 90);
    }
    pending = Buffer.from(pending.subarray(size * 3));
    for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) rowMax[y] = Math.max(rowMax[y], frame[y * W + x]);
    const at = blur(frame);
    window.push({ at, lo: spread(at, false), hi: spread(at, true), fire });
    frames++;
    if (window.length > 5) window.shift();
    if (window.length === 5) find(frames - 3, window);
  }
});
decoder.on("close", (code) => {
  if (code !== 0 || frames < 5) {
    console.error(`popin: ffmpeg read ${frames} frames (exit ${code})`);
    process.exit(2);
  }
  // The picture is between the letterbox's bars; a patch touching its edge came in from it.
  let top = 0;
  while (top < H - 1 && rowMax[top] < BAR) top++;
  let bottom = H - 1;
  while (bottom > top && rowMax[bottom] < BAR) bottom--;
  const edge = 2;
  const inside = patches.filter((p) => p.x0 > edge && p.x1 < W - 1 - edge && p.y0 > top + edge && p.y1 < bottom - edge);
  const name = basename(input);
  console.log(`popin: ${name}: ${frames} frames at ${fps.toFixed(0)} fps, picture rows ${top}-${bottom} of ${H}, ${cuts} cut or flash frames passed over, ${flames} patches of flame`);
  // Each pop-in marked: the frame before and the frame, side by side, the patch boxed.
  const scale = 1920 / W;
  const marked = new Set([...inside].sort((a, b) => b.area - a.area).slice(0, MARKS));
  for (const p of inside) {
    const t = p.frame / fps;
    const [x, y, w, h] = [p.x0, p.y0, p.x1 - p.x0 + 1, p.y1 - p.y0 + 1].map((v) => Math.round(v * scale));
    console.log(`popin: pop-in at ${t.toFixed(2)} s (frame ${p.frame}): ${w}x${h} px at ${x},${y} of 1920x1080, ${p.area} px changed by up to ${Math.round(p.peak)}`);
    const pad = 60;
    const box = `drawbox=x=${x - 8}:y=${y - 8}:w=${w + 16}:h=${h + 16}:color=red:t=3`;
    const crop = `crop=${Math.min(1920, w + 2 * pad + 300)}:${Math.min(1080, h + 2 * pad + 200)}:${Math.max(0, x - pad - 150)}:${Math.max(0, y - pad - 100)}`;
    if (!marked.has(p)) continue;
    const mark = join(out, `popin-${String(p.frame).padStart(5, "0")}.jpg`);
    const from = isDir ? source : ["-ss", ((p.frame - 1) / fps).toFixed(4), "-i", input];
    const skip = isDir ? `select=gte(n\\,${p.frame - 1}),` : "";
    spawnSync("ffmpeg", ["-v", "error", "-y", ...from, "-vf", `${skip}scale=1920:1080,${box},${crop},tile=2x1`, "-frames:v", "1", mark]);
  }
  console.log(inside.length ? `popin: ✗ ${inside.length} pop-ins, marked in ${out}` : `popin: ✓ nothing popped in inside the picture`);
  process.exit(inside.length ? 1 : 0);
});
