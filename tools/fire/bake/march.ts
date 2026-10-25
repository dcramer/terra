#!/usr/bin/env node
// Films a baked fire volume offline, for checking its motion with motion.ts (D493), not the
// game's marcher: each frame's flame marched along every pixel's ray from fire round g23's camera
// (clipm.ts's view in the sim's Z-up space: from (0, -3.95, 2.6) at (0, 0, 0.65), 29 mm on a
// 13.5 mm sensor, shifted up 0.08, 600 px square) and shaded as g23's films were (its sim.py):
// emission 160 flame^3 a metre through a red-orange-yellow-pale ramp of the flame, turning toward
// linear (1, 0.1, 0.06) from 0.5 to 0.85 m up, over g23's dusk sky, the Standard view (sRGB,
// clipped). Unlike g23's Cycles films there is no smoke to absorb: the volume holds none.
//
//   node tools/fire/bake/march.ts <set> --out <film.mkv> [--plays N] [--raw N] [--dds file]
//
// By default it plays the packed loop (assets/textures/fire/<set>.dds, decoded, or `--dds` such
// as the Rg8 copy) `--plays` times back to back (2); `--raw N` films instead the sim's first N
// frames as read (shots/fire/bake/<set>/raw.bin, no crossfade, not compressed), the control the
// loop is compared with. Frames are marched on every core; the film is lossless x264 at the
// set's rate. Node 22.18 or later and ffmpeg; no dependencies.

import { spawn } from "node:child_process";
import { readFileSync } from "node:fs";
import { availableParallelism } from "node:os";
import { join } from "node:path";
import { Worker, isMainThread, parentPort, workerData } from "node:worker_threads";
import { DXGI_BC5_UNORM, decodeBc5, readDds3d } from "./bc5.ts";
import { readRaw, untile } from "./volume.ts";

const PX = 600;

type Job = { flame: SharedArrayBuffer; frames: number; cells: [number, number, number]; cell: number; boxMin: [number, number, number]; order: number[] };

// g23's ramp of the flame (sim.py), linear RGB.
const RAMP: [number, [number, number, number]][] = [
  [0, [0.6, 0.04, 0]],
  [0.25, [1, 0.22, 0.01]],
  [0.55, [1, 0.55, 0.08]],
  [1, [1, 0.92, 0.65]],
];
const TOP: [number, number, number] = [1, 0.1, 0.06];
const SKY: [number, number, number] = [0.006, 0.008, 0.014];

function ramp(f: number, out: number[]) {
  let i = 0;
  while (i < RAMP.length - 2 && f > RAMP[i + 1][0]) i++;
  const [p0, c0] = RAMP[i];
  const [p1, c1] = RAMP[i + 1];
  const t = Math.min(1, Math.max(0, (f - p0) / (p1 - p0)));
  for (let k = 0; k < 3; k++) out[k] = c0[k] + (c1[k] - c0[k]) * t;
}

const srgb = (x: number) => {
  const c = Math.min(1, Math.max(0, x));
  return Math.round(255 * (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055));
};

/// One frame of flame (nx * ny * nz, x fastest) filmed as RGB bytes.
function film(v: Float32Array, cells: [number, number, number], h: number, boxMin: [number, number, number]): Uint8Array {
  const [nx, ny, nz] = cells;
  const cam = [0, -3.95, 2.6];
  const look = [0, 0, 0.65];
  let fw = look.map((x, i) => x - cam[i]);
  const n = Math.hypot(fw[0], fw[1], fw[2]);
  fw = fw.map((x) => x / n);
  let right = [fw[1], -fw[0], 0];
  const rn = Math.hypot(right[0], right[1]);
  right = right.map((x) => x / rn);
  const up = [right[1] * fw[2] - right[2] * fw[1], right[2] * fw[0] - right[0] * fw[2], right[0] * fw[1] - right[1] * fw[0]];
  const tan = 13.5 / 29;
  const lo = boxMin;
  const hi = [lo[0] + nx * h, lo[1] + ny * h, lo[2] + nz * h];
  const ds = h / 2;
  const out = new Uint8Array(PX * PX * 3);
  const col = [0, 0, 0];
  const sample = (x: number, y: number, z: number) => {
    // Cell centres at lo + (i + 0.5) h; outside the box's cells, nothing.
    const u = (x - lo[0]) / h - 0.5;
    const w = (y - lo[1]) / h - 0.5;
    const s = (z - lo[2]) / h - 0.5;
    if (u < 0 || w < 0 || s < 0 || u > nx - 1 || w > ny - 1 || s > nz - 1) return 0;
    const x0 = Math.min(nx - 2, Math.floor(u));
    const y0 = Math.min(ny - 2, Math.floor(w));
    const z0 = Math.min(nz - 2, Math.floor(s));
    const fx = u - x0;
    const fy = w - y0;
    const fz = s - z0;
    const i = (z0 * ny + y0) * nx + x0;
    const sx = nx * ny;
    const a = v[i] + (v[i + 1] - v[i]) * fx;
    const b = v[i + nx] + (v[i + nx + 1] - v[i + nx]) * fx;
    const c = v[i + sx] + (v[i + sx + 1] - v[i + sx]) * fx;
    const d = v[i + sx + nx] + (v[i + sx + nx + 1] - v[i + sx + nx]) * fx;
    const e = a + (b - a) * fy;
    const g = c + (d - c) * fy;
    return e + (g - e) * fz;
  };
  for (let py = 0; py < PX; py++) {
    for (let px = 0; px < PX; px++) {
      const su = ((px + 0.5) / PX - 0.5) * tan;
      const sv = (0.5 - (py + 0.5) / PX + 0.08) * tan;
      const dir = [0, 1, 2].map((k) => fw[k] + right[k] * su + up[k] * sv);
      let t0 = 0;
      let t1 = Infinity;
      for (let k = 0; k < 3; k++) {
        const inv = 1 / dir[k];
        let a = (lo[k] - cam[k]) * inv;
        let b = (hi[k] - cam[k]) * inv;
        if (a > b) [a, b] = [b, a];
        t0 = Math.max(t0, a);
        t1 = Math.min(t1, b);
      }
      const L = [SKY[0], SKY[1], SKY[2]];
      if (t1 > t0) {
        const len = Math.hypot(dir[0], dir[1], dir[2]);
        const dt = ds / len;
        // A fixed offset a pixel (a hash of its place), so steps do not band and do not flicker.
        const j = (((px * 73856093) ^ (py * 19349663)) >>> 0) / 2 ** 32;
        for (let t = t0 + j * dt; t < t1; t += dt) {
          const x = cam[0] + dir[0] * t;
          const y = cam[1] + dir[1] * t;
          const z = cam[2] + dir[2] * t;
          const f = sample(x, y, z);
          if (f <= 1e-4) continue;
          ramp(f, col);
          const red = Math.min(1, Math.max(0, (z - 0.5) / 0.35));
          const e = 160 * f * f * f * ds;
          for (let k = 0; k < 3; k++) L[k] += (col[k] + (TOP[k] - col[k]) * red) * e;
        }
      }
      const o = (py * PX + px) * 3;
      out[o] = srgb(L[0]);
      out[o + 1] = srgb(L[1]);
      out[o + 2] = srgb(L[2]);
    }
  }
  return out;
}

async function main() {
  const args = process.argv.slice(2);
  const opt = (name: string) => {
    const i = args.indexOf(name);
    return i >= 0 ? args[i + 1] : undefined;
  };
  const set = args[0];
  const outPath = opt("--out");
  if (!set || set.startsWith("--") || !outPath) {
    console.error("usage: node tools/fire/bake/march.ts <set> --out <film.mkv> [--plays N] [--raw N] [--dds file]");
    process.exit(2);
  }
  const ROOT = join(import.meta.dirname, "../../..");
  const side = JSON.parse(readFileSync(join(ROOT, "assets/textures/fire", `${set}.json`), "utf8"));
  const cells = side.cells as [number, number, number];
  const n = cells[0] * cells[1] * cells[2];
  let frames: Float32Array[];
  let order: number[];
  let what: string;
  const rawN = opt("--raw");
  if (rawN) {
    const raw = readRaw(join(ROOT, "shots/fire/bake", set));
    frames = raw.flame.slice(0, Number(rawN));
    order = frames.map((_, i) => i);
    what = `the sim's first ${frames.length} frames as read`;
  } else {
    const path = opt("--dds") ?? join(ROOT, "assets/textures/fire", side.texture);
    const dds = readDds3d(readFileSync(path));
    let flame: Float32Array;
    if (dds.format === DXGI_BC5_UNORM) [flame] = decodeBc5(dds.data, dds.w, dds.h, dds.d);
    else {
      flame = new Float32Array(dds.w * dds.h * dds.d);
      for (let i = 0; i < flame.length; i++) flame[i] = dds.data[2 * i] / 255;
    }
    frames = untile(flame, [dds.w, dds.h, dds.d], cells, side.columns, side.frames);
    const plays = Number(opt("--plays") ?? 2);
    order = Array.from({ length: plays * side.frames }, (_, i) => i % side.frames);
    what = `the loop ${path.split("/").pop()} played ${plays} times`;
  }
  const shared = new SharedArrayBuffer(frames.length * n * 4);
  const all = new Float32Array(shared);
  frames.forEach((f, i) => all.set(f, i * n));

  const ff = spawn("ffmpeg", ["-y", "-v", "error", "-f", "rawvideo", "-pix_fmt", "rgb24", "-s", `${PX}x${PX}`, "-r", String(side.fps), "-i", "-", "-an", "-c:v", "libx264", "-qp", "0", "-pix_fmt", "yuv444p", outPath], {
    stdio: ["pipe", "inherit", "inherit"],
  });
  const done = new Map<number, Uint8Array>();
  let next = 0;
  const start = Date.now();
  const flush = async () => {
    while (done.has(next)) {
      const buf = done.get(next)!;
      done.delete(next);
      next++;
      if (!ff.stdin.write(buf)) await new Promise((r) => ff.stdin.once("drain", r));
    }
  };
  const workers = Math.min(order.length, Math.max(1, availableParallelism() - 2));
  const job: Omit<Job, "order"> = { flame: shared, frames: frames.length, cells, cell: side.cell, boxMin: side.box_min };
  await Promise.all(
    Array.from({ length: workers }, (_, w) => {
      const mine = order.map((f, i) => [i, f]).filter(([i]) => i % workers === w);
      return new Promise<void>((resolve, reject) => {
        const worker = new Worker(new URL(import.meta.url), { workerData: { ...job, order: mine } });
        worker.on("message", async (m: { i: number; rgb: Uint8Array }) => {
          done.set(m.i, m.rgb);
          await flush();
        });
        worker.on("error", reject);
        worker.on("exit", () => resolve());
      });
    }),
  );
  await flush();
  ff.stdin.end();
  await new Promise((r) => ff.on("close", r));
  console.log(`${outPath}: ${order.length} frames at ${side.fps} fps of ${what}, ${((Date.now() - start) / 1000).toFixed(0)} s on ${workers} threads`);
}

if (isMainThread) {
  if (import.meta.main) main();
} else {
  const job = workerData as Omit<Job, "order"> & { order: [number, number][] };
  const n = job.cells[0] * job.cells[1] * job.cells[2];
  const all = new Float32Array(job.flame);
  for (const [i, f] of job.order) {
    const rgb = film(all.subarray(f * n, (f + 1) * n), job.cells, job.cell, job.boxMin);
    parentPort!.postMessage({ i, rgb }, [rgb.buffer]);
  }
}
