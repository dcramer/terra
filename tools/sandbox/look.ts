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
  /// The art style's rules that one picture can show (docs/art-style.md, "Places"; D356): how
  /// much of it is a debug tint, a pure lit hue (`tint`), or lilac (`lilac`); what share of
  /// its shading edges are hard (`hardEdges`); how much long straight colour edge its lower
  /// third has, as triangles show through the ground's layers (`facets`); and how much of the
  /// land is flat colour, untextured (`flat`). References measured before them lack them.
  tint?: number;
  lilac?: number;
  hardEdges?: number;
  facets?: number;
  flat?: number;
  /// Flat foliage (docs/art-style.md, "Leaves read as leaves"; D368), in squares of leaves below
  /// the skyline: the share of them one flat green, their leaves' saturation hardly varying
  /// (`flatFoliage`); how much it varies (`foliageColour`); and how much of their leaves catch
  /// the light, half as bright again as the rest (`foliageLit`). None where a picture has too
  /// few leaves to say; references measured before them lack them.
  flatFoliage?: number;
  foliageColour?: number;
  foliageLit?: number;
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
/// out, and `water` pixels out of the debug-tint rule alone (`tints`).
export function look(p: Picture, masked: (x: number, y: number) => boolean = () => false, water?: (x: number, y: number) => boolean): Look {
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
    ...tints(p, masked, water),
    hardEdges: hardEdges(p, masked),
    facets: facets(p, masked),
    flat: flat(p, masked),
    ...foliage(p, masked),
  };
}

/// Whether a pixel is leaf, sRGB 0 to 1: green-hued (55 to 170 degrees, as lime crowns are
/// counted), saturated past 0.15 in HSV and brighter than 0.03, so its hue is its own.
export function leafy(r: number, g: number, b: number): boolean {
  const [h, s] = hsv(r, g, b);
  return h >= 55 && h < 170 && s > 0.15 && 0.2126 * r + 0.7152 * g + 0.0722 * b > 0.03;
}

/// Flat foliage (docs/art-style.md, "Leaves read as leaves"; D368), over the leaf pixels
/// (`leafy`) below the skyline, in 64-pixel squares at least two fifths leaf, a bush or a part
/// of a crown. A leaf lit by the sun and the sky alone, one colour scaled brighter or darker,
/// keeps its saturation however it is lit; real leaves differ in more than brightness, glinting
/// pale where they mirror the sky or the sun, yellower where it shines through them, bluer in
/// the shade, each a green of its own. So `foliageColour` is the spread of the leaf pixels'
/// saturation (HSV) in a square, the mean over squares; `flatFoliage` the share of squares
/// where it is under 0.06, a bush or a crown of one flat green; and `foliageLit` the share of a
/// square's leaf pixels brighter than one and a half times its median leaf, a crown's lit top
/// and the leaves that catch the light against its darker inside, the mean. Brightness alone in
/// small squares does not tell them apart: flat leaves' dark gaps between them give them as much
/// local contrast as real ones, and the references' frames are softened by their films'
/// compression. None of the three where a picture has fewer than `LEAFY_SQUARES` such squares,
/// too few to say (open tundra, a bare street).
export const LEAFY_SQUARES = 6;
export function foliage(p: Picture, masked: (x: number, y: number) => boolean = () => false): { flatFoliage?: number; foliageColour?: number; foliageLit?: number } {
  const { width: W, height: H, rgba } = p;
  const line = skyline(p);
  let squares = 0, flatSquares = 0, colour = 0, lit = 0;
  const sat: number[] = [], luma: number[] = [];
  for (let by = 0; by + 64 <= H; by += 64) {
    for (let bx = 0; bx + 64 <= W; bx += 64) {
      sat.length = 0;
      luma.length = 0;
      for (let y = by; y < by + 64; y++) {
        for (let x = bx; x < bx + 64; x++) {
          if (masked(x, y) || y < line[x]) continue;
          const i = (y * W + x) * 4;
          const [r, g, b] = [rgba[i] / 255, rgba[i + 1] / 255, rgba[i + 2] / 255];
          if (!leafy(r, g, b)) continue;
          sat.push(hsv(r, g, b)[1]);
          luma.push(0.2126 * r + 0.7152 * g + 0.0722 * b);
        }
      }
      if (sat.length < 0.4 * 64 * 64) continue;
      const m = sat.reduce((s, v) => s + v, 0) / sat.length;
      const spread = Math.sqrt(sat.reduce((s, v) => s + (v - m) ** 2, 0) / sat.length);
      const median = [...luma].sort((a, c) => a - c)[luma.length >> 1];
      squares++;
      colour += spread;
      if (spread < 0.06) flatSquares++;
      lit += luma.filter((v) => v > 1.5 * median).length / luma.length;
    }
  }
  if (squares < LEAFY_SQUARES) return {};
  return { flatFoliage: flatSquares / squares, foliageColour: colour / squares, foliageLit: lit / squares };
}

/// Hue in degrees, saturation (chroma over value) and value of sRGB values 0 to 1, as HSV has
/// them.
function hsv(r: number, g: number, b: number): [number, number, number] {
  const high = Math.max(r, g, b), d = high - Math.min(r, g, b);
  if (d === 0) return [0, 0, high];
  const h = high === r ? 60 * (((g - b) / d + 6) % 6) : high === g ? 60 * ((b - r) / d + 2) : 60 * ((r - g) / d + 4);
  return [h, d / high, high];
}

/// Which pixels of a picture are water, from the same view shot again with the lakes drawn
/// flat magenta (the sandbox's `--water-mask`, D429): those that turned magenta, by what
/// draws them rather than by their colour. Magenta in both pictures, a flower's, is not water.
export function waterMask(p: Picture, masked: Picture): (x: number, y: number) => boolean {
  const { width: W, height: H } = p;
  if (masked.width !== W || masked.height !== H) throw new Error("the water's mask is not the picture's size");
  const water = new Uint8Array(W * H);
  const magenta = (rgba: Uint8Array, i: number) => rgba[i] + rgba[i + 2] - 2 * rgba[i + 1];
  for (let i = 0; i < W * H; i++) {
    const m = masked.rgba;
    if (m[i * 4] - m[i * 4 + 1] > 40 && m[i * 4 + 2] - m[i * 4 + 1] > 40 && magenta(m, i * 4) - magenta(p.rgba, i * 4) > 80) water[i] = 1;
  }
  return (x, y) => water[y * W + x] === 1;
}

/// Debug tints (docs/art-style.md, "Nothing reads as a debug tint"). `tint`: of the 8-pixel
/// squares below the skyline, the share that are a saturated fill: their mean colour saturated
/// past 0.45 and brighter than 0.3 in HSV, and their brightness smooth, spread under 4% of it,
/// as orange rocks and a lime log were. Real saturated things, autumn leaves, flowers, painted
/// cloth, are textured, and water's sky blue (195 to 245 degrees) is never a fill, nor is any
/// square with `water` in it: the water as the game says it drew it (`waterMask`, D429), so
/// a lake's smooth teal is not taken for a fill, while a teal fill on land still is. `lilac`: the
/// share of the picture that is lilac, a hue from 255 to 335 degrees saturated past 0.12 and
/// brighter than 0.25, which nothing in our places' references is but a flower.
export function tints(p: Picture, masked: (x: number, y: number) => boolean = () => false, water: (x: number, y: number) => boolean = () => false): { tint: number; lilac: number } {
  const { width: W, height: H, rgba } = p;
  let n = 0, lilac = 0;
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      if (masked(x, y)) continue;
      const i = (y * W + x) * 4;
      const [h, s, v] = hsv(rgba[i] / 255, rgba[i + 1] / 255, rgba[i + 2] / 255);
      n++;
      if (h >= 255 && h < 335 && s > 0.12 && v > 0.25) lilac++;
    }
  }
  const line = skyline(p);
  let squares = 0, fills = 0;
  for (let by = 0; by + 8 <= H; by += 8) {
    for (let bx = 0; bx + 8 <= W; bx += 8) {
      let r = 0, g = 0, b = 0, m = 0, m2 = 0, out = false, wet = false;
      for (let y = by; y < by + 8 && !out; y++) {
        for (let x = bx; x < bx + 8; x++) {
          if (masked(x, y) || y < line[x]) {
            out = true;
            break;
          }
          wet ||= water(x, y);
          const i = (y * W + x) * 4;
          r += rgba[i];
          g += rgba[i + 1];
          b += rgba[i + 2];
          const v = (0.2126 * rgba[i] + 0.7152 * rgba[i + 1] + 0.0722 * rgba[i + 2]) / 255;
          m += v;
          m2 += v * v;
        }
      }
      if (out) continue;
      squares++;
      const [h, s, v] = hsv(r / (64 * 255), g / (64 * 255), b / (64 * 255));
      m /= 64;
      const spread = Math.sqrt(Math.max(0, m2 / 64 - m * m)) / Math.max(m, 0.02);
      if (!wet && s > 0.45 && v > 0.3 && !(h >= 195 && h < 245) && spread < 0.04) fills++;
    }
  }
  return { tint: squares ? fills / squares : 0, lilac: n ? lilac / n : 0 };
}

/// The brightness of every pixel as its logarithm (Rec. 709 luma of the sRGB values, lifted by
/// 0.02 so black is finite), NaN where masked.
function logLuma(p: Picture, masked: (x: number, y: number) => boolean): Float32Array {
  const { width: W, height: H, rgba } = p;
  const out = new Float32Array(W * H);
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const i = (y * W + x) * 4;
      out[y * W + x] = masked(x, y) ? NaN : Math.log(0.02 + (0.2126 * rgba[i] + 0.7152 * rgba[i + 1] + 0.0722 * rgba[i + 2]) / 255);
    }
  }
  return out;
}

/// Hard shade (docs/art-style.md, "Shade is soft and takes the sky's colour"): of the
/// picture's shading edges, the share that are hard. A shading edge is where brightness at
/// least halves across 24 pixels and the colour stays the same, a shadow or a surface turning
/// from the light, not one thing against another: the mean colours 9 to 12 pixels either side
/// differ by under 0.04 in each of red's and blue's share of the sum, or the dark side is black.
/// Each edge is counted once a row and once a column, where its one-pixel step is the steepest
/// within six, and it is hard when seven tenths of its change happens within four pixels, as a
/// shadow map's crisp edge does and a real penumbra, metres from what casts it, does not.
export function hardEdges(p: Picture, masked: (x: number, y: number) => boolean = () => false): number {
  const { width: W, height: H, rgba } = p;
  const l = logLuma(p, masked);
  let edges = 0, hard = 0;
  const side = (at: (k: number) => number, from: number, to: number) => {
    let r = 0, g = 0, b = 0;
    for (let k = from; k <= to; k++) {
      const i = at(k) * 4;
      r += rgba[i];
      g += rgba[i + 1];
      b += rgba[i + 2];
    }
    return [r, g, b];
  };
  const scan = (length: number, at: (k: number) => number) => {
    for (let k = 12; k < length - 13; k++) {
      const big = l[at(k + 12)] - l[at(k - 11)];
      if (!(Math.abs(big) > Math.LN2)) continue;
      const step = l[at(k + 1)] - l[at(k)];
      if (Math.sign(step) !== Math.sign(big)) continue;
      let steepest = true;
      for (let j = -6; j <= 6 && steepest; j++) {
        if (j === 0) continue;
        const other = Math.abs(l[at(k + j + 1)] - l[at(k + j)]);
        if (other > Math.abs(step) || (j > 0 && other === Math.abs(step))) steepest = false;
      }
      if (!steepest) continue;
      const a = side(at, k - 11, k - 9), c = side(at, k + 10, k + 12);
      const sa = a[0] + a[1] + a[2], sc = c[0] + c[1] + c[2];
      const black = Math.min(sa, sc) < 3 * 3 * 0.03 * 255;
      const same = black || (Math.abs(a[0] / sa - c[0] / sc) < 0.04 && Math.abs(a[2] / sa - c[2] / sc) < 0.04);
      if (!same) continue;
      edges++;
      if (Math.abs(l[at(k + 2)] - l[at(k - 2)]) > 0.7 * Math.abs(big)) hard++;
    }
  };
  for (let y = 0; y < H; y++) scan(W, (k) => y * W + k);
  for (let x = 0; x < W; x++) scan(H, (k) => k * W + x);
  return edges ? hard / edges : 0;
}

/// Facets (docs/art-style.md, "Ground layers blend by height and noise"): the length of long
/// straight colour edges in the lower half, in picture widths, as a ground whose layers change
/// along its triangles shows them. At a quarter size, smoothed twice by a 3 by 3 box, so grass
/// and a texture's grain melt into the patches they make, an edge pixel is where the colour
/// changes by more than 0.06 (the colour's gradient, Di Zenzo's, thinned to its ridge). It is
/// straight when the edge runs on through it in a straight line for a thirty-second of the
/// picture's width each way: edge pixels lying on that line, within a pixel and their direction
/// within 15 degrees, at both its ends and at half its steps or more (tufts crossing it break it
/// in places). A facet's edge does; a plant's or a stone's curves away. Edges within 12 degrees
/// of upright or 20 of level are left out: a trunk, a post, a shore or water's ripples is
/// straight, and a triangle's edge seldom lies so. `marks`, if given (a quarter size), is set 1
/// at each edge pixel looked at and 2 at each straight one, to draw what counted.
export function facets(p: Picture, masked: (x: number, y: number) => boolean = () => false, marks?: Uint8Array): number {
  const S = 4;
  const W = Math.floor(p.width / S), H = Math.floor(p.height / S);
  // A quarter size, each pixel the mean of sixteen; masked ones NaN.
  const c = new Float32Array(W * H * 3);
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      let r = 0, g = 0, b = 0;
      for (let dy = 0; dy < S; dy++) {
        for (let dx = 0; dx < S; dx++) {
          const [sx, sy] = [S * x + dx, S * y + dy];
          const i = (sy * p.width + sx) * 4;
          if (masked(sx, sy)) r = NaN;
          r += p.rgba[i];
          g += p.rgba[i + 1];
          b += p.rgba[i + 2];
        }
      }
      c.set([r / (255 * S * S), g / (255 * S * S), b / (255 * S * S)], (y * W + x) * 3);
    }
  }
  for (let pass = 0; pass < 2; pass++) {
    const was = c.slice();
    for (let y = 1; y < H - 1; y++) {
      for (let x = 1; x < W - 1; x++) {
        for (let ch = 0; ch < 3; ch++) {
          let s = 0;
          for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) s += was[((y + dy) * W + x + dx) * 3 + ch];
          c[(y * W + x) * 3 + ch] = s / 9;
        }
      }
    }
  }
  const first = Math.floor(H / 2);
  const magnitude = new Float32Array(W * H), angle = new Float32Array(W * H);
  for (let y = Math.max(1, first - 2); y < H - 1; y++) {
    for (let x = 1; x < W - 1; x++) {
      let xx = 0, yy = 0, xy = 0;
      for (let ch = 0; ch < 3; ch++) {
        const v = (dx: number, dy: number) => c[((y + dy) * W + x + dx) * 3 + ch];
        // Sobel, an eighth of it, so a step of d reads d once doubled below.
        const gx = (v(1, -1) + 2 * v(1, 0) + v(1, 1) - v(-1, -1) - 2 * v(-1, 0) - v(-1, 1)) / 8;
        const gy = (v(-1, 1) + 2 * v(0, 1) + v(1, 1) - v(-1, -1) - 2 * v(0, -1) - v(1, -1)) / 8;
        xx += gx * gx;
        yy += gy * gy;
        xy += gx * gy;
      }
      const largest = (xx + yy + Math.sqrt((xx - yy) ** 2 + 4 * xy * xy)) / 2;
      magnitude[y * W + x] = Number.isNaN(largest) ? 0 : 2 * Math.sqrt(largest);
      angle[y * W + x] = 0.5 * Math.atan2(2 * xy, xx - yy);
    }
  }
  // Thinned: kept where the gradient is largest across the edge.
  const edge = new Uint8Array(W * H);
  for (let y = first; y < H - 1; y++) {
    for (let x = 1; x < W - 1; x++) {
      const m = magnitude[y * W + x];
      if (m <= 0.06) continue;
      const [nx, ny] = [Math.round(Math.cos(angle[y * W + x])), Math.round(Math.sin(angle[y * W + x]))];
      if (m >= magnitude[(y + ny) * W + x + nx] && m > magnitude[(y - ny) * W + x - nx]) edge[y * W + x] = 1;
    }
  }
  const reach = Math.max(4, Math.round(W / 32));
  const degrees = Math.PI / 180;
  const turned = (a: number, b: number) => {
    const d = Math.abs(a - b) % Math.PI;
    return Math.min(d, Math.PI - d);
  };
  let straight = 0;
  for (let y = first; y < H - 1; y++) {
    for (let x = 1; x < W - 1; x++) {
      if (!edge[y * W + x]) continue;
      // The gradient's direction: 0 across an upright edge, a right angle across a level one.
      const a = angle[y * W + x];
      if (Math.abs(a) < 12 * degrees || Math.PI / 2 - Math.abs(a) < 20 * degrees) continue;
      // Along the edge, across the gradient.
      const [tx, ty] = [-Math.sin(a), Math.cos(a)];
      const on = (k: number) => {
        for (const off of [0, -1, 1]) {
          const qx = Math.round(x + k * tx + off * Math.cos(a)), qy = Math.round(y + k * ty + off * Math.sin(a));
          if (qx >= 0 && qy >= first && qx < W && qy < H && edge[qy * W + qx] && turned(angle[qy * W + qx], a) < 15 * degrees) return true;
        }
        return false;
      };
      let hits = 0;
      for (let k = 1; k <= reach; k++) hits += (on(k) ? 1 : 0) + (on(-k) ? 1 : 0);
      const ends = (on(reach) || on(reach - 1)) && (on(-reach) || on(1 - reach));
      const long = ends && hits >= reach;
      if (marks) marks[y * W + x] = long ? 2 : 1;
      if (long) straight++;
    }
  }
  return straight / W;
}

/// Flat colour (docs/art-style.md, "Every surface is textured in the style"): of the 16-pixel
/// squares below the skyline that are not near black, the share whose brightness hardly varies,
/// spread under 1.5% of their brightness, as an untextured fill or a debug colour is and a
/// textured material, snow and still water included, seldom is at this size.
export function flat(p: Picture, masked: (x: number, y: number) => boolean = () => false): number {
  const { width: W, height: H, rgba } = p;
  const line = skyline(p);
  let squares = 0, smooth = 0;
  for (let by = 0; by + 16 <= H; by += 16) {
    for (let bx = 0; bx + 16 <= W; bx += 16) {
      let m = 0, m2 = 0, out = false;
      for (let y = by; y < by + 16 && !out; y++) {
        for (let x = bx; x < bx + 16; x++) {
          if (masked(x, y) || y < line[x]) {
            out = true;
            break;
          }
          const i = (y * W + x) * 4;
          const v = (0.2126 * rgba[i] + 0.7152 * rgba[i + 1] + 0.0722 * rgba[i + 2]) / 255;
          m += v;
          m2 += v * v;
        }
      }
      if (out) continue;
      m /= 256;
      if (m < 0.06) continue;
      squares++;
      if (Math.sqrt(Math.max(0, m2 / 256 - m * m)) / m < 0.015) smooth++;
    }
  }
  return squares ? smooth / squares : 0;
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
  const keys = [...new Set(looks.flatMap((l) => Object.keys(l)))] as (keyof Look)[];
  // Each number over the looks that have it: a picture with too few leaves has no flat foliage.
  return Object.fromEntries(
    keys.map((k) => {
      const have = looks.filter((l) => typeof l[k] === "number");
      return [k, have.reduce((s, l) => s + (l[k] as number), 0) / have.length];
    }),
  ) as Look;
}

/// One line for a look: each number, and the reference's beside it (Sons of the Forest's by day
/// unless another is given).
export function line(name: string, l: Look, against: Look = SOTF, called = "SotF"): string {
  const SOTF = against;
  const f = (v: number) => v.toFixed(3);
  const share = (v: number | undefined) => (v === undefined ? "-" : `${(100 * v).toFixed(1)}%`);
  return `${name}: mean ${f(l.mean)} (${called} ${f(SOTF.mean)}), p5 ${f(l.p5)} (${f(SOTF.p5)}), below 0.08 ${(100 * l.dark).toFixed(0)}% (${(100 * SOTF.dark).toFixed(0)}%), red/green ${f(l.redOverGreen)} (${f(SOTF.redOverGreen)}), blue/green ${f(l.blueOverGreen)} (${f(SOTF.blueOverGreen)}), contrast ${f(l.contrast)} (${f(SOTF.contrast)}), lit litter ${(100 * l.litLitter).toFixed(1)}% (${(100 * SOTF.litLitter).toFixed(1)}%), shade red/green ${f(l.shadeRedOverGreen)} (${f(SOTF.shadeRedOverGreen)}), shade blue/green ${f(l.shadeBlueOverGreen)} (${f(SOTF.shadeBlueOverGreen)}), middle blue/green ${f(l.middleBlueOverGreen)} (${f(SOTF.middleBlueOverGreen)}), lime top ${(100 * l.limeTop).toFixed(1)}% (${(100 * SOTF.limeTop).toFixed(1)}%), trunks middle ${share(l.trunkMiddle)} (${share(SOTF.trunkMiddle)}), flat foliage ${share(l.flatFoliage)} (${share(SOTF.flatFoliage)}), leaf colour spread ${l.foliageColour === undefined ? "-" : f(l.foliageColour)} (${SOTF.foliageColour === undefined ? "-" : f(SOTF.foliageColour)}), leaves catching light ${share(l.foliageLit)} (${share(SOTF.foliageLit)})`;
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
