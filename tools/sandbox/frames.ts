// What changed between frames of a scripted run, pixel by pixel, for artifacts.ts: each
// measure a mask of the pixels it finds, so its share of the picture can be held to a limit
// and the pixels marked where a person can see them. Node 22.18 or later; no dependencies.

import type { Picture } from "./png.ts";

/// A picture as brightness, 0 to 1.
export type Grey = { width: number; height: number; v: Float32Array };

/// A pixel counts as changed past this much brightness.
export const CHANGED = 0.04;

/// A picture as brightness at half its size, each value the mean of four pixels.
export function grey(p: Picture): Grey {
  const width = p.width >> 1;
  const height = p.height >> 1;
  const v = new Float32Array(width * height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      let sum = 0;
      for (const [dx, dy] of [[0, 0], [1, 0], [0, 1], [1, 1]]) {
        const i = ((y * 2 + dy) * p.width + x * 2 + dx) * 4;
        sum += 0.2126 * p.rgba[i] + 0.7152 * p.rgba[i + 1] + 0.0722 * p.rgba[i + 2];
      }
      v[y * width + x] = sum / (4 * 255);
    }
  }
  return { width, height, v };
}

/// Blurred over a square so many pixels each side, so a camera's smooth motion cancels.
export function blur(g: Grey, r: number): Grey {
  const { width, height } = g;
  const out = new Float32Array(width * height);
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      let sum = 0;
      let n = 0;
      for (let dy = -r; dy <= r; dy++) {
        const yy = y + dy;
        if (yy < 0 || yy >= height) continue;
        for (let dx = -r; dx <= r; dx++) {
          const xx = x + dx;
          if (xx < 0 || xx >= width) continue;
          sum += g.v[yy * width + xx];
          n++;
        }
      }
      out[y * width + x] = sum / n;
    }
  }
  return { width, height, v: out };
}

export const mean = (g: Grey) => g.v.reduce((s, x) => s + x, 0) / g.v.length;

/// The share of a mask set, in percent.
export const share = (mask: Uint8Array) => (100 * mask.reduce((s, x) => s + x, 0)) / mask.length;

/// Pixels that changed from one frame to the next: with nothing moving, the renderer's own
/// noise (acne flickering, dither crawling, a texture shimmering).
export function changed(a: Grey, b: Grey, threshold = CHANGED): Uint8Array {
  const m = new Uint8Array(a.v.length);
  for (let i = 0; i < m.length; i++) m[i] = Math.abs(a.v[i] - b.v[i]) > threshold ? 1 : 0;
  return m;
}

/// Pixels that went one way for one frame and came back: the frames either side agree, the
/// middle one does not. `by` is how far each way it must go.
export function flicked(before: Grey, now: Grey, after: Grey, by = CHANGED): Uint8Array {
  const m = new Uint8Array(now.v.length);
  for (let k = 0; k < m.length; k++) {
    const d1 = now.v[k] - before.v[k];
    const d2 = after.v[k] - now.v[k];
    m[k] = Math.abs(d1) > by && Math.abs(d2) > by && Math.sign(d1) !== Math.sign(d2) && Math.abs(after.v[k] - before.v[k]) < by * 0.5 ? 1 : 0;
  }
  return m;
}

/// Pixels changing in each of three frames running: a still camera's picture should settle
/// in two once something has gone in; one that is still changing is TAA trailing it.
export function trailing(d0: Uint8Array, d1: Uint8Array, d2: Uint8Array): Uint8Array {
  const m = new Uint8Array(d0.length);
  for (let k = 0; k < m.length; k++) m[k] = d0[k] & d1[k] & d2[k];
  return m;
}

/// How far the picture's mean brightness moved from one frame to the next, in percent.
export const jumped = (before: number, now: number) => (100 * Math.abs(now - before)) / Math.max(0.02, before);
