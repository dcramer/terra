// Bakes one billow of smoke without a sim (a recipe with `billow`, blaze-smoke; docs/fire.md,
// "The smoke"): lumps on lumps of soot, each a pyroclastic sphere, a sphere whose radius is
// pushed out by turbulence (the sum of |noise| octaves), which creases between its bumps as a
// cauliflower does (Magnus Wrenninge, "Production Volume Rendering", SIGGRAPH 2011 course notes,
// pyroclastic noise; Ken Perlin's improved noise, 2002). Mantaflow at a size the worker bakes in
// half an hour gave a fuzzy, evenly lit ball (castle round 7).
//
// The density is voxelised a frame at a time, and lit as bake.py lights a six-way set: single
// scatter, albedo 1, from six suns along the axes, each sun's light reaching a voxel through
// the smoke toward it (a sweep along its axis), seen by an orthographic camera looking +Y. So it
// writes bake.py's raw passes (light0, light1, glow) and meta.json, and smoke.ts packs them as
// it packs a sim's: a white Lambert surface facing a sun is 1, thin smoke lit from one side 1/4.
// Over the frames the billow ages: its lumps move out and grow, their bumps roll up over them,
// its skin softens and it thins; the young core is hot (glow).

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Recipe } from "./bake.ts";

export type BillowRecipe = {
  seed: number;
  /// Lumps round the middle one.
  lumps: number;
  /// Extinction a metre of the young core's smoke.
  density: number;
  /// How far turbulence pushes a lump's surface out, as a share of its radius, and how many
  /// bumps round it (the noise's frequency over the unit sphere).
  amplitude: number;
  frequency: number;
};

// Perlin's improved noise (2002), its permutation shuffled by the recipe's seed.
const PERM = new Uint8Array(512);
function mulberry(seed: number) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
function shuffle(rand: () => number) {
  const p = Array.from({ length: 256 }, (_, i) => i);
  for (let i = 255; i > 0; i--) {
    const j = Math.floor(rand() * (i + 1));
    [p[i], p[j]] = [p[j], p[i]];
  }
  for (let i = 0; i < 512; i++) PERM[i] = p[i & 255];
}
const fade = (t: number) => t * t * t * (t * (t * 6 - 15) + 10);
const lerp = (t: number, a: number, b: number) => a + t * (b - a);
function grad(h: number, x: number, y: number, z: number) {
  const k = h & 15;
  const u = k < 8 ? x : y;
  const v = k < 4 ? y : k === 12 || k === 14 ? x : z;
  return (k & 1 ? -u : u) + (k & 2 ? -v : v);
}
export function noise(x: number, y: number, z: number): number {
  const [X, Y, Z] = [Math.floor(x) & 255, Math.floor(y) & 255, Math.floor(z) & 255];
  x -= Math.floor(x);
  y -= Math.floor(y);
  z -= Math.floor(z);
  const [u, v, w] = [fade(x), fade(y), fade(z)];
  const A = PERM[X] + Y;
  const AA = PERM[A] + Z;
  const AB = PERM[A + 1] + Z;
  const B = PERM[X + 1] + Y;
  const BA = PERM[B] + Z;
  const BB = PERM[B + 1] + Z;
  return lerp(
    w,
    lerp(v, lerp(u, grad(PERM[AA], x, y, z), grad(PERM[BA], x - 1, y, z)), lerp(u, grad(PERM[AB], x, y - 1, z), grad(PERM[BB], x - 1, y - 1, z))),
    lerp(v, lerp(u, grad(PERM[AA + 1], x, y, z - 1), grad(PERM[BA + 1], x - 1, y, z - 1)), lerp(u, grad(PERM[AB + 1], x, y - 1, z - 1), grad(PERM[BB + 1], x - 1, y - 1, z - 1))),
  );
}
/// Turbulence: |noise| summed over octaves, about 0 to 1.
function turbulence(x: number, y: number, z: number, octaves: number) {
  let [s, a, f, norm] = [0, 1, 1, 0];
  for (let o = 0; o < octaves; o++) {
    s += a * Math.abs(noise(x * f, y * f, z * f));
    norm += a;
    a *= 0.5;
    f *= 2.03;
  }
  return Math.min(1, (2.2 * s) / norm);
}

/// A lump at one age: its middle, radius, and its surface's push by direction, a latitude-
/// longitude table.
type Lump = { c: [number, number, number]; r: number; reach: number; table: Float32Array };
const TW = 256;
const TH = 128;

function lumpsAt(b: BillowRecipe, age: number): Lump[] {
  const rand = mulberry(b.seed * 7919 + 1);
  const main = 0.5 * (1 + 0.15 * age);
  const out: Lump[] = [];
  const add = (c: [number, number, number], r: number, k: number) => {
    // Its bumps roll up over it as it ages, and grow.
    const roll = age * 0.9;
    const [cr, sr] = [Math.cos(roll), Math.sin(roll)];
    const amp = b.amplitude * (1 + 0.35 * age);
    const table = new Float32Array(TW * TH);
    let most = 0;
    for (let v = 0; v < TH; v++) {
      const th = ((v + 0.5) / TH) * Math.PI;
      for (let u = 0; u < TW; u++) {
        const ph = (u / TW) * 2 * Math.PI;
        const d = [Math.sin(th) * Math.cos(ph), Math.sin(th) * Math.sin(ph), Math.cos(th)];
        // Turned about X: the front's bumps climb toward the top.
        const y = d[1] * cr - d[2] * sr;
        const z = d[1] * sr + d[2] * cr;
        const f = b.frequency;
        const t = turbulence(d[0] * f + k * 17.3, y * f + k * 5.1, z * f - age * 0.4, 5);
        const push = 1 + amp * t;
        table[v * TW + u] = push;
        most = Math.max(most, push);
      }
    }
    out.push({ c, r, reach: r * most, table });
  };
  add([0, 0, 0], main, 0);
  for (let k = 1; k <= b.lumps; k++) {
    const az = rand() * 2 * Math.PI;
    const el = -0.25 + rand() * 1.35;
    const dist = main * (0.6 + 0.2 * rand()) * (1 + 0.35 * age);
    const r = main * (0.42 + 0.3 * rand()) * (1 + 0.3 * age);
    const c: [number, number, number] = [dist * Math.cos(el) * Math.cos(az), dist * Math.cos(el) * Math.sin(az), dist * Math.sin(el)];
    add(c, r, k);
  }
  return out;
}

function push(l: Lump, dx: number, dy: number, dz: number, d: number) {
  const th = Math.acos(Math.max(-1, Math.min(1, dz / d)));
  let ph = Math.atan2(dy, dx);
  if (ph < 0) ph += 2 * Math.PI;
  const fu = (ph / (2 * Math.PI)) * TW;
  const fv = Math.max(0, Math.min(TH - 1.001, (th / Math.PI) * TH - 0.5));
  const [u0, v0] = [Math.floor(fu), Math.floor(fv)];
  const [tu, tv] = [fu - u0, fv - v0];
  const [ua, ub] = [u0 % TW, (u0 + 1) % TW];
  const t = l.table;
  const top = t[v0 * TW + ua] * (1 - tu) + t[v0 * TW + ub] * tu;
  const bot = t[(v0 + 1) * TW + ua] * (1 - tu) + t[(v0 + 1) * TW + ub] * tu;
  return top * (1 - tv) + bot * tv;
}

const F32 = new Float32Array(1);
const U32 = new Uint32Array(F32.buffer);
/// A float as IEEE 754 half's bits (pack.ts's `halves` reads them back).
export function half(v: number): number {
  F32[0] = v;
  const x = U32[0];
  const sign = (x >>> 16) & 0x8000;
  const e = ((x >>> 23) & 0xff) - 127 + 15;
  const m = x & 0x7fffff;
  if (e <= 0) {
    if (e < -10) return sign;
    return sign + (((m | 0x800000) >> (1 - e)) + 0x1000 >> 13);
  }
  if (e >= 31) return sign | 0x7c00;
  return sign + (e << 10) + ((m + 0x1000) >> 13);
}

/// Bakes the billow's raw passes into `out` as bake.py would.
export function bakeBillow(r: Recipe & { billow: BillowRecipe }, out: string) {
  const t0 = Date.now();
  const b = r.billow;
  shuffle(mulberry(b.seed));
  const n = r.size;
  const NX = n;
  const NZ = n;
  const NY = Math.round(n * 0.6);
  const e = r.camera.extent;
  const [dx, dy] = [e / NX, e / NY];
  const at = (i: number, N: number) => -e / 2 + ((i + 0.5) / N) * e;
  const V = NX * NY * NZ;
  const idx = (x: number, y: number, z: number) => (z * NY + y) * NX + x;
  const rho = new Float32Array(V);
  const heat = new Float32Array(V);
  const T = new Float32Array(V);
  const raw = new Uint16Array(r.frames * 3 * n * n * 3);
  for (let f = 0; f < r.frames; f++) {
    const age = f / Math.max(1, r.frames - 1);
    rho.fill(0);
    heat.fill(0);
    // Its skin softens and it thins as it ages.
    const soft = 0.05 + 0.22 * age;
    const thick = 1 - 0.55 * age;
    const lumps = lumpsAt(b, age);
    for (const l of lumps) {
      const lo = (c: number, N: number, d: number) => Math.max(0, Math.floor((c - l.reach + e / 2) / d));
      const hi = (c: number, N: number, d: number) => Math.min(N - 1, Math.ceil((c + l.reach + e / 2) / d));
      for (let z = lo(l.c[2], NZ, dx); z <= hi(l.c[2], NZ, dx); z++) {
        const pz = at(z, NZ) - l.c[2];
        for (let y = lo(l.c[1], NY, dy); y <= hi(l.c[1], NY, dy); y++) {
          const py = at(y, NY) - l.c[1];
          for (let x = lo(l.c[0], NX, dx); x <= hi(l.c[0], NX, dx); x++) {
            const px = at(x, NX) - l.c[0];
            const d = Math.hypot(px, py, pz);
            if (d >= l.reach || d < 1e-6) continue;
            const s = l.r * push(l, px, py, pz, d);
            const v = thick * Math.min(1, Math.max(0, (s - d) / soft));
            const i = idx(x, y, z);
            if (v > rho[i]) rho[i] = v;
          }
        }
      }
    }
    // The young core is hot: its heat falls off from the middle and with age.
    const hot = (1 - age) ** 2;
    for (let z = 0; z < NZ; z++)
      for (let y = 0; y < NY; y++)
        for (let x = 0; x < NX; x++) {
          const i = idx(x, y, z);
          if (rho[i] > 0) heat[i] = hot * Math.exp(-((at(x, NX) ** 2 + at(y, NY) ** 2 + at(z, NZ) ** 2) / 0.12));
        }
    const k = b.density;
    // A sun's light at each voxel, through the smoke toward it: a sweep along its axis, half
    // the voxel's own depth counted.
    const sweep = (axis: 0 | 2, sign: 1 | -1) => {
      const N = axis === 0 ? NX : NZ;
      for (let a = 0; a < (axis === 0 ? NZ : NX); a++)
        for (let y = 0; y < NY; y++) {
          let tau = 0;
          for (let s = 0; s < N; s++) {
            const c = sign > 0 ? N - 1 - s : s;
            const i = axis === 0 ? idx(c, y, a) : idx(a, y, c);
            const own = k * rho[i] * dx;
            T[i] = Math.exp(-(tau + own / 2));
            tau += own;
          }
        }
    };
    // What the camera sees of the light from one side: single scatter, isotropic, albedo 1.
    const seen = (from: "x+" | "x-" | "z+" | "z-" | "y+" | "y-") => {
      if (from[0] === "x") sweep(0, from[1] === "+" ? 1 : -1);
      if (from[0] === "z") sweep(2, from[1] === "+" ? 1 : -1);
      const img = new Float32Array(n * n);
      for (let z = 0; z < NZ; z++)
        for (let x = 0; x < NX; x++) {
          let [cam, sum] = [1, 0];
          let behind = 0;
          if (from === "y+") for (let y = 0; y < NY; y++) behind += k * rho[idx(x, y, z)] * dy;
          for (let y = 0; y < NY; y++) {
            const i = idx(x, y, z);
            const own = k * rho[i] * dy;
            if (own <= 0) continue;
            let light: number;
            if (from === "y-") light = cam * Math.exp(-own / 2);
            else if (from === "y+") light = Math.exp(-(behind - own / 2));
            else light = T[i];
            if (from === "y+") behind -= own;
            sum += cam * (1 - Math.exp(-own)) * light * 0.25;
            cam *= Math.exp(-own);
          }
          img[(NZ - 1 - z) * n + x] = sum;
        }
      return img;
    };
    const lights = (["x+", "z+", "y+", "x-", "z-", "y-"] as const).map(seen);
    const glowR = new Float32Array(n * n);
    const alpha = new Float32Array(n * n);
    for (let z = 0; z < NZ; z++)
      for (let x = 0; x < NX; x++) {
        let [cam, sum] = [1, 0];
        for (let y = 0; y < NY; y++) {
          const i = idx(x, y, z);
          const own = k * rho[i] * dy;
          if (own <= 0) continue;
          sum += cam * (1 - Math.exp(-own)) * heat[i];
          cam *= Math.exp(-own);
        }
        glowR[(NZ - 1 - z) * n + x] = sum;
        alpha[(NZ - 1 - z) * n + x] = 1 - cam;
      }
    const per = n * n * 3;
    for (let p = 0; p < n * n; p++) {
      for (let c = 0; c < 3; c++) {
        raw[(f * 3 + 0) * per + p * 3 + c] = half(lights[c][p]);
        raw[(f * 3 + 1) * per + p * 3 + c] = half(lights[3 + c][p]);
      }
      raw[(f * 3 + 2) * per + p * 3] = half(glowR[p]);
      raw[(f * 3 + 2) * per + p * 3 + 1] = half(alpha[p]);
    }
  }
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "raw.bin"), raw);
  const s = Math.round((Date.now() - t0) / 100) / 10;
  const meta = { set: r.name, size: n, frames: r.frames, first: 1, step: r.step ?? 1, passes: ["light0", "light1", "glow"], fps: r.fps ?? 24, extent: e, sim_s: 0, render_s: s };
  writeFileSync(join(out, "meta.json"), `${JSON.stringify(meta, null, 1)}\n`);
}
