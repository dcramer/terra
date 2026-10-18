// The flames' flipbook (D377): a fire simulated as a fluid and its temperature saved frame
// by frame, as shipped games' fire is drawn from EmberGen's or Houdini's simulated sheets
// (research/2026-10-16-real-time-fire-rendering.md). Jos Stam's stable fluids on a grid
// twice as tall as wide: velocity advected semi-Lagrangian, buoyancy from the heat,
// vorticity confinement for the curls a flame's edge breaks into, made divergence-free by a
// Jacobi pressure solve; heat fed in along a ragged, wandering fuel bed at the foot,
// advected and cooling as it rises. Three fires from three seeds, one a channel (red, green,
// blue), so each card picks one of three; each loops, its second half crossfaded into its
// first. The fire's shader (assets/shaders/fire.wesl) reads a texel's heat as how hot the
// soot there glows and how much of the card it covers.
//
//   node tools/textures/flames.ts [out.png]     # assets/textures/fire/flames.png by default

import { deflateSync } from "node:zlib";
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

/// The grid the fire is simulated on, cells across and up.
const W = 96;
const H = 192;
/// The frames saved, a sheet COLUMNS across, and each frame's size in texels.
const FRAMES = 64;
const COLUMNS = 8;
const FW = 128;
const FH = 256;
/// Steps of the simulation a frame, and a step's length (in the grid's own time).
const SUBSTEPS = 2;
const DT = 0.5;
/// How high the flame's gas burns above its bed at most, in cells, and how fast it makes
/// heat there.
const BODY = Math.round(H * 0.42);
const BURN = 0.35;
/// How much faster the heat cools at the top of the frame than in the body.
const TIPS = 0.22;
/// Steps run before the first frame is kept, so the fire is burning steadily.
const WARM = 240;

/// A small seeded random number source (mulberry32).
function random(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/// Smooth 1D noise from a table of random values, repeating every `n`.
function noise1(table: Float32Array, x: number): number {
  const n = table.length;
  const i = Math.floor(x);
  const f = x - i;
  const a = table[((i % n) + n) % n];
  const b = table[(((i + 1) % n) + n) % n];
  const u = f * f * (3 - 2 * f);
  return a + (b - a) * u;
}

const at = (x: number, y: number) => y * W + x;

/// A field sampled bilinearly at a place in cells, clamped to the grid.
function sample(f: Float32Array, x: number, y: number): number {
  x = Math.min(Math.max(x, 0), W - 1.001);
  y = Math.min(Math.max(y, 0), H - 1.001);
  const i = Math.floor(x);
  const j = Math.floor(y);
  const fx = x - i;
  const fy = y - j;
  const a = f[at(i, j)] + (f[at(i + 1, j)] - f[at(i, j)]) * fx;
  const b = f[at(i, j + 1)] + (f[at(i + 1, j + 1)] - f[at(i, j + 1)]) * fx;
  return a + (b - a) * fy;
}

/// Moves a field along the velocity for a step: each cell takes what was where the flow
/// carried it from.
function advect(f: Float32Array, u: Float32Array, v: Float32Array, out: Float32Array) {
  for (let y = 0; y < H; y++) {
    for (let x = 0; x < W; x++) {
      const k = at(x, y);
      out[k] = sample(f, x - u[k] * DT, y - v[k] * DT);
    }
  }
}

/// One fire, simulated: its heat each frame, `FRAMES * 2` of them.
function simulate(seed: number): Float32Array[] {
  const rand = random(seed);
  const table = new Float32Array(64).map(() => rand());
  const wander = new Float32Array(64).map(() => rand());
  let u = new Float32Array(W * H);
  let v = new Float32Array(W * H);
  let heat = new Float32Array(W * H);
  const scratch = new Float32Array(W * H);
  const scratch2 = new Float32Array(W * H);
  const pressure = new Float32Array(W * H);
  const divergence = new Float32Array(W * H);
  const curl = new Float32Array(W * H);
  const frames: Float32Array[] = [];
  const steps = WARM + FRAMES * 2 * SUBSTEPS;
  for (let step = 0; step < steps; step++) {
    const time = step * DT;
    // The fuel bed: heat fed in over the foot, ragged across and wandering in time, wider
    // in the middle, with a little upward push where it burns hottest.
    for (let x = 0; x < W; x++) {
      const across = (x - W / 2) / (W * 0.4);
      if (Math.abs(across) >= 1) continue;
      // Burning hardest in a few places along it, which wander, so it rises as a few
      // tongues rather than one column.
      const drift = noise1(wander, time * 0.02) * 20;
      const tongues = 0.25 + 0.75 * Math.min(1, Math.max(0, (noise1(table, x * 0.11 + time * 0.035 + drift) - 0.3) / 0.4));
      const bed = (1 - across * across) * tongues * (0.65 + 0.35 * noise1(table, x * 0.3 + time * 0.12 + drift + 7));
      // Above the bed the fuel's gas still burns, as a real flame's continuous region does to
      // about half its height: heat made through a body whose top is ragged and moves, so
      // the flame is solid low down and breaks into licks above.
      const body = BODY * (0.35 + 0.65 * tongues) * (0.6 + 0.4 * noise1(table, x * 0.07 + time * 0.05 + drift + 3));
      for (let y = 1; y < Math.max(9, body); y++) {
        const k = at(x, y);
        if (y < 9) {
          heat[k] = Math.max(heat[k], bed * 1.15);
          v[k] += bed * 0.25 * DT;
        } else {
          // In pockets that rise through it, so the body is never one flat glow.
          const pocket = noise1(wander, y * 0.11 - time * 0.3 + x * 0.06) * noise1(table, x * 0.17 + y * 0.03 + drift + 11);
          heat[k] = Math.min(1.1, heat[k] + bed * (1 - y / body) * BURN * (0.3 + 1.4 * pocket) * DT);
        }
      }
    }
    // Buoyancy: hot gas rises.
    for (let k = 0; k < W * H; k++) v[k] += heat[k] * 0.8 * DT;
    // Vorticity confinement: the curls a flame's edge breaks into, put back where the grid's
    // own smoothing would lose them.
    for (let y = 1; y < H - 1; y++) {
      for (let x = 1; x < W - 1; x++) {
        curl[at(x, y)] = (v[at(x + 1, y)] - v[at(x - 1, y)] - (u[at(x, y + 1)] - u[at(x, y - 1)])) * 0.5;
      }
    }
    for (let y = 2; y < H - 2; y++) {
      for (let x = 2; x < W - 2; x++) {
        const gx = (Math.abs(curl[at(x + 1, y)]) - Math.abs(curl[at(x - 1, y)])) * 0.5;
        const gy = (Math.abs(curl[at(x, y + 1)]) - Math.abs(curl[at(x, y - 1)])) * 0.5;
        const len = Math.hypot(gx, gy) + 1e-5;
        const w = curl[at(x, y)];
        const eps = 0.6;
        u[at(x, y)] += (gy / len) * w * eps * DT;
        v[at(x, y)] -= (gx / len) * w * eps * DT;
      }
    }
    // Advect the velocity by itself.
    advect(u, u, v, scratch);
    advect(v, u, v, scratch2);
    u.set(scratch);
    v.set(scratch2);
    // Make it divergence-free: open at the top and sides, closed at the foot.
    for (let y = 1; y < H - 1; y++) {
      for (let x = 1; x < W - 1; x++) {
        divergence[at(x, y)] = -0.5 * (u[at(x + 1, y)] - u[at(x - 1, y)] + v[at(x, y + 1)] - v[at(x, y - 1)]);
      }
    }
    pressure.fill(0);
    for (let iteration = 0; iteration < 40; iteration++) {
      for (let y = 1; y < H - 1; y++) {
        for (let x = 1; x < W - 1; x++) {
          pressure[at(x, y)] = (divergence[at(x, y)] + pressure[at(x - 1, y)] + pressure[at(x + 1, y)] + pressure[at(x, y - 1)] + pressure[at(x, y + 1)]) * 0.25;
        }
      }
      for (let x = 0; x < W; x++) pressure[at(x, 0)] = pressure[at(x, 1)];
    }
    for (let y = 1; y < H - 1; y++) {
      for (let x = 1; x < W - 1; x++) {
        u[at(x, y)] -= 0.5 * (pressure[at(x + 1, y)] - pressure[at(x - 1, y)]);
        v[at(x, y)] -= 0.5 * (pressure[at(x, y + 1)] - pressure[at(x, y - 1)]);
      }
    }
    for (let x = 0; x < W; x++) v[at(x, 0)] = 0;
    // The heat carried with the gas and cooling as it goes, more where it is thin, so the
    // flame's tips break into licks that wink out; and past the burning body faster the
    // higher it rises, as soot there no longer glows for long, so the tips break off as
    // flamelets and are gone by about three quarters of the frame.
    advect(heat, u, v, scratch);
    for (let k = 0; k < W * H; k++) {
      const h = scratch[k];
      const high = Math.min(1, Math.max(0, (Math.floor(k / W) / H - 0.3) / 0.45));
      heat[k] = Math.max(0, h - DT * (0.012 + 0.015 * h + TIPS * high * high));
    }
    // Damp what is left near the sides, so no flame reaches a card's edge.
    for (let y = 0; y < H; y++) {
      for (let x = 0; x < W; x++) {
        const edge = Math.min(x, W - 1 - x, H - 1 - y) / 8;
        if (edge < 1) {
          const k = at(x, y);
          heat[k] *= edge;
          u[k] *= 0.9 + 0.1 * edge;
          v[k] *= 0.9 + 0.1 * edge;
        }
      }
    }
    if (step >= WARM && (step - WARM) % SUBSTEPS === SUBSTEPS - 1) frames.push(heat.slice());
  }
  return frames;
}

/// The frames looped: the second half crossfaded into the first, so the last runs on into
/// the first.
function looped(frames: Float32Array[]): Float32Array[] {
  const out: Float32Array[] = [];
  for (let i = 0; i < FRAMES; i++) {
    const w = i / FRAMES;
    const a = frames[FRAMES + i];
    const b = frames[i];
    out.push(a.map((value, k) => value * (1 - w) + b[k] * w));
  }
  return out;
}

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

/// How far up its frame a fire reaches: where it stops being solid (the height at which, on
/// average over its frames, fewer than half as many cells glow as at its widest), and where
/// its tips end (the highest row any cell glows in on one frame in ten); each of the frame's
/// height. The shader sizes its cards on these (`fire.wesl`: a flame fills about the lower
/// two thirds of its card).
function reach(frames: Float32Array[]): [number, number] {
  const glows = 0.25;
  const rows = new Float32Array(H);
  const highest: number[] = [];
  for (const f of frames) {
    let top = 0;
    for (let y = 0; y < H; y++) {
      let n = 0;
      for (let x = 0; x < W; x++) if (f[at(x, y)] > glows) n++;
      rows[y] += n / frames.length;
      if (n > 0) top = y;
    }
    highest.push(top);
  }
  const widest = Math.max(...rows);
  let solid = 0;
  for (let y = 0; y < H; y++) if (rows[y] >= widest * 0.5) solid = y;
  highest.sort((a, b) => a - b);
  return [solid / H, highest[Math.floor(highest.length * 0.9)] / H];
}

function main() {
  const out = process.argv[2] ?? "assets/textures/fire/flames.png";
  const fires = [11, 23, 37].map((seed) => looped(simulate(seed)));
  const rows = FRAMES / COLUMNS;
  const width = COLUMNS * FW;
  const height = rows * FH;
  const rgba = new Uint8Array(width * height * 4);
  for (let f = 0; f < FRAMES; f++) {
    const ox = (f % COLUMNS) * FW;
    const oy = Math.floor(f / COLUMNS) * FH;
    for (let y = 0; y < FH; y++) {
      for (let x = 0; x < FW; x++) {
        // The frame's top row is the fire's top: the grid's y runs up.
        const gx = (x + 0.5) * (W / FW) - 0.5;
        const gy = (FH - 1 - y + 0.5) * (H / FH) - 0.5;
        const k = ((oy + y) * width + ox + x) * 4;
        for (let c = 0; c < 3; c++) {
          rgba[k + c] = Math.round(Math.min(1, Math.max(0, sample(fires[c][f], gx, gy))) * 255);
        }
        rgba[k + 3] = 255;
      }
    }
  }
  mkdirSync(dirname(out), { recursive: true });
  writeFileSync(out, png(width, height, rgba));
  const shape = fires.map(reach).map(([solid, tips]) => `${solid.toFixed(2)}/${tips.toFixed(2)}`);
  console.log(`flames: ${out}, ${FRAMES} frames of ${FW}x${FH}, three fires, ${width}x${height}; solid to/tips to (of the frame's height): ${shape.join(", ")}`);
}

main();
