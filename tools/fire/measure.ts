#!/usr/bin/env node
// How a fire's pixels sit in a crop (docs/fire.md, "The automatic check"; D415), for holding
// renders to photographs of real fire by the same numbers:
//
//   node tools/fire/measure.ts <png|jpg> [WxH+X+Y] [--class structure] [--json]
//
// Flame = max channel >= 110 with red the largest, blue under green and saturated (max - min at
// least 20 and a fifth of max, so a lit pink stone is not flame), or near white (min >= 235).
// Its bands (white, pale yellow, yellow, orange, red), B/R and G/R, and its dim share (luma
// under 0.45 of its p99: thin, see-through flame) are taken at the picture's own size, as they are shares. What depends on scale (soot round the flame, its edge, the dark
// pockets in it, detail by octave) is taken after the crop is resampled so the flame's box is
// `FLAME_H` pixels tall, so a small fire in a render and a frame-filling one in a photograph
// compare. With --class it prints each number against that class's range in references.json
// (a ✗ where one falls out) and exits 1 when one does. Ported from the measurer the castle
// film's rounds used (MEASURED.md's method). Node 22.18 or later, ffmpeg for anything but PNG;
// no dependencies.

import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { type Picture, readPng } from "../sandbox/png.ts";

/// A crop, in pixels of the picture.
export type Box = { w: number; h: number; x: number; y: number };

/// The numbers a fire is held to. Shares are percent of the flame's pixels unless named.
export type Metrics = {
  flamePx: number;
  white: number;
  pale: number;
  yellow: number;
  orange: number;
  red: number;
  redOrange: number;
  BR: number;
  GR: number;
  dim: number;
  /// Soot (dark pixels near the flame) over flame, by area.
  soot: number;
  /// Of the flame's edge, the percent meeting soot just outside it.
  edgeSoot: number;
  /// Dark pockets enclosed in the flame, per `FLAME_H`-square of its box.
  pockets: number;
  /// Luma detail by octave, percent of the six: fine (cells of 1-3% of the flame's height),
  /// mid (3-12%) and coarse (12-50%).
  fine: number;
  mid: number;
  coarse: number;
};

/// How tall the flame's box is resampled to for the measures that depend on scale.
export const FLAME_H = 256;
/// How near the flame soot is counted, and how far out its edge is looked at, in pixels of
/// the resampled crop (3% and 1% of the flame's height).
const SOOT_R = 8;
const EDGE_OUT = 3;

/// A crop written as ImageMagick's geometry, `WxH+X+Y`.
export function parseBox(text: string): Box {
  const m = /^(\d+)x(\d+)\+(\d+)\+(\d+)$/.exec(text);
  if (!m) throw new Error(`${text}: a crop is WxH+X+Y`);
  return { w: +m[1], h: +m[2], x: +m[3], y: +m[4] };
}

/// A picture from a file: a PNG read here, anything else decoded by ffmpeg.
export function load(path: string): Picture {
  if (path.toLowerCase().endsWith(".png")) return readPng(path);
  const ppm = execFileSync("ffmpeg", ["-v", "error", "-i", path, "-frames:v", "1", "-f", "image2pipe", "-vcodec", "ppm", "-"], {
    maxBuffer: 1 << 30,
  });
  return fromPpm(ppm);
}

/// A binary PPM (P6, 8 bits) as a picture.
export function fromPpm(ppm: Uint8Array): Picture {
  let at = 0;
  const token = () => {
    while (ppm[at] === 0x20 || ppm[at] === 0x0a || ppm[at] === 0x0d || ppm[at] === 0x09) at++;
    let s = "";
    while (at < ppm.length && ppm[at] > 0x20) s += String.fromCharCode(ppm[at++]);
    return s;
  };
  if (token() !== "P6") throw new Error("not a binary PPM");
  const width = +token();
  const height = +token();
  token();
  at++;
  const rgba = new Uint8Array(width * height * 4);
  for (let p = 0; p < width * height; p++) {
    rgba[4 * p] = ppm[at + 3 * p];
    rgba[4 * p + 1] = ppm[at + 3 * p + 1];
    rgba[4 * p + 2] = ppm[at + 3 * p + 2];
    rgba[4 * p + 3] = 255;
  }
  return { width, height, rgba };
}

/// RGB of a crop, three bytes a pixel.
function cropRgb(pic: Picture, box?: Box): { w: number; h: number; px: Uint8Array } {
  const x0 = Math.max(0, box?.x ?? 0);
  const y0 = Math.max(0, box?.y ?? 0);
  const w = Math.min(pic.width - x0, box?.w ?? pic.width);
  const h = Math.min(pic.height - y0, box?.h ?? pic.height);
  if (w <= 0 || h <= 0) throw new Error("the crop is outside the picture");
  const px = new Uint8Array(w * h * 3);
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      const s = 4 * ((y0 + y) * pic.width + x0 + x);
      const d = 3 * (y * w + x);
      px[d] = pic.rgba[s];
      px[d + 1] = pic.rgba[s + 1];
      px[d + 2] = pic.rgba[s + 2];
    }
  return { w, h, px };
}

/// A coloured flame pixel's least saturation (chroma over its max): a lit stone or skin by a
/// fire is pale and pinkish (252,233,222 is 0.12), real flame hardly ever (photographs' campfire
/// crops: under 1%, but a dusk shot with the sky through it). A flame's blue never passes its
/// green (a blackbody's does not), so a pink pixel (195,114,123) is a lit surface too.
export const FLAME_SAT = 0.2;
export const isFlame = (r: number, g: number, b: number) => {
  const max = Math.max(r, g, b);
  const min = Math.min(r, g, b);
  return min >= 235 || (max >= 110 && r >= max && g >= b && max - min >= Math.max(20, FLAME_SAT * max));
};
const luma = (r: number, g: number, b: number) => 0.2126 * r + 0.7152 * g + 0.0722 * b;
const pct = (a: ArrayLike<number>, q: number) => (a.length ? a[Math.min(a.length - 1, Math.floor(q * a.length))] : 0);

/// Bilinear resample of RGB to a new size (area-averaged first when shrinking by 2 or more).
function resample(src: { w: number; h: number; px: Uint8Array }, w: number, h: number) {
  let cur = src;
  while (cur.w >= 2 * w && cur.h >= 2 * h) {
    const nw = cur.w >> 1;
    const nh = cur.h >> 1;
    const px = new Uint8Array(nw * nh * 3);
    for (let y = 0; y < nh; y++)
      for (let x = 0; x < nw; x++)
        for (let c = 0; c < 3; c++) {
          const a = (i: number, j: number) => cur.px[3 * ((2 * y + j) * cur.w + 2 * x + i) + c];
          px[3 * (y * nw + x) + c] = (a(0, 0) + a(1, 0) + a(0, 1) + a(1, 1) + 2) >> 2;
        }
    cur = { w: nw, h: nh, px };
  }
  const px = new Uint8Array(w * h * 3);
  for (let y = 0; y < h; y++)
    for (let x = 0; x < w; x++) {
      const fx = Math.min(cur.w - 1, Math.max(0, ((x + 0.5) * cur.w) / w - 0.5));
      const fy = Math.min(cur.h - 1, Math.max(0, ((y + 0.5) * cur.h) / h - 0.5));
      const x0 = Math.floor(fx);
      const y0 = Math.floor(fy);
      const x1 = Math.min(cur.w - 1, x0 + 1);
      const y1 = Math.min(cur.h - 1, y0 + 1);
      const tx = fx - x0;
      const ty = fy - y0;
      for (let c = 0; c < 3; c++) {
        const v = (i: number, j: number) => cur.px[3 * (j * cur.w + i) + c];
        const top = v(x0, y0) * (1 - tx) + v(x1, y0) * tx;
        const bot = v(x0, y1) * (1 - tx) + v(x1, y1) * tx;
        px[3 * (y * w + x) + c] = Math.round(top * (1 - ty) + bot * ty);
      }
    }
  return { w, h, px };
}

/// The least flame share a matte pixel has to be measured as flame, of 255: half, so a flame's
/// soft edge counts where the flame is most of what is seen, as a matte's edge is cut in
/// compositing (D433).
export const MATTE_MIN = 128;

/// A crop of a matte (`--flame-matte`, its red channel the share), a byte a pixel: whether
/// each is flame enough to measure.
function cropMatte(matte: Picture, box: Box | undefined, w: number, h: number): Uint8Array {
  const [x0, y0] = [Math.max(0, box?.x ?? 0), Math.max(0, box?.y ?? 0)];
  if (matte.width < x0 + w || matte.height < y0 + h) throw new Error("the matte is smaller than the picture");
  const keep = new Uint8Array(w * h);
  for (let y = 0; y < h; y++) for (let x = 0; x < w; x++) keep[y * w + x] = matte.rgba[4 * ((y0 + y) * matte.width + x0 + x)] >= MATTE_MIN ? 1 : 0;
  return keep;
}

/// The flame's box: its pixels' rows and columns from the 1st to the 99th percentile, so stray
/// sparks don't stretch it.
function flameBox(flame: Uint8Array, w: number, h: number) {
  const xs: number[] = [];
  const ys: number[] = [];
  for (let p = 0; p < w * h; p++)
    if (flame[p]) {
      xs.push(p % w);
      ys.push((p / w) | 0);
    }
  xs.sort((a, b) => a - b);
  ys.sort((a, b) => a - b);
  return { x0: pct(xs, 0.01), x1: pct(xs, 0.99), y0: pct(ys, 0.01), y1: pct(ys, 0.99) };
}

/// Measures the fire in a crop of a picture (the whole of it without one). With the run's flame
/// matte, only what the renderer drew as flame is measured as flame (D433): a lit stone or
/// ground can have a flame's colour, and at night most of a campfire's crop is lit ground.
export function measure(pic: Picture, box?: Box, matte?: Picture): Metrics {
  const crop = cropRgb(pic, box);
  const { w: W, h: H, px } = crop;
  const N = W * H;
  const keep = matte ? cropMatte(matte, box, W, H) : undefined;
  const flame = new Uint8Array(N);
  const band = { white: 0, pale: 0, yellow: 0, orange: 0, red: 0 };
  let nf = 0;
  let br = 0;
  let gr = 0;
  const fl: number[] = [];
  for (let p = 0; p < N; p++) {
    const r = px[3 * p];
    const g = px[3 * p + 1];
    const b = px[3 * p + 2];
    if (!isFlame(r, g, b) || (keep && !keep[p])) continue;
    flame[p] = 1;
    nf++;
    br += b / Math.max(r, 1);
    gr += g / Math.max(r, 1);
    fl.push(luma(r, g, b));
    const max = Math.max(r, g, b);
    const min = Math.min(r, g, b);
    const d = max - min;
    if (min >= 235) {
      band.white++;
      continue;
    }
    const hue = g >= b ? (60 * (g - b)) / d : 360 - (60 * (b - g)) / d;
    const sat = d / max;
    if (hue >= 45 && hue <= 65 && sat < 0.45) band.pale++;
    else if (hue >= 40 && hue <= 65) band.yellow++;
    else if (hue >= 25 && hue < 40) band.orange++;
    else band.red++;
  }
  fl.sort((a, b) => a - b);
  const p99 = pct(fl, 0.99);
  let dim = 0;
  for (const v of fl) if (v < 0.45 * p99) dim++;
  const share = (v: number) => (nf ? (100 * v) / nf : 0);
  const out: Metrics = {
    flamePx: nf,
    white: share(band.white),
    pale: share(band.pale),
    yellow: share(band.yellow),
    orange: share(band.orange),
    red: share(band.red),
    redOrange: share(band.red + band.orange),
    BR: br / Math.max(nf, 1),
    GR: gr / Math.max(nf, 1),
    dim: fl.length ? (100 * dim) / fl.length : 0,
    soot: 0,
    edgeSoot: 0,
    pockets: 0,
    fine: 0,
    mid: 0,
    coarse: 0,
  };
  if (nf === 0) return out;
  // The scale-dependent measures, on the crop resampled so the flame's box is FLAME_H tall.
  const fb = flameBox(flame, W, H);
  const k = FLAME_H / Math.max(1, fb.y1 - fb.y0 + 1);
  const [sw, sh] = [Math.max(8, Math.round(W * k)), Math.max(8, Math.round(H * k))];
  const scaled = resample(crop, sw, sh);
  // The matte resampled with the crop, as a grey picture: kept where it is still half flame.
  const grey = keep && resample({ w: W, h: H, px: Uint8Array.from({ length: 3 * N }, (_, i) => 255 * keep[(i / 3) | 0]) }, sw, sh);
  const kept = grey && Uint8Array.from({ length: sw * sh }, (_, p) => (grey.px[3 * p] >= MATTE_MIN ? 1 : 0));
  Object.assign(out, structure(scaled, kept));
  return out;
}

/// Soot, edge, pockets and octaves of a crop already at the flame's scale.
function structure({ w: W, h: H, px }: { w: number; h: number; px: Uint8Array }, keep?: Uint8Array) {
  const N = W * H;
  const flame = new Uint8Array(N);
  const maxc = new Uint8Array(N);
  const lum = new Float32Array(N);
  let nf = 0;
  for (let p = 0; p < N; p++) {
    const r = px[3 * p];
    const g = px[3 * p + 1];
    const b = px[3 * p + 2];
    maxc[p] = Math.max(r, g, b);
    lum[p] = luma(r, g, b);
    if (isFlame(r, g, b) && (!keep || keep[p])) {
      flame[p] = 1;
      nf++;
    }
  }
  const { x0, x1, y0, y1 } = flameBox(flame, W, H);
  const dark = (p: number) => !flame[p] && maxc[p] < 60;
  // Soot near flame: dark pixels within SOOT_R (square) of a flame pixel, by a separable max.
  const rowNear = new Uint8Array(N);
  const near = new Uint8Array(N);
  for (let y = 0; y < H; y++) {
    let cnt = 0;
    for (let x = -SOOT_R; x < W; x++) {
      if (x + SOOT_R < W) cnt += flame[y * W + x + SOOT_R];
      if (x - SOOT_R - 1 >= 0) cnt -= flame[y * W + x - SOOT_R - 1];
      if (x >= 0) rowNear[y * W + x] = cnt > 0 ? 1 : 0;
    }
  }
  for (let x = 0; x < W; x++) {
    let cnt = 0;
    for (let y = -SOOT_R; y < H; y++) {
      if (y + SOOT_R < H) cnt += rowNear[(y + SOOT_R) * W + x];
      if (y - SOOT_R - 1 >= 0) cnt -= rowNear[(y - SOOT_R - 1) * W + x];
      if (y >= 0) near[y * W + x] = cnt > 0 ? 1 : 0;
    }
  }
  let soot = 0;
  for (let p = 0; p < N; p++) if (near[p] && dark(p)) soot++;
  // The edge: each flame pixel with a non-flame 4-neighbour, looked at EDGE_OUT px out that way.
  const dirs = [
    [1, 0],
    [-1, 0],
    [0, 1],
    [0, -1],
  ];
  let edge = 0;
  let edgeSoot = 0;
  for (let y = 0; y < H; y++)
    for (let x = 0; x < W; x++) {
      if (!flame[y * W + x]) continue;
      for (const [dx, dy] of dirs) {
        const nx = x + dx;
        const ny = y + dy;
        if (nx < 0 || ny < 0 || nx >= W || ny >= H || flame[ny * W + nx]) continue;
        const ox = Math.min(W - 1, Math.max(0, x + EDGE_OUT * dx));
        const oy = Math.min(H - 1, Math.max(0, y + EDGE_OUT * dy));
        edge++;
        if (maxc[oy * W + ox] < 60) edgeSoot++;
        break;
      }
    }
  // Dark pockets: 4-connected dark components inside the flame's box not touching its edge,
  // of at least 4 px (a pocket a sixtieth of the flame tall).
  const seen = new Uint8Array(N);
  let pockets = 0;
  for (let y = y0; y <= y1; y++)
    for (let x = x0; x <= x1; x++) {
      const start = y * W + x;
      if (seen[start] || !dark(start)) continue;
      const stack = [start];
      seen[start] = 1;
      let size = 0;
      let touches = false;
      while (stack.length) {
        const p = stack.pop() as number;
        size++;
        const px_ = p % W;
        const py = (p / W) | 0;
        if (px_ === x0 || px_ === x1 || py === y0 || py === y1) touches = true;
        for (const [dx, dy] of dirs) {
          const nx = px_ + dx;
          const ny = py + dy;
          if (nx < x0 || ny < y0 || nx > x1 || ny > y1) continue;
          const q = ny * W + nx;
          if (seen[q] || !dark(q)) continue;
          seen[q] = 1;
          stack.push(q);
        }
      }
      if (!touches && size >= 4) pockets++;
    }
  const boxArea = Math.max(1, (x1 - x0 + 1) * (y1 - y0 + 1));
  const [fine, mid, coarse] = octaves(lum, W, H);
  return {
    soot: soot / Math.max(1, nf),
    edgeSoot: edge ? (100 * edgeSoot) / edge : 0,
    pockets: (pockets * FLAME_H * FLAME_H) / boxArea,
    fine,
    mid,
    coarse,
  };
}

/// Laplacian-pyramid energy of luma at cells of 2 to 64 px (binomial 5-tap blur, 2x
/// decimation), each level's squared detail times the area a cell covers; grouped as fine
/// (2+4), mid (8+16) and coarse (32+64), percent of the six.
export function octaves(lum: Float32Array, W: number, H: number): [number, number, number] {
  const blur = (a: Float32Array, w: number, h: number) => {
    const k = [1, 4, 6, 4, 1];
    const t = new Float32Array(w * h);
    const o = new Float32Array(w * h);
    for (let y = 0; y < h; y++)
      for (let x = 0; x < w; x++) {
        let s = 0;
        for (let i = -2; i <= 2; i++) s += k[i + 2] * a[y * w + Math.min(w - 1, Math.max(0, x + i))];
        t[y * w + x] = s / 16;
      }
    for (let y = 0; y < h; y++)
      for (let x = 0; x < w; x++) {
        let s = 0;
        for (let i = -2; i <= 2; i++) s += k[i + 2] * t[Math.min(h - 1, Math.max(0, y + i)) * w + x];
        o[y * w + x] = s / 16;
      }
    return o;
  };
  const energy: number[] = [];
  let g = Float32Array.from(lum, (v) => v / 255);
  let gw = W;
  let gh = H;
  for (let level = 0; level < 6 && gw >= 4 && gh >= 4; level++) {
    const b = blur(g, gw, gh);
    const nw = Math.ceil(gw / 2);
    const nh = Math.ceil(gh / 2);
    const next = new Float32Array(nw * nh);
    for (let y = 0; y < nh; y++) for (let x = 0; x < nw; x++) next[y * nw + x] = b[Math.min(gh - 1, 2 * y) * gw + Math.min(gw - 1, 2 * x)];
    const up = new Float32Array(gw * gh);
    for (let y = 0; y < gh; y += 2) for (let x = 0; x < gw; x += 2) up[y * gw + x] = next[(y / 2) * nw + x / 2];
    const ub = blur(up, gw, gh);
    let e = 0;
    for (let i = 0; i < gw * gh; i++) {
      const d = g[i] - 4 * ub[i];
      e += d * d;
    }
    energy.push((e * 4 ** level) / (W * H));
    g = next;
    gw = nw;
    gh = nh;
  }
  while (energy.length < 6) energy.push(0);
  const total = energy.reduce((a, b) => a + b, 0) || 1;
  const s = (i: number, j: number) => (100 * (energy[i] + energy[j])) / total;
  return [s(0, 1), s(2, 3), s(4, 5)];
}

/// What each number is called, how it prints, and the tolerance its class's range is widened
/// by either side (docs/fire.md: ranges are min to max widened, not the median).
export const METRICS: { key: keyof Metrics; called: string; digits: number; tolerance: number }[] = [
  { key: "white", called: "white %", digits: 0, tolerance: 2 },
  { key: "pale", called: "pale %", digits: 0, tolerance: 2 },
  { key: "yellow", called: "yellow %", digits: 0, tolerance: 2 },
  { key: "orange", called: "orange %", digits: 0, tolerance: 3 },
  { key: "red", called: "red %", digits: 0, tolerance: 3 },
  { key: "redOrange", called: "red+orange %", digits: 0, tolerance: 2 },
  { key: "BR", called: "B/R", digits: 2, tolerance: 0.03 },
  { key: "GR", called: "G/R", digits: 2, tolerance: 0.03 },
  { key: "dim", called: "dim %", digits: 0, tolerance: 3 },
  { key: "soot", called: "soot/flame", digits: 2, tolerance: 0.02 },
  { key: "edgeSoot", called: "edge soot %", digits: 0, tolerance: 2 },
  { key: "pockets", called: "pockets", digits: 1, tolerance: 1 },
  { key: "fine", called: "fine %", digits: 0, tolerance: 2 },
  { key: "mid", called: "mid %", digits: 0, tolerance: 2 },
  { key: "coarse", called: "coarse %", digits: 0, tolerance: 3 },
];

/// A class's band of one number: its photos' median, least and most.
export type Band = { median: number; min: number; max: number };
export type References = {
  about: string;
  classes: Record<string, { photos: number; light: Record<string, number>; bands: Record<string, Band> }>;
  photos: { file: string; class: string; light: string; metrics: Metrics }[];
};

export const REFERENCES_FILE = resolve(import.meta.dirname, "references.json");
export function references(): References {
  return JSON.parse(readFileSync(REFERENCES_FILE, "utf8")) as References;
}

/// One number held to its class: within the range widened by the tolerance.
export type Held = { called: string; value: number; low: number; high: number; holds: boolean; digits: number };
export function judge(m: Metrics, bands: Record<string, Band>): Held[] {
  return METRICS.filter(({ key }) => bands[key]).map(({ key, called, digits, tolerance }) => {
    const band = bands[key];
    const low = band.min - tolerance;
    const high = band.max + tolerance;
    const value = m[key];
    return { called, value, low, high, holds: value >= low && value <= high, digits };
  });
}

/// One line of numbers, ✗ on each that falls out.
export function said(held: Held[]): string {
  return held
    .map((h) => `${h.holds ? "" : "✗ "}${h.called} ${h.value.toFixed(h.digits)} (${h.low.toFixed(h.digits)}-${h.high.toFixed(h.digits)})`)
    .join(", ");
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const flag = (name: string) => {
    const i = argv.indexOf(name);
    if (i < 0) return undefined;
    const v = argv[i + 1];
    argv.splice(i, 2);
    return v;
  };
  const cls = flag("--class");
  const json = argv.includes("--json");
  const [file, crop] = argv.filter((a) => a !== "--json");
  if (!file) {
    console.error("usage: node tools/fire/measure.ts <png|jpg> [WxH+X+Y] [--class <class>] [--json]");
    process.exit(2);
  }
  const m = measure(load(file), crop ? parseBox(crop) : undefined);
  if (json) console.log(JSON.stringify(m));
  else if (!cls) console.log(METRICS.map(({ key, called, digits }) => `${called} ${m[key].toFixed(digits)}`).join(", "));
  if (cls) {
    const known = references().classes[cls];
    if (!known) {
      console.error(`no class ${cls}; the classes are ${Object.keys(references().classes).join(", ")}`);
      process.exit(2);
    }
    const held = judge(m, known.bands);
    const out = held.filter((h) => !h.holds).length;
    console.log(`${out ? "✗" : "✓"} ${file.split("/").pop()} as ${cls}: ${said(held)}`);
    process.exit(out ? 1 : 0);
  }
}
