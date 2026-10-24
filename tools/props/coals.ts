// A fire's coal bed, added to its refined model (`coals` in prop.json, after `char`): a low ash
// mound inside its logs with chunky rounded coals heaped on it, densest and highest under the
// apex, as Red Dead Redemption 2's and Enshrouded's ember beds are. Each coal is dark crust,
// grey ash on what faces up, its cracks narrow glowing seams, sunk in the bed, its underside only dull;
// the ash between a crackle of glowing seams where the heap is. Its own material, `Coals`,
// with an emissive map, so a burning prop's embers (`props::Embers`) flicker it with the fire.
// Every glow is at most 1,440 K's colour, orange-red: past it a coal reads salmon-pink through
// the tonemapper (fire round 11). Run it once on a freshly refined model.
//
// node tools/props/coals.ts <in.glb> <out.glb> [seed]

import { readFileSync, writeFileSync } from "node:fs";
import { encodePng } from "../sandbox/png.ts";

/// The triangles the bed may add: the prop's own budget again, no more (fire round 16's
/// first bed was 4,844).
export const BED_BUDGET = 4500;
/// The bed: its radius under the stones, its height in the middle and of the ash's berm round
/// it, metres: dished, as a fire burnt down in its pit is, never a raised plate (campfire char round:
/// RDR2's shallow pit of grey ash, against our raised lit disc).
const MOUND_R = 0.46;
const MOUND_TOP = 0.012;
const BERM = 0.02;
/// The heap: big coals (subdivided once, 80 triangles) in the middle, small (an icosahedron,
/// 20) round them, all inside this radius under the logs.
const BIG = 6;
const SMALL = 30;
const HEAP_R = 0.22;
/// Its glow only under the logs: a ragged patch round where they cross, this far out, and down
/// each log's line as far as `ARM_R` (RDR2's glow ~0.3-0.4 of the ring, under the logs).
const CORE_R = 0.17;
const ARM_R = 0.34;
/// What lies over the bed, seen from above: each log's line on the ground (x, z at its two ends)
/// and its half width, metres; the campfire's from `tools/props/hearth.ts` (`arms`).
export type Arm = { a: [number, number]; b: [number, number]; w: number };
// How far a point is from an arm's line on the ground, and how far along it (0-1).
const toArm = (x: number, z: number, { a, b }: Arm): [number, number] => {
  const [dx, dz] = [b[0] - a[0], b[1] - a[1]], len2 = dx * dx + dz * dz || 1;
  const t = Math.min(Math.max(((x - a[0]) * dx + (z - a[1]) * dz) / len2, 0), 1);
  return [Math.hypot(x - a[0] - t * dx, z - a[1] - t * dz), t];
};
/// How far under the logs a point on the bed is (1 under the crossing or a log's middle over the
/// coals, 0 out in the ash), ragged at its edge.
export function under(x: number, z: number, arms: readonly Arm[]): number {
  const r = Math.hypot(x, z), rag = 0.035 * (fbm(x * 14, z * 14, 21) - 0.5) + 0.02 * (fbm(x * 40, z * 40, 22) - 0.5);
  let u = 1 - smooth(CORE_R - 0.05, CORE_R + 0.03, r + rag);
  for (const arm of arms) {
    const [d] = toArm(x, z, arm);
    u = Math.max(u, (1 - smooth(arm.w * 0.6, arm.w * 1.5, d + rag)) * (1 - smooth(ARM_R - 0.08, ARM_R, r + rag)));
  }
  return u;
}
/// The hottest glow, sRGB: a coal at 1,440 K (the logs' cracks', tools/props/char.py).
export const HOTTEST: [number, number, number] = [255, 107, 22];
/// The atlas: 512 square; 128-pixel coal tiles in the top half and the bottom right, the
/// mound's disc seen from above in the bottom left.
const S = 512;
const T = 128;
export const TILES = [0, 1, 2, 3, 4, 5, 6, 7].map((i) => [(i % 4) * T, Math.floor(i / 4) * T]).concat([[2 * T, 2 * T], [3 * T, 2 * T], [2 * T, 3 * T], [3 * T, 3 * T]]);

// A coal: its middle, its size, and the first of its triangles in the bed's.
export type Coal = { at: [number, number, number]; size: number; from: number };
export type Bed = { pos: number[]; nrm: number[]; uv: number[]; idx: number[]; coals: Coal[] };

export const smooth = (a: number, b: number, x: number) => {
  const t = Math.min(Math.max((x - a) / (b - a), 0), 1);
  return t * t * (3 - 2 * t);
};
const mound = (r: number) => {
  const k = Math.min(r / MOUND_R, 1);
  return (MOUND_TOP + BERM * smooth(0.5, 0.85, k)) * smooth(1, 0.85, k);
};
export function random(seed: number) {
  let s = (seed * 2654435761) >>> 0;
  return () => ((s = (Math.imul(s ^ (s >>> 15), 2246822519) + 0x9e3779b9) >>> 0), s / 4294967296);
}
export const hash = (x: number, y: number, z = 0) => {
  let h = Math.imul(x * 374761393 + y * 668265263 + z * 2147483647, 1274126177) >>> 0;
  h = Math.imul(h ^ (h >>> 13), 1274126177) >>> 0;
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
};
export function noise(x: number, y: number, z = 0) {
  const [xi, yi] = [Math.floor(x), Math.floor(y)];
  const [u, v] = [smooth(0, 1, x - xi), smooth(0, 1, y - yi)];
  const at = (a: number, b: number) => hash(xi + a, yi + b, z);
  return (at(0, 0) * (1 - u) + at(1, 0) * u) * (1 - v) + (at(0, 1) * (1 - u) + at(1, 1) * u) * v;
}
export const fbm = (x: number, y: number, z = 0) => 0.5 * noise(x, y, z) + 0.3 * noise(x * 2.1, y * 2.1, z) + 0.2 * noise(x * 4.3, y * 4.3, z);

// The unit icosahedron, or it subdivided once.
function sphere(subdivide: boolean) {
  const t = (1 + Math.sqrt(5)) / 2;
  const v = [[-1, t, 0], [1, t, 0], [-1, -t, 0], [1, -t, 0], [0, -1, t], [0, 1, t], [0, -1, -t], [0, 1, -t], [t, 0, -1], [t, 0, 1], [-t, 0, -1], [-t, 0, 1]].map((p) => p.map((c) => c / Math.hypot(...p)));
  let f = [[0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8], [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1]];
  if (subdivide) {
    const mid = new Map<string, number>();
    const m = (a: number, b: number) => {
      const k = a < b ? `${a},${b}` : `${b},${a}`;
      if (!mid.has(k)) {
        const p = v[a].map((c, i) => c + v[b][i]);
        v.push(p.map((c) => c / Math.hypot(...p)));
        mid.set(k, v.length - 1);
      }
      return mid.get(k)!;
    };
    f = f.flatMap(([a, b, c]) => [[a, m(a, b), m(c, a)], [b, m(b, c), m(a, b)], [c, m(c, a), m(b, c)], [m(a, b), m(b, c), m(c, a)]]);
  }
  return { v, f };
}
const SPHERES = [sphere(false), sphere(true)];

// A direction's place in a tile, octahedrally: its top the middle, its underside the corners,
// with no pole pinched as a latitude map pinches one.
function octahedral([x, y, z]: number[]): [number, number] {
  const l = Math.abs(x) + Math.abs(y) + Math.abs(z);
  let [a, b] = [x / l, z / l];
  if (y < 0) [a, b] = [(1 - Math.abs(b)) * Math.sign(a || 1), (1 - Math.abs(a)) * Math.sign(b || 1)];
  return [0.5 + 0.5 * a, 0.5 + 0.5 * b];
}
function unoctahedral(u: number, v: number): number[] {
  let [a, b] = [u * 2 - 1, v * 2 - 1];
  const y = 1 - Math.abs(a) - Math.abs(b);
  if (y < 0) [a, b] = [(1 - Math.abs(b)) * Math.sign(a || 1), (1 - Math.abs(a)) * Math.sign(b || 1)];
  const l = Math.hypot(a, y, b);
  return [a / l, y / l, b / l];
}

// Adds a triangle facing away from `inside`.
function face(bed: Bed, a: number, b: number, c: number, inside: number[]) {
  const p = (i: number) => bed.pos.slice(i * 3, i * 3 + 3);
  const [A, B, C] = [p(a), p(b), p(c)];
  const u = B.map((q, i) => q - A[i]), w = C.map((q, i) => q - A[i]);
  const n = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
  const out = A.map((q, i) => q - inside[i]);
  bed.idx.push(...(n[0] * out[0] + n[1] * out[1] + n[2] * out[2] >= 0 ? [a, b, c] : [a, c, b]));
}

function addMound(bed: Bed, rnd: () => number) {
  const base = bed.pos.length / 3, RINGS = 5, SEG = 18;
  const point = (x: number, z: number, y: number) => {
    const r = Math.hypot(x, z), dh = (mound(r + 0.01) - mound(r - 0.01)) / 0.02, l = Math.hypot(dh, 1);
    bed.pos.push(x, y, z);
    bed.nrm.push((-dh * x) / (r || 1) / l, 1 / l, (-dh * z) / (r || 1) / l);
    bed.uv.push((0.5 + (0.49 * x) / MOUND_R) * 0.5, 0.5 + (0.5 + (0.49 * z) / MOUND_R) * 0.5);
  };
  point(0, 0, mound(0));
  for (let r = 1; r <= RINGS; r++) {
    for (let k = 0; k < SEG; k++) {
      const rr = (MOUND_R * r) / RINGS, a = (k / SEG) * Math.PI * 2 + r * 0.3;
      const j = r < RINGS ? 1 + (rnd() - 0.5) * 0.1 : 1;
      point(Math.cos(a) * rr * j, Math.sin(a) * rr * j, r === RINGS ? 0 : mound(rr) + (rnd() - 0.5) * 0.01);
    }
  }
  const below = [0, -1, 0];
  const ring = (r: number, k: number) => base + 1 + (r - 1) * SEG + (k % SEG);
  for (let k = 0; k < SEG; k++) face(bed, base, ring(1, k), ring(1, k + 1), below);
  for (let r = 1; r < RINGS; r++) {
    for (let k = 0; k < SEG; k++) {
      face(bed, ring(r, k), ring(r, k + 1), ring(r + 1, k), below);
      face(bed, ring(r, k + 1), ring(r + 1, k + 1), ring(r + 1, k), below);
    }
  }
}

// A coal: a lumpy rounded chunk, longer one way, a little squat, smooth-shaded.
function addCoal(bed: Bed, at: number[], size: number, big: boolean, tile: number, rnd: () => number) {
  const { v, f } = SPHERES[1];
  const base = bed.pos.length / 3;
  const turn = rnd() * Math.PI * 2, tilt = (rnd() - 0.5) * 0.7, long = 1.05 + 0.3 * rnd(), squat = 0.55 + 0.15 * rnd();
  const lumps = [rnd() * 6, rnd() * 6, rnd() * 6];
  const P = v.map(([x, y, z], i) => {
    const k = 1 + 0.1 * Math.sin(x * 2.6 + lumps[0]) * Math.sin(z * 2.2 + lumps[1]) + 0.07 * Math.sin(y * 3.1 + lumps[2]) + 0.05 * (hash(i, tile, base) - 0.5);
    const [px, py, pz] = [x * k * size * long, y * k * size * squat, z * k * size];
    const [qy, qz] = [py * Math.cos(tilt) - pz * Math.sin(tilt), py * Math.sin(tilt) + pz * Math.cos(tilt)];
    return [at[0] + px * Math.cos(turn) + qz * Math.sin(turn), at[1] + qy, at[2] - px * Math.sin(turn) + qz * Math.cos(turn)];
  });
  const N = P.map(() => [0, 0, 0]);
  for (const [a, b, c] of f) {
    const u = P[b].map((q, i) => q - P[a][i]), w = P[c].map((q, i) => q - P[a][i]);
    const n = [u[1] * w[2] - u[2] * w[1], u[2] * w[0] - u[0] * w[2], u[0] * w[1] - u[1] * w[0]];
    const s = Math.sign(n[0] * (P[a][0] - at[0]) + n[1] * (P[a][1] - at[1]) + n[2] * (P[a][2] - at[2])) || 1;
    for (const q of [a, b, c]) for (let i = 0; i < 3; i++) N[q][i] += s * n[i];
  }
  const [tu, tv] = TILES[tile];
  P.forEach((p, i) => {
    bed.pos.push(...p);
    const l = Math.hypot(...N[i]) || 1;
    bed.nrm.push(N[i][0] / l, N[i][1] / l, N[i][2] / l);
    const [ou, ov] = octahedral(v[i]);
    bed.uv.push((tu + 2 + (T - 4) * ou) / S, (tv + 2 + (T - 4) * ov) / S);
  });
  for (const [a, b, c] of f) face(bed, base + a, base + b, base + c, at);
}

/** The bed's mesh: the mound, then the coals heaped on it, big ones first, each resting on
 * the mound or on the coals already under it. */
export function bed(seed = 1, arms: readonly Arm[] = []): Bed {
  const rnd = random(seed);
  const out: Bed = { pos: [], nrm: [], uv: [], idx: [], coals: [] };
  addMound(out, rnd);
  // A few more under each log, out along it past the heap, where it burns over the bed.
  const along = arms.flatMap((arm) => [0.18, 0.28, 0.38].map((f) => ({ arm, f })));
  for (let i = 0; i < BIG + SMALL + along.length; i++) {
    const big = i < BIG;
    const r = big ? 0.13 * Math.sqrt((i + 0.5) / BIG) : Math.sqrt(0.01 + (HEAP_R * HEAP_R - 0.01) * ((i - BIG + rnd()) / SMALL));
    const a = i * 2.39996 + rnd() * 0.5;
    let size = big ? 0.06 + 0.02 * rnd() : (0.035 + 0.016 * rnd()) * (1.1 - 0.3 * Math.min(r / HEAP_R, 1));
    let [x, z] = [Math.cos(a) * r, Math.sin(a) * r];
    if (i >= BIG + SMALL) {
      // Out along a log's line, between the heap and ARM_R, a little to either side.
      const { arm, f } = along[i - BIG - SMALL];
      const [px, pz] = arm.a, [qx, qz] = arm.b;
      const t = (rnd() * 0.2 - 0.1) + f;
      let [ax, az] = [qx + (px - qx) * t, qz + (pz - qz) * t];
      const k = Math.min(1, (ARM_R - 0.04) / (Math.hypot(ax, az) || 1));
      [ax, az] = [ax * k, az * k];
      const side = (rnd() - 0.5) * arm.w;
      const len = Math.hypot(px - qx, pz - qz) || 1;
      [x, z] = [ax - ((pz - qz) / len) * side, az + ((px - qx) / len) * side];
      size = 0.022 + 0.01 * rnd();
    }
    // Rests on what is under it: sunk a third into the ash, or on a coal it overlaps.
    let y = mound(Math.hypot(x, z)) + size * 0.1;
    for (const c of out.coals) {
      const d = Math.hypot(c.at[0] - x, c.at[2] - z), reach = (c.size + size) * 0.85;
      if (d < reach) y = Math.max(y, c.at[1] + Math.sqrt(reach * reach - d * d) * 0.3);
    }
    out.coals.push({ at: [x, y, z], size, from: out.idx.length / 3 });
    // Its tile by how far out it lies: the core's hottest, cooling to red at the edge.
    const rr = Math.hypot(x, z);
    const band = Math.max((rr < 0.12 ? 0 : rr < 0.22 ? 1 : 2) - (rnd() < 0.35 ? 1 : 0), 0);
    addCoal(out, [x, y, z], size, big, band * 4 + Math.floor(rnd() * 4), rnd);
  }
  return out;
}

// A glow's colour, sRGB, at a heat from 0 (dark) to 1 (1,440 K): deep red, then orange.
// At most so bright, sRGB: half the logs' cracks' light, as the embers' strength is theirs
// (`EMBER_GLOW`); at the logs' a coal went past the tonemapper's shoulder to salmon-red.
const BRIGHTEST = 1.0;
export function glowColour(heat: number): number[] {
  const h = Math.min(Math.max(heat, 0), 1);
  // Its green falls nearly as its red does (h^1.1 against h^0.95), so a dull coal is a deep
  // orange-red, the core still yellower than the edge: with the green
  // falling faster (h^1.2 against h^0.7) a dim coal was near pure red, which the night's
  // desaturation turned pink (fire round 31: "pink pebbles on a pink-magenta base").
  const r = HOTTEST[0] * Math.pow(h, 0.95);
  // Never yellower than the core's own orange-yellow, as the dimmest would be.
  const red = [r, Math.min(HOTTEST[1] * Math.pow(h, 1.1), 0.6 * r), HOTTEST[2] * h * h];
  // Past 1, toward the orange-yellow of the bed's hottest middle; its blue kept low, so it
  // goes yellow through the tonemapper rather than salmon.
  const y = smooth(1, 1.3, heat);
  return red.map((c, i) => (c + (YELLOW[i] - c) * y) * BRIGHTEST);
}
/// The core's glow, sRGB: about 2,300 K's orange-yellow, its blue low.
export const YELLOW = [255, 176, 64];

/** The atlas's base colour, emissive and metal-roughness maps, RGBA a byte each. */
export function maps(seed = 1, arms: readonly Arm[] = []): Uint8Array[] {
  const rnd = random(seed + 7);
  const out = [0, 1, 2].map(() => new Uint8Array(S * S * 4).fill(255));
  const put = (x: number, y: number, base: number[], glow: number[], rough: number) => {
    const o = (y * S + x) * 4;
    for (let i = 0; i < 3; i++) {
      out[0][o + i] = Math.round(Math.min(255, base[i]));
      out[1][o + i] = Math.round(Math.min(255, glow[i]));
    }
    out[2].set([0, Math.round(rough * 255), 0], o);
  };
  // The mound: pale ash at the stones, darker char and dull red smoulder under the heap.
  for (let y = 0; y < 2 * T; y++) {
    for (let x = 0; x < 2 * T; x++) {
      const [X, Z] = [(x / (2 * T) - 0.5) / 0.49, (y / (2 * T) - 0.5) / 0.49];
      const r = Math.hypot(X, Z), n = fbm(X * 9, Z * 9, 1), speck = fbm(X * 30, Z * 30, 2);
      // Black char under the logs, glowing only there; grey ash over the rest of the pit.
      const below = under(X * MOUND_R, Z * MOUND_R, arms);
      const char = Math.max(below, 0.6 * (1 - smooth(0.3, 0.6, r)));
      // A warm dark grey ash skirt round the heap, its edge soft and broken into the earth in
      // patches: pale and even, it was a disc that went salmon-pink by firelight (fire round 20).
      const edge = r + 0.2 * (fbm(X * 4, Z * 4, 4) - 0.5) + 0.1 * (fbm(X * 14, Z * 14, 9) - 0.5);
      const patchy = (0.45 + 0.55 * smooth(0.3, 0.65, fbm(X * 7, Z * 7, 8))) * (1 - 0.5 * smooth(0.75, 0.95, edge) * (1 - smooth(0.4, 0.6, fbm(X * 16, Z * 16, 10))));
      const skirt = smooth(0.55, 0.8, r) * (1 - smooth(0.86, 1.06, edge)) * patchy;
      // Its ash grey-white where the fire has burnt out to it, in patches (blind judge, campfire
      // tongue round: "no ember bed"): at 50-66 it read as earth. Its grey 72-98 and a little
      // less red: at 100-134 it was the firelit ring's brightest, salmon, at the groundlight
      // check's floor of hue 20 once the flame was drawn lower (campfire low-lay round).
      // Dark grey, never warm: the firelight warms it, and tan ash at 72-98 went salmon at the
      // firelit ring's brightest, twice the ground's luma (campfire char round: RDR2's ash in
      // its ring is about the firelit ground's, 1.1 times it).
      const ash = (1 - skirt) * (1 - char) + skirt;
      const grey = (16 + 10 * n) * char * (1 - skirt) + (28 + 12 * n - 6 * smooth(0.6, 0.8, speck)) * ash;
      // Under the heap a crackle of glowing seams through dark ash, as a bed of coals breaks
      // into a mesh of cracks, not a lit disc (fire round 29: separate red baubles).
      const ridge = (f: number, k: number) => 1 - smooth(0.0, 0.06, Math.abs(fbm(X * f, Z * f, k) - 0.5));
      const crackle = below * Math.max(ridge(7, 11), 0.8 * ridge(15, 12));
      // No hot core lit whole under the logs: from the camp's eye, above, it read as a patch of
      // yellow chips through the logs (campfire history panel).
      const smoulder = Math.max(below * smooth(0.3, 0.6, fbm(X * 5, Z * 5, 3)) * 0.85, crackle * 1.3) + (1 - char) * smooth(0.78, 0.84, speck) * 0.15;
      put(x, 2 * T + y, [grey + 10 * smoulder * below, grey * 0.99, grey * 0.97], glowColour(smoulder), 0.95);
    }
  }
  // The coals: a tile each of several, read by direction (octahedral), top at the middle.
  TILES.forEach(([ox, oy], tile) => {
    // Rows of four by where in the bed: its middle past 1,440 K to orange-yellow, then orange,
    // then dull red at the edge, sitting in ash (campfire fill round: at 1.25/1.1/0.95, 0.6 ash at the edge, the
    // heap read as flat bright yellow chips to three blind judges).
    const band = Math.floor(tile / 4);
    // Its middle 1.2, not 1.45: under black logs at 1.45 it read as a yellow honeycomb plate, and
    // the campfire's yellow passed the sheet's at night (campfire char round).
    // From the camp's eye, above, its coals' tops show most: under more ash, few cells hot over
    // their whole face (campfire history panel: e9e5b733's bed, 4.75, against yellow chips).
    const heat = [1.2, 1.0, 0.75][band] + 0.1 * rnd();
    const ashy = [0.2, 0.6, 1.1][band];
    const cells = Array.from({ length: 11 }, () => ({ at: unoctahedral(rnd(), rnd()), hot: rnd() < 0.3 ? 0.45 + 0.2 * rnd() : 0 }));
    for (let y = 0; y < T; y++) {
      for (let x = 0; x < T; x++) {
        const d = unoctahedral((x - 2) / (T - 4), (y - 2) / (T - 4));
        let [d1, d2, hot] = [9, 9, 0];
        for (const c of cells) {
          const e = Math.hypot(d[0] - c.at[0], d[1] - c.at[1], d[2] - c.at[2]);
          if (e < d1) [d1, d2, hot] = [e, d1, c.hot];
          else if (e < d2) d2 = e;
        }
        const wob = fbm(d[0] * 5 + tile, d[2] * 5 + d[1] * 3, 4) - 0.5;
        const gap = d2 - d1 + 0.12 * wob;
        // The seam: a hot core with a soft falloff into the crust, not a hard line.
        const seam = 1 - smooth(0.0, 0.07, gap), halo = 1 - smooth(0.0, 0.2, gap);
        const under = smooth(0.15, -0.55, d[1]);
        const mottle = fbm(d[0] * 4 + tile * 3, d[2] * 4 - d[1] * 2, 5);
        const glow = heat * Math.max(1.3 * seam * (1 - 0.25 * smooth(0.3, 0.9, d[1])), halo * 0.4, under * (0.15 + 0.1 * mottle), hot * (0.7 + 0.5 * mottle));
        const ash = Math.min(1, ashy * (hot ? 0 : 1) * smooth(0.4, 0.9, d[1]) * smooth(0.45, 0.65, fbm(d[0] * 6 + tile, d[2] * 6, 6)) * (1 - halo));
        const crust = 16 + 12 * mottle;
        const grey = crust + ash * (50 + 25 * mottle);
        put(ox + x, oy + y, [grey + 20 * glow, grey * 0.97 + 6 * glow, grey * 0.94], glowColour(glow), 0.9);
      }
    }
  });
  return out;
}

/** The atlas's maps as PNGs. */
export function atlas(seed = 1, arms: readonly Arm[] = []): Buffer[] {
  return maps(seed, arms).map((rgba) => encodePng({ width: S, height: S, rgba }));
}

/** The model with the bed added: a primitive of its own on its first mesh, material `Coals`. */
export function addBed(glb: Buffer, seed = 1, arms: readonly Arm[] = []): Buffer {
  const b = bed(seed, arms);
  const jl = glb.readUInt32LE(12);
  const j = JSON.parse(glb.subarray(20, 20 + jl).toString());
  if (j.materials.some((m: { name?: string }) => m.name === "Coals")) throw new Error("it has a coal bed already; run coals on a freshly refined model");
  const bin = glb.subarray(20 + jl + 8, 20 + jl + 8 + glb.readUInt32LE(20 + jl));
  const parts: Buffer[] = [bin];
  let len = bin.length;
  const view = (data: Buffer, target?: number) => {
    const pad = (4 - (len % 4)) % 4;
    if (pad) parts.push(Buffer.alloc(pad));
    len += pad;
    j.bufferViews.push({ buffer: 0, byteOffset: len, byteLength: data.length, ...(target ? { target } : {}) });
    parts.push(data);
    len += data.length;
    return j.bufferViews.length - 1;
  };
  const acc = (data: number[], type: string, n: number) => {
    const a: Record<string, unknown> = { bufferView: view(Buffer.from(new Float32Array(data).buffer), 34962), componentType: 5126, count: data.length / n, type };
    if (type === "VEC3") {
      a.min = [0, 1, 2].map((i) => Math.min(...data.filter((_, k) => k % 3 === i)));
      a.max = [0, 1, 2].map((i) => Math.max(...data.filter((_, k) => k % 3 === i)));
    }
    return j.accessors.push(a) - 1;
  };
  const attributes = { POSITION: acc(b.pos, "VEC3", 3), NORMAL: acc(b.nrm, "VEC3", 3), TEXCOORD_0: acc(b.uv, "VEC2", 2) };
  const indices = j.accessors.push({ bufferView: view(Buffer.from(new Uint32Array(b.idx).buffer), 34963), componentType: 5125, count: b.idx.length, type: "SCALAR" }) - 1;
  j.samplers ??= [{}];
  const tex = atlas(seed, arms).map((png) => {
    j.images.push({ mimeType: "image/png", bufferView: view(png) });
    return j.textures.push({ sampler: 0, source: j.images.length - 1 }) - 1;
  });
  j.materials.push({ name: "Coals", emissiveFactor: [1, 1, 1], emissiveTexture: { index: tex[1] }, pbrMetallicRoughness: { baseColorTexture: { index: tex[0] }, metallicRoughnessTexture: { index: tex[2] }, metallicFactor: 0 } });
  j.meshes[0].primitives.push({ attributes, indices, material: j.materials.length - 1 });
  const pad = (4 - (len % 4)) % 4;
  if (pad) parts.push(Buffer.alloc(pad));
  len += pad;
  j.buffers[0].byteLength = len;
  let js = Buffer.from(JSON.stringify(j));
  js = Buffer.concat([js, Buffer.alloc((4 - (js.length % 4)) % 4, 0x20)]);
  const chunk = (n: number, t: number) => Buffer.from(new Uint32Array([n, t]).buffer);
  const head = Buffer.from(new Uint32Array([0x46546c67, 2, 12 + 8 + js.length + 8 + len]).buffer);
  return Buffer.concat([head, chunk(js.length, 0x4e4f534a), js, chunk(len, 0x004e4942), ...parts]);
}

if (import.meta.main) {
  const [, , input, output, seed] = process.argv;
  if (!input || !output) throw new Error("node tools/props/coals.ts <in.glb> <out.glb> [seed]");
  const glb = addBed(readFileSync(input), Number(seed ?? 1));
  writeFileSync(output, glb);
  const b = bed(Number(seed ?? 1));
  console.log(`${output}: a coal bed of ${b.coals.length} coals, ${b.idx.length / 3} triangles`);
}
