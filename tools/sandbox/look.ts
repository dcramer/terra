// How a picture of the forest looks, in the numbers it is held to (D286): how bright it is
// and how much of it is near black, how warm, how much local contrast, and how much of its
// lower third is lit orange litter, measured as research/2026-10-14-sotf-forest-look.md
// measured Sons of the Forest's own daytime forest frames, whose means are `SOTF`. Values are
// the picture's sRGB values, as a person sees them, not linear light. Node 22.18 or later;
// no dependencies.

import type { Picture } from "./png.ts";

export type Look = {
  /// Mean brightness (Rec. 709 luma of the sRGB values), and its 5th, 50th and 95th
  /// percentiles.
  mean: number;
  p5: number;
  p50: number;
  p95: number;
  /// Shares of the picture darker than 0.08 and brighter than 0.85.
  dark: number;
  bright: number;
  /// The mean red and blue against the mean green.
  redOverGreen: number;
  blueOverGreen: number;
  /// Local contrast: the spread of brightness in 32-pixel squares, over their brightness.
  contrast: number;
  /// The share of the lower third that is lit orange litter: orange, saturated and bright.
  litLitter: number;
  /// The shade's colour: red and blue against green over the pixels darker than 0.08; and
  /// blue against green in the middle third's mid-tones (0.05 to 0.3), the middle distance.
  shadeRedOverGreen: number;
  shadeBlueOverGreen: number;
  middleBlueOverGreen: number;
};

/// Sons of the Forest's twenty daytime forest frames, measured so (the research note's
/// table).
export const SOTF: Look = {
  mean: 0.148,
  p5: 0.016,
  p50: 0.106,
  p95: 0.407,
  dark: 0.461,
  bright: 0.003,
  redOverGreen: 1.095,
  blueOverGreen: 0.637,
  contrast: 0.426,
  litLitter: 0.034,
  shadeRedOverGreen: 1.261,
  shadeBlueOverGreen: 0.377,
  middleBlueOverGreen: 0.514,
};

/// The picture's look; `masked` pixels (such as the placeholder player in a film) are left
/// out.
export function look(p: Picture, masked: (x: number, y: number) => boolean = () => false): Look {
  const { width: W, height: H, rgba } = p;
  const luma = new Float32Array(W * H);
  const ys: number[] = [];
  let r = 0, g = 0, b = 0, dark = 0, bright = 0, lower = 0, litter = 0;
  const shade = [0, 0, 0], middle = [0, 0, 0];
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const i = (y * W + x) * 4;
      const [cr, cg, cb] = [rgba[i] / 255, rgba[i + 1] / 255, rgba[i + 2] / 255];
      const v = 0.2126 * cr + 0.7152 * cg + 0.0722 * cb;
      if (masked(x, y)) {
        luma[y * W + x] = NaN;
        continue;
      }
      luma[y * W + x] = v;
      ys.push(v);
      r += cr;
      g += cg;
      b += cb;
      if (v < 0.08) {
        dark++;
        shade[0] += cr;
        shade[1] += cg;
        shade[2] += cb;
      }
      if (y >= H / 3 && y < (H * 2) / 3 && v > 0.05 && v < 0.3) {
        middle[1] += cg;
        middle[2] += cb;
      }
      if (v > 0.85) bright++;
      if (y >= (H * 2) / 3) {
        lower++;
        const high = Math.max(cr, cg, cb), low = Math.min(cr, cg, cb), d = high - low;
        const l = (high + low) / 2;
        const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
        let h = 0;
        if (d > 0) h = high === cr ? 60 * (((cg - cb) / d + 6) % 6) : high === cg ? 60 * ((cb - cr) / d + 2) : 60 * ((cr - cg) / d + 4);
        if (h >= 8 && h < 45 && s > 0.25 && v > 0.25) litter++;
      }
    }
  }
  ys.sort((a, c) => a - c);
  const at = (q: number) => ys[Math.min(ys.length - 1, Math.floor(q * ys.length))];
  let spread = 0, squares = 0;
  for (let by = 0; by + 32 <= H; by += 32) {
    for (let bx = 0; bx + 32 <= W; bx += 32) {
      let m = 0, m2 = 0, k = 0, skip = false;
      for (let y = by; y < by + 32 && !skip; y++) {
        for (let x = bx; x < bx + 32; x++) {
          const v = luma[y * W + x];
          if (Number.isNaN(v)) {
            skip = true;
            break;
          }
          m += v;
          m2 += v * v;
          k++;
        }
      }
      if (skip) continue;
      m /= k;
      spread += Math.sqrt(Math.max(0, m2 / k - m * m)) / Math.max(m, 0.02);
      squares++;
    }
  }
  const n = ys.length;
  return {
    mean: ys.reduce((s, v) => s + v, 0) / n,
    p5: at(0.05),
    p50: at(0.5),
    p95: at(0.95),
    dark: dark / n,
    bright: bright / n,
    redOverGreen: r / g,
    blueOverGreen: b / g,
    contrast: squares ? spread / squares : 0,
    litLitter: lower ? litter / lower : 0,
    shadeRedOverGreen: shade[1] ? shade[0] / shade[1] : 0,
    shadeBlueOverGreen: shade[1] ? shade[2] / shade[1] : 0,
    middleBlueOverGreen: middle[1] ? middle[2] / middle[1] : 0,
  };
}

/// The mean of several pictures' looks.
export function meanOf(looks: Look[]): Look {
  const keys = Object.keys(SOTF) as (keyof Look)[];
  return Object.fromEntries(keys.map((k) => [k, looks.reduce((s, l) => s + l[k], 0) / looks.length])) as Look;
}

/// One line for a look: each number, and Sons of the Forest's beside it.
export function line(name: string, l: Look): string {
  const f = (v: number) => v.toFixed(3);
  return `${name}: mean ${f(l.mean)} (SotF ${f(SOTF.mean)}), p5 ${f(l.p5)} (${f(SOTF.p5)}), below 0.08 ${(100 * l.dark).toFixed(0)}% (${(100 * SOTF.dark).toFixed(0)}%), red/green ${f(l.redOverGreen)} (${f(SOTF.redOverGreen)}), blue/green ${f(l.blueOverGreen)} (${f(SOTF.blueOverGreen)}), contrast ${f(l.contrast)} (${f(SOTF.contrast)}), lit litter ${(100 * l.litLitter).toFixed(1)}% (${(100 * SOTF.litLitter).toFixed(1)}%), shade red/green ${f(l.shadeRedOverGreen)} (${f(SOTF.shadeRedOverGreen)}), shade blue/green ${f(l.shadeBlueOverGreen)} (${f(SOTF.shadeBlueOverGreen)}), middle blue/green ${f(l.middleBlueOverGreen)} (${f(SOTF.middleBlueOverGreen)})`;
}
