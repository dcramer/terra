// The iso stills' measures (iso-art, judging): luma percentiles, fine detail, saturation and
// the dark, mid and bright shares, each still at 960x540 so frames of any size compare at one
// size. Ported from the judges' stats.py (numpy and Pillow) to Node, no dependencies: a PNG is
// decoded with node:zlib, any other image through ffmpeg; the downscale is Pillow's Lanczos
// (its Resample.c: the same coefficients, fixed point and two passes, horizontal first), so a
// PNG's numbers are stats.py's to the fourth place.
//
//   node tools/iso-art/stats.ts <out.json> <image>...   (an image may be name=path: copied
//   beside out.json as name.<ext> first, measured under that name)

import { copyFileSync, existsSync, readFileSync, writeFileSync } from "node:fs";
import { basename, dirname, extname, join } from "node:path";
import { execFileSync } from "node:child_process";
import { inflateSync } from "node:zlib";

export type Rgb = { width: number; height: number; data: Uint8Array }; // 3 bytes a pixel

/** A PNG's pixels as 8-bit RGB (alpha dropped, as Pillow's convert("RGB")). */
export function decodePng(bytes: Buffer): Rgb {
  if (bytes.readUInt32BE(0) !== 0x89504e47) throw new Error("not a PNG");
  let at = 8;
  let width = 0, height = 0, depth = 0, type = 0, interlace = 0;
  const idat: Buffer[] = [];
  while (at < bytes.length) {
    const len = bytes.readUInt32BE(at);
    const kind = bytes.toString("latin1", at + 4, at + 8);
    const body = bytes.subarray(at + 8, at + 8 + len);
    if (kind === "IHDR") {
      width = body.readUInt32BE(0);
      height = body.readUInt32BE(4);
      [depth, type, interlace] = [body[8], body[9], body[12]];
    } else if (kind === "IDAT") idat.push(body);
    else if (kind === "IEND") break;
    at += 12 + len;
  }
  const channels = { 0: 1, 2: 3, 4: 2, 6: 4 }[type];
  if (depth !== 8 || !channels || interlace) {
    throw new Error(`PNG of bit depth ${depth}, colour type ${type}, interlace ${interlace}: stats.ts reads 8-bit grey, RGB or RGBA, not interlaced; convert it first (ffmpeg -i in.png -pix_fmt rgb24 out.png)`);
  }
  const raw = inflateSync(Buffer.concat(idat));
  const stride = width * channels;
  const px = new Uint8Array(stride * height);
  for (let y = 0; y < height; y++) {
    const filter = raw[y * (stride + 1)];
    const line = raw.subarray(y * (stride + 1) + 1, (y + 1) * (stride + 1));
    const out = y * stride;
    for (let i = 0; i < stride; i++) {
      const a = i >= channels ? px[out + i - channels] : 0;
      const b = y > 0 ? px[out - stride + i] : 0;
      const c = i >= channels && y > 0 ? px[out - stride + i - channels] : 0;
      let pred = 0;
      if (filter === 1) pred = a;
      else if (filter === 2) pred = b;
      else if (filter === 3) pred = (a + b) >> 1;
      else if (filter === 4) {
        const p = a + b - c;
        const [pa, pb, pc] = [Math.abs(p - a), Math.abs(p - b), Math.abs(p - c)];
        pred = pa <= pb && pa <= pc ? a : pb <= pc ? b : c;
      }
      px[out + i] = (line[i] + pred) & 255;
    }
  }
  const data = new Uint8Array(width * height * 3);
  for (let p = 0; p < width * height; p++) {
    for (let k = 0; k < 3; k++) data[p * 3 + k] = px[p * channels + (channels < 3 ? 0 : k)];
  }
  return { width, height, data };
}

/** Any image ffmpeg reads, as 8-bit RGB (its JPEG decoder may differ from libjpeg's by a level). */
export function decodeAny(path: string): Rgb {
  if (extname(path).toLowerCase() === ".png") return decodePng(readFileSync(path));
  const probe = execFileSync("ffprobe", ["-v", "error", "-select_streams", "v:0", "-show_entries", "stream=width,height", "-of", "csv=p=0", path]).toString().trim();
  const [width, height] = probe.split(",").map(Number);
  const data = execFileSync("ffmpeg", ["-v", "error", "-i", path, "-frames:v", "1", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"], { maxBuffer: 1 << 28 });
  return { width, height, data: new Uint8Array(data) };
}

// Pillow's Resample.c, LANCZOS: a = 3, support widened by the scale when shrinking.
const PRECISION_BITS = 32 - 8 - 2;
const sinc = (x: number) => (x === 0 ? 1 : Math.sin(x * Math.PI) / (x * Math.PI));
const lanczos = (x: number) => (x >= -3 && x < 3 ? sinc(x) * sinc(x / 3) : 0);

function coeffs(inSize: number, outSize: number) {
  const scale = inSize / outSize;
  const filterscale = Math.max(scale, 1);
  const support = 3 * filterscale;
  const ksize = Math.ceil(support) * 2 + 1;
  const bounds: number[] = [];
  const kk = new Float64Array(outSize * ksize);
  for (let xx = 0; xx < outSize; xx++) {
    const center = (xx + 0.5) * scale;
    const ss = 1 / filterscale;
    const xmin = Math.max(Math.trunc(center - support + 0.5), 0);
    const xmax = Math.min(Math.trunc(center + support + 0.5), inSize) - xmin;
    let ww = 0;
    const k = new Float64Array(xmax);
    for (let x = 0; x < xmax; x++) {
      k[x] = lanczos((x + xmin - center + 0.5) * ss);
      ww += k[x];
    }
    for (let x = 0; x < xmax; x++) {
      const w = ww !== 0 ? k[x] / ww : k[x];
      kk[xx * ksize + x] = w < 0 ? Math.trunc(-0.5 + w * (1 << PRECISION_BITS)) : Math.trunc(0.5 + w * (1 << PRECISION_BITS));
    }
    bounds.push(xmin, xmax);
  }
  return { ksize, bounds, kk };
}

const clip8 = (v: number) => (v >= 1 << PRECISION_BITS << 8 ? 255 : v <= 0 ? 0 : Math.floor(v / (1 << PRECISION_BITS)));

/** Pillow's `resize(size, Image.LANCZOS)` on an RGB image: horizontal pass, then vertical. */
export function resizeLanczos(im: Rgb, width: number, height: number): Rgb {
  const h = coeffs(im.width, width);
  const v = coeffs(im.height, height);
  const first = v.bounds[0];
  const last = v.bounds[height * 2 - 2] + v.bounds[height * 2 - 1];
  const rows = last - first;
  let src = im.data, srcW = im.width;
  if (width !== im.width) {
    const tmp = new Uint8Array(width * rows * 3);
    for (let y = 0; y < rows; y++) {
      for (let xx = 0; xx < width; xx++) {
        const [xmin, xmax] = [h.bounds[xx * 2], h.bounds[xx * 2 + 1]];
        for (let c = 0; c < 3; c++) {
          let ss = 1 << (PRECISION_BITS - 1);
          for (let x = 0; x < xmax; x++) ss += im.data[((y + first) * im.width + x + xmin) * 3 + c] * h.kk[xx * h.ksize + x];
          tmp[(y * width + xx) * 3 + c] = clip8(ss);
        }
      }
    }
    src = tmp;
    srcW = width;
  }
  const out = new Uint8Array(width * height * 3);
  const shift = width !== im.width ? first : 0;
  for (let yy = 0; yy < height; yy++) {
    const [ymin, ymax] = [v.bounds[yy * 2] - shift, v.bounds[yy * 2 + 1]];
    for (let x = 0; x < width; x++) {
      for (let c = 0; c < 3; c++) {
        let ss = 1 << (PRECISION_BITS - 1);
        for (let y = 0; y < ymax; y++) ss += src[((y + ymin) * srcW + x) * 3 + c] * v.kk[yy * v.ksize + y];
        out[(yy * width + x) * 3 + c] = clip8(ss);
      }
    }
  }
  return { width, height, data: out };
}

/** numpy's percentile (linear) of sorted values. */
function percentile(sorted: Float64Array, q: number): number {
  const i = (q / 100) * (sorted.length - 1);
  const lo = Math.floor(i), hi = Math.min(lo + 1, sorted.length - 1);
  return sorted[lo] + (sorted[hi] - sorted[lo]) * (i - lo);
}

export type Stats = Record<"luma_p5" | "luma_p50" | "luma_p95" | "luma_mean" | "fine_detail" | "saturation" | "dark_share" | "mid_share" | "bright_share", number>;

/** stats.py's measures of one image. */
export function measure(im: Rgb): Stats {
  const s = im.width === 960 && im.height === 540 ? im : resizeLanczos(im, 960, 540);
  const [w, h] = [960, 540];
  const lin = (v: number) => (v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
  const L = new Float64Array(w * h);
  let satSum = 0, satN = 0;
  for (let p = 0; p < w * h; p++) {
    const [r, g, b] = [s.data[p * 3] / 255, s.data[p * 3 + 1] / 255, s.data[p * 3 + 2] / 255];
    L[p] = lin(r) * 0.2126 + lin(g) * 0.7152 + lin(b) * 0.0722;
    const mx = Math.max(r, g, b), mn = Math.min(r, g, b);
    if (mx > 0.02) {
      satSum += (mx - mn) / mx;
      satN++;
    }
  }
  let mean = 0, detail = 0, dark = 0, mid = 0, bright = 0;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      let blur = 0;
      for (let dy = -1; dy <= 1; dy++) {
        for (let dx = -1; dx <= 1; dx++) {
          const [yy, xx] = [Math.min(Math.max(y + dy, 0), h - 1), Math.min(Math.max(x + dx, 0), w - 1)];
          blur += L[yy * w + xx];
        }
      }
      const l = L[y * w + x];
      detail += Math.abs(l - blur / 9);
      mean += l;
      if (l < 0.05) dark++;
      else if (l <= 0.35) mid++;
      else bright++;
    }
  }
  const n = w * h;
  mean /= n;
  const sorted = Float64Array.from(L).sort();
  const r = (v: number) => Math.round(v * 1e4) / 1e4;
  return {
    luma_p5: r(percentile(sorted, 5)),
    luma_p50: r(percentile(sorted, 50)),
    luma_p95: r(percentile(sorted, 95)),
    luma_mean: r(mean),
    fine_detail: r(detail / n / mean),
    saturation: r(satSum / satN),
    dark_share: r(dark / n),
    mid_share: r(mid / n),
    bright_share: r(bright / n),
  };
}

function main(argv: string[]) {
  const [out, ...images] = argv;
  if (!out || images.length === 0) {
    console.error("usage: node tools/iso-art/stats.ts <out.json> <image>... (name=path copies the image beside out.json first)");
    process.exit(2);
  }
  const prev: Record<string, Record<string, number>> = existsSync(out) ? JSON.parse(readFileSync(out, "utf8")) : {};
  for (const arg of images) {
    const eq = arg.indexOf("=");
    let name: string, path: string;
    if (eq > 0) {
      name = arg.slice(0, eq);
      path = join(dirname(out), name + extname(arg.slice(eq + 1)));
      copyFileSync(arg.slice(eq + 1), path);
    } else {
      path = arg;
      name = basename(arg, extname(arg));
    }
    prev[name] = { ...prev[name], ...measure(decodeAny(path)) };
  }
  writeFileSync(out, JSON.stringify(prev, null, 1));
  for (const [k, v] of Object.entries(prev)) {
    console.log(k, Object.entries(v).filter(([a]) => a !== "character_height").map(([a, b]) => `${a}=${b}`).join(" "));
  }
}

if (import.meta.main) main(process.argv.slice(2));
