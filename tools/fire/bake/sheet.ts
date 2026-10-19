// A set's contact sheet and its numbers against its class (docs/fire.md, "The automatic check";
// D415), before any game integration: eight frames down, and across, each raw channel (heat,
// cover, soot, distortion, motion as colour) and the heat-to-ramp preview over black and over a
// dusk-grey sky. The preview is drawn as the format says the engine will, premultiplied:
//
//   rgb = ramp(heat).rgb * cover * exposure + soot colour * soot,  a = cover * ramp(heat).a + soot
//
// over the sky, then through a per-channel Hable curve (Godot's Filmic, the stage 1 bake-off's
// predicted winner) and, for the second number, KhronosPbrNeutral (the camera's today). The
// exposure is a preset's brightness, not the flipbook's, so each curve's is fitted to bring the
// bands nearest the class's medians; what is held is whether the flipbook's heat, cover and soot
// can reach the class at all. Each of eight frames is measured by tools/fire/measure.ts over the
// background most of the class's photographs have (night or sky).

import { execFileSync } from "node:child_process";
import { join } from "node:path";
import { writePng, type Picture } from "../../sandbox/png.ts";
import { judge, measure, references, type Band, type Metrics } from "../measure.ts";
import { table } from "../ramp.ts";
import type { Recipe } from "./bake.ts";
import type { Frame, Packed } from "./pack.ts";

const RAMP = table();
const SOOT_RGB = [0.012, 0.011, 0.01];
/// The sky the preview is measured over: dark enough that a flame stands out, light enough that
/// soot reads as dark (measure.ts's soot is under 60 of 255).
const SKY_SRGB = [100, 106, 120];

const hable = (x: number) => {
  const [A, B, C, D, E, F] = [0.15, 0.5, 0.1, 0.2, 0.02, 0.3];
  return (x * (A * x + C * B) + D * E) / (x * (A * x + B) + D * F) - E / F;
};
const HABLE_W = hable(11.2);
export const TONEMAPS: Record<string, (c: number[]) => number[]> = {
  hable: (c) => c.map((v) => hable(2 * v) / HABLE_W),
  neutral: (c) => {
    const start = 0.8 - 0.04;
    const x = Math.min(...c);
    const offset = x < 0.08 ? x - 6.25 * x * x : 0.04;
    let col = c.map((v) => v - offset);
    const peak = Math.max(...col);
    if (peak < start) return col;
    const d = 1 - start;
    const np = 1 - (d * d) / (peak + d - start);
    col = col.map((v) => (v * np) / peak);
    const g = 1 - 1 / (0.15 * (peak - np) + 1);
    return col.map((v) => v * (1 - g) + np * g);
  },
};
const srgb = (v: number) => {
  v = Math.min(1, Math.max(0, v));
  return Math.round((v <= 0.0031308 ? 12.92 * v : 1.055 * v ** (1 / 2.4) - 0.055) * 255);
};
const linear = (s: number) => ((s /= 255) <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4);

/// The linear sky that a curve draws as SKY_SRGB, by bisection.
function sky(curve: (c: number[]) => number[]): number[] {
  return SKY_SRGB.map((s) => {
    const want = linear(s);
    let [lo, hi] = [0, 4];
    for (let i = 0; i < 40; i++) {
      const mid = (lo + hi) / 2;
      if (curve([mid, mid, mid])[0] < want) lo = mid;
      else hi = mid;
    }
    return (lo + hi) / 2;
  });
}

/// A frame drawn as the engine will, over a linear background, through a curve.
export function preview(fr: Frame, n: number, exposure: number, curve: (c: number[]) => number[], bg: number[]): Picture {
  const rgba = new Uint8Array(n * n * 4);
  for (let p = 0; p < n * n; p++) {
    const r = Math.min(255, Math.round(fr.heat[p] * 255)) * 4;
    const cover = fr.cover[p];
    const soot = fr.soot[p];
    const a = Math.min(1, cover * RAMP[r + 3] + soot);
    const lin = [0, 1, 2].map((k) => RAMP[r + k] * cover * exposure + SOOT_RGB[k] * soot + (1 - a) * bg[k]);
    const out = curve(lin);
    for (let k = 0; k < 3; k++) rgba[p * 4 + k] = srgb(out[k]);
    rgba[p * 4 + 3] = 255;
  }
  return { width: n, height: n, rgba };
}

/// measure.ts on a preview, or nothing when under 400 pixels pass as flame (measure.ts's test):
/// it scales a crop so the flame is 256 pixels tall, which a speck would make enormous.
function measured(pic: Picture): Metrics | undefined {
  let flame = 0;
  for (let p = 0; p < pic.width * pic.height; p++) {
    const [r, g, b] = [pic.rgba[p * 4], pic.rgba[p * 4 + 1], pic.rgba[p * 4 + 2]];
    const [mx, mn] = [Math.max(r, g, b), Math.min(r, g, b)];
    if ((mx >= 110 && r === mx && mx - mn >= 20) || mn >= 235) flame++;
  }
  return flame >= 400 ? measure(pic) : undefined;
}

const BANDS: (keyof Metrics)[] = ["white", "pale", "yellow", "orange", "red", "redOrange", "BR", "GR", "dim"];

/// How far a frame's bands sit from the class's medians, each in its class's spans.
function distance(m: Metrics | undefined, bands: Record<string, Band>): number {
  if (!m) return 1e9;
  let d = 0;
  for (const k of BANDS) {
    const b = bands[k];
    if (b) d += ((m[k] - b.median) / Math.max(1e-3, b.max - b.min)) ** 2;
  }
  return d;
}

/// The exposure that brings the frames' bands nearest the class, by golden section on its log.
function fitExposure(frames: Frame[], n: number, curve: (c: number[]) => number[], bg: number[], bands: Record<string, Band>): number {
  const cost = (e: number) => frames.reduce((s, fr) => s + distance(measured(preview(fr, n, e, curve, bg)), bands), 0);
  let [lo, hi] = [Math.log(0.05), Math.log(200)];
  const g = (Math.sqrt(5) - 1) / 2;
  for (let i = 0; i < 24; i++) {
    const c = hi - g * (hi - lo);
    const d = lo + g * (hi - lo);
    if (cost(Math.exp(c)) < cost(Math.exp(d))) hi = d;
    else lo = c;
  }
  return Math.exp((lo + hi) / 2);
}

/// Draws the sheet and returns the line: per curve, its exposure and the metrics the median
/// frame holds, with what falls out.
export function sheet(r: Recipe, out: string, packed: Packed): string {
  const { frames, size: n } = packed;
  const cls = references().classes[r.class];
  if (!cls) throw new Error(`✗ ${r.name}: no reference class "${r.class}" in tools/fire/references.json`);
  const picks = Array.from({ length: Math.min(8, frames.length) }, (_, k) => frames[Math.floor((k * frames.length) / Math.min(8, frames.length))]);
  // A one-shot's late frames are smoke alone: only frames with flame in them are measured.
  const lit = picks.filter((fr) => fr.cover.reduce((c, v) => c + (v > 0.3 ? 1 : 0), 0) >= 200);
  if (!lit.length) throw new Error(`✗ ${r.name}: no frame has flame in it`);
  const said: string[] = [];
  const shown: Record<string, { e: number; bg: number[] }> = {};
  // Measured over the background most of the class's photographs have: the night's black for a
  // class shot mostly at night or dusk (whose dark round the flame measure.ts counts as soot and
  // pockets, as it does in the photographs), else the sky.
  const light = cls.light as Record<string, number>;
  const dark = (light.night ?? 0) + (light.dusk ?? 0) >= (light.day ?? 0);
  for (const [name, curve] of Object.entries(TONEMAPS)) {
    const bg = dark ? [0, 0, 0] : sky(curve);
    const e = fitExposure(lit, n, curve, bg, cls.bands);
    shown[name] = { e, bg };
    const held = lit.flatMap((fr) => {
      const m = measured(preview(fr, n, e, curve, bg));
      return m ? [judge(m, cls.bands)] : [];
    });
    if (!held.length) {
      said.push(`${dark ? "night" : "sky"} ${name}: no flame at any exposure`);
      continue;
    }
    const holds = held.map((h) => h.filter((x) => x.holds).length).sort((a, b) => a - b);
    const median = holds[Math.floor(holds.length / 2)];
    const outs = new Map<string, number>();
    for (const h of held) for (const x of h) if (!x.holds) outs.set(x.called, (outs.get(x.called) ?? 0) + 1);
    const worst = [...outs].filter(([, c]) => c > held.length / 2).map(([k]) => k);
    said.push(`${dark ? "night" : "sky"} ${name} x${e.toFixed(1)} holds ${median}/${held[0].length} over ${lit.length} frames${worst.length ? ` (✗ ${worst.join(", ")})` : ""}`);
  }
  // The sheet: a row a picked frame, a column a channel.
  const cols = 7;
  const gap = 4;
  const W = cols * (n + gap);
  const H = picks.length * (n + gap);
  const rgba = new Uint8Array(W * H * 4).fill(30);
  const put = (row: number, col: number, px: (p: number) => [number, number, number]) => {
    for (let y = 0; y < n; y++) {
      for (let x = 0; x < n; x++) {
        const o = ((row * (n + gap) + y) * W + col * (n + gap) + x) * 4;
        const [a, b, c] = px(y * n + x);
        rgba.set([a, b, c, 255], o);
      }
    }
  };
  const grey = (v: number): [number, number, number] => [Math.round(v * 255), Math.round(v * 255), Math.round(v * 255)];
  const ms = (packed.sidecar.motion as { scale: number }).scale;
  picks.forEach((fr, row) => {
    put(row, 0, (p) => grey(fr.heat[p] * Math.min(1, fr.cover[p] * 4)));
    put(row, 1, (p) => grey(fr.cover[p]));
    put(row, 2, (p) => grey(fr.soot[p]));
    put(row, 3, (p) => grey(fr.dist[p]));
    put(row, 4, (p) => {
      const w = Math.min(1, (fr.cover[p] + fr.soot[p]) * 3);
      return [0.5 + (fr.mx[p] / ms) * 0.5, 0.5 + (fr.my[p] / ms) * 0.5, 0.5].map((v) => Math.round(Math.min(1, Math.max(0, v)) * 255 * w)) as [number, number, number];
    });
    const { e } = shown.hable;
    const black = preview(fr, n, e, TONEMAPS.hable, [0, 0, 0]);
    const over = preview(fr, n, e, TONEMAPS.hable, sky(TONEMAPS.hable));
    put(row, 5, (p) => [black.rgba[p * 4], black.rgba[p * 4 + 1], black.rgba[p * 4 + 2]]);
    put(row, 6, (p) => [over.rgba[p * 4], over.rgba[p * 4 + 1], over.rgba[p * 4 + 2]]);
  });
  const png = join(out, "sheet.png");
  writePng(png, { width: W, height: H, rgba });
  const title = `${r.name} (${r.class}): heat · cover · soot · distortion · motion · ramp over black · over sky (Hable x${shown.hable.e.toFixed(1)})`;
  execFileSync("magick", [png, "-background", "#1e1e1e", "-fill", "#ddd", "-pointsize", n >= 256 ? "22" : "13", "-gravity", "north", "-splice", `0x${n >= 256 ? 34 : 22}`, "-annotate", "+0+6", title, "-quality", "88", join(out, "sheet.jpg")]);
  return said.join("; ");
}
