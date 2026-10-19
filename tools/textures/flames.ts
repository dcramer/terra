// The flames' flipbook (D377): real fire, filmed against the night, its frames keyed and laid
// on a sheet, as games draw fire from filmed flipbooks where a simulation cannot match it
// (research/2026-10-18-dragon-breath.md §6). Real flames carry their own colour bands,
// translucency and turbulence: a simulated fire's temperature coloured through a gradient
// stayed a pastel hedge after four rounds of tuning (castle films r7 to r10, 6 to 6.5 of 10).
//
// The footage is Wikimedia Commons' "Fire.ogv" by Cary Bass, released into the public domain
// (https://commons.wikimedia.org/wiki/File:Fire.ogv): a wood fire at night, 480x640 at
// 29.97 frames a second. It is downloaded once into the system's temporary folder; ffmpeg
// decodes it. Each frame is cropped round the fire (the union of its frames' bounds), scaled to
// 160 by 320 and keyed by its brightness (fire against black is light over nothing, so its
// alpha is how bright it is over the black), its colour kept premultiplied. Its loop is its
// last frames crossfaded into its first. Soot is a pass of its own: what was flame a few
// frames before, risen and blurred, where there is no flame now, so dark wisps break off the
// tongues' tips and rise as they do over a burning building. Each texel is the flame's red and
// green (sRGB, premultiplied: blue is worked out from green, `BLUE` below, in the shader), its
// soot (blue) and its cover (alpha).
//
//   node tools/textures/flames.ts [out.png]     # assets/textures/fire/flames.png by default

import { deflateSync } from "node:zlib";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";

const SOURCE = "https://upload.wikimedia.org/wikipedia/commons/8/8f/Fire.ogv";
/// The frames on the sheet, how many across, and each's size in texels.
const FRAMES = 96;
const COLUMNS = 12;
const FW = 160;
const FH = 320;
/// The first frame of the footage used, and how many after the last are crossfaded into the
/// first so it loops.
const START = 40;
const LOOP = 18;
/// The footage's black and the brightness at which a flame covers wholly (sRGB, weighted as
/// the eye weighs it): a dull red rim is see-through, as a thin flame is, an orange one mostly
/// covers and a yellow one wholly.
const BLACK = 0.07;
const SOLID = 0.75;
/// The soot pass: how many frames back it looks, how far the soot rises a frame (of the
/// frame's height), and how dark.
const SOOT_BACK = [3, 6, 10, 15];
const SOOT_RISE = 0.006;
const SOOT = 1.6;

/// A PNG of 8-bit RGBA texels.
function png(width: number, height: number, rgba: Uint8Array): Buffer {
  const crcTable = new Uint32Array(256).map((_, n) => {
    let c = n;
    for (let k = 0; k < 8; k++) c = c & 1 ? 0xedb88320 ^ (c >>> 1) : c >>> 1;
    return c >>> 0;
  });
  const crc = (bytes: Buffer) => {
    let c = 0xffffffff;
    for (const b of bytes) c = crcTable[(c ^ b) & 0xff] ^ (c >>> 8);
    return (c ^ 0xffffffff) >>> 0;
  };
  const chunk = (type: string, data: Buffer) => {
    const length = Buffer.alloc(4);
    length.writeUInt32BE(data.length);
    const body = Buffer.concat([Buffer.from(type, "ascii"), data]);
    const sum = Buffer.alloc(4);
    sum.writeUInt32BE(crc(body));
    return Buffer.concat([length, body, sum]);
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header[8] = 8;
  header[9] = 6;
  const raw = Buffer.alloc((width * 4 + 1) * height);
  for (let y = 0; y < height; y++) {
    raw[y * (width * 4 + 1)] = 0;
    Buffer.from(rgba.buffer, y * width * 4, width * 4).copy(raw, y * (width * 4 + 1) + 1);
  }
  return Buffer.concat([
    Buffer.from([0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]),
    chunk("IHDR", header),
    chunk("IDAT", deflateSync(raw, { level: 9 })),
    chunk("IEND", Buffer.alloc(0)),
  ]);
}

/// The footage, downloaded once.
async function footage(): Promise<string> {
  const path = join(tmpdir(), "terra-fire-footage.ogv");
  if (!existsSync(path)) {
    const reply = await fetch(SOURCE, { headers: { "User-Agent": "terra-tools/1.0" } });
    if (!reply.ok) throw new Error(`flames: ${SOURCE} answered ${reply.status}`);
    writeFileSync(path, Buffer.from(await reply.arrayBuffer()));
  }
  return path;
}

/// So many frames from a frame on, RGB bytes, through ffmpeg's filter.
function decode(video: string, from: number, count: number, filter: string, w: number, h: number): Uint8Array[] {
  const raw = execFileSync(
    "ffmpeg",
    ["-v", "error", "-i", video, "-vf", `trim=start_frame=${from}:end_frame=${from + count},${filter}`, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
    { maxBuffer: 1 << 30 },
  );
  const size = w * h * 3;
  if (raw.length < size * count) throw new Error(`flames: ffmpeg gave ${Math.floor(raw.length / size)} frames, wanted ${count}`);
  return Array.from({ length: count }, (_, i) => raw.subarray(i * size, (i + 1) * size));
}

const smooth = (x: number) => {
  const t = Math.min(1, Math.max(0, x));
  return t * t * (3 - 2 * t);
};

/// How much a pixel of so bright a colour covers.
const cover = (r: number, g: number, b: number) => smooth((0.3 * r + 0.6 * g + 0.1 * b - BLACK) / (SOLID - BLACK));

/// The crop round the fire, of the frame's aspect: the union of every frame's bounds (where
/// it covers more than half: its glowing bed is left out), its foot at the bounds' foot, in
/// the footage's pixels.
function crop(video: string, count: number): string {
  const [w, h] = [480, 640];
  const frames = decode(video, START, count, "null", w, h);
  let [x0, x1, y0, y1] = [w, 0, h, 0];
  for (const f of frames)
    for (let y = 0; y < h; y++)
      for (let x = 0; x < w; x++) {
        const k = (y * w + x) * 3;
        if (cover(f[k] / 255, f[k + 1] / 255, f[k + 2] / 255) > 0.5) {
          x0 = Math.min(x0, x);
          x1 = Math.max(x1, x);
          y0 = Math.min(y0, y);
          y1 = Math.max(y1, y);
        }
      }
  const ch = Math.min(h, y1 - y0 + 8);
  const cw = Math.round((ch * FW) / FH);
  const cx = Math.max(0, Math.min(w - cw, Math.round((x0 + x1 - cw) / 2)));
  const cy = Math.max(0, Math.min(h - Math.round(ch), y1 + 4 - Math.round(ch)));
  return `crop=${cw}:${Math.round(ch)}:${cx}:${cy}`;
}

/// A box blur of a frame's cover, so far each way.
function blur(a: Float32Array, r: number): Float32Array {
  const out = new Float32Array(a.length);
  const tmp = new Float32Array(a.length);
  for (let y = 0; y < FH; y++)
    for (let x = 0; x < FW; x++) {
      let s = 0;
      for (let d = -r; d <= r; d++) s += a[y * FW + Math.min(FW - 1, Math.max(0, x + d))];
      tmp[y * FW + x] = s / (2 * r + 1);
    }
  for (let y = 0; y < FH; y++)
    for (let x = 0; x < FW; x++) {
      let s = 0;
      for (let d = -r; d <= r; d++) s += tmp[Math.min(FH - 1, Math.max(0, y + d)) * FW + x];
      out[y * FW + x] = s / (2 * r + 1);
    }
  return out;
}

async function main() {
  const out = process.argv[2] ?? "assets/textures/fire/flames.png";
  const video = await footage();
  const count = FRAMES + LOOP;
  const box = crop(video, count);
  const src = decode(video, START, count, `${box},scale=${FW}:${FH}:flags=area`, FW, FH);
  // Looped: the frames past the last crossfaded into the first.
  const frames = Array.from({ length: FRAMES }, (_, f) => {
    const px = new Float32Array(FW * FH * 3);
    const w = f < LOOP ? f / LOOP : 1;
    for (let i = 0; i < px.length; i++) px[i] = (f < LOOP ? src[f][i] * w + src[f + FRAMES][i] * (1 - w) : src[f][i]) / 255;
    return px;
  });
  const covers = frames.map((px) => {
    const a = new Float32Array(FW * FH);
    for (let i = 0; i < a.length; i++) a[i] = cover(px[i * 3], px[i * 3 + 1], px[i * 3 + 2]);
    return a;
  });
  const blurred = covers.map((a) => blur(a, 4));
  const rows = FRAMES / COLUMNS;
  const width = COLUMNS * FW;
  const height = rows * FH;
  const rgba = new Uint8Array(width * height * 4);
  const byte = (x: number) => Math.round(Math.min(1, Math.max(0, x)) * 255);
  // How well blue goes as green does among the flame's texels: b = BLUE_K * g^BLUE_P.
  const fit = { n: 0, err: 0 };
  let [sooty, covered, yellow] = [0, 0, 0];
  for (let f = 0; f < FRAMES; f++) {
    const ox = (f % COLUMNS) * FW;
    const oy = Math.floor(f / COLUMNS) * FH;
    const px = frames[f];
    const a = covers[f];
    for (let y = 0; y < FH; y++) {
      for (let x = 0; x < FW; x++) {
        const i = y * FW + x;
        // What was flame a few frames before, risen, where there is none now; more of it
        // higher up, where the gas has cooled.
        let s = 0;
        for (const [n, back] of SOOT_BACK.entries()) {
          const ry = Math.round(y + back * SOOT_RISE * FH);
          if (ry >= FH) continue;
          s = Math.max(s, blurred[(f - back + FRAMES * 4) % FRAMES][ry * FW + x] * (1 - n / (SOOT_BACK.length + 1)));
        }
        const high = smooth((FH - y) / FH / 0.55);
        const soot = Math.max(0, s * SOOT - a[i] * 1.6) * high;
        const k = ((oy + y) * width + ox + x) * 4;
        // Faded out at the frame's sides, where the crop cuts the widest tongues.
        const side = smooth(Math.min(x, FW - 1 - x) / 10);
        a[i] *= side;
        const lift = (c: number) => (side * Math.max(0, c - BLACK * (1 - a[i]))) / (1 - BLACK);
        const [r, g, b] = [lift(px[i * 3]), lift(px[i * 3 + 1]), lift(px[i * 3 + 2])];
        rgba[k] = byte(r);
        rgba[k + 1] = byte(g);
        rgba[k + 2] = byte(soot);
        rgba[k + 3] = byte(a[i]);
        if (a[i] > 0.3) {
          covered++;
          if (g > 0.7 * r) yellow++;
          fit.n++;
          fit.err += (b - BLUE_K * g ** BLUE_P) ** 2;
        }
        if (soot > 0.25) sooty++;
      }
    }
  }
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, png(width, height, rgba));
  const share = (n: number, of: number) => ((100 * n) / of).toFixed(1);
  console.log(
    `flames: ${out}, ${FRAMES} frames of ${FW}x${FH} on ${width}x${height} from ${box}; flame covers ${share(covered, FRAMES * FW * FH)}% of texels, ${share(yellow, covered)}% of it yellow; soot ${share(sooty, FRAMES * FW * FH)}%; blue from green off by ${Math.sqrt(fit.err / Math.max(1, fit.n)).toFixed(3)} rms`,
  );
}

/// Blue as the shader works it out from green (`fire.wesl`, FOOTAGE_BLUE), sRGB: a flame's blue
/// rises only as its green nears white. Fitted over this footage's flame: 0.11 off, rms.
const BLUE_K = 0.23;
const BLUE_P = 4.0;

await main();
