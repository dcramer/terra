#!/usr/bin/env node
// How a picture of a place looks, in the numbers its look is held to (D286): how bright it is
// and how much of it is near black, how warm, how much local contrast, the shade's colour and
// the middle distance's, how much of its lower third is lit orange litter and of its top third
// lit lime. Measured against a reference: the same numbers over a set of frames of the look
// aimed at (another game's own frames, photographs), kept as `looks/<name>.json` (the
// pictures themselves stay out of the repository, which is public). Values are the picture's
// sRGB values, as a person sees them, not linear light.
//
//   node tools/sandbox/look.ts measure [--against sotf-forest-day] [--player] <png|jpg|mp4>...
//   node tools/sandbox/look.ts reference <name> --about "<what it is>" [--open] [--source <url>]... <png|jpg|mp4>...
//
// `measure` prints each picture's look and their mean beside the reference's; a film is
// measured every four seconds; `--player` leaves out the middle of the lower half, where a
// third-person film's placeholder figure runs. `reference` writes `looks/<name>.json` from
// the pictures given. The vantage sheet measures its forest views so (`vantage.ts`). Needs
// ImageMagick, and ffmpeg for films. Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, resolve } from "node:path";
import { type Picture, readPng, writePng } from "./png.ts";

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
  /// The share of the top third that is lit green: green, saturated, brighter than 0.3, the
  /// lime of low crowns in the sun.
  limeTop: number;
  /// The share of the middle third that is brown by hue (8 to 55 degrees): trunks, and olive
  /// shade with them, as the research measured it (its `brownMid`), the trunks a walk among
  /// tall boles shows ahead (D301). Older references may not have it.
  trunkMiddle?: number;
};

/// One band of a picture by distance (D326): how bright, how blue against green, how
/// saturated, how much local contrast, and how much of the picture it is.
export type Band = { share: number; mean: number; blueOverGreen: number; saturation: number; contrast: number };

/// A picture by distance, for open views, where the whole frame's means hide whether far land
/// is lost in haze or stands too sharp (D326): the sky; the far land, the strip just under the
/// skyline; the middle distance; and the near ground, the lower third. The far land against the
/// sky: how much darker it is, as a share of the sky's brightness (0 lost in the sky, 1 black),
/// and how much bluer it is than the middle distance (the air's colour shift).
export type Bands = { sky: Band; far: Band; middle: Band; near: Band; farUnderSky: number; farBluerThanMiddle: number };

/// A look aimed at: the mean look of a set of frames, what they are and where from, and, for a
/// reference of open views, its bands.
export type Reference = { name: string; about: string; sources: string[]; frames: number; measured: string; look: Look; bands?: Bands };

const LOOKS = resolve(import.meta.dirname, "looks");

/// A reference kept in `looks/<name>.json`.
export function reference(name: string): Reference {
  return JSON.parse(readFileSync(resolve(LOOKS, `${name}.json`), "utf8")) as Reference;
}

/// The references kept, by name.
export function references(): string[] {
  return readdirSync(LOOKS).filter((f) => f.endsWith(".json")).map((f) => f.replace(/\.json$/, ""));
}

/// Sons of the Forest's daytime forest, the lush forest's target by day (D286).
export const SOTF: Look = reference("sotf-forest-day").look;

/// The picture's look; `masked` pixels (such as the placeholder player in a film) are left
/// out.
export function look(p: Picture, masked: (x: number, y: number) => boolean = () => false): Look {
  const { width: W, height: H, rgba } = p;
  const luma = new Float32Array(W * H);
  const ys: number[] = [];
  let r = 0, g = 0, b = 0, dark = 0, bright = 0, lower = 0, litter = 0;
  const shade = [0, 0, 0], middle = [0, 0, 0];
  let top = 0, lime = 0, centre = 0, brown = 0;
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
      const high = Math.max(cr, cg, cb), low = Math.min(cr, cg, cb), d = high - low;
      const l = (high + low) / 2;
      const s = d === 0 ? 0 : d / (1 - Math.abs(2 * l - 1));
      let h = 0;
      if (d > 0) h = high === cr ? 60 * (((cg - cb) / d + 6) % 6) : high === cg ? 60 * ((cb - cr) / d + 2) : 60 * ((cr - cg) / d + 4);
      if (y >= (H * 2) / 3) {
        lower++;
        if (h >= 8 && h < 45 && s > 0.25 && v > 0.25) litter++;
      }
      if (y >= H / 3 && y < (H * 2) / 3) {
        centre++;
        if (d > 0 && h >= 8 && h < 55) brown++;
      }
      if (y < H / 3) {
        top++;
        if (h >= 55 && h < 170 && s > 0.12 && v > 0.3) lime++;
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
    limeTop: top ? lime / top : 0,
    trunkMiddle: centre ? brown / centre : 0,
  };
}

/// How far down from the top each column's sky reaches (D326): sky is read down from the top
/// while it stays sky-like (bluer than it is green and red, or bright and smooth, as cloud and
/// haze are), and ends at the first run of three pixels that are not; the columns' skylines
/// are then smoothed by a running median, so a tree's twig or a bird does not cut them. A
/// column whose top is not sky has its skyline at 0.
export function skyline(p: Picture): Float32Array {
  const { width: W, height: H, rgba } = p;
  const at = (x: number, y: number) => {
    const i = (y * W + x) * 4;
    return [rgba[i] / 255, rgba[i + 1] / 255, rgba[i + 2] / 255];
  };
  const luma = (c: number[]) => 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
  const skyLike = (x: number, y: number) => {
    const c = at(x, y);
    const v = luma(c);
    if (c[2] > c[0] * 1.05 && c[2] >= c[1] * 0.97 && v > 0.25) return true;
    const high = Math.max(...c), low = Math.min(...c);
    if (v < 0.55 || (high - low) / high > 0.3) return false;
    // Bright, pale and smooth against the pixels round it, as cloud and haze are and lit rock
    // is not.
    let spread = 0;
    for (const [dx, dy] of [[-2, 0], [2, 0], [0, 2], [0, -2]]) {
      const n = at(Math.min(W - 1, Math.max(0, x + dx)), Math.min(H - 1, Math.max(0, y + dy)));
      spread = Math.max(spread, Math.abs(luma(n) - v));
    }
    return spread < 0.08;
  };
  const raw = new Float32Array(W);
  for (let x = 0; x < W; x++) {
    // From a hundredth of the way down, past what a frame's edge or a trimmed letterbox
    // leaves.
    let y = Math.floor(H / 100), misses = 0;
    while (y < H && misses < 3) {
      misses = skyLike(x, y) ? 0 : misses + 1;
      y++;
    }
    raw[x] = Math.max(0, y - misses);
  }
  const half = Math.max(1, Math.round(W / 64));
  const out = new Float32Array(W);
  for (let x = 0; x < W; x++) {
    const window = Array.from(raw.subarray(Math.max(0, x - half), Math.min(W, x + half + 1))).sort((a, b) => a - b);
    out[x] = window[window.length >> 1];
  }
  return out;
}

/// A picture's bands (`Bands`): above each column's skyline is sky; the far land is the strip
/// below it a tenth of the picture deep; the near ground is the lower third; the middle
/// distance is between. `masked` pixels are left out.
export function bands(p: Picture, masked: (x: number, y: number) => boolean = () => false): Bands {
  const { width: W, height: H, rgba } = p;
  const line = skyline(p);
  const sums = [0, 1, 2, 3].map(() => ({ n: 0, v: 0, r: 0, g: 0, b: 0, s: 0 }));
  const which = new Int8Array(W * H).fill(-1);
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      if (masked(x, y)) continue;
      const k = y < line[x] ? 0 : y < line[x] + H / 10 ? 1 : y < (H * 2) / 3 ? 2 : 3;
      which[y * W + x] = k;
      const i = (y * W + x) * 4;
      const [r, g, b] = [rgba[i] / 255, rgba[i + 1] / 255, rgba[i + 2] / 255];
      const high = Math.max(r, g, b), low = Math.min(r, g, b);
      const sum = sums[k];
      sum.n++;
      sum.v += 0.2126 * r + 0.7152 * g + 0.0722 * b;
      sum.r += r;
      sum.g += g;
      sum.b += b;
      sum.s += high > 0 ? (high - low) / high : 0;
    }
  }
  // Local contrast per band: the spread of brightness in 16-pixel squares wholly in the band.
  const spread = [0, 0, 0, 0], squares = [0, 0, 0, 0];
  for (let by = 0; by + 16 <= H; by += 16) {
    for (let bx = 0; bx + 16 <= W; bx += 16) {
      const k = which[by * W + bx];
      let m = 0, m2 = 0, same = k >= 0;
      for (let y = by; y < by + 16 && same; y++) {
        for (let x = bx; x < bx + 16; x++) {
          if (which[y * W + x] !== k) {
            same = false;
            break;
          }
          const i = (y * W + x) * 4;
          const v = (0.2126 * rgba[i] + 0.7152 * rgba[i + 1] + 0.0722 * rgba[i + 2]) / 255;
          m += v;
          m2 += v * v;
        }
      }
      if (!same) continue;
      m /= 256;
      spread[k] += Math.sqrt(Math.max(0, m2 / 256 - m * m)) / Math.max(m, 0.02);
      squares[k]++;
    }
  }
  const band = (k: number): Band => {
    const s = sums[k];
    const n = Math.max(1, s.n);
    return {
      share: s.n / (W * H),
      mean: s.v / n,
      blueOverGreen: s.g ? s.b / s.g : 0,
      saturation: s.s / n,
      contrast: squares[k] ? spread[k] / squares[k] : 0,
    };
  };
  const [sky, far, middle, near] = [0, 1, 2, 3].map(band);
  return {
    sky,
    far,
    middle,
    near,
    farUnderSky: sky.share > 0.01 && far.share > 0.01 ? (sky.mean - far.mean) / Math.max(sky.mean, 0.02) : 0,
    farBluerThanMiddle: far.share > 0.01 && middle.share > 0.01 ? far.blueOverGreen - middle.blueOverGreen : 0,
  };
}

/// The mean of several pictures' bands.
export function meanOfBands(all: Bands[]): Bands {
  const m = (f: (b: Bands) => number) => all.reduce((s, b) => s + f(b), 0) / all.length;
  const band = (k: "sky" | "far" | "middle" | "near"): Band => ({
    share: m((b) => b[k].share),
    mean: m((b) => b[k].mean),
    blueOverGreen: m((b) => b[k].blueOverGreen),
    saturation: m((b) => b[k].saturation),
    contrast: m((b) => b[k].contrast),
  });
  return {
    sky: band("sky"),
    far: band("far"),
    middle: band("middle"),
    near: band("near"),
    farUnderSky: m((b) => b.farUnderSky),
    farBluerThanMiddle: m((b) => b.farBluerThanMiddle),
  };
}

/// One line for a picture's bands, the reference's beside each number.
export function bandsLine(name: string, b: Bands, against?: Bands, called = "ref"): string {
  const f = (v: number) => v.toFixed(3);
  const one = (k: "sky" | "far" | "middle" | "near") => {
    const a = against?.[k];
    const r = (v: number | undefined) => (v === undefined ? "" : ` (${f(v)})`);
    return `${k} ${(100 * b[k].share).toFixed(0)}%${a ? ` (${(100 * a.share).toFixed(0)}%)` : ""} mean ${f(b[k].mean)}${r(a?.mean)} b/g ${f(b[k].blueOverGreen)}${r(a?.blueOverGreen)} sat ${f(b[k].saturation)}${r(a?.saturation)} contrast ${f(b[k].contrast)}${r(a?.contrast)}`;
  };
  const r = (v: number | undefined) => (v === undefined ? "" : ` (${called} ${f(v)})`);
  return `${name}: far under sky ${f(b.farUnderSky)}${r(against?.farUnderSky)}, far bluer than middle ${f(b.farBluerThanMiddle)}${r(against?.farBluerThanMiddle)}; ${one("sky")}; ${one("far")}; ${one("middle")}; ${one("near")}`;
}

/// The picture with its bands drawn over it, to check where they fall: the skyline red, the
/// far land's lower edge yellow, the near ground's upper edge green.
export function drawBands(p: Picture): Picture {
  const { width: W, height: H } = p;
  const rgba = new Uint8Array(p.rgba);
  const line = skyline(p);
  const mark = (x: number, y: number, c: number[]) => {
    for (let dy = -1; dy <= 1; dy++) {
      const yy = Math.round(y) + dy;
      if (yy >= 0 && yy < H) rgba.set([c[0], c[1], c[2], 255], (yy * W + x) * 4);
    }
  };
  for (let x = 0; x < W; x++) {
    mark(x, line[x], [255, 0, 0]);
    mark(x, Math.min(H - 1, line[x] + H / 10), [255, 220, 0]);
    mark(x, (H * 2) / 3, [0, 255, 0]);
  }
  return { width: W, height: H, rgba };
}

/// The mean of several pictures' looks.
export function meanOf(looks: Look[]): Look {
  const keys = Object.keys(SOTF) as (keyof Look)[];
  return Object.fromEntries(keys.map((k) => [k, looks.reduce((s, l) => s + (l[k] ?? 0), 0) / looks.length])) as Look;
}

/// One line for a look: each number, and the reference's beside it (Sons of the Forest's by day
/// unless another is given).
export function line(name: string, l: Look, against: Look = SOTF, called = "SotF"): string {
  const SOTF = against;
  const f = (v: number) => v.toFixed(3);
  const share = (v: number | undefined) => (v === undefined ? "-" : `${(100 * v).toFixed(1)}%`);
  return `${name}: mean ${f(l.mean)} (${called} ${f(SOTF.mean)}), p5 ${f(l.p5)} (${f(SOTF.p5)}), below 0.08 ${(100 * l.dark).toFixed(0)}% (${(100 * SOTF.dark).toFixed(0)}%), red/green ${f(l.redOverGreen)} (${f(SOTF.redOverGreen)}), blue/green ${f(l.blueOverGreen)} (${f(SOTF.blueOverGreen)}), contrast ${f(l.contrast)} (${f(SOTF.contrast)}), lit litter ${(100 * l.litLitter).toFixed(1)}% (${(100 * SOTF.litLitter).toFixed(1)}%), shade red/green ${f(l.shadeRedOverGreen)} (${f(SOTF.shadeRedOverGreen)}), shade blue/green ${f(l.shadeBlueOverGreen)} (${f(SOTF.shadeBlueOverGreen)}), middle blue/green ${f(l.middleBlueOverGreen)} (${f(SOTF.middleBlueOverGreen)}), lime top ${(100 * l.limeTop).toFixed(1)}% (${(100 * SOTF.limeTop).toFixed(1)}%), trunks middle ${share(l.trunkMiddle)} (${share(SOTF.trunkMiddle)})`;
}

/// The middle of the lower half of a picture, where a third-person film's placeholder figure
/// runs, as the research left it out.
export const player = (width: number, height: number) => (x: number, y: number) =>
  x >= width * 0.36 && x < width * 0.64 && y >= height * 0.5;

/// Pictures as 1920 by 1080 PNGs in a folder of their own: each picture, and a film's frame
/// every four seconds.
function frames(files: string[], into: string): string[] {
  const out: string[] = [];
  files.forEach((file, k) => {
    if (file.endsWith(".mp4")) {
      const done = spawnSync("ffmpeg", ["-loglevel", "error", "-i", file, "-vf", "fps=0.25,scale=1920:1080", resolve(into, `${k}-%03d.png`)]);
      if (done.status !== 0) throw new Error(`ffmpeg ${file}: ${done.stderr}`);
      out.push(...readdirSync(into).filter((f) => f.startsWith(`${k}-`)).sort().map((f) => resolve(into, f)));
    } else {
      const png = resolve(into, `${k}-${basename(file).replace(/\.[^.]+$/, "")}.png`);
      const done = spawnSync("magick", [file, "-resize", "1920x1080!", "-depth", "8", png]);
      if (done.status !== 0) throw new Error(`magick ${file}: ${done.stderr}`);
      out.push(png);
    }
  });
  return out;
}

if (import.meta.main) {
  const [command, ...rest] = process.argv.slice(2);
  const flag = (name: string) => {
    const at = rest.indexOf(name);
    return at >= 0 ? rest.splice(at, 2)[1] : undefined;
  };
  const sources: string[] = [];
  for (let s = flag("--source"); s !== undefined; s = flag("--source")) sources.push(s);
  const against = flag("--against") ?? "sotf-forest-day";
  const about = flag("--about");
  const drawn = flag("--draw-bands");
  const masked = rest.includes("--player");
  const open = rest.includes("--open");
  const files = rest.filter((a) => a !== "--player" && a !== "--open");
  const name = command === "reference" ? files.shift() : undefined;
  if ((command !== "measure" && command !== "reference") || !files.length || (command === "reference" && (!name || !about))) {
    console.error(`usage: look.ts measure [--against <${references().join("|")}>] [--player] [--draw-bands <dir>] <png|jpg|mp4>...`);
    console.error('       look.ts reference <name> --about "<what it is>" [--open] [--source <url>]... <png|jpg|mp4>...');
    console.error("  --open: a reference of open views, measured by band too (sky, far land, middle, near; D326)");
    console.error("  --draw-bands: also write each picture with its bands' edges drawn on it into the folder");
    process.exit(2);
  }
  const into = mkdtempSync(resolve(tmpdir(), "look-"));
  try {
    const looks = frames(files, into).map((file) => {
      const p = readPng(file);
      const mask = masked ? player(p.width, p.height) : undefined;
      if (drawn) writePng(resolve(drawn, basename(file)), drawBands(p));
      return { file, look: look(p, mask), bands: bands(p, mask) };
    });
    if (command === "reference") {
      const kept: Reference = { name: name!, about: about!, sources, frames: looks.length, measured: new Date().toISOString().slice(0, 10), look: meanOf(looks.map((l) => l.look)) };
      if (open) kept.bands = meanOfBands(looks.map((l) => l.bands));
      writeFileSync(resolve(LOOKS, `${name}.json`), JSON.stringify(kept, null, 2) + "\n");
      console.log(line(`${name}, ${looks.length} frames`, kept.look, kept.look, name!));
      if (kept.bands) console.log(bandsLine(`${name} by band`, kept.bands));
      console.log(`look: wrote tools/sandbox/looks/${name}.json`);
    } else {
      const target = reference(against);
      for (const { file, look: l, bands: b } of looks) {
        console.log(line(basename(file), l, target.look, against));
        if (target.bands) console.log(bandsLine(`${basename(file)} by band`, b, target.bands, against));
      }
      console.log(line(`mean of ${looks.length}`, meanOf(looks.map((l) => l.look)), target.look, against));
      if (target.bands) console.log(bandsLine(`mean of ${looks.length} by band`, meanOfBands(looks.map((l) => l.bands)), target.bands, against));
    }
  } finally {
    rmSync(into, { recursive: true, force: true });
  }
}
