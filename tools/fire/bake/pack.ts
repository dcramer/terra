// Packs a set's raw passes (bake.py's raw.bin) into the flipbook format (docs/fire.md, "The
// flipbook format"; D415):
//
// - <set>.png: the flame map, 8x8 frames of the recipe's size, R heat, G cover, B soot,
//   A distortion, each 0 to 1;
// - <set>.motion.png: motion vectors at half the size, R and G the frame's flow to the next
//   frame (0.5 still), in frame widths times the sidecar's `motion.scale`, B and A unused;
// - <set>.json: the sidecar the engine reads, frames, grid, rates and scales.
//
// From the passes (bake.py's header): heat is the flame's own emission-weighted mean flame,
// scaled so the set's 99th percentile is 0.97; cover is 1 - e^-depth of the flame, soot of the
// smoke outside it. A loop's last `crossfade` frames are blended into its first, premultiplied.
// The motion pass's velocity is in metres a second, so its flow in frame widths a frame is
// exact; how well it carries each frame onto the next is its check (`flow`, the warp's error
// against no warp, and `fit`, the scale that would carry best against the exact one).

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { writePng } from "../../sandbox/png.ts";
import type { Recipe } from "./bake.ts";

export const COLUMNS = 8;

/// A packed frame, each channel 0 to 1 over size x size texels, rows top to bottom; mx and my
/// are its flow to the next frame in frame widths (y down).
export type Frame = { heat: Float32Array; cover: Float32Array; soot: Float32Array; dist: Float32Array; mx: Float32Array; my: Float32Array };
export type Packed = { frames: Frame[]; size: number; line: string; sidecar: Record<string, unknown> };

/// IEEE half floats to numbers.
export function halves(bytes: Buffer): Float32Array {
  const u = new Uint16Array(bytes.buffer, bytes.byteOffset, bytes.byteLength / 2);
  const out = new Float32Array(u.length);
  for (let i = 0; i < u.length; i++) {
    const h = u[i];
    const e = (h >> 10) & 31;
    const m = h & 1023;
    const v = e === 0 ? m * 2 ** -24 : e === 31 ? (m ? NaN : Infinity) : (1 + m / 1024) * 2 ** (e - 15);
    out[i] = h & 0x8000 ? -v : v;
  }
  return out;
}

const quantile = (values: Float32Array | number[], q: number) => {
  const s = Float32Array.from(values).sort();
  return s.length ? s[Math.min(s.length - 1, Math.floor(q * s.length))] : 0;
};

/// Bilinear sample of a size x size channel at (x, y) texels, clamped.
function sample(c: Float32Array, n: number, x: number, y: number): number {
  x = Math.min(n - 1, Math.max(0, x));
  y = Math.min(n - 1, Math.max(0, y));
  const x0 = Math.floor(x);
  const y0 = Math.floor(y);
  const x1 = Math.min(n - 1, x0 + 1);
  const y1 = Math.min(n - 1, y0 + 1);
  const fx = x - x0;
  const fy = y - y0;
  const top = c[y0 * n + x0] * (1 - fx) + c[y0 * n + x1] * fx;
  const bottom = c[y1 * n + x0] * (1 - fx) + c[y1 * n + x1] * fx;
  return top * (1 - fy) + bottom * fy;
}

/// The error of carrying `a` onto `b` along flow (mx, my) times `k` (frame widths): each texel
/// of b is looked up in a where it came from, as the shader's flow blend does.
export function warpError(a: Float32Array, b: Float32Array, mx: Float32Array, my: Float32Array, n: number, k: number): number {
  let e = 0;
  for (let y = 0; y < n; y++) {
    for (let x = 0; x < n; x++) {
      const p = y * n + x;
      const d = b[p] - sample(a, n, x - k * mx[p] * n, y - k * my[p] * n);
      e += d * d;
    }
  }
  return e;
}

/// The flow scale that best carries each frame onto the next, by golden section on its log.
export function fitFlow(pairs: [Float32Array, Float32Array, Float32Array, Float32Array][], n: number): { k: number; gain: number } {
  const err = (k: number) => pairs.reduce((s, [a, b, mx, my]) => s + warpError(a, b, mx, my, n, k), 0);
  let [lo, hi] = [Math.log(1e-3), Math.log(1e2)];
  const g = (Math.sqrt(5) - 1) / 2;
  for (let i = 0; i < 40; i++) {
    const c = hi - g * (hi - lo);
    const d = lo + g * (hi - lo);
    if (err(Math.exp(c)) < err(Math.exp(d))) hi = d;
    else lo = c;
  }
  const k = Math.exp((lo + hi) / 2);
  return { k, gain: err(k) / Math.max(1e-12, err(0)) };
}

/// The texel-wise RMS difference of two frames' cover and soot.
const rms = (a: Frame, b: Frame) => {
  let s = 0;
  for (let i = 0; i < a.cover.length; i++) s += (a.cover[i] - b.cover[i]) ** 2 + (a.soot[i] - b.soot[i]) ** 2;
  return Math.sqrt(s / a.cover.length);
};

export function pack(r: Recipe, raw: string, outDir: string): Packed {
  const meta = JSON.parse(readFileSync(join(raw, "meta.json"), "utf8"));
  const n: number = meta.size;
  const N = n * n;
  const vmax: number = meta.velocity_max;
  const data = halves(readFileSync(join(raw, "raw.bin")));
  const per = N * 3;
  const pass = (frame: number, which: number, ch: number) => {
    const out = new Float32Array(N);
    const base = (frame * 3 + which) * per;
    for (let i = 0; i < N; i++) out[i] = data[base + i * 3 + ch];
    return out;
  };
  // Raw frames: heat (unscaled), cover, soot and velocity in Mantaflow's units (y down).
  const rawFrames: Frame[] = [];
  for (let f = 0; f < meta.frames; f++) {
    const [ff, f1] = [pass(f, 0, 0), pass(f, 0, 1)];
    const [vx, vz, w] = [pass(f, 1, 0), pass(f, 1, 1), pass(f, 1, 2)];
    const [tf, ts] = [pass(f, 2, 0), pass(f, 2, 1)];
    const fr: Frame = { heat: new Float32Array(N), cover: new Float32Array(N), soot: new Float32Array(N), dist: new Float32Array(N), mx: new Float32Array(N), my: new Float32Array(N) };
    for (let i = 0; i < N; i++) {
      fr.heat[i] = f1[i] > 1e-5 ? ff[i] / f1[i] : 0;
      fr.cover[i] = 1 - Math.exp(-Math.max(0, tf[i]));
      fr.soot[i] = 1 - Math.exp(-Math.max(0, ts[i]));
      if (w[i] > 1e-5) {
        fr.mx[i] = (vx[i] / w[i] - 0.5) * 2 * vmax;
        fr.my[i] = -(vz[i] / w[i] - 0.5) * 2 * vmax;
      }
    }
    rawFrames.push(fr);
  }
  // Heat to 0-1 by the set's 99th percentile of covered texels.
  const covered: number[] = [];
  for (const fr of rawFrames) for (let i = 0; i < N; i += 7) if (fr.cover[i] > 0.05) covered.push(fr.heat[i]);
  const heatScale = 0.97 / Math.max(1e-6, quantile(covered, 0.99));
  for (const fr of rawFrames) for (let i = 0; i < N; i++) fr.heat[i] = Math.min(1, fr.heat[i] * heatScale);

  // The loop: frames [0, frames) with the `crossfade` after them blended into the first.
  const count = r.frames;
  const fade = r.kind === "loop" ? (r.crossfade ?? 0) : 0;
  const frames: Frame[] = [];
  for (let i = 0; i < count; i++) {
    const a = rawFrames[i];
    if (i >= fade || !rawFrames[count + i]) {
      frames.push(a);
      continue;
    }
    const b = rawFrames[count + i];
    const t = (i + 1) / (fade + 1);
    const fr: Frame = { heat: new Float32Array(N), cover: new Float32Array(N), soot: new Float32Array(N), dist: new Float32Array(N), mx: new Float32Array(N), my: new Float32Array(N) };
    for (let p = 0; p < N; p++) {
      const cover = b.cover[p] * (1 - t) + a.cover[p] * t;
      fr.cover[p] = cover;
      fr.soot[p] = b.soot[p] * (1 - t) + a.soot[p] * t;
      fr.heat[p] = cover > 1e-5 ? (b.heat[p] * b.cover[p] * (1 - t) + a.heat[p] * a.cover[p] * t) / cover : 0;
      const wa = a.cover[p] + a.soot[p];
      const wb = b.cover[p] + b.soot[p];
      const ws = wb * (1 - t) + wa * t;
      fr.mx[p] = ws > 1e-5 ? (b.mx[p] * wb * (1 - t) + a.mx[p] * wa * t) / ws : 0;
      fr.my[p] = ws > 1e-5 ? (b.my[p] * wb * (1 - t) + a.my[p] * wa * t) / ws : 0;
    }
    frames.push(fr);
  }

  // The flow in frame widths a frame: the velocity (m/s) over the frame rate and the camera's
  // extent. Its check is how much it lowers the error of carrying each frame onto the next
  // (`flow`, under 1 helps), and how far the scale that would lower it most is from it (`fit`).
  const k = 1 / (meta.fps * meta.extent);
  const glow = (fr: Frame) => fr.cover.map((c, i) => c * fr.heat[i] + fr.soot[i]);
  const pairs: [Float32Array, Float32Array, Float32Array, Float32Array][] = [];
  for (let j = 0; j < 8; j++) {
    const i = Math.floor(((j + 0.5) * (count - 1)) / 8);
    pairs.push([glow(frames[i]), glow(frames[i + 1]), frames[i].mx, frames[i].my]);
  }
  const fitted = fitFlow(pairs, n);
  const err = (s: number) => pairs.reduce((t, [a, b, mx, my]) => t + warpError(a, b, mx, my, n, s), 0);
  const flow = { k, gain: err(k) / Math.max(1e-12, err(0)), fit: fitted.k / k };
  const speeds: number[] = [];
  for (const fr of frames) {
    for (let p = 0; p < N; p++) {
      fr.mx[p] *= flow.k;
      fr.my[p] *= flow.k;
      if ((p & 7) === 0 && fr.cover[p] + fr.soot[p] > 0.05) speeds.push(Math.hypot(fr.mx[p], fr.my[p]));
    }
  }
  // Distortion: the faster a texel moves the more it bends what is behind, so tips move and the
  // base stays (Back 4 Blood), only where there is flame or smoke.
  const fast = Math.max(1e-6, quantile(speeds, 0.95));
  for (const fr of frames) {
    for (let p = 0; p < N; p++) {
      fr.dist[p] = Math.min(1, Math.hypot(fr.mx[p], fr.my[p]) / fast) * Math.sqrt(Math.max(fr.cover[p], fr.soot[p]));
    }
  }
  // Motion is stored to the motion scale: the 99.5th percentile flow, so few texels clip.
  const motionScale = Math.max(1e-4, quantile(speeds, 0.995));

  // The atlases.
  const rows = Math.ceil(count / COLUMNS);
  const W = n * COLUMNS;
  const flame = new Uint8Array(W * n * rows * 4);
  const m = n / 2;
  const MW = m * COLUMNS;
  const motion = new Uint8Array(MW * m * rows * 4);
  const byte = (v: number) => Math.round(Math.min(1, Math.max(0, v)) * 255);
  frames.forEach((fr, f) => {
    const [cx, cy] = [(f % COLUMNS) * n, Math.floor(f / COLUMNS) * n];
    for (let y = 0; y < n; y++) {
      for (let x = 0; x < n; x++) {
        const p = y * n + x;
        const o = ((cy + y) * W + cx + x) * 4;
        flame[o] = byte(fr.heat[p]);
        flame[o + 1] = byte(fr.cover[p]);
        flame[o + 2] = byte(fr.soot[p]);
        flame[o + 3] = byte(fr.dist[p]);
      }
    }
    for (let y = 0; y < m; y++) {
      for (let x = 0; x < m; x++) {
        let [sx, sy, sw] = [0, 0, 0];
        for (const p of [2 * y * n + 2 * x, 2 * y * n + 2 * x + 1, (2 * y + 1) * n + 2 * x, (2 * y + 1) * n + 2 * x + 1]) {
          const w = fr.cover[p] + fr.soot[p] + 1e-4;
          sx += fr.mx[p] * w;
          sy += fr.my[p] * w;
          sw += w;
        }
        const o = ((cy / 2 + y) * MW + cx / 2 + x) * 4;
        motion[o] = byte(0.5 + sx / sw / (2 * motionScale));
        motion[o + 1] = byte(0.5 + sy / sw / (2 * motionScale));
        motion[o + 3] = 255;
      }
    }
  });
  mkdirSync(outDir, { recursive: true });
  writePng(join(outDir, `${r.name}.png`), { width: W, height: n * rows, rgba: flame });
  writePng(join(outDir, `${r.name}.motion.png`), { width: MW, height: m * rows, rgba: motion });

  // What the line says: how much of a frame the flame covers, its heat in quarters (by cover),
  // soot over flame, the loop's seam against an ordinary step, how high the fire reaches.
  let [coverShare, sootPx, flamePx, reach] = [0, 0, 0, 0];
  const quarters = [0, 0, 0, 0];
  for (const fr of frames) {
    let [c, s, top] = [0, 0, n];
    for (let p = 0; p < N; p++) {
      if (fr.cover[p] > 0.1) {
        c++;
        top = Math.min(top, Math.floor(p / n));
      }
      if (fr.soot[p] > 0.1 && fr.cover[p] <= 0.1) s++;
      quarters[Math.min(3, Math.floor(fr.heat[p] * 4))] += fr.cover[p];
    }
    coverShare += c / N / count;
    sootPx += s;
    flamePx += c;
    reach = Math.max(reach, 1 - top / n);
  }
  const sootShare = sootPx / Math.max(1, flamePx);
  const qsum = quarters.reduce((a, b) => a + b, 0) || 1;
  let step = 0;
  for (let i = 0; i + 1 < count; i++) step += rms(frames[i], frames[i + 1]) / (count - 1);
  const seam = r.kind === "loop" ? rms(frames[count - 1], frames[0]) / Math.max(1e-6, step) : 0;
  const pct = (v: number) => `${Math.round(v * 100)}%`;
  const sidecar = {
    set: r.name,
    about: r.about,
    kind: r.kind,
    class: r.class,
    frames: count,
    columns: COLUMNS,
    rows,
    frame: n,
    fps: meta.fps,
    channels: "R heat, G cover, B soot, A distortion",
    motion: { file: `${r.name}.motion.png`, frame: m, scale: +motionScale.toFixed(5), channels: "RG flow to the next frame in frame widths: (v - 0.5) * 2 * scale, y down" },
    baked: { heat_scale: +heatScale.toFixed(4), flow_gain: +flow.gain.toFixed(3), flow_fit: +flow.fit.toFixed(2), seam: +seam.toFixed(2) },
  };
  writeFileSync(join(outDir, `${r.name}.json`), `${JSON.stringify(sidecar, null, 2)}\n`);
  const line =
    `${count} frames, cover ${pct(coverShare)}, heat by quarter ${quarters.map((q) => pct(q / qsum)).join("/")}, ` +
    `soot ${sootShare.toFixed(2)}, seam ${r.kind === "loop" ? seam.toFixed(2) : "-"}, reach ${pct(reach)}, flow ${flow.gain.toFixed(2)} (fit x${flow.fit.toFixed(2)})`;
  return { frames, size: n, line, sidecar };
}
