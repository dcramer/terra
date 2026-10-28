// Packs a six-way smoke set's raw passes (bake.py's light0, light1 and glow) into the six-way
// lightmap layout (docs/fire.md, "The flipbook format" and "The smoke"; D415), the one Unity's
// VFX Graph reads:
//
// - <set>.png: map 0, 8x8 frames, R lit from the right (+X), G from the top (+Z), B from the
//   back (+Y, away from the camera), A alpha;
// - <set>.opposite.png: map 1, R lit from the left, G from the bottom, B from the front, A glow
//   (the smoke's mean temperature, for the hot core's own light);
// - <set>.json: the sidecar, frames, grid, light_scale and channels.
//
// A recipe with `fit` (blaze-smoke) has each frame moved and scaled so its puff sits in the
// frame's middle at a fixed share of it (`fitFrame`), so the frame's texels all go to the puff
// and the card that draws it need not know where the bake left it.
//
// alpha is the glow pass's G, 1 - e^-tau exactly. Each map is the light over alpha, so straight
// (not premultiplied) blending gives the response back; its texel is that times `light_scale`,
// which puts the set's 99th percentile at 0.97. In bake.py's units a white Lambert surface facing
// the light is 1, and thin smoke lit from one side 1/4. No motion vectors: frames crossfade.
// A maps preview, shots/fire/bake/<set>/maps.png, shows a row per sample frame: the six maps,
// alpha and glow.

import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { writePng } from "../../sandbox/png.ts";
import type { Recipe } from "./bake.ts";
import { COLUMNS as EIGHT, halves, type Packed } from "./pack.ts";

/// The six maps (right, top, back, left, bottom, front), alpha and glow of one frame.
export type SmokeFrame = { maps: Float32Array[]; alpha: Float32Array; glow: Float32Array };

const quantile = (values: number[], q: number) => {
  const s = Float32Array.from(values).sort();
  return s.length ? s[Math.min(s.length - 1, Math.floor(q * s.length))] : 0;
};

/// One frame's maps from its raw passes, each N texels of RGB: the light over alpha (floored at
/// 0.02, so a wisp's edge does not blow up), alpha and the unscaled mean temperature.
export function smokeFrame(light0: Float32Array, light1: Float32Array, glow: Float32Array, N: number): SmokeFrame {
  const maps = Array.from({ length: 6 }, () => new Float32Array(N));
  const alpha = new Float32Array(N);
  const heat = new Float32Array(N);
  for (let i = 0; i < N; i++) {
    const a = Math.min(1, Math.max(0, glow[i * 3 + 1]));
    alpha[i] = a;
    heat[i] = a > 1e-4 ? Math.max(0, glow[i * 3]) / a : 0;
    const k = 1 / Math.max(a, 0.02);
    for (let c = 0; c < 3; c++) {
      maps[c][i] = Math.max(0, light0[i * 3 + c]) * k;
      maps[3 + c][i] = Math.max(0, light1[i * 3 + c]) * k;
    }
  }
  return { maps, alpha, glow: heat };
}

/// Where a frame's puff is and how big: its alpha's centroid, in texels from the frame's top
/// left corner, and the radius about it holding `share` of its alpha.
export function puffOf(alpha: Float32Array, n: number, share = 0.9): { x: number; y: number; radius: number } {
  let [sw, sx, sy] = [0, 0, 0];
  for (let p = 0; p < n * n; p++) {
    const a = alpha[p];
    sw += a;
    sx += a * ((p % n) + 0.5);
    sy += a * (Math.floor(p / n) + 0.5);
  }
  if (sw <= 1e-6) return { x: n / 2, y: n / 2, radius: n / 4 };
  const [x, y] = [sx / sw, sy / sw];
  // A histogram of the alpha by distance, in half-texel rings.
  const rings = new Float64Array(n * 2 + 2);
  for (let p = 0; p < n * n; p++) {
    const d = Math.hypot((p % n) + 0.5 - x, Math.floor(p / n) + 0.5 - y);
    rings[Math.min(rings.length - 1, Math.floor(d * 2))] += alpha[p];
  }
  let held = 0;
  for (let k = 0; k < rings.length; k++) {
    held += rings[k];
    if (held >= share * sw) return { x, y, radius: (k + 1) / 2 };
  }
  return { x, y, radius: n / 2 };
}

/// One frame moved and scaled so its puff (`puffOf`) sits in the middle of a frame of `m`
/// texels with `radius` of the frame for its radius, as a blaze's card draws it (fire_smoke.wesl's
/// `blaze_maps`): resampled premultiplied, 3x3 bilinear taps a texel, nothing outside the frame.
export function fitFrame(fr: SmokeFrame, n: number, m: number, radius: number): SmokeFrame {
  const puff = puffOf(fr.alpha, n);
  const s = puff.radius / (radius * m);
  const pre = [...fr.maps.map((map) => map.map((v, p) => v * fr.alpha[p])), fr.alpha, fr.glow.map((v, p) => v * fr.alpha[p])];
  const tap = (img: Float32Array, x: number, y: number) => {
    const [fx, fy] = [x - 0.5, y - 0.5];
    const [x0, y0] = [Math.floor(fx), Math.floor(fy)];
    const [tx, ty] = [fx - x0, fy - y0];
    const at = (i: number, j: number) => (i < 0 || j < 0 || i >= n || j >= n ? 0 : img[j * n + i]);
    return (at(x0, y0) * (1 - tx) + at(x0 + 1, y0) * tx) * (1 - ty) + (at(x0, y0 + 1) * (1 - tx) + at(x0 + 1, y0 + 1) * tx) * ty;
  };
  const out = pre.map(() => new Float32Array(m * m));
  for (let j = 0; j < m; j++) {
    for (let i = 0; i < m; i++) {
      for (let c = 0; c < out.length; c++) {
        let v = 0;
        for (let sj = 0; sj < 3; sj++) {
          for (let si = 0; si < 3; si++) {
            const x = puff.x + (i + (si + 0.5) / 3 - m / 2) * s;
            const y = puff.y + (j + (sj + 0.5) / 3 - m / 2) * s;
            v += tap(pre[c], x, y);
          }
        }
        out[c][j * m + i] = v / 9;
      }
    }
  }
  const alpha = out[6];
  const k = (p: number) => 1 / Math.max(alpha[p], 0.02);
  return { maps: out.slice(0, 6).map((map) => map.map((v, p) => v * k(p))), alpha, glow: out[7].map((v, p) => v * k(p)) };
}

export function packSmoke(r: Recipe, raw: string, outDir: string): Packed {
  const meta = JSON.parse(readFileSync(join(raw, "meta.json"), "utf8"));
  const baked: number = meta.size;
  const data = halves(readFileSync(join(raw, "raw.bin")));
  const per = baked * baked * 3;
  const pass = (f: number, which: number) => data.subarray((f * 3 + which) * per, (f * 3 + which + 1) * per);
  let frames: SmokeFrame[] = [];
  for (let f = 0; f < meta.frames; f++) frames.push(smokeFrame(pass(f, 0), pass(f, 1), pass(f, 2), baked * baked));
  const fit = r.fit;
  if (fit) frames = frames.map((fr) => fitFrame(fr, baked, fit.frame, fit.radius));
  const n = fit ? fit.frame : baked;
  const N = n * n;
  const COLUMNS = r.columns ?? EIGHT;

  // The scales, from the texels the smoke really covers.
  const peaks: number[] = [];
  const heats: number[] = [];
  for (const fr of frames) {
    for (let i = 0; i < N; i += 3) {
      if (fr.alpha[i] <= 0.05) continue;
      peaks.push(Math.max(...fr.maps.map((m) => m[i])));
      heats.push(fr.glow[i]);
    }
  }
  const lightScale = 0.97 / Math.max(1e-4, quantile(peaks, 0.99));
  const heatScale = 0.97 / Math.max(1e-4, quantile(heats, 0.99));

  const count = frames.length;
  const rows = Math.ceil(count / COLUMNS);
  const W = n * COLUMNS;
  const near = new Uint8Array(W * n * rows * 4);
  const far = new Uint8Array(W * n * rows * 4);
  const byte = (v: number) => Math.round(Math.min(1, Math.max(0, v)) * 255);
  frames.forEach((fr, f) => {
    const [cx, cy] = [(f % COLUMNS) * n, Math.floor(f / COLUMNS) * n];
    for (let p = 0; p < N; p++) {
      const o = ((cy + Math.floor(p / n)) * W + cx + (p % n)) * 4;
      for (let c = 0; c < 3; c++) {
        near[o + c] = byte(fr.maps[c][p] * lightScale);
        far[o + c] = byte(fr.maps[3 + c][p] * lightScale);
      }
      near[o + 3] = byte(fr.alpha[p]);
      far[o + 3] = byte(fr.glow[p] * heatScale);
    }
  });
  mkdirSync(outDir, { recursive: true });
  writePng(join(outDir, `${r.name}.png`), { width: W, height: n * rows, rgba: near });
  writePng(join(outDir, `${r.name}.opposite.png`), { width: W, height: n * rows, rgba: far });
  preview(frames, n, join(raw, "maps.png"), lightScale);

  // What the line says: how much of a frame the smoke covers at its thickest, how bright its
  // sides are against one another (lit from the camera's side against from behind: a thick puff
  // is far brighter in front), and its tallest reach.
  let [cover, reach] = [0, 0];
  const side = [0, 0, 0, 0, 0, 0];
  let sw = 0;
  for (const fr of frames) {
    let [c, top] = [0, n];
    for (let p = 0; p < N; p++) {
      const a = fr.alpha[p];
      if (a > 0.1) {
        c++;
        top = Math.min(top, Math.floor(p / n));
      }
      for (let k = 0; k < 6; k++) side[k] += fr.maps[k][p] * a;
      sw += a;
    }
    cover = Math.max(cover, c / N);
    reach = Math.max(reach, 1 - top / n);
  }
  const mean = side.map((s) => s / Math.max(1e-6, sw));
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
    fps: meta.fps / (meta.step ?? 1),
    metres: meta.extent,
    reach: +reach.toFixed(3),
    channels: "map 0 (<set>.png): R right +X, G top +Z, B back +Y, A alpha; map 1 (<set>.opposite.png): R left, G bottom, B front, A glow",
    light_scale: +lightScale.toFixed(4),
    baked: { heat_scale: +heatScale.toFixed(4), mean_response: mean.map((m) => +m.toFixed(3)) },
  };
  writeFileSync(join(outDir, `${r.name}.json`), `${JSON.stringify(sidecar, null, 2)}\n`);
  const line =
    `${count} frames, cover at most ${pct(cover)}, reach ${pct(reach)}, light x${lightScale.toFixed(2)}, ` +
    `mean response R/T/B/L/Bo/F ${mean.map((m) => m.toFixed(2)).join("/")}`;
  return { frames: [], size: n, line, sidecar };
}

/// A row per sample frame: the six maps (as the engine reads them), alpha and glow, grey.
function preview(frames: SmokeFrame[], n: number, path: string, lightScale: number) {
  const picks = [0.1, 0.3, 0.55, 0.85].map((q) => Math.min(frames.length - 1, Math.floor(q * frames.length)));
  const W = n * 8;
  const rgba = new Uint8Array(W * n * picks.length * 4);
  let peakGlow = 1e-4;
  for (const f of frames) for (const g of f.glow) peakGlow = Math.max(peakGlow, g);
  picks.forEach((f, row) => {
    const fr = frames[f];
    const tiles = [...fr.maps.map((m) => (p: number) => m[p] * lightScale * fr.alpha[p]), (p: number) => fr.alpha[p], (p: number) => (fr.glow[p] / peakGlow) * fr.alpha[p]];
    tiles.forEach((tile, t) => {
      for (let p = 0; p < n * n; p++) {
        const o = ((row * n + Math.floor(p / n)) * W + t * n + (p % n)) * 4;
        const v = Math.round(Math.min(1, Math.max(0, tile(p))) ** (1 / 2.2) * 255);
        rgba[o] = rgba[o + 1] = rgba[o + 2] = v;
        rgba[o + 3] = 255;
      }
    });
  });
  writePng(path, { width: W, height: n * picks.length, rgba });
}
