#!/usr/bin/env node
// The foliage check (docs/art-style.md, "Leaves read as leaves"; D368): every leaf, needle,
// frond and flower card the world's plants are drawn from, measured from its files, the atlas,
// its normal map and the card's line in `assets/data/<atlas>.ron`, and held to the numbers that
// tell the kinds that read as leaves (the oaks after D291 and D292, the Scots pine's tufts,
// D318) from the ones the developers called flat (D291). CPU only, the same answer every run.
//
//   node tools/textures/foliage.ts [--card <name>] [--draw <dir>]
//
// For each card: its distinct leaves (or needles, fronds), found as regions of the card whose
// normal, or for a card with a flat normal map whose colour, runs on smoothly, a leaf ending
// where the next one's tilt or shade begins; how long a leaf is in texels, and how many
// texels it has for each pixel it covers on screen at 2 m at 1080p; how much of the card the
// normal map tilts and how far each leaf's own tilt differs from the next; whether a
// thickness map says where light passes through; how much the brightness varies within a leaf
// and in its finest detail, and from leaf to leaf in hue and lightness; and how much of it is
// a living leaf's colour. `kinds` in foliage.json says which plants wear each card, how big it
// is grown near the camera, and what it is (a spray of leaves, needles, one leaf or frond, or
// flowers); `known` lists the failures already known, each with why, so the check passes until
// it is fixed and fails once it is, to be taken off. `--draw` writes each card's leaves in
// false colour, to see what was counted. Exits 1 on a failure not known, or a known one fixed.
// Node 22.18 or later; no dependencies.

import { existsSync, mkdirSync, readFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { type Picture, readPng, writePng } from "../sandbox/png.ts";

const ROOT = resolve(import.meta.dirname, "../..");
export const FILE = resolve(import.meta.dirname, "foliage.json");

/// What a card is: a spray of leaves along a twig, a tuft or spray of needles, one leaf or
/// frond (a rosette's), or flowers on their stems.
export type Form = "spray" | "needles" | "single" | "flowers";

/// A card as its atlas's `.ron` has it.
export type Card = { name: string; cell: [number, number]; leaf?: number };
export type Atlas = { file: string; atlas: string; normals: string; cut: number; across: number; cards: Card[] };

/// One card's measured numbers.
export type Measured = {
  /// The share of its cell it covers past the cut.
  cover: number;
  /// Its distinct leaves, needles or fronds, green and big enough to be one.
  leaves: number;
  /// The median length of a leaf, in texels (by area: what most of the card is made of).
  leafTexels: number;
  /// The texels a leaf has for each pixel it covers on screen at 2 m (1080p, 55 degrees
  /// high): under 1 it is magnified, soft.
  sharpness: number;
  /// How long a near leaf is on screen at 2 m, in pixels.
  onScreen: number;
  /// The share of the card whose normal map tilts it more than 8 degrees off the card's face.
  relief: number;
  /// How far a leaf's own tilt is from the card's mean, in degrees, the mean over its leaves.
  tilts: number;
  /// Whether a thickness map gives where light passes through it (1) or not (0).
  thickness: number;
  /// The spread of brightness within a leaf over its brightness, the mean over its leaves by
  /// area: veins, blemishes, its own shading.
  spread: number;
  /// The finest detail: the mean difference of each texel's brightness from its 3 by 3
  /// neighbours', over the mean brightness, inside the card.
  detail: number;
  /// From leaf to leaf: the spread of their mean hues, in degrees, and of their mean
  /// brightness over its mean.
  leafHue: number;
  leafLight: number;
  /// The share of its green that is a living leaf's colour (`living`), and its green's mean
  /// brightness and saturation (sRGB luma; HSV).
  gamut: number;
  luma: number;
  saturation: number;
};

/// How each number is held: at least `over`, at most `under`; `forms` limits a limit to those
/// forms of card. Each sits between the cards that read as leaves (the lush oaks' `oakbranch`
/// after D291 and D292, the Scots pine's `scots` tufts after D318) and a made-up card of the
/// fault it guards against (foliage.test.ts): the oak branch's 93 texels a leaf and 0.102
/// spread in a leaf, the composed cards' 0.019 to 0.035 fine detail, their 77 to 100% living
/// green against the pale mint cards before D292 (a leaf's mean luma 0.43 to 0.45, half its
/// green past 0.45).
export type Limit = { over?: number; under?: number; forms?: Form[]; called: string; why: string };

export const LIMITS: Partial<Record<keyof Measured, Limit>> = {
  leaves: { over: 8, forms: ["spray", "needles"], called: "leaves a card", why: "a card is a spray of many leaves or needles, not a picture of a few" },
  leafTexels: { over: 90, forms: ["spray", "single"], called: "texels a leaf", why: "a leaf is drawn with some 100 texels along it or more (D292), so its veins and margin hold near" },
  sharpness: { over: 1, called: "texels a pixel at 2 m", why: "the card has a texel for every pixel it covers 2 m off; under 1 it is magnified, soft" },
  relief: { over: 0.3, forms: ["spray", "needles", "single"], called: "relief", why: "its normal map tilts its leaves, veins and needles; a flat one lights the card as one plane (D269)" },
  tilts: { over: 12, forms: ["spray"], called: "tilt leaf to leaf", why: "each leaf its own tilt, so the light catches them one by one (D291)" },
  thickness: { over: 0.5, called: "thickness map", why: "light passes through a leaf's thin blade and not its veins or twig, as SpeedTree's and Unreal's foliage have it (D291: lit through from behind)" },
  spread: { over: 0.09, called: "spread in a leaf", why: "veins, blemishes and its own shading, not one flat fill (D291)" },
  detail: { over: 0.015, called: "fine detail", why: "the scan's grain, not a smooth fill" },
  leafLight: { over: 0.08, forms: ["spray"], called: "light leaf to leaf", why: "young leaves lighter, old ones darker, not one tone (D269)" },
  leafHue: { over: 2.5, forms: ["spray"], called: "hue leaf to leaf", why: "real leaves' varied greens (D292)" },
  gamut: { over: 0.75, forms: ["spray", "needles", "single"], called: "living green", why: "a living leaf's green: not pastel mint, not neon, not grey (D292)" },
};

/// A living leaf's colour in sRGB (D292): hue 50 to 160 degrees, saturated 0.25 to 0.92 in HSV
/// and no brighter than 0.45 in luma, as the scanned summer leaves are; the composed cards' pale
/// mint before D292 (luma 0.43 to 0.45, saturation 0.44 in HSL) was past it.
export function living(r: number, g: number, b: number): boolean {
  const [h, s] = hsv(r, g, b);
  const l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  return h >= 50 && h <= 160 && s >= 0.25 && s <= 0.92 && l <= 0.45;
}

/// Green enough to be leaf, for the gamut and for which regions are leaves: hue 40 to 170 and
/// saturated past 0.12, so twigs, berries and petals are left out.
export function green(r: number, g: number, b: number): boolean {
  const [h, s] = hsv(r, g, b);
  return h >= 40 && h <= 170 && s > 0.12;
}

function hsv(r: number, g: number, b: number): [number, number, number] {
  const high = Math.max(r, g, b), d = high - Math.min(r, g, b);
  if (d === 0) return [0, 0, high];
  const h = high === r ? 60 * (((g - b) / d + 6) % 6) : high === g ? 60 * ((b - r) / d + 2) : 60 * ((r - g) / d + 4);
  return [h, d / high, high];
}

/// A cell of an atlas: its colour as linear-free sRGB values 0 to 1 and alpha, its normal
/// (unit, OpenGL's convention), `size` a side.
export type Cell = { size: number; rgb: Float32Array; alpha: Float32Array; normal: Float32Array };

/// The cell at `cell` (its top-left corner, 0 to 1) `across` of the atlas wide.
export function cellOf(atlas: Picture, normals: Picture | undefined, cell: [number, number], across: number): Cell {
  const size = Math.round(atlas.width * across);
  const [x0, y0] = [Math.round(cell[0] * atlas.width), Math.round(cell[1] * atlas.height)];
  const rgb = new Float32Array(size * size * 3), alpha = new Float32Array(size * size), normal = new Float32Array(size * size * 3);
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = (y * size + x), a = ((y0 + y) * atlas.width + x0 + x) * 4;
      rgb[i * 3] = atlas.rgba[a] / 255;
      rgb[i * 3 + 1] = atlas.rgba[a + 1] / 255;
      rgb[i * 3 + 2] = atlas.rgba[a + 2] / 255;
      alpha[i] = atlas.rgba[a + 3] / 255;
      let n = [0, 0, 1];
      if (normals) {
        const j = ((y0 + y) * normals.width + x0 + x) * 4;
        n = [normals.rgba[j] / 127.5 - 1, normals.rgba[j + 1] / 127.5 - 1, normals.rgba[j + 2] / 127.5 - 1];
        const l = Math.hypot(n[0], n[1], n[2]) || 1;
        n = n.map((v) => v / l);
      }
      normal.set(n, i * 3);
    }
  }
  return { size, rgb, alpha, normal };
}

const lumaAt = (c: Cell, i: number) => 0.2126 * c.rgb[i * 3] + 0.7152 * c.rgb[i * 3 + 1] + 0.0722 * c.rgb[i * 3 + 2];

/// The card's leaves: each covered texel labelled by the region it belongs to (-1 for none).
/// On a card with a normal map, a region runs on while the normal, smoothed 3 by 3, turns by
/// under `NORMAL_STEP` from texel to texel: overlapping leaves part where one's tilt breaks
/// against the other's. On a picture, whose normal map is flat, a texel is an edge where its
/// brightness changes across five texels by more than `SHADE_EDGE` (in natural log units) or
/// its colour by more than `COLOUR_EDGE` of the sum, as the shaded margins between a picture's
/// leaves do, or on the card's outline, and the rest run on as regions, edges left out. A leaf tilted as its neighbour
/// may be counted with it, so the count is nearly what a person would see apart, at least.
export const NORMAL_STEP = 0.07;
export const SHADE_EDGE = 0.22;
export const COLOUR_EDGE = 0.05;
export function regions(c: Cell, cut: number, byNormal: boolean): Int32Array {
  const { size } = c;
  const n = size * size;
  const covered = (i: number) => c.alpha[i] >= cut;
  const parent = new Int32Array(n).map((_, i) => i);
  const find = (i: number): number => {
    while (parent[i] !== i) {
      parent[i] = parent[parent[i]];
      i = parent[i];
    }
    return i;
  };
  let same: (i: number, j: number) => boolean;
  let kept: (i: number) => boolean = covered;
  if (byNormal) {
    const f = new Float32Array(n * 3);
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const i = y * size + x;
        if (!covered(i)) continue;
        let m = 0;
        const s = [0, 0, 0];
        for (let dy = -1; dy <= 1; dy++) {
          for (let dx = -1; dx <= 1; dx++) {
            const [xx, yy] = [x + dx, y + dy];
            if (xx < 0 || yy < 0 || xx >= size || yy >= size || !covered(yy * size + xx)) continue;
            for (let q = 0; q < 3; q++) s[q] += c.normal[(yy * size + xx) * 3 + q];
            m++;
          }
        }
        for (let q = 0; q < 3; q++) f[i * 3 + q] = s[q] / m;
      }
    }
    same = (i, j) => Math.hypot(f[i * 3] - f[j * 3], f[i * 3 + 1] - f[j * 3 + 1], f[i * 3 + 2] - f[j * 3 + 2]) < NORMAL_STEP;
  } else {
    // Brightness opened, its minimum then its maximum over 5 by 5, so pale veins and glints
    // narrower than that are taken out and do not split a leaf; the shaded margins stay.
    const log = opened(c, covered);
    const red = new Float32Array(n), blue = new Float32Array(n);
    for (let i = 0; i < n; i++) {
      const sum = c.rgb[i * 3] + c.rgb[i * 3 + 1] + c.rgb[i * 3 + 2] + 1e-3;
      red[i] = c.rgb[i * 3] / sum;
      blue[i] = c.rgb[i * 3 + 2] / sum;
    }
    const edge = new Uint8Array(n);
    for (let y = 2; y < size - 2; y++) {
      for (let x = 2; x < size - 2; x++) {
        const i = y * size + x;
        if (!covered(i)) continue;
        // The card's outline is an edge too, so leaves do not join round their rims.
        if (!covered(i - 1) || !covered(i + 1) || !covered(i - size) || !covered(i + size)) {
          edge[i] = 1;
          continue;
        }
        for (const d of [2, 2 * size, 2 * size + 2, 2 * size - 2]) {
          const [a, b] = [i - d, i + d];
          if (!covered(a) || !covered(b)) continue;
          if (Math.abs(log[a] - log[b]) > SHADE_EDGE || Math.abs(red[a] - red[b]) > COLOUR_EDGE || Math.abs(blue[a] - blue[b]) > COLOUR_EDGE) {
            edge[i] = 1;
            break;
          }
        }
      }
    }
    kept = (i) => covered(i) && !edge[i];
    same = () => true;
  }
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = y * size + x;
      if (!kept(i)) continue;
      for (const j of [x + 1 < size ? i + 1 : -1, y + 1 < size ? i + size : -1]) {
        if (j >= 0 && kept(j) && same(i, j)) {
          const [a, b] = [find(i), find(j)];
          if (a !== b) parent[Math.max(a, b)] = Math.min(a, b);
        }
      }
    }
  }
  const label = new Int32Array(n).fill(-1);
  for (let i = 0; i < n; i++) if (kept(i)) label[i] = find(i);
  return label;
}

/// One region's numbers: its texels, length (four times the root of its pixels' largest
/// variance, an ellipse's long axis), mean colour, brightness and its spread, and mean normal.
type Region = { pixels: number; length: number; rgb: [number, number, number]; luma: number; spread: number; normal: [number, number, number] };

function summarise(c: Cell, label: Int32Array): Region[] {
  const { size } = c;
  const sums = new Map<number, number[]>();
  for (let y = 0; y < size; y++) {
    for (let x = 0; x < size; x++) {
      const i = y * size + x, l = label[i];
      if (l < 0) continue;
      let s = sums.get(l);
      if (!s) sums.set(l, (s = new Array(14).fill(0)));
      const v = lumaAt(c, i);
      s[0]++;
      s[1] += x;
      s[2] += y;
      s[3] += x * x;
      s[4] += y * y;
      s[5] += x * y;
      s[6] += c.rgb[i * 3];
      s[7] += c.rgb[i * 3 + 1];
      s[8] += c.rgb[i * 3 + 2];
      s[9] += v;
      s[10] += v * v;
      s[11] += c.normal[i * 3];
      s[12] += c.normal[i * 3 + 1];
      s[13] += c.normal[i * 3 + 2];
    }
  }
  return [...sums.values()].map((s) => {
    const n = s[0];
    const [mx, my] = [s[1] / n, s[2] / n];
    const [xx, yy, xy] = [s[3] / n - mx * mx, s[4] / n - my * my, s[5] / n - mx * my];
    const largest = (xx + yy) / 2 + Math.sqrt(((xx - yy) / 2) ** 2 + xy * xy);
    const luma = s[9] / n;
    const nl = Math.hypot(s[11], s[12], s[13]) || 1;
    return {
      pixels: n,
      length: 4 * Math.sqrt(Math.max(0, largest)),
      rgb: [s[6] / n, s[7] / n, s[8] / n],
      luma,
      spread: Math.sqrt(Math.max(0, s[10] / n - luma * luma)) / Math.max(luma, 0.02),
      normal: [s[11] / nl, s[12] / nl, s[13] / nl],
    };
  });
}

/// How big a region must be to count as a leaf: 240 texels of a 1024-texel cell, scaled with
/// the cell's area, a needle of 6 by 40 or a leaf of 17 across; the specks veins and glints
/// break off are smaller; and at least two texels wide on average, so a sliver of shaded margin
/// is not one.
export const LEAST = 240;

/// A card's numbers. `metres` is how wide it is grown near the camera; `thick` whether a
/// thickness map is beside its atlas; `share`, for a composed card, how long its leaves are as
/// a share of it, as `textures.ts cards` measured them laying each leaf (`leaf` in its `.ron`),
/// which is used for its leaves' length over the regions', which overlapping leaves shorten;
/// `labels`, if given, is filled with its leaves' regions.
export function measure(c: Cell, cut: number, metres: number, thick: boolean, share?: number, labels?: (l: Int32Array, leaves: Set<number>) => void): Measured {
  const { size } = c;
  const n = size * size;
  let cover = 0, tilted = 0, faceUp = 0;
  for (let i = 0; i < n; i++) {
    if (c.alpha[i] < cut) continue;
    cover++;
    if (c.normal[i * 3 + 2] < Math.cos((8 * Math.PI) / 180)) tilted++;
    if (Math.abs(c.normal[i * 3 + 2] - 1) < 1e-3) faceUp++;
  }
  // A flat normal map: nine in ten covered texels face straight out of the card.
  const byNormal = cover > 0 && faceUp / cover < 0.9;
  const label = regions(c, cut, byNormal);
  const least = LEAST * (size / 1024) ** 2;
  const all = summarise(c, label);
  const leaves = all.filter((r) => r.pixels >= least && r.pixels >= 2 * r.length && green(...r.rgb));
  if (labels) {
    const kept = new Set<number>();
    const big = new Map<number, number>();
    for (let i = 0; i < n; i++) if (label[i] >= 0) big.set(label[i], (big.get(label[i]) ?? 0) + 1);
    for (let i = 0; i < n; i++) {
      const l = label[i];
      if (l >= 0 && (big.get(l) ?? 0) >= least) kept.add(l);
    }
    labels(label, kept);
  }
  // The median leaf length by area, the spread within a leaf by area.
  const byLength = [...leaves].sort((a, b) => a.length - b.length);
  const area = leaves.reduce((s, r) => s + r.pixels, 0);
  let half = 0, leafTexels = 0;
  for (const r of byLength) {
    half += r.pixels;
    if (half >= area / 2) {
      leafTexels = r.length;
      break;
    }
  }
  const spread = area ? leaves.reduce((s, r) => s + r.spread * r.pixels, 0) / area : 0;
  // Each leaf's tilt from the card's mean normal, over its leaves.
  const mean = [0, 0, 0];
  for (const r of leaves) for (let k = 0; k < 3; k++) mean[k] += r.normal[k] * r.pixels;
  const ml = Math.hypot(mean[0], mean[1], mean[2]) || 1;
  const tilts = leaves.length
    ? leaves.reduce((s, r) => s + (Math.acos(Math.min(1, (r.normal[0] * mean[0] + r.normal[1] * mean[1] + r.normal[2] * mean[2]) / ml)) * 180) / Math.PI, 0) / leaves.length
    : 0;
  // Leaf to leaf, over the leaves big enough to have a colour of their own.
  const hues = leaves.map((r) => hsv(...r.rgb)[0]);
  const lights = leaves.map((r) => r.luma);
  const sd = (v: number[]) => {
    if (v.length < 2) return 0;
    const m = v.reduce((s, x) => s + x, 0) / v.length;
    return Math.sqrt(v.reduce((s, x) => s + (x - m) ** 2, 0) / v.length);
  };
  const lightMean = lights.reduce((s, x) => s + x, 0) / Math.max(1, lights.length);
  // Fine detail inside the card, and its green's gamut.
  let fine = 0, inside = 0, all3 = 0, greens = 0, alive = 0, gl = 0, gs = 0;
  for (let y = 1; y < size - 1; y++) {
    for (let x = 1; x < size - 1; x++) {
      const i = y * size + x;
      if (c.alpha[i] < cut) continue;
      const [r, g, b] = [c.rgb[i * 3], c.rgb[i * 3 + 1], c.rgb[i * 3 + 2]];
      if (green(r, g, b)) {
        greens++;
        if (living(r, g, b)) alive++;
        gl += 0.2126 * r + 0.7152 * g + 0.0722 * b;
        gs += hsv(r, g, b)[1];
      }
      let s = 0, whole = true;
      for (let dy = -1; dy <= 1 && whole; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          const j = i + dy * size + dx;
          if (c.alpha[j] < cut) {
            whole = false;
            break;
          }
          s += lumaAt(c, j);
        }
      }
      if (!whole) continue;
      const v = lumaAt(c, i);
      fine += Math.abs(v - s / 9);
      all3 += v;
      inside++;
    }
  }
  if (share) leafTexels = share * size;
  const leafM = (leafTexels / size) * metres;
  const onScreen = leafM * PIXELS_A_METRE_AT_2M;
  return {
    cover: cover / n,
    leaves: leaves.length,
    leafTexels,
    sharpness: onScreen > 0 ? leafTexels / onScreen : 0,
    onScreen,
    relief: cover ? tilted / cover : 0,
    tilts,
    thickness: thick ? 1 : 0,
    spread,
    detail: inside && all3 ? fine / all3 : 0,
    leafHue: sd(hues),
    leafLight: lightMean > 0 ? sd(lights) / lightMean : 0,
    gamut: greens ? alive / greens : 0,
    luma: greens ? gl / greens : 0,
    saturation: greens ? gs / greens : 0,
  };
}

/// Pixels a metre covers on a 1080-pixel-high screen 2 m off, at the game's field of view, 55
/// degrees high (`terra_walker::controls::Settings`).
export const PIXELS_A_METRE_AT_2M = 540 / (2 * Math.tan((55 * Math.PI) / 360));

/// An atlas's `.ron` (`textures.ts cards` writes them): its files, cut, cell width and cards.
export function atlasOf(file: string): Atlas {
  const text = readFileSync(resolve(ROOT, file), "utf8");
  const field = (name: string) => text.match(new RegExp(`${name}: "([^"]+)"`))?.[1];
  const number = (name: string) => Number(text.match(new RegExp(`\\b${name}: ([0-9.]+)`))?.[1]);
  const cards: Card[] = [];
  for (const m of text.matchAll(/"([a-z0-9_]+)": \(\s*cell: \(([0-9.]+), ([0-9.]+)\)([^)]*\)[^]*?)\n {8}\),/g)) {
    const leaf = m[4].match(/leaf: ([0-9.]+)/)?.[1];
    cards.push({ name: m[1], cell: [Number(m[2]), Number(m[3])], leaf: leaf ? Number(leaf) : undefined });
  }
  return { file, atlas: field("atlas")!, normals: field("normals")!, cut: number("cut"), across: number("across"), cards };
}

/// A kind of card: the atlas it is in, what it is, the plants that wear it, and how wide it is
/// grown near the camera, in metres, and where that comes from.
export type Kind = { atlas: string; form: Form; grows: string; metres: number; from: string };
export type Kinds = { about: string; atlases: Record<string, string>; kinds: Record<string, Kind>; known: Record<string, Record<string, string>> };

/// Each card's numbers judged: the failures, and of them those known.
export type Judged = { card: string; kind: Kind; m: Measured; fails: string[]; known: string[]; fixed: string[] };

export function judge(card: string, kind: Kind, m: Measured, known: Record<string, string> = {}, every: Record<string, string> = {}): Judged {
  const fails: string[] = [];
  for (const [k, l] of Object.entries(LIMITS) as [keyof Measured, Limit][]) {
    if (l.forms && !l.forms.includes(kind.form)) continue;
    const v = m[k];
    if ((l.over !== undefined && !(v >= l.over)) || (l.under !== undefined && !(v <= l.under))) fails.push(k);
  }
  const listed = { ...every, ...known };
  return {
    card,
    kind,
    m,
    fails,
    known: fails.filter((f) => f in listed),
    fixed: Object.keys(known).filter((f) => !fails.includes(f)),
  };
}

/// The table: a line a card, each judged number, a star by a failure (`*` known, `!` not).
export function table(all: Judged[]): string {
  const cols: [keyof Measured, (v: number) => string][] = [
    ["leaves", (v) => v.toFixed(0)],
    ["leafTexels", (v) => v.toFixed(0)],
    ["sharpness", (v) => v.toFixed(2)],
    ["relief", (v) => `${(100 * v).toFixed(0)}%`],
    ["tilts", (v) => `${v.toFixed(0)}°`],
    ["thickness", (v) => (v ? "yes" : "no")],
    ["spread", (v) => v.toFixed(3)],
    ["detail", (v) => v.toFixed(3)],
    ["leafHue", (v) => `${v.toFixed(1)}°`],
    ["leafLight", (v) => v.toFixed(3)],
    ["gamut", (v) => `${(100 * v).toFixed(0)}%`],
  ];
  const head = ["card", "form", ...cols.map(([k]) => LIMITS[k]!.called), "verdict"];
  const rows = all.map((j) => [
    j.card,
    j.kind.form,
    ...cols.map(([k, f]) => {
      const l = LIMITS[k]!;
      const judged = !l.forms || l.forms.includes(j.kind.form);
      const mark = j.fails.includes(k) ? (j.known.includes(k) ? "*" : "!") : "";
      return judged ? `${f(j.m[k])}${mark}` : `(${f(j.m[k])})`;
    }),
    j.fails.length === j.known.length && !j.fixed.length ? (j.fails.length ? "known" : "ok") : "FAIL",
  ]);
  const limits = ["", "", ...cols.map(([k]) => {
    const l = LIMITS[k]!;
    return l.over !== undefined ? `>= ${k === "thickness" ? "yes" : l.over}` : `<= ${l.under}`;
  }), ""];
  const widths = head.map((h, k) => Math.max(h.length, limits[k].length, ...rows.map((r) => r[k].length)));
  const pad = (r: string[]) => `  ${r.map((c, k) => c.padEnd(widths[k])).join("  ")}`.trimEnd();
  return [pad(head), pad(limits), ...rows.map(pad)].join("\n");
}

export function kinds(): Kinds {
  return JSON.parse(readFileSync(FILE, "utf8")) as Kinds;
}

if (import.meta.main) {
  const arg = (name: string) => {
    const at = process.argv.indexOf(name);
    return at > 0 ? process.argv[at + 1] : undefined;
  };
  const only = arg("--card");
  const draw = arg("--draw");
  const k = kinds();
  const results: Judged[] = [];
  const seen = new Set<string>();
  for (const [atlasName, file] of Object.entries(k.atlases)) {
    const a = atlasOf(file);
    const picture = readPng(resolve(ROOT, "assets", a.atlas));
    const normals = existsSync(resolve(ROOT, "assets", a.normals)) ? readPng(resolve(ROOT, "assets", a.normals)) : undefined;
    const thick = existsSync(resolve(ROOT, "assets", dirname(a.atlas), "thickness.png"));
    for (const card of a.cards) {
      const kind = k.kinds[card.name];
      if (!kind || kind.atlas !== atlasName) continue;
      seen.add(card.name);
      if (only && card.name !== only) continue;
      const cell = cellOf(picture, normals, card.cell, a.across);
      const m = measure(cell, a.cut, kind.metres, thick, card.leaf, draw ? (label, kept) => drawn(draw, card.name, cell, label, kept) : undefined);
      results.push(judge(card.name, kind, m, k.known[card.name], k.known["*"]));
    }
  }
  const missing = Object.keys(k.kinds).filter((n) => !seen.has(n));
  console.log(`foliage: every card the plants wear, measured from its atlas and normal map (${FILE.replace(`${ROOT}/`, "")}); ! fails, * fails and is known, (n) not judged for its form`);
  console.log(table(results));
  let failed = missing.length > 0;
  for (const n of missing) console.log(`foliage: ${n} is in foliage.json but in none of its atlases`);
  for (const j of results) {
    const unknown = j.fails.filter((f) => !j.known.includes(f));
    for (const f of unknown) console.log(`foliage: FAIL ${j.card} ${LIMITS[f as keyof Measured]!.called} ${j.m[f as keyof Measured].toFixed(3)}: ${LIMITS[f as keyof Measured]!.why}`);
    for (const f of j.fixed) console.log(`foliage: FAIL ${j.card} ${LIMITS[f as keyof Measured]?.called ?? f} now holds: take it off foliage.json's known`);
    if (unknown.length || j.fixed.length) failed = true;
  }
  // A failure known of every card, once no card fails it.
  for (const f of Object.keys(k.known["*"] ?? {})) {
    if (!only && !results.some((j) => j.fails.includes(f))) {
      console.log(`foliage: FAIL every card's ${LIMITS[f as keyof Measured]?.called ?? f} now holds: take it off foliage.json's known`);
      failed = true;
    }
  }
  const clean = results.filter((j) => !j.fails.length).length;
  const known = results.filter((j) => j.fails.length && j.fails.length === j.known.length).length;
  console.log(`foliage: ${clean} of ${results.length} cards hold every number, ${known} only with known failures (foliage.json's known), ${results.length - clean - known} fail${failed ? "; FAILED" : ""}`);
  process.exit(failed ? 1 : 0);
}

/// A card's leaves in false colour, each its own, uncounted ones grey, into `dir`.
function drawn(dir: string, name: string, c: Cell, label: Int32Array, kept: Set<number>) {
  mkdirSync(dir, { recursive: true });
  const { size } = c;
  const rgba = new Uint8Array(size * size * 4);
  for (let i = 0; i < size * size; i++) {
    const l = label[i];
    if (l < 0) {
      rgba.set([0, 0, 0, 255], i * 4);
      continue;
    }
    if (!kept.has(l)) {
      rgba.set([90, 90, 90, 255], i * 4);
      continue;
    }
    const h = Math.imul(l + 1, 2654435761) >>> 0;
    rgba.set([64 + (h & 191), 64 + ((h >> 8) & 191), 64 + ((h >> 16) & 191), 255], i * 4);
  }
  writePng(resolve(dir, `${name}.png`), { width: size, height: size, rgba });
}

/// The log of a cell's brightness, opened: its minimum over 5 by 5 texels, then the maximum of
/// that, over the covered texels, so bright lines and specks under 5 texels wide are gone.
function opened(c: Cell, covered: (i: number) => boolean): Float32Array {
  const { size } = c;
  const n = size * size;
  let v = new Float32Array(n);
  for (let i = 0; i < n; i++) v[i] = Math.log(0.02 + lumaAt(c, i));
  const pass = (from: Float32Array, across: boolean, pick: (a: number, b: number) => number) => {
    const out = new Float32Array(from);
    for (let y = 0; y < size; y++) {
      for (let x = 0; x < size; x++) {
        const i = y * size + x;
        if (!covered(i)) continue;
        let m = from[i];
        for (let k = -2; k <= 2; k++) {
          const [xx, yy] = across ? [x + k, y] : [x, y + k];
          if (xx < 0 || yy < 0 || xx >= size || yy >= size) continue;
          const j = yy * size + xx;
          if (covered(j)) m = pick(m, from[j]);
        }
        out[i] = m;
      }
    }
    return out;
  };
  v = pass(pass(v, true, Math.min), false, Math.min);
  return pass(pass(v, true, Math.max), false, Math.max);
}
