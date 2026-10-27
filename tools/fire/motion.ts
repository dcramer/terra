#!/usr/bin/env node
// How a fire moves in a clip, for holding our films to films of real fire by numbers (the stills
// measure.ts holds say nothing of motion):
//
//   node tools/fire/motion.ts <video | 'dir/f-*[0-9].png'> [--box WxH+X+Y] [--matte <video | glob>]
//        [--from S] [--seconds S] [--fps N] [--cut L] [--overlay out.png] [--json]
//
// The flame is measure.ts's flame colour (`isFlame`) brighter than `--cut`, and where the run's
// flame matte says flame (`--matte`, D433). The cut defaults to LUMA_CUT of the clip's
// 99th-percentile flame luma for footage, so the ground the fire lights is not flame; a render
// with a matte is cut at LUMA_CUT of white, where a filmed flame, clipped white inside, is cut. Specks under MIN_PIECE px are dropped (sparks). The clip is decoded twice: once small
// to find the flame, then cropped round it and scaled so the flame is NORM_H px tall, so a far
// fire and a close one compare. Heights are from the flame's base (its pooled 99th-percentile
// row) in flame heights H (the 98th percentile of its pixels' heights over the clip). Per clip:
//
// - flicker: the spectrum of the flame's area over time (Welch, SEGMENT s windows): the dominant
//   and median frequency in 0.5-8 Hz, and how much the area and the height pump (sd / mean);
// - tongues: the 8-connected pieces of flame above TONGUE_FROM H (and above 3/4 H), per frame;
//   and, tracked by tongues.ts (peaks by prominence, forecast and matched as storm cells are),
//   how long each lives, how often one sheds a blob that rises away or forks in two (per tongue
//   a second), how far it leans from vertical, and how fast tracked pieces rise;
// - rise: by time of flight, as flame flicker is measured in the lab: the lag at which the
//   flicker of the flame's area low in it (LAG_BANDS) best correlates with the area high in it,
//   the bands' separation over that lag in H a second (a lag near 0: it pumps in place). Block
//   matching of luma was tried first and failed on footage: a filmed flame is clipped white
//   inside and its outline changes shape faster than it moves, so matches scatter either way;
// - roughness: the outer outline's length (pixel edges, holes filled) over the square root of
//   the area, per height band and whole (a square is 4, a disc 4.5 at this count);
// - colour: pale (with white), yellow, orange and red shares per height band (measure.ts's bands),
//   and how much the pale share changes over time;
// - gate: those of them a film of ours is held to, against the real clips' ranges (GATE).
//
// --overlay writes the middle frame with the flame tinted and the base, thirds and tongue level
// drawn, to check the flame was found. Node 22.18 or later and ffmpeg; no dependencies.

import { execFileSync } from "node:child_process";
import { basename } from "node:path";
import { type Box, colourBand, isFlame, luma, parseBox } from "./measure.ts";
import { writePng } from "../sandbox/png.ts";
import { type TongueTracks, trackTongues } from "./tongues.ts";

/// A clip at the size it is measured at: RGB frames, three bytes a pixel, and the flame matte's
/// share (a byte a pixel) where the renderer gave one.
export type Clip = { w: number; h: number; fps: number; frames: Uint8Array[]; mattes?: Uint8Array[] };

/// The flame's height at analysis, px: tall enough for its tongues and edge to show, small
/// enough to match blocks over a clip in seconds.
export const NORM_H = 160;
/// The flame cut, of footage's 99th-percentile flame luma (of white for a render): lit ground is
/// far dimmer than flame.
export const LUMA_CUT = 0.5;
/// The least piece of flame kept, px at analysis: smaller is a spark or noise.
export const MIN_PIECE = 6;
/// Where tongues are counted from, of H: above the fire's body.
export const TONGUE_FROM = 0.5;
/// A tongue's least area, of H²: 8 x 8 px at NORM_H; smaller pieces are shards that flicker in
/// and out from frame to frame.
export const TONGUE_AREA = 0.0025;
/// The rise's two height bands, of H (the lower in the fire's body, the upper in its tongues),
/// and the longest lag looked for, s.
export const LAG_BANDS: [[number, number], [number, number]] = [
  [0.4, 0.55],
  [0.75, 0.9],
];
export const MAX_LAG = 0.6;
/// The flicker spectrum's window, s: Welch's, so a longer film averages more of them. Film ours 20 s an
/// hour and pass `--seconds 20`: a 6 s film gives too few tongues for their rates (fire round g20).
export const SEGMENT = 6;

/// The bars a film of ours is held to, from the real clips (research/2026-10-25-fire-motion.md):
/// each measure must lie in [lo, hi]. Flicker is the median frequency, as the real clips' range
/// was found (1.83-3.83 Hz, fire round g12), not the dominant one: a baked flame's area has a
/// narrow swell at 0.5 Hz beside broad puffing at 2.4 Hz, and which of them peaks flips from
/// film to film of one flame while the median holds (fire round g43).
export const GATE: { name: string; of: (m: Motion) => number; lo: number; hi: number }[] = [
  { name: "flick", of: (m) => m.flicker.median, lo: 1.83, hi: 3.83 },
  { name: "area", of: (m) => m.flicker.areaCv, lo: 0, hi: 0.19 },
  { name: "life", of: (m) => m.tongues.lifetime, lo: 0.04, hi: 0.12 },
  { name: "splits", of: (m) => m.tongues.splits, lo: 0, hi: 1.2 },
  { name: "lean", of: (m) => m.tongues.lean, lo: 7.6, hi: 24.5 },
  { name: "topred", of: (m) => m.colour.red[2], lo: 0, hi: 47 },
];

/// A film against GATE: each bar's value, to the two decimals the bars and the report give, and
/// whether it meets it.
/// Several films of one build (its fire at several placements, each a seed's offset, mirror,
/// turn and scale) are held by the median of each measure: placement alone moved the night
/// campfire's lean between 6.8 and 11.6 deg (fire round g52), so one film is one draw of it.
export const gate = (...ms: Motion[]) =>
  GATE.map((g) => {
    const value = Math.round(100 * median(ms.map(g.of))) / 100;
    return { name: g.name, value, met: value >= g.lo && value <= g.hi };
  });

export type Geometry = { base: number; height: number; x0: number; x1: number };
export type Bands = [number, number, number];
export type Colours = { pale: Bands; yellow: Bands; orange: Bands; red: Bands; paleSd: number };
export type Motion = {
  frames: number;
  fps: number;
  height: number;
  flicker: { dominant: number; median: number; areaCv: number; heightCv: number };
  tongues: { count: number; countHigh: number } & TongueTracks;
  rise: { lag: number; r: number; r0: number; speed: number };
  roughness: { whole: number; bands: Bands };
  colour: Colours;
};

const quantile = (sorted: ArrayLike<number>, q: number) => (sorted.length ? sorted[Math.min(sorted.length - 1, Math.max(0, Math.floor(q * sorted.length)))] : NaN);
const median = (xs: number[]) => quantile([...xs].sort((a, b) => a - b), 0.5);
const mean = (xs: number[]) => (xs.length ? xs.reduce((a, b) => a + b, 0) / xs.length : NaN);
const sd = (xs: number[]) => {
  const m = mean(xs);
  return Math.sqrt(mean(xs.map((x) => (x - m) ** 2)));
};

/// 8-connected pieces of a mask among its rows above `below` (all rows without): a label a pixel
/// (0 none) and each label's area.
export function pieces(mask: Uint8Array, w: number, h: number, below = h): { label: Int32Array; area: number[] } {
  const label = new Int32Array(w * h);
  const area = [0];
  const stack: number[] = [];
  for (let p0 = 0; p0 < w * below; p0++) {
    if (!mask[p0] || label[p0]) continue;
    const id = area.length;
    area.push(0);
    label[p0] = id;
    stack.push(p0);
    while (stack.length) {
      const p = stack.pop() as number;
      area[id]++;
      const x = p % w;
      const y = (p / w) | 0;
      for (let yy = Math.max(0, y - 1); yy <= Math.min(below - 1, y + 1); yy++)
        for (let xx = Math.max(0, x - 1); xx <= Math.min(w - 1, x + 1); xx++) {
          const q = yy * w + xx;
          if (mask[q] && !label[q]) {
            label[q] = id;
            stack.push(q);
          }
        }
    }
  }
  return { label, area };
}

/// A mask without its pieces under MIN_PIECE px.
function clean(mask: Uint8Array, w: number, h: number): Uint8Array {
  const { label, area } = pieces(mask, w, h);
  for (let p = 0; p < w * h; p++) if (label[p] && area[label[p]] < MIN_PIECE) mask[p] = 0;
  return mask;
}

/// The least matte share (of 255) where a render's pixel may be flame: any flame drawn there.
/// The matte only takes out what is not flame (lit ground, stones); what is flame is then cut
/// at half white as clipped footage is (`lumaCut`), so a render and a film are measured alike.
export const MATTE_SEEN = 16;
const seen = (clip: Clip, i: number, p: number) => !clip.mattes || clip.mattes[i][p] >= MATTE_SEEN;

/// The flame cut: LUMA_CUT of the 99th percentile of the flame-coloured pixels' luma for
/// footage, whose core clips white so that is about half of white; for a render with a matte,
/// LUMA_CUT of white itself. A render's core need not clip, and cut from its own 99th
/// percentile, a paler core raised the cut and dropped the dim tips, so the night tongues'
/// lean read the core's brightness, not their motion (fire round g43: two builds differing
/// only in the core read 7.7 and 6.4 deg at their own cuts, 7.8 and 7.7 at one cut).
export function lumaCut(clip: Clip): number {
  if (clip.mattes) return LUMA_CUT * 255;
  const ls: number[] = [];
  clip.frames.forEach((f, i) => {
    for (let p = 0; p < clip.w * clip.h; p += 3) if (isFlame(f[3 * p], f[3 * p + 1], f[3 * p + 2]) && seen(clip, i, p)) ls.push(luma(f[3 * p], f[3 * p + 1], f[3 * p + 2]));
  });
  return LUMA_CUT * quantile(ls.sort((a, b) => a - b), 0.99);
}

/// Each frame's flame: flame-coloured, at least `cut` in luma, and in the matte where there is one.
export function flameMasks(clip: Clip, cut: number): Uint8Array[] {
  return clip.frames.map((f, i) => {
    const m = new Uint8Array(clip.w * clip.h);
    for (let p = 0; p < m.length; p++) {
      const [r, g, b] = [f[3 * p], f[3 * p + 1], f[3 * p + 2]];
      if (isFlame(r, g, b) && luma(r, g, b) >= cut && seen(clip, i, p)) m[p] = 1;
    }
    return clean(m, clip.w, clip.h);
  });
}

/// What is flame-coloured but does not move: a pixel flame in at least `share` of the clip's
/// frames whose luma's sd over the clip is at most `sd` (of 255). A log's glowing crack seen
/// through the flame's thin foot, or a filmed fire's coal bed, is flame by colour in every frame
/// and hardly changes; flame flickers. As fire detection in video keeps only fire-coloured pixels
/// that move (Töreyin, Dedeoğlu, Güdükbay and Çetin, "Computer vision based method for real-time
/// fire and flame detection", Pattern Recognition Letters 27, 2006), it is left out of the
/// flame's base (`geometry`); it stays in the masks, so the flame's area and colour keep it.
/// Counting the glow lowered the base and raised H, so the tongue level fell into the upright
/// body: the night campfire's lean read 6.8 deg with its logs glowing and 10.2 with them dark
/// (fire round g52).
export type Still = { share: number; sd: number };
export const STILL: Still = { share: 0.95, sd: 8 };
export function still(clip: Clip, masks: Uint8Array[], s: Still = STILL): Uint8Array {
  const n = clip.frames.length;
  const out = new Uint8Array(clip.w * clip.h);
  for (let p = 0; p < out.length; p++) {
    let on = 0;
    let sum = 0;
    let sq = 0;
    for (let i = 0; i < n; i++) {
      on += masks[i][p];
      const f = clip.frames[i];
      const l = luma(f[3 * p], f[3 * p + 1], f[3 * p + 2]);
      sum += l;
      sq += l * l;
    }
    const m = sum / n;
    if (on >= s.share * n && Math.sqrt(Math.max(0, sq / n - m * m)) <= s.sd) out[p] = 1;
  }
  return out;
}

/// The flame's base row, its height H and its columns, pooled over the clip; the base from the
/// pixels that move only (`still`, when given).
export function geometry(masks: Uint8Array[], w: number, h: number, still?: Uint8Array): Geometry {
  const rows = new Float64Array(h);
  const moving = new Float64Array(h);
  const cols = new Float64Array(w);
  for (const m of masks)
    for (let p = 0; p < w * h; p++)
      if (m[p]) {
        rows[(p / w) | 0]++;
        if (!still?.[p]) moving[(p / w) | 0]++;
        cols[p % w]++;
      }
  const at = (hist: Float64Array, q: number) => {
    const total = hist.reduce((a, b) => a + b, 0);
    let acc = 0;
    for (let i = 0; i < hist.length; i++) if ((acc += hist[i]) >= q * total) return i;
    return hist.length - 1;
  };
  const base = at(moving.some((v) => v > 0) ? moving : rows, 0.99);
  const heights = new Float64Array(base + 1);
  for (let y = 0; y <= base; y++) heights[base - y] = rows[y];
  return { base, height: Math.max(1, at(heights, 0.98)), x0: at(cols, 0.005), x1: at(cols, 0.995) };
}

/// A row's height band (0 low, 1 mid, 2 top thirds of H; past H is top).
const bandOf = (y: number, g: Geometry) => Math.min(2, Math.max(0, Math.floor((3 * (g.base - y)) / g.height)));

/// The power spectrum of a series (Welch: Hann windows of `seconds`, half overlapping, the
/// series' mean and slope taken out of each).
export function spectrum(series: number[], fps: number, seconds = SEGMENT): { hz: number[]; power: number[] } {
  const n = Math.min(series.length, Math.round(seconds * fps));
  const power = new Array(Math.floor(n / 2) + 1).fill(0);
  let windows = 0;
  for (let s = 0; s + n <= series.length; s += Math.max(1, Math.floor(n / 2))) {
    const seg = series.slice(s, s + n);
    const tm = (n - 1) / 2;
    const m = mean(seg);
    let num = 0;
    let den = 0;
    seg.forEach((v, i) => {
      num += (i - tm) * (v - m);
      den += (i - tm) ** 2;
    });
    const slope = den ? num / den : 0;
    const x = seg.map((v, i) => (v - m - slope * (i - tm)) * (0.5 - 0.5 * Math.cos((2 * Math.PI * i) / (n - 1))));
    for (let k = 0; k < power.length; k++) {
      let re = 0;
      let im = 0;
      for (let i = 0; i < n; i++) {
        re += x[i] * Math.cos((2 * Math.PI * k * i) / n);
        im -= x[i] * Math.sin((2 * Math.PI * k * i) / n);
      }
      power[k] += re * re + im * im;
    }
    windows++;
  }
  return { hz: power.map((_, k) => (k * fps) / n), power: power.map((p) => p / Math.max(1, windows)) };
}

/// The flicker of a series: its dominant frequency (the spectrum's peak in 0.5-8 Hz, refined
/// between bins), the frequency halving its power there, and its sd over its mean.
export function flicker(series: number[], fps: number): { dominant: number; median: number; cv: number } {
  const { hz, power } = spectrum(series, fps);
  const keep = hz.map((f, k) => k).filter((k) => hz[k] >= 0.5 && hz[k] <= 8);
  let peak = keep[0];
  for (const k of keep) if (power[k] > power[peak]) peak = k;
  let dominant = hz[peak];
  if (peak > 0 && peak < power.length - 1) {
    const [a, b, c] = [power[peak - 1], power[peak], power[peak + 1]];
    const d = a - 2 * b + c;
    if (d < 0) dominant += ((0.5 * (a - c)) / d) * (hz[1] - hz[0]);
  }
  const total = keep.reduce((s, k) => s + power[k], 0);
  let acc = 0;
  let mid = hz[keep[keep.length - 1]];
  for (const k of keep)
    if ((acc += power[k]) >= total / 2) {
      mid = hz[k];
      break;
    }
  return { dominant, median: mid, cv: sd(series) / mean(series) };
}

/// Tongues: pieces above TONGUE_FROM H of at least TONGUE_AREA H² (the mean count a frame, and
/// of those above 3/4 H), and tongues.ts's tracks of them: lifetime, shedding, splits, lean.
export function tongues(masks: Uint8Array[], w: number, h: number, g: Geometry, fps: number): Motion["tongues"] {
  const level = (of: number) => Math.max(0, Math.min(h, Math.round(g.base - of * g.height)));
  const least = Math.max(MIN_PIECE, Math.round(TONGUE_AREA * g.height * g.height));
  const above = (m: Uint8Array, of: number) => pieces(m, w, h, level(of)).area.filter((a, i) => i > 0 && a >= least).length;
  return { count: mean(masks.map((m) => above(m, TONGUE_FROM))), countHigh: mean(masks.map((m) => above(m, 0.75))), ...trackTongues(masks, w, h, g, fps, TONGUE_FROM, TONGUE_AREA) };
}

/// The flame's area in a height band (of H) a frame, its slow part (a moving mean over a second)
/// taken out, so what is left is the flicker.
function bandSeries(masks: Uint8Array[], w: number, g: Geometry, [lo, hi]: [number, number], fps: number): number[] {
  const [y0, y1] = [Math.round(g.base - hi * g.height), Math.round(g.base - lo * g.height)];
  const raw = masks.map((m) => count(m, w, y0, y1));
  const half = Math.round(fps / 2);
  return raw.map((v, i) => v - mean(raw.slice(Math.max(0, i - half), i + half + 1)));
}

/// Pearson's correlation of `a` with `b` seen `lag` frames later.
function correlation(a: number[], b: number[], lag: number): number {
  const n = a.length - lag;
  if (n < 10) return NaN;
  const x = a.slice(0, n);
  const y = b.slice(lag, lag + n);
  const [mx, my] = [mean(x), mean(y)];
  let [sxy, sxx, syy] = [0, 0, 0];
  for (let i = 0; i < n; i++) {
    sxy += (x[i] - mx) * (y[i] - my);
    sxx += (x[i] - mx) ** 2;
    syy += (y[i] - my) ** 2;
  }
  return sxx && syy ? sxy / Math.sqrt(sxx * syy) : NaN;
}

/// The rise by time of flight: how long the flicker of the band LAG_BANDS[0] takes to show in
/// LAG_BANDS[1] (the lag, up to MAX_LAG s, of their best correlation, refined between frames);
/// the bands' separation over it is the speed features rise at, in H a second. A lag near 0 is
/// a flame that pumps in place, every height at once.
export function rise(masks: Uint8Array[], w: number, g: Geometry, fps: number): Motion["rise"] {
  const a = bandSeries(masks, w, g, LAG_BANDS[0], fps);
  const b = bandSeries(masks, w, g, LAG_BANDS[1], fps);
  const lags = Array.from({ length: Math.round(MAX_LAG * fps) + 1 }, (_, k) => correlation(a, b, k));
  let best = 0;
  lags.forEach((r, k) => {
    if (r > lags[best]) best = k;
  });
  let lag = best;
  if (best > 0 && best < lags.length - 1) {
    const [p, q, s] = [lags[best - 1], lags[best], lags[best + 1]];
    const d = p - 2 * q + s;
    if (d < 0) lag += (0.5 * (p - s)) / d;
  }
  const apart = (LAG_BANDS[1][0] + LAG_BANDS[1][1] - LAG_BANDS[0][0] - LAG_BANDS[0][1]) / 2;
  const seconds = Math.max(lag, 0.5) / fps;
  return { lag: lag / fps, r: lags[best], r0: lags[0], speed: apart / seconds };
}

/// A mask's outline (pixel edges to no flame, the frame's edge counting as none; where flame
/// goes on past the rows' cut there is no edge) over the square root of its area, among rows y0
/// to y1 (exclusive).
export function roughness(mask: Uint8Array, w: number, h: number, y0 = 0, y1 = h): number {
  let edge = 0;
  let area = 0;
  for (let y = Math.max(0, y0); y < Math.min(h, y1); y++)
    for (let x = 0; x < w; x++) {
      if (!mask[y * w + x]) continue;
      area++;
      if (x === 0 || !mask[y * w + x - 1]) edge++;
      if (x === w - 1 || !mask[y * w + x + 1]) edge++;
      if (y === 0 || !mask[(y - 1) * w + x]) edge++;
      if (y === h - 1 || !mask[(y + 1) * w + x]) edge++;
    }
  return area ? edge / Math.sqrt(area) : NaN;
}

/// Roughness whole and per height band, the mean over frames with enough flame in the band.
function roughnessOf(masks: Uint8Array[], w: number, h: number, g: Geometry): Motion["roughness"] {
  const cut = (k: number) => Math.round(g.base + 1 - (k * g.height) / 3);
  const rows: [number, number][] = [
    [cut(1), g.base + 1],
    [cut(2), cut(1)],
    [0, cut(2)],
  ];
  const solid = masks.map((m) => filled(m, w, h));
  const per = rows.map(([y0, y1]) => solid.map((m) => roughness(m, w, h, y0, y1)).filter((v, i) => Number.isFinite(v) && count(solid[i], w, y0, y1) >= 30));
  return { whole: mean(solid.map((m) => roughness(m, w, h))), bands: per.map(mean) as Bands };
}

/// A mask with its holes filled (what no flame-free path joins to the frame's edge), so its
/// outline is the outer one, not the logs and dark pockets inside it.
export function filled(mask: Uint8Array, w: number, h: number): Uint8Array {
  const out = new Uint8Array(w * h).fill(1);
  const stack: number[] = [];
  for (let x = 0; x < w; x++) stack.push(x, (h - 1) * w + x);
  for (let y = 0; y < h; y++) stack.push(y * w, y * w + w - 1);
  while (stack.length) {
    const p = stack.pop() as number;
    if (mask[p] || !out[p]) continue;
    out[p] = 0;
    const x = p % w;
    if (x > 0) stack.push(p - 1);
    if (x < w - 1) stack.push(p + 1);
    if (p >= w) stack.push(p - w);
    if (p < w * (h - 1)) stack.push(p + w);
  }
  return out;
}
const count = (m: Uint8Array, w: number, y0: number, y1: number) => {
  let n = 0;
  for (let p = Math.max(0, y0) * w; p < y1 * w && p < m.length; p++) n += m[p];
  return n;
};

/// Colour shares per height band, the mean over frames, and the pale share's sd over frames.
export function colours(clip: Clip, masks: Uint8Array[], g: Geometry): Colours {
  const share = { pale: [[], [], []], yellow: [[], [], []], orange: [[], [], []], red: [[], [], []] } as Record<"pale" | "yellow" | "orange" | "red", number[][]>;
  const paleOver: number[] = [];
  clip.frames.forEach((f, t) => {
    const n = [0, 0, 0];
    const c = { pale: [0, 0, 0], yellow: [0, 0, 0], orange: [0, 0, 0], red: [0, 0, 0] };
    for (let p = 0; p < clip.w * clip.h; p++) {
      if (!masks[t][p]) continue;
      const k = bandOf((p / clip.w) | 0, g);
      const band = colourBand(f[3 * p], f[3 * p + 1], f[3 * p + 2]);
      c[band === "white" ? "pale" : band][k]++;
      n[k]++;
    }
    for (const key of Object.keys(c) as (keyof typeof c)[]) for (let k = 0; k < 3; k++) if (n[k] >= 30) share[key][k].push((100 * c[key][k]) / n[k]);
    const all = n[0] + n[1] + n[2];
    if (all) paleOver.push((100 * (c.pale[0] + c.pale[1] + c.pale[2])) / all);
  });
  const of = (key: keyof typeof share) => share[key].map(mean) as Bands;
  return { pale: of("pale"), yellow: of("yellow"), orange: of("orange"), red: of("red"), paleSd: sd(paleOver) };
}

/// Every measure of a clip at its analysis size.
export function analyse(clip: Clip, cut = lumaCut(clip), s: Still = STILL): Motion {
  const { w, h, fps } = clip;
  const masks = flameMasks(clip, cut);
  const g = geometry(masks, w, h, still(clip, masks, s));
  const area = masks.map((m) => m.reduce((a, b) => a + b, 0));
  const tall = masks.map((m) => {
    for (let p = 0; p < m.length; p++) if (m[p]) return g.base - ((p / w) | 0);
    return 0;
  });
  const f = flicker(area, fps);
  return {
    frames: clip.frames.length,
    fps,
    height: g.height,
    flicker: { dominant: f.dominant, median: f.median, areaCv: f.cv, heightCv: sd(tall) / mean(tall) },
    tongues: tongues(masks, w, h, g, fps),
    rise: rise(masks, w, g, fps),
    roughness: roughnessOf(masks, w, h, g),
    colour: colours(clip, masks, g),
  };
}

// The command: decoding with ffmpeg.

type Source = { input: string; fps: number; from: number; seconds: number };
const even = (v: number) => Math.max(2, 2 * Math.round(v / 2));
const inputArgs = (s: Source) =>
  s.input.includes("*") ? ["-framerate", String(s.fps), "-pattern_type", "glob", "-i", s.input] : ["-ss", String(s.from), "-t", String(s.seconds), "-i", s.input];

function probe(s: Source): { w: number; h: number; fps: number } {
  const glob = s.input.includes("*") ? ["-pattern_type", "glob"] : [];
  const out = execFileSync("ffprobe", ["-v", "error", ...glob, "-i", s.input, "-select_streams", "v:0", "-show_entries", "stream=width,height,r_frame_rate", "-of", "csv=p=0"], { encoding: "utf8" });
  const [w, h, rate] = out.trim().split("\n")[0].split(",");
  const [a, b] = rate.split("/").map(Number);
  return { w: +w, h: +h, fps: s.input.includes("*") ? s.fps : a / (b || 1) };
}

/// A source's frames cropped to `box` and scaled to `size`, RGB or grey.
function decode(s: Source, box: Box, size: [number, number], grey = false): Uint8Array[] {
  const vf = `crop=${box.w}:${box.h}:${box.x}:${box.y},scale=${size[0]}:${size[1]}:flags=area`;
  const raw = execFileSync("ffmpeg", ["-v", "error", ...inputArgs(s), "-vf", vf, "-f", "rawvideo", "-pix_fmt", grey ? "gray" : "rgb24", "-"], { maxBuffer: 2 ** 31 });
  const bytes = size[0] * size[1] * (grey ? 1 : 3);
  const frames: Uint8Array[] = [];
  for (let at = 0; at + bytes <= raw.length; at += bytes) frames.push(new Uint8Array(raw.buffer, raw.byteOffset + at, bytes));
  return frames;
}

/// A clip found small, then cropped round its flame and scaled so the flame is NORM_H px tall.
export function load(s: Source, matte?: Source, crop?: Box, cut?: number): { clip: Clip; cut: number; sourceHeight: number } {
  const p = probe(s);
  const box = crop ?? { w: p.w, h: p.h, x: 0, y: 0 };
  const k = Math.min(1, 360 / Math.max(box.w, box.h));
  const small: [number, number] = [even(box.w * k), even(box.h * k)];
  const first: Clip = { w: small[0], h: small[1], fps: p.fps, frames: decode(s, box, small), mattes: matte && decode(matte, box, small, true) };
  const level = cut ?? lumaCut(first);
  const firstMasks = flameMasks(first, level);
  const g = geometry(firstMasks, first.w, first.h, still(first, firstMasks));
  const [sx, sy] = [box.w / small[0], box.h / small[1]];
  const H = g.height * sy;
  const x0 = Math.max(box.x, Math.round(box.x + g.x0 * sx - 0.3 * H));
  const x1 = Math.min(box.x + box.w, Math.round(box.x + g.x1 * sx + 0.3 * H));
  const y0 = Math.max(box.y, Math.round(box.y + g.base * sy - 1.4 * H));
  const y1 = Math.min(box.y + box.h, Math.round(box.y + g.base * sy + 0.1 * H));
  const fbox = { x: x0, y: y0, w: x1 - x0, h: y1 - y0 };
  const scale = NORM_H / H;
  const size: [number, number] = [even(fbox.w * scale), even(fbox.h * scale)];
  const clip: Clip = { w: size[0], h: size[1], fps: p.fps, frames: decode(s, fbox, size), mattes: matte && decode(matte, fbox, size, true) };
  if (clip.mattes && clip.mattes.length !== clip.frames.length) throw new Error(`${clip.frames.length} frames but ${clip.mattes.length} mattes`);
  return { clip, cut: level, sourceHeight: H };
}

/// The middle frame with the flame tinted and the base, thirds and tongue level drawn.
function overlay(path: string, clip: Clip, cut: number) {
  const masks = flameMasks(clip, cut);
  const g = geometry(masks, clip.w, clip.h, still(clip, masks));
  const t = clip.frames.length >> 1;
  const rgba = new Uint8Array(clip.w * clip.h * 4);
  const lines = new Map([g.base, g.base - g.height / 3, g.base - (2 * g.height) / 3, g.base - g.height].map((y) => [Math.round(y), [255, 255, 255]]));
  lines.set(Math.round(g.base - TONGUE_FROM * g.height), [0, 255, 0]);
  for (let p = 0; p < clip.w * clip.h; p++) {
    const f = clip.frames[t];
    const on = masks[t][p];
    const line = lines.get((p / clip.w) | 0);
    const px = line && p % 4 < 2 ? line : on ? [f[3 * p] >> 1, (f[3 * p + 1] >> 1) + 60, (f[3 * p + 2] >> 1) + 110] : [f[3 * p], f[3 * p + 1], f[3 * p + 2]];
    rgba.set([...px, 255], 4 * p);
  }
  writePng(path, { width: clip.w, height: clip.h, rgba });
}

const f2 = (v: number, d = 2) => (Number.isFinite(v) ? v.toFixed(d) : "-");
export function said(name: string, m: Motion, how: string): string {
  const b = (x: Bands, d = 2) => x.map((v) => f2(v, d)).join(" / ");
  return [
    `${name}: ${m.frames} frames at ${f2(m.fps, 1)} fps; ${how}`,
    `  flicker    dominant ${f2(m.flicker.dominant)} Hz, median ${f2(m.flicker.median)} Hz; area sd/mean ${f2(m.flicker.areaCv)}, height sd/mean ${f2(m.flicker.heightCv)}`,
    `  tongues    ${f2(m.tongues.count, 1)} above ${TONGUE_FROM} H, ${f2(m.tongues.countHigh, 1)} above 0.75 H; ${f2(m.tongues.births, 1)} born a second; tracked pieces' rise median ${f2(m.tongues.rise)} H/s, p90 ${f2(m.tongues.rise90)}`,
    `  tracked    lifetime mean ${f2(m.tongues.lifetime)} s, p90 ${f2(m.tongues.lifetime90)} (${m.tongues.tracks} tracks); per tongue a second: sheds ${f2(m.tongues.sheds)}, splits ${f2(m.tongues.splits)}, merges ${f2(m.tongues.merges)}; lean median ${f2(m.tongues.lean, 1)} deg, p90 ${f2(m.tongues.lean90, 1)}; shed blobs last ${f2(m.tongues.shedLife)} s, rise ${f2(m.tongues.shedRise)} H/s`,
    `  rise       ${f2(m.rise.speed)} H/s by time of flight: the flicker at ${LAG_BANDS[0].join("-")} H shows at ${LAG_BANDS[1].join("-")} H ${f2(m.rise.lag * 1000, 0)} ms later (r ${f2(m.rise.r)}; r at no lag ${f2(m.rise.r0)})`,
    `  roughness  whole ${f2(m.roughness.whole, 1)}; low / mid / top ${b(m.roughness.bands, 1)}`,
    `  colour %   pale ${b(m.colour.pale, 0)}, yellow ${b(m.colour.yellow, 0)}, orange ${b(m.colour.orange, 0)}, red ${b(m.colour.red, 0)} (low / mid / top); pale sd over time ${f2(m.colour.paleSd, 1)}`,
    `  gate       met ${gate(m).filter((g) => g.met).length} of ${GATE.length}: ${gate(m).map((g) => `${g.name} ${f2(g.value)}${g.met ? "" : "x"}`).join(" ")}`,
  ].join("\n");
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const opt = (k: string) => {
    const i = argv.indexOf(k);
    return i >= 0 ? argv[i + 1] : undefined;
  };
  const first = argv.findIndex((a) => a.startsWith("--"));
  const inputs = argv.slice(0, first < 0 ? argv.length : first);
  const mattes = argv.flatMap((a, i) => (a === "--matte" && i + 1 < argv.length ? [argv[i + 1]] : []));
  if (!inputs.length || (mattes.length && mattes.length !== inputs.length)) {
    console.error("usage: node tools/fire/motion.ts <video | 'dir/f-*[0-9].png'>... [--matte <video|glob>]... (a matte per clip) [--box WxH+X+Y] [--from S] [--seconds S] [--fps N] [--cut L] [--overlay out.png] [--json]");
    console.error("  several clips: each is measured, then the gate holds the median of each measure (one build's fire at several placements)");
    process.exit(2);
  }
  const src = (path: string): Source => ({ input: path, fps: +(opt("--fps") ?? 30), from: +(opt("--from") ?? 0), seconds: +(opt("--seconds") ?? 10) });
  const box = opt("--box");
  const given = opt("--cut");
  const ms = inputs.map((input, i) => {
    const { clip, cut, sourceHeight } = load(src(input), mattes[i] ? src(mattes[i]) : undefined, box ? parseBox(box) : undefined, given === undefined ? undefined : +given);
    const ov = opt("--overlay");
    if (ov && i === 0) overlay(ov, clip, cut);
    const m = analyse(clip, cut);
    const how = `flame luma >= ${cut.toFixed(0)}${clip.mattes ? " in the matte" : ""}, ${sourceHeight.toFixed(0)} px tall in the source, measured at ${m.height} px`;
    console.log(argv.includes("--json") ? JSON.stringify({ name: basename(input), how, ...m }, null, 1) : said(basename(input), m, how));
    return m;
  });
  if (ms.length > 1 && !argv.includes("--json")) {
    const g = gate(...ms);
    console.log(`median of ${ms.length}: gate met ${g.filter((x) => x.met).length} of ${GATE.length}: ${g.map((x) => `${x.name} ${f2(x.value)}${x.met ? "" : "x"}`).join(" ")}`);
  }
}
