#!/usr/bin/env node
// The material bench (D386): how a construction material looks, shot the same way every time
// and measured in the numbers a look was judged by by hand (research/texture-detail-2026-10-15.md,
// research/colour-balance-2026-10-15.md), each against its pattern's targets, so a change that
// makes a material worse fails here rather than in a review.
//
//   cargo xtask look <material>... | --all [--out shots/look]
//
// For each material, the kit sheet lays its preview alone (`--sheet-rows "<name>: preview"`,
// tools/kits/preview.ts) and two scripted runs on the real GPU shoot fixed vantages of it: at
// noon, its wall (a roof's covering) square on from 1, 2 and 5 m, an outside corner (a roof's
// hip or valley), an opening (a roof's verge and eaves), its roof (a gable, or the whole) and
// the whole from 12 m; the same wall in the shade; and in the evening its sunlit face again.
// An 18% grey card stands beside each face measured, in the same light (`sandbox.card`).
// Then:
//
// - texel density: its surface's colour texels a metre at the high preset (from its data);
// - sharpness: the finest octave's share of contrast in the middle of the 1, 2 and 5 m shots;
// - colour balance: the grey card's red over blue in linear light, noon, shade and evening,
//   which at noon is the light's neutrality (D370), and the surface's brightness against the
//   card in the same light, lit and in shade;
// - value range: its surface's 5th to 95th percentile of brightness at 2 m at noon;
// - stretch: each texture set's share of area drawn past 1.5:1 and 2.5:1 (`cargo xtask texels`).
//
// Each number against its pattern's `look` targets (assets/data/kits/patterns/*.ron), passing
// or failing; one JPG under 2 MB a material beside its pack's reference photograph
// (`reference` in assets/data/packs/*.ron, fetched where it is missing), and a table, in
// shots/look/. `--all` is the building baseline's materials (D333). Exits 1 if any fails.
// Needs ImageMagick. Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, rmSync, statSync, writeFileSync } from "node:fs";
import { dirname, relative, resolve } from "node:path";
import { parseArgs } from "node:util";
import { buildOffscreen, imageMagick, offscreenPath, offscreenRun, ROOT } from "../sandbox/client.ts";
import { readPng, type Picture } from "../sandbox/png.ts";
import { readKits } from "../sandbox/qa/script.ts";
import { ORIGIN } from "./preview.ts";

type Vec3 = [number, number, number];

/** A view: what it shows, its eye and what it looks at in the preview's own metres (east, up
 * from the ground, south, as `--vantage-ground` takes them), and what is measured in it: the
 * material's `surface`, square on, at a `distance`, or a grey `card`. */
export type View = { name: string; eye: Vec3; at: Vec3; measure?: "surface" | "card"; distance?: number };
/** A light: the hour it is shot at, and its views; shade is shot at noon on the face away from
 * the sun. */
export type Light = { name: "noon" | "shade" | "evening"; hour: number; views: View[] };
/** A grey card stood beside a face, as `sandbox.card` takes it. */
export type Card = { at: Vec3; facing: Vec3 };

/** The distances the surface is measured from. */
export const DISTANCES = [1, 2, 5];
/** The middle of a shot measured, in pixels of the 1920x1080 frame: at 1 m it is 37 by 25 cm
 * of the face, at 5 m 1.85 by 1.23 m. */
export const CROP = { width: 384, height: 256 };
/** The middle of a card's shot measured: well inside the half-metre card a metre off. */
export const CARD_CROP = 120;
/** How far the 18% card's red over blue may be from one at noon (D370: a neutral grade, the
 * noon sun a little warm of D65; measured 1.04 when it was decided). */
export const NEUTRAL: [number, number] = [0.9, 1.15];

/** The views square on to a face, `normal` its outward way, at each of `DISTANCES`. */
function square(target: Vec3, normal: Vec3): View[] {
  const n = unit(normal);
  return DISTANCES.map((d) => ({
    name: `${d} m`,
    eye: [target[0] + n[0] * d, target[1] + n[1] * d, target[2] + n[2] * d] as Vec3,
    at: target,
    measure: "surface" as const,
    distance: d,
  }));
}

/** A card's view: a metre out from its face. */
function cardView(card: Card): View {
  const n = unit(card.facing);
  return { name: "18% grey card", eye: [card.at[0] + n[0], card.at[1] + n[1], card.at[2] + n[2]], at: card.at, measure: "card" };
}

const unit = (v: Vec3): Vec3 => {
  const l = Math.hypot(...v);
  return [v[0] / l, v[1] / l, v[2] / l];
};

// A wall material's preview (crates/terra_build/src/scenes/sheet.rs, `preview`): a house 5 m
// east by 4 m south, its walls 3 m, its front (south) with a window, the doorway, a carved hole
// and a damaged east end, a window in its west and north walls, a gable east and west; 8.5 m
// east of it a platform 4 m by 3 m, its front a clean face 1.5 m high from 2 m to 4 m along.
// On the yard the noon sun stands in the south-south-west and the evening's (18:00) low in the
// north-west, so the platform's front is measured lit at noon and the house's north wall in
// shade at noon and lit in the evening.
const WALL_NOON_CARD: Card = { at: [13.6, 0.75, 3.05], facing: [0, 0, 1] };
const WALL_NORTH_CARD: Card = { at: [-1.0, 1.5, -0.05], facing: [0, 0, -1] };
const WALL_GABLE: View = { name: "gable and eaves, 5 m", eye: [-5, 3.5, 2], at: [0, 3.5, 2] };
const WALL_LIGHTS: Light[] = [
  {
    name: "noon",
    hour: 12,
    views: [
      ...square([11.5, 0.75, 3.0], [0, 0, 1]),
      { name: "corner, 2 m", eye: [-1.41, 1.5, 5.41], at: [0, 1.5, 4] },
      { name: "openings, 2 m", eye: [1.75, 1.4, 6], at: [1.75, 1.3, 4] },
      WALL_GABLE,
      { name: "whole, 12 m", eye: [6.25, 5, 14], at: [6.25, 1.5, 2] },
      cardView(WALL_NOON_CARD),
    ],
  },
  { name: "shade", hour: 12, views: [...square([3.5, 1.5, 0], [0, 0, -1]), cardView(WALL_NORTH_CARD)] },
  {
    name: "evening",
    hour: 18,
    views: [
      ...square([3.5, 1.5, 0], [0, 0, -1]),
      { name: "corner, 2 m", eye: [-1.41, 1.5, -1.41], at: [0, 1.5, 0] },
      { name: "opening, 2 m", eye: [1.5, 1.5, -2], at: [1.5, 1.5, 0] },
      WALL_GABLE,
      { name: "whole, 12 m", eye: [-6, 5, -9], at: [3, 1.5, 2] },
      cardView(WALL_NORTH_CARD),
    ],
  },
];

// A roof covering's preview: an L of stone walls 3 m high, its main block 6 m east by 4 m
// south under a hip, its wing 4 m by 4 m south of the main block's west end under a gable,
// its ridge north to south, its south end not yet all laid, the main roof's east end damaged;
// its slopes at 45 degrees. The wing's west slope is measured lit at noon, the main roof's
// north slope in shade at noon and lit in the evening, each with a card at its pitch.
const S = Math.SQRT1_2;
const ROOF_WEST_CARD: Card = { at: [-1.5, 4, 5.75], facing: [-S, S, 0] };
const ROOF_NORTH_CARD: Card = { at: [-1.5, 4, 1], facing: [0, S, -S] };
const ROOF_LIGHTS: Light[] = [
  {
    name: "noon",
    hour: 12,
    views: [
      ...square([1, 4, 5.75], [-S, S, 0]),
      { name: "valley, 2 m", eye: [6.5, 3.6, 6.5], at: [4.5, 3.0, 4.5] },
      { name: "verge, ridge's end, 3 m", eye: [2, 4.4, 10.5], at: [2, 3.6, 7.5] },
      { name: "hip and eaves, 5 m", eye: [10, 5, -2], at: [5, 3.5, 2] },
      { name: "whole, 12 m", eye: [12, 7, 14], at: [3.5, 2.5, 4] },
      cardView(ROOF_WEST_CARD),
    ],
  },
  { name: "shade", hour: 12, views: [...square([3, 4, 1], [0, S, -S]), cardView(ROOF_NORTH_CARD)] },
  {
    name: "evening",
    hour: 18,
    views: [
      ...square([3, 4, 1], [0, S, -S]),
      { name: "hip, 2 m", eye: [-1.2, 5.5, -1.2], at: [0.6, 4.2, 0.6] },
      { name: "eaves, 2 m", eye: [3, 2.6, -2], at: [3, 3.0, 0] },
      { name: "verge, 5 m", eye: [-4, 5, 7], at: [1, 3.5, 6] },
      { name: "whole, 12 m", eye: [-6, 7, -9], at: [3, 2.5, 3] },
      cardView(ROOF_NORTH_CARD),
    ],
  },
];

/** A material's lights and views by its pattern (`Logs`, `Masonry` or `Roof`, as its kit says). */
export const lights = (pattern: string): Light[] => (pattern === "Roof" ? ROOF_LIGHTS : WALL_LIGHTS);
/** The cards a material's runs stand. */
export const cards = (pattern: string): Card[] => (pattern === "Roof" ? [ROOF_WEST_CARD, ROOF_NORTH_CARD] : [WALL_NOON_CARD, WALL_NORTH_CARD]);

/** How many frames the first view is held, while the preview is laid and its textures made
 * resident, and each after it, for the camera's detail and the frames' blend to settle. */
export const FIRST = 300;
export const HOLD = 120;

/** One hour's run: the sandbox's arguments, the calls standing its cards, and the frame each
 * view is shot on. Points are moved from the preview's metres to the start's. */
export function run(material: string, pattern: string, hour: number, calls: string, prefix: string) {
  const views = lights(pattern).filter((light) => light.hour === hour).flatMap((light) => light.views);
  const place = (p: Vec3) => [p[0] + ORIGIN[0], p[1], p[2] + ORIGIN[1]];
  const starts = views.map((_, i) => (i === 0 ? 0 : FIRST + (i - 1) * HOLD));
  const frames = views.map((_, i) => (i === 0 ? FIRST : FIRST + i * HOLD) - 5);
  const args = [
    "--experiment", "play", "--level", "yard", "--kit-sheet", "--sheet-rows", `${material}: preview`,
    "--clean", "--time", String(hour), "--vantage-ground", "--calls", calls,
    ...views.flatMap((view, i) => ["--vantage", `${[...place(view.eye), ...place(view.at)].map((n) => +n.toFixed(3)).join(",")}@${starts[i]}`]),
    "--shots", `${prefix}@${frames.join(",")}`,
  ];
  const stand = cards(pattern).map((card) => JSON.stringify({ frame: 5, method: "sandbox.card", params: { at: place(card.at), facing: card.facing } }));
  return { args, views, frames, calls: stand.join("\n") + "\n" };
}

// --- Measuring a picture -------------------------------------------------------------------

const toLinear = (v: number) => {
  const c = v / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
};
const LINEAR = Float64Array.from({ length: 256 }, (_, i) => toLinear(i));

/** The middle `width` by `height` of a picture. */
export function crop(picture: Picture, width: number, height: number): Picture {
  const x0 = Math.floor((picture.width - width) / 2);
  const y0 = Math.floor((picture.height - height) / 2);
  const rgba = new Uint8Array(width * height * 4);
  for (let y = 0; y < height; y++) rgba.set(picture.rgba.subarray(((y0 + y) * picture.width + x0) * 4, ((y0 + y) * picture.width + x0 + width) * 4), y * width * 4);
  return { width, height, rgba };
}

/** Each pixel's brightness as a person sees it: Rec. 709 luma of its sRGB values, 0 to 1. */
export function luma(picture: Picture): Float64Array {
  const out = new Float64Array(picture.width * picture.height);
  for (let i = 0; i < out.length; i++) out[i] = (0.2126 * picture.rgba[i * 4] + 0.7152 * picture.rgba[i * 4 + 1] + 0.0722 * picture.rgba[i * 4 + 2]) / 255;
  return out;
}

/** The mean colour in linear light, and its luminance. */
export function linearMean(picture: Picture): { rgb: Vec3; luminance: number } {
  const sum = [0, 0, 0];
  const n = picture.width * picture.height;
  for (let i = 0; i < n; i++) for (let c = 0; c < 3; c++) sum[c] += LINEAR[picture.rgba[i * 4 + c]];
  const rgb = sum.map((s) => s / n) as Vec3;
  return { rgb, luminance: 0.2126 * rgb[0] + 0.7152 * rgb[1] + 0.0722 * rgb[2] };
}

/** Sharpness, the finest octave (research/texture-detail-2026-10-15.md): the RMS of what is lost
 * by halving a picture (a box over each two by two) and doubling it again (between the four
 * nearest, a triangle), as a percentage of its RMS contrast. A sharp scan loses a third or more;
 * a blurred or upscaled picture almost nothing. */
export function finestOctave(values: Float64Array, width: number, height: number): number {
  const hw = Math.floor(width / 2);
  const hh = Math.floor(height / 2);
  const half = new Float64Array(hw * hh);
  for (let y = 0; y < hh; y++)
    for (let x = 0; x < hw; x++) {
      const i = 2 * y * width + 2 * x;
      half[y * hw + x] = (values[i] + values[i + 1] + values[i + width] + values[i + width + 1]) / 4;
    }
  const at = (x: number, y: number) => half[Math.min(hh - 1, Math.max(0, y)) * hw + Math.min(hw - 1, Math.max(0, x))];
  let mean = 0;
  for (let i = 0; i < 2 * hh * width; i++) mean += values[i];
  const n = 2 * hh * 2 * hw;
  mean /= n;
  let lost = 0;
  let contrast = 0;
  for (let y = 0; y < 2 * hh; y++)
    for (let x = 0; x < 2 * hw; x++) {
      // The pixel's middle in the half picture's pixels.
      const u = (x + 0.5) / 2 - 0.5;
      const v = (y + 0.5) / 2 - 0.5;
      const [x0, y0] = [Math.floor(u), Math.floor(v)];
      const [fx, fy] = [u - x0, v - y0];
      const up = (at(x0, y0) * (1 - fx) + at(x0 + 1, y0) * fx) * (1 - fy) + (at(x0, y0 + 1) * (1 - fx) + at(x0 + 1, y0 + 1) * fx) * fy;
      const value = values[y * width + x];
      lost += (value - up) ** 2;
      contrast += (value - mean) ** 2;
    }
  return contrast > 0 ? (100 * Math.sqrt(lost / n)) / Math.sqrt(contrast / n) : 0;
}

/** The `p`th percentile (0 to 100) of some values. */
export function percentile(values: ArrayLike<number>, p: number): number {
  const sorted = Float64Array.from(values).sort();
  return sorted[Math.min(sorted.length - 1, Math.max(0, Math.round((p / 100) * (sorted.length - 1))))];
}

// --- Stretch, from `cargo xtask texels` ----------------------------------------------------------

/** Each texture set's share of area past 1.5:1 and 2.5:1, by kit, as `texels` prints it. */
export function parseTexels(text: string): Map<string, Map<string, [number, number]>> {
  const out = new Map<string, Map<string, [number, number]>>();
  for (const line of text.split("\n")) {
    const row = /^(\S+)\s+(\w+)\s+[\d.]+\s+([\d.]+)%\s+([\d.]+)%\s*$/.exec(line);
    if (!row) continue;
    if (!out.has(row[1])) out.set(row[1], new Map());
    out.get(row[1])!.set(row[2], [Number(row[3]), Number(row[4])]);
  }
  return out;
}

// --- Judging ------------------------------------------------------------------------------------

/** A pattern's targets, as `examples/look.rs` gives them (terra_build::look::Targets). */
export type Targets = {
  surface: string;
  density: number;
  sharpness: [number, number, number];
  lit: [number, number];
  shade: [number, number];
  values: [number, number];
  stretch: { smeared: number; streaked: number; roles: Record<string, [number, number]> };
};

/** What was measured of a material. */
export type Measured = {
  density?: [number, number];
  sharpness: (number | undefined)[];
  values?: { p5: number; p50: number; p95: number };
  /** The surface against its card, lit at noon and in the evening, and in shade. */
  lit: (number | undefined)[];
  shade?: number;
  /** Each card's red over blue in linear light, and its colour as seen, by light. */
  cards: Record<string, { redOverBlue: number; seen: Vec3 } | undefined>;
  stretch?: Map<string, [number, number]>;
};

/** One line of the verdict: what, the number as said, the target as said, and whether it holds. */
export type Line = { what: string; value: string; target: string; ok: boolean };

const fmt = (n: number, digits = 1) => (Math.abs(n) >= 100 ? n.toFixed(0) : n.toFixed(digits));
const range = ([a, b]: [number, number], digits = 2) => (b >= 1e30 ? `>= ${fmt(a, digits)}` : `${fmt(a, digits)} to ${fmt(b, digits)}`);
const within = (value: number, [a, b]: [number, number]) => value >= a && value <= b;

/** Every number against its target: a number not measured fails, as a bench that saw nothing
 * must not pass. */
export function judge(measured: Measured, targets: Targets): Line[] {
  const lines: Line[] = [];
  const missing = (what: string, target: string) => lines.push({ what, value: "not measured", target, ok: false });
  if (measured.density) {
    const least = Math.min(...measured.density);
    lines.push({ what: "texel density", value: `${measured.density.map((d) => d.toFixed(0)).join(" x ")} /m`, target: `>= ${targets.density.toFixed(0)}`, ok: least >= targets.density });
  } else missing("texel density", `>= ${targets.density}`);
  DISTANCES.forEach((d, i) => {
    const value = measured.sharpness[i];
    const target = `>= ${fmt(targets.sharpness[i])}%`;
    if (value === undefined) missing(`sharpness, ${d} m`, target);
    else lines.push({ what: `sharpness, ${d} m`, value: `${fmt(value)}%`, target, ok: value >= targets.sharpness[i] });
  });
  if (measured.values) {
    const spread = measured.values.p95 - measured.values.p5;
    lines.push({ what: "value range, 2 m noon", value: `${fmt(spread, 0)} (${measured.values.p5.toFixed(0)} to ${measured.values.p95.toFixed(0)})`, target: range(targets.values, 0), ok: within(spread, targets.values) });
  } else missing("value range, 2 m noon", range(targets.values, 0));
  ["noon", "evening"].forEach((when, i) => {
    const value = measured.lit[i];
    if (value === undefined) missing(`against the grey card, ${when}`, range(targets.lit));
    else lines.push({ what: `against the grey card, ${when}`, value: value.toFixed(2), target: range(targets.lit), ok: within(value, targets.lit) });
  });
  if (measured.shade === undefined) missing("against the grey card, shade", range(targets.shade));
  else lines.push({ what: "against the grey card, shade", value: measured.shade.toFixed(2), target: range(targets.shade), ok: within(measured.shade, targets.shade) });
  for (const light of ["noon", "shade", "evening"]) {
    const card = measured.cards[light];
    // Only the noon card has a target: the shade is meant blue and the evening amber.
    const target = light === "noon" ? range(NEUTRAL) : "(the light's)";
    if (!card) missing(`grey card red/blue, ${light}`, target);
    else lines.push({ what: `grey card red/blue, ${light}`, value: `${card.redOverBlue.toFixed(2)} (${card.seen.join(",")})`, target, ok: light !== "noon" || within(card.redOverBlue, NEUTRAL) });
  }
  if (!measured.stretch) missing("stretch", "measured");
  else
    for (const [role, [smeared, streaked]] of measured.stretch) {
      const [most, worst] = targets.stretch.roles[role] ?? [targets.stretch.smeared, targets.stretch.streaked];
      lines.push({ what: `stretch, ${role}`, value: `${fmt(smeared)}% / ${fmt(streaked)}%`, target: `<= ${fmt(most)}% / ${fmt(worst)}%`, ok: smeared <= most && streaked <= worst });
    }
  return lines;
}

/** The verdict as a table, a line a number. */
export function table(name: string, about: string, lines: Line[]): string {
  const failed = lines.filter((line) => !line.ok).length;
  const rows = lines.map((line) => `  ${line.ok ? "ok  " : "FAIL"}  ${line.what.padEnd(32)} ${line.value.padEnd(26)} ${line.target}`);
  return [`${name} (${about}): ${failed ? `${failed} FAILED` : "holds"}`, `  ${"".padEnd(4)}  ${"what".padEnd(32)} ${"measured".padEnd(26)} target`, ...rows].join("\n");
}

// --- The run ------------------------------------------------------------------------------------

type Material = {
  name: string;
  id: number;
  pattern: string;
  surface?: { role: string; path: string; metres: [number, number]; density?: [number, number] };
  targets: Targets;
  roles: string[];
  reference?: { photo: string; source: string; licence: string; about: string } | null;
};

const PATTERNS: Record<string, string> = { Logs: "coursed", Masonry: "block", Roof: "roof" };

function materials(names: string[]): Material[] {
  const done = spawnSync("cargo", ["run", "-q", "--release", "-p", "terra_build", "--example", "look", "--", ...names], { cwd: ROOT, encoding: "utf8" });
  if (done.status !== 0) throw new Error(`the materials' data: ${done.stderr}`);
  return JSON.parse(done.stdout);
}

/** The building baseline (D333): the settlement's wall materials and the roofs they take. */
function baseline(): string[] {
  const kits = readKits();
  const walls = kits.filter((kit) => kit.baseline);
  return [...new Set([...walls.map((kit) => kit.name), ...walls.flatMap((kit) => (kit.roof ? [kit.roof] : []))])].sort();
}

/** `texels` over the bases a material is drawn in: its own walls' and those mixed with them, or
 * for a roof the walls it is laid over; every base for several. */
function texels(chosen: Material[]): Map<string, Map<string, [number, number]>> {
  const kits = readKits();
  const filter =
    chosen.length === 1
      ? chosen[0].pattern === "Roof"
        ? kits.find((kit) => kit.roof === chosen[0].name && kit.baseline) ?? kits.find((kit) => kit.roof === chosen[0].name)
        : kits.find((kit) => kit.name === chosen[0].name)
      : undefined;
  const args = ["run", "-q", "--release", "-p", "terra_build", "--example", "texels", "--", "--worst", "0", ...(filter ? [`${filter.id}: `] : [])];
  const done = spawnSync("cargo", args, { cwd: ROOT, encoding: "utf8", maxBuffer: 64 * 1024 * 1024 });
  if (done.status !== 0) throw new Error(`texels: ${done.stderr}`);
  return parseTexels(done.stdout);
}

/** The reference photograph, fetched to its place if it is not there. */
async function photo(material: Material): Promise<string | undefined> {
  const reference = material.reference;
  if (!reference) return undefined;
  const path = resolve(ROOT, reference.photo);
  if (existsSync(path)) return path;
  try {
    const reply = await fetch(reference.source, { headers: { "User-Agent": "terra-material-bench/1.0 (https://github.com/dcramer/terra)" } });
    if (!reply.ok) throw new Error(`${reply.status} ${reply.statusText}`);
    mkdirSync(dirname(path), { recursive: true });
    writeFileSync(path, Buffer.from(await reply.arrayBuffer()));
    console.log(`look: fetched ${material.name}'s reference photograph to ${relative(ROOT, path)} (${reference.licence})`);
    return path;
  } catch (error) {
    console.log(`look: ${material.name}'s reference photograph could not be fetched from ${reference.source}: ${(error as Error).message}`);
    return undefined;
  }
}

/** Shoots a material's views, both hours, and says where each view's picture is. */
function shoot(material: Material, out: string): Map<View, string> {
  const shots = new Map<View, string>();
  for (const hour of [12, 18]) {
    const prefix = resolve(out, `${material.name}-${hour}`);
    const callsFile = resolve(out, `${material.name}-${hour}.calls.jsonl`);
    const { args, views, frames, calls } = run(material.name, material.pattern, hour, offscreenPath(callsFile), offscreenPath(prefix));
    writeFileSync(callsFile, calls);
    const [command, words] = offscreenRun("look", args);
    const started = Date.now();
    let done = spawnSync(command, words, { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT }, maxBuffer: 256 * 1024 * 1024 });
    // Windows will not replace a program still closing, such as the last run's.
    for (let attempt = 0; attempt < 30 && `${done.stdout}${done.stderr}`.includes("Could not copy the program"); attempt++) {
      spawnSync("sleep", ["2"]);
      done = spawnSync(command, words, { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT }, maxBuffer: 256 * 1024 * 1024 });
    }
    const said = `${done.stdout}${done.stderr}`;
    if (!said.includes("SANDBOX saved")) throw new Error(`the run at ${hour}:00 did not finish:\n${said.split("\n").slice(-20).join("\n")}`);
    if (said.includes("sandbox.card") && /Calls: .*refused|Calls: .*no method/.test(said)) throw new Error(`the grey cards were refused:\n${said.split("\n").filter((line) => line.includes("Calls")).join("\n")}`);
    views.forEach((view, i) => shots.set(view, `${prefix}-${String(frames[i]).padStart(5, "0")}.png`));
    console.log(`look: ${material.name} at ${hour}:00, ${views.length} views in ${((Date.now() - started) / 1000).toFixed(0)} s`);
  }
  return shots;
}

/** What the shots measure. */
function measure(material: Material, shots: Map<View, string>, stretch: Map<string, [number, number]> | undefined): Measured {
  const all = lights(material.pattern);
  const shotOf = (light: string, test: (view: View) => boolean) => {
    const view = all.find((each) => each.name === light)?.views.find(test);
    const file = view && shots.get(view);
    return file && existsSync(file) ? readPng(file) : undefined;
  };
  const surface = (light: string, distance: number) => shotOf(light, (view) => view.measure === "surface" && view.distance === distance);
  const card = (light: string) => {
    const picture = shotOf(light, (view) => view.measure === "card");
    return picture && linearMean(crop(picture, CARD_CROP, CARD_CROP));
  };
  const middle = (picture: Picture) => crop(picture, CROP.width, CROP.height);
  const sharpness = DISTANCES.map((d) => {
    const picture = surface("noon", d);
    if (!picture) return undefined;
    const part = middle(picture);
    return finestOctave(luma(part), part.width, part.height);
  });
  const against = (light: string) => {
    const picture = surface(light, 2);
    const grey = card(light);
    return picture && grey ? linearMean(middle(picture)).luminance / grey.luminance : undefined;
  };
  const noon = surface("noon", 2);
  const values = noon && luma(middle(noon)).map((v) => v * 255);
  const cards: Measured["cards"] = {};
  for (const light of ["noon", "shade", "evening"]) {
    const grey = card(light);
    const toSrgb = (l: number) => Math.round(255 * (l <= 0.0031308 ? l * 12.92 : 1.055 * l ** (1 / 2.4) - 0.055));
    cards[light] = grey && { redOverBlue: grey.rgb[0] / grey.rgb[2], seen: grey.rgb.map(toSrgb) as Vec3 };
  }
  const roles = new Set(material.roles);
  return {
    density: material.surface?.density,
    sharpness,
    values: values && { p5: percentile(values, 5), p50: percentile(values, 50), p95: percentile(values, 95) },
    lit: [against("noon"), against("evening")],
    shade: against("shade"),
    cards,
    stretch: stretch && new Map([...stretch].filter(([role]) => roles.has(role))),
  };
}

/** The sheet: each light a row of its views, the 1 and 2 m shots as 1:1 crops of their middle
 * (what the sharpness is measured on), the reference photograph after the shade's, and the
 * table under them; one JPG under 2 MB. */
function sheet(material: Material, shots: Map<View, string>, reference: string | undefined, text: string, out: string): string {
  const work = resolve(out, `${material.name}-tiles`);
  rmSync(work, { recursive: true, force: true });
  mkdirSync(work, { recursive: true });
  const tiles: string[] = [];
  const columns = Math.max(...lights(material.pattern).map((light) => light.views.length));
  let n = 0;
  for (const light of lights(material.pattern)) {
    const row: string[] = [];
    for (const view of light.views) {
      const file = shots.get(view);
      if (!file || !existsSync(file)) {
        row.push("-label", `${light.name}: ${view.name}, missing`, "null:");
        continue;
      }
      const tile = resolve(work, `${n++}.png`);
      const close = view.measure === "surface" && (view.distance ?? 9) <= 2;
      const [command, words] = imageMagick(close ? [file, "-gravity", "center", "-crop", "480x270+0+0", "+repage", tile] : [file, "-resize", "480x270", tile]);
      spawnSync(command, words);
      row.push("-label", `${light.name}: ${view.measure === "surface" ? `${material.surface?.role ?? "surface"} ${view.name}${close ? " (1:1)" : ""}` : view.name}`, tile);
    }
    if (light.name === "shade" && reference) row.push("-label", `reference: ${material.reference?.about ?? ""}`.slice(0, 70), `${reference}[480x270]`);
    while (row.length / 3 < columns) row.push("-label", "", "null:");
    tiles.push(...row);
  }
  const grid = resolve(work, "grid.png");
  const [m, mw] = imageMagick(["montage", "-font", "DejaVu-Sans", "-pointsize", "14", "-fill", "white", "-background", "#16181c", ...tiles, "-tile", `${columns}x`, "-geometry", "480x270+3+3", grid]);
  const montage = spawnSync(m, mw, { encoding: "utf8" });
  if (montage.status !== 0) throw new Error(`montage: ${montage.stderr}`);
  const file = resolve(out, `${material.name}.jpg`);
  for (const quality of [85, 75, 65, 55, 45]) {
    // The table as a label: its percent signs doubled, as ImageMagick reads `%` as an escape.
    const [c, cw] = imageMagick([
      grid,
      "(", "-background", "#16181c", "-fill", "white", "-font", "DejaVu-Sans-Mono", "-pointsize", "16", `label:${text.replaceAll("%", "%%")}`, ")",
      "-gravity", "west", "-background", "#16181c", "-append", "-quality", String(quality), file,
    ]);
    const made = spawnSync(c, cw, { encoding: "utf8" });
    if (made.status !== 0) throw new Error(`convert: ${made.stderr}`);
    if (statSync(file).size < 2e6) break;
  }
  rmSync(work, { recursive: true, force: true });
  return file;
}

async function main() {
  const { positionals, values } = parseArgs({
    allowPositionals: true,
    options: { all: { type: "boolean" }, out: { type: "string", default: "shots/look" }, help: { type: "boolean", short: "h" } },
  });
  if (values.help || (!values.all && !positionals.length)) {
    console.log("usage: cargo xtask look <material>... | --all [--out shots/look]");
    process.exit(values.help ? 0 : 2);
  }
  const started = Date.now();
  const names = values.all ? baseline() : positionals;
  const out = resolve(ROOT, values.out!);
  mkdirSync(out, { recursive: true });
  const chosen = materials(names);
  if (!buildOffscreen()) throw new Error("the sandbox does not build");
  console.log(`look: ${chosen.map((m) => m.name).join(", ")}; measuring stretch (texels)…`);
  const stretched = texels(chosen);
  console.log(`look: stretch measured in ${((Date.now() - started) / 1000).toFixed(0)} s`);
  const verdicts: string[] = [];
  let failed = 0;
  for (const material of chosen) {
    const shots = shoot(material, out);
    const measured = measure(material, shots, stretched.get(material.name));
    const lines = judge(measured, material.targets);
    const about = `${PATTERNS[material.pattern] ?? material.pattern} pattern, ${material.surface?.role ?? "?"}: ${material.surface?.path ?? "?"}`;
    const text = table(material.name, about, lines);
    const reference = await photo(material);
    const file = sheet(material, shots, reference, text, out);
    writeFileSync(resolve(out, `${material.name}.txt`), text + "\n");
    for (const shot of shots.values()) rmSync(shot, { force: true });
    console.log(`\n${text}\n  ${relative(ROOT, file)}, ${(statSync(file).size / 1e6).toFixed(2)} MB\n`);
    verdicts.push(`${lines.every((line) => line.ok) ? "ok  " : "FAIL"}  ${material.name}`);
    if (!lines.every((line) => line.ok)) failed++;
  }
  console.log(verdicts.join("\n"));
  console.log(`look: ${chosen.length} materials in ${((Date.now() - started) / 60000).toFixed(1)} min; ${failed ? `${failed} FAILED` : "every one holds"} (shots/look/)`);
  process.exit(failed ? 1 : 0);
}

if (import.meta.main) {
  try {
    await main();
  } catch (error) {
    console.error(`look: ${(error as Error).message}`);
    process.exit(1);
  }
}
