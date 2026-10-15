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
//   node tools/sandbox/look.ts reference <name> --about "<what it is>" [--source <url>]... <png|jpg|mp4>...
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
import { type Picture, readPng } from "./png.ts";

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
};

/// A look aimed at: the mean look of a set of frames, what they are and where from.
export type Reference = { name: string; about: string; sources: string[]; frames: number; measured: string; look: Look };

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
  let top = 0, lime = 0;
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
  };
}

/// The mean of several pictures' looks.
export function meanOf(looks: Look[]): Look {
  const keys = Object.keys(SOTF) as (keyof Look)[];
  return Object.fromEntries(keys.map((k) => [k, looks.reduce((s, l) => s + l[k], 0) / looks.length])) as Look;
}

/// One line for a look: each number, and the reference's beside it (Sons of the Forest's by day
/// unless another is given).
export function line(name: string, l: Look, against: Look = SOTF, called = "SotF"): string {
  const SOTF = against;
  const f = (v: number) => v.toFixed(3);
  return `${name}: mean ${f(l.mean)} (${called} ${f(SOTF.mean)}), p5 ${f(l.p5)} (${f(SOTF.p5)}), below 0.08 ${(100 * l.dark).toFixed(0)}% (${(100 * SOTF.dark).toFixed(0)}%), red/green ${f(l.redOverGreen)} (${f(SOTF.redOverGreen)}), blue/green ${f(l.blueOverGreen)} (${f(SOTF.blueOverGreen)}), contrast ${f(l.contrast)} (${f(SOTF.contrast)}), lit litter ${(100 * l.litLitter).toFixed(1)}% (${(100 * SOTF.litLitter).toFixed(1)}%), shade red/green ${f(l.shadeRedOverGreen)} (${f(SOTF.shadeRedOverGreen)}), shade blue/green ${f(l.shadeBlueOverGreen)} (${f(SOTF.shadeBlueOverGreen)}), middle blue/green ${f(l.middleBlueOverGreen)} (${f(SOTF.middleBlueOverGreen)}), lime top ${(100 * l.limeTop).toFixed(1)}% (${(100 * SOTF.limeTop).toFixed(1)}%)`;
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
  const masked = rest.includes("--player");
  const files = rest.filter((a) => a !== "--player");
  const name = command === "reference" ? files.shift() : undefined;
  if ((command !== "measure" && command !== "reference") || !files.length || (command === "reference" && (!name || !about))) {
    console.error(`usage: look.ts measure [--against <${references().join("|")}>] [--player] <png|jpg|mp4>...`);
    console.error('       look.ts reference <name> --about "<what it is>" [--source <url>]... <png|jpg|mp4>...');
    process.exit(2);
  }
  const into = mkdtempSync(resolve(tmpdir(), "look-"));
  try {
    const looks = frames(files, into).map((file) => {
      const p = readPng(file);
      return { file, look: look(p, masked ? player(p.width, p.height) : undefined) };
    });
    if (command === "reference") {
      const kept: Reference = { name: name!, about: about!, sources, frames: looks.length, measured: new Date().toISOString().slice(0, 10), look: meanOf(looks.map((l) => l.look)) };
      writeFileSync(resolve(LOOKS, `${name}.json`), JSON.stringify(kept, null, 2) + "\n");
      console.log(line(`${name}, ${looks.length} frames`, kept.look, kept.look, name!));
      console.log(`look: wrote tools/sandbox/looks/${name}.json`);
    } else {
      const target = reference(against);
      for (const { file, look: l } of looks) console.log(line(basename(file), l, target.look, against));
      console.log(line(`mean of ${looks.length}`, meanOf(looks.map((l) => l.look)), target.look, against));
    }
  } finally {
    rmSync(into, { recursive: true, force: true });
  }
}
