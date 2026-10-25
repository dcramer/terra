// Packs a sim's box of cells (volume.py's raw.bin) into the looping 3D volume a baked fire's
// marched field reads (docs/fire-module.md, the last section; D493):
//
// - <set>.dds: one 3D texture, BC5 (bc5.ts), R the sim's flame (0-1, as simulated), G its
//   temperature over the loop's largest (0-1). The loop's frames are tiled across and down,
//   `columns` a row: frame f's cell (x, y, z) is the texel (col * nx + x, row * ny + y, z) with
//   col = f % columns and row = floor(f / columns); x lies along the bake's wind, z up;
// - <set>.rg8.dds: the same uncompressed (Rg8Unorm), in shots/ only: an adapter without BC5 3D
//   reads the BC5 decoded (D493), so the repository carries the bake once;
// - <set>.json: the sidecar: frames, rate, cells, the box in metres from the fire's base, the
//   tiling, the temperature's scale, the loop's light a frame (its flame's emission, flame^3,
//   over the loop's mean) and the seam's numbers.
//
// The loop is the recipe's `frames` from the sim's `volume.first`, with the `crossfade` frames
// after them blended linearly into its first (as pack.ts blends a flipbook): frame i < crossfade
// is raw[frames + i] * (1 - t) + raw[i] * t, t = (i + 1) / (crossfade + 1), so the step from the
// last frame to the first is the sim's own step from frames - 1 to frames.
//
// The seam's check (stated before it was first measured, fire round g24): the flame's mean
// absolute change from the loop's last frame to its first, decoded from the BC5, is at most 1.25
// times the mean of its other 239 steps and no more than their 95th percentile. Cell by cell
// that barely sees a cut (the flame rises about four cells a frame, so even the uncrossfaded
// loop's jump was 1.14 times its mean step), so the same bound holds as well on the flame
// averaged over blocks of COARSE cells a side, the scale of the body and its gusts (stated
// before that was measured too).

import { readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Recipe } from "./bake.ts";
import { DXGI_BC5_UNORM, DXGI_R8G8_UNORM, dds3d, decodeBc5, encodeBc5 } from "./bc5.ts";

export type VolumeRecipe = Recipe & {
  volume: { cache: string; first: number; extract: number; box_min: [number, number, number]; cells: [number, number, number]; domain_min: [number, number, number]; domain_size: [number, number, number]; columns: number };
};

/// A volume.py run: its frames' flame and temperature, each nx * ny * nz, x fastest.
export type Raw = { flame: Float32Array[]; temp: Float32Array[]; cells: [number, number, number]; meta: Record<string, number> };

export function readRaw(out: string): Raw {
  const meta = JSON.parse(readFileSync(join(out, "meta.json"), "utf8"));
  const cells = meta.cells as [number, number, number];
  const n = cells[0] * cells[1] * cells[2];
  const bytes = readFileSync(join(out, "raw.bin"));
  const all = new Float32Array(bytes.buffer, bytes.byteOffset, bytes.byteLength / 4);
  const flame: Float32Array[] = [];
  const temp: Float32Array[] = [];
  for (let f = 0; f < meta.frames; f++) {
    flame.push(all.subarray(2 * f * n, (2 * f + 1) * n));
    temp.push(all.subarray((2 * f + 1) * n, (2 * f + 2) * n));
  }
  return { flame, temp, cells, meta };
}

/// The loop: `count` frames, the `fade` after them blended into the first.
export function crossfade(raw: Float32Array[], count: number, fade: number): Float32Array[] {
  if (raw.length < count + fade) throw new Error(`a loop of ${count} with ${fade} to blend needs ${count + fade} frames, not ${raw.length}`);
  const out: Float32Array[] = [];
  for (let i = 0; i < count; i++) {
    if (i >= fade) {
      out.push(raw[i]);
      continue;
    }
    const a = raw[i];
    const b = raw[count + i];
    const t = (i + 1) / (fade + 1);
    const fr = new Float32Array(a.length);
    for (let p = 0; p < a.length; p++) fr[p] = b[p] * (1 - t) + a[p] * t;
    out.push(fr);
  }
  return out;
}

/// Mean absolute change a frame to the next, round the loop: steps[i] is frame i to i + 1, the
/// last the seam (the last frame to the first).
export function steps(frames: Float32Array[]): number[] {
  const out: number[] = [];
  for (let i = 0; i < frames.length; i++) {
    const a = frames[i];
    const b = frames[(i + 1) % frames.length];
    let s = 0;
    for (let p = 0; p < a.length; p++) s += Math.abs(b[p] - a[p]);
    out.push(s / a.length);
  }
  return out;
}

export const COARSE = 8;

/// Frames averaged over blocks of k cells a side (a remainder at the far sides dropped).
export function coarse(frames: Float32Array[], cells: [number, number, number], k: number): Float32Array[] {
  const [nx, ny, nz] = cells;
  const [cx, cy, cz] = [Math.floor(nx / k), Math.floor(ny / k), Math.floor(nz / k)];
  return frames.map((fr) => {
    const out = new Float32Array(cx * cy * cz);
    for (let z = 0; z < cz * k; z++)
      for (let y = 0; y < cy * k; y++)
        for (let x = 0; x < cx * k; x++) out[(Math.floor(z / k) * cy + Math.floor(y / k)) * cx + Math.floor(x / k)] += fr[(z * ny + y) * nx + x] / k ** 3;
    return out;
  });
}

export type Seam = { seam: number; mean: number; p95: number; min: number; max: number; ratio: number; pass: boolean };

/// The seam's check (the header): the last step against the others.
export function seam(frames: Float32Array[]): Seam {
  const s = steps(frames);
  const seamStep = s[s.length - 1];
  const rest = s.slice(0, -1);
  const sorted = [...rest].sort((a, b) => a - b);
  const mean = rest.reduce((a, b) => a + b, 0) / rest.length;
  const p95 = sorted[Math.min(sorted.length - 1, Math.floor(0.95 * sorted.length))];
  const ratio = seamStep / mean;
  return { seam: seamStep, mean, p95, min: sorted[0], max: sorted[sorted.length - 1], ratio, pass: ratio <= 1.25 && seamStep <= p95 };
}

/// Frames tiled into one 3D texture's channel (the header's layout).
export function tile(frames: Float32Array[], cells: [number, number, number], columns: number): { data: Float32Array; size: [number, number, number] } {
  const [nx, ny, nz] = cells;
  const rows = Math.ceil(frames.length / columns);
  const W = columns * nx;
  const H = rows * ny;
  const data = new Float32Array(W * H * nz);
  frames.forEach((fr, f) => {
    const ox = (f % columns) * nx;
    const oy = Math.floor(f / columns) * ny;
    for (let z = 0; z < nz; z++)
      for (let y = 0; y < ny; y++) {
        const src = (z * ny + y) * nx;
        data.set(fr.subarray(src, src + nx), (z * H + oy + y) * W + ox);
      }
  });
  return { data, size: [W, H, nz] };
}

/// The frames back out of a tiled channel.
export function untile(data: Float32Array, size: [number, number, number], cells: [number, number, number], columns: number, count: number): Float32Array[] {
  const [nx, ny, nz] = cells;
  const [W, H] = size;
  const out: Float32Array[] = [];
  for (let f = 0; f < count; f++) {
    const fr = new Float32Array(nx * ny * nz);
    const ox = (f % columns) * nx;
    const oy = Math.floor(f / columns) * ny;
    for (let z = 0; z < nz; z++)
      for (let y = 0; y < ny; y++) {
        const at = (z * H + oy + y) * W + ox;
        fr.set(data.subarray(at, at + nx), (z * ny + y) * nx);
      }
    out.push(fr);
  }
  return out;
}

const fmt = (x: number) => x.toFixed(4);

export function packVolume(r: VolumeRecipe, out: string, dest: string): { line: string; seam: Seam; size: number } {
  const raw = readRaw(out);
  const v = r.volume;
  const count = r.frames;
  const fade = r.crossfade ?? 0;
  const tScale = raw.meta.temperature_max || 1;
  const temps = raw.temp.map((t) => t.map((x) => Math.min(1, Math.max(0, x / tScale))));
  const flames = raw.flame.map((f) => f.map((x) => Math.min(1, Math.max(0, x))));
  const loopFlame = crossfade(flames, count, fade);
  const loopTemp = crossfade(temps, count, fade);

  const R = tile(loopFlame, raw.cells, v.columns);
  const G = tile(loopTemp, raw.cells, v.columns);
  const [W, H, D] = R.size;
  const bc5 = encodeBc5(R.data, G.data, W, H, D);
  const file = dds3d(DXGI_BC5_UNORM, W, H, D, bc5);
  writeFileSync(join(dest, `${r.name}.dds`), file);
  const rg = new Uint8Array(W * H * D * 2);
  for (let i = 0; i < W * H * D; i++) {
    rg[2 * i] = Math.round(R.data[i] * 255);
    rg[2 * i + 1] = Math.round(G.data[i] * 255);
  }
  writeFileSync(join(out, `${r.name}.rg8.dds`), dds3d(DXGI_R8G8_UNORM, W, H, D, rg));

  // The seam, measured on what the game will read; and what it would be with no crossfade.
  const [dr] = decodeBc5(bc5, W, H, D);
  const decoded = untile(dr, R.size, raw.cells, v.columns, count);
  const s = seam(decoded);
  const bare = seam(flames.slice(0, count));
  const sc = seam(coarse(decoded, raw.cells, COARSE));
  const bareC = seam(coarse(flames.slice(0, count), raw.cells, COARSE));
  const pass = s.pass && sc.pass;
  let err = 0;
  for (let i = 0; i < dr.length; i++) err += Math.abs(dr[i] - R.data[i]);

  // The loop's light a frame: its flame's emission (flame^3, as the bake's films shade it).
  const power = loopFlame.map((f) => f.reduce((a, x) => a + x * x * x, 0));
  const meanPower = power.reduce((a, b) => a + b, 0) / power.length;
  const h = raw.meta.cell;
  const sidecar = {
    about: r.about,
    texture: `${r.name}.dds`,
    format: "BC5 3D (R flame, G temperature); an adapter without BC sliced 3D decodes it to Rg8Unorm",
    frames: count,
    fps: r.fps,
    cells: raw.cells,
    cell: h,
    columns: v.columns,
    rows: Math.ceil(count / v.columns),
    size: R.size,
    box_min: v.domain_min.map((m, i) => m + v.box_min[i] * h),
    box_size: raw.cells.map((c) => c * h),
    wind: "+x",
    temperature_scale: tScale,
    crossfade: fade,
    light: power.map((p) => Number((p / meanPower).toFixed(3))),
    source: { cache: v.cache, first: v.first, box_min: v.box_min, flame_outside_max: raw.meta.outside_flame_max },
    seam: {
      cells: { step: s.seam, mean: s.mean, p95: s.p95, ratio: s.ratio, uncrossfaded_ratio: bare.ratio },
      [`blocks_of_${COARSE}`]: { step: sc.seam, mean: sc.mean, p95: sc.p95, ratio: sc.ratio, uncrossfaded_ratio: bareC.ratio },
    },
  };
  writeFileSync(join(dest, `${r.name}.json`), JSON.stringify(sidecar, null, 1) + "\n");
  const mb = (file.length / 2 ** 20).toFixed(1);
  const line =
    `${W}x${H}x${D} BC5 ${mb} MiB (Rg8 ${((rg.length + 148) / 2 ** 20).toFixed(1)} MiB in shots), mean error ${((err / dr.length) * 255).toFixed(2)}/255; ` +
    `seam ${pass ? "✓" : "✗"} by cells ${s.ratio.toFixed(2)}x the mean step ${fmt(s.mean)} (p95 ${(s.p95 / s.mean).toFixed(2)}x, uncrossfaded ${bare.ratio.toFixed(2)}x), ` +
    `by ${COARSE}-cell blocks ${sc.ratio.toFixed(2)}x ${fmt(sc.mean)} (p95 ${(sc.p95 / sc.mean).toFixed(2)}x, steps ${(sc.min / sc.mean).toFixed(2)}-${(sc.max / sc.mean).toFixed(2)}x, uncrossfaded ${bareC.ratio.toFixed(2)}x)`;
  return { line, seam: { ...s, pass }, size: file.length };
}
