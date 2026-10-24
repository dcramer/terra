// A campfire made as a hero prop (`hearth` in prop.json, fire round 18): split firewood leaned
// low in an open teepee over its coal bed (`coals`, added after), in a ring of rounded
// fieldstones half sunk in the ground. Tripo's low-poly campfire read as faceted black prisms
// with scaly end caps (round 17's review, 4/10); a campfire is a prop the player sits beside, so
// it is modelled as games model a hero prop, with the triangles for round wood and stone
// (D427, art-style.md) and its detail in its maps, as The Forest's and RDR2's campfires are:
//
// - Each log is a half, a quarter or a whole round of a trunk: bark on its round face, pale
//   split wood on its flat faces with the growth rings showing as stripes along them, end grain
//   with rings, a few radial checks and a bark rim on its ends; charred to cracked black blocks
//   with glowing seams up its inner end, over the flames, and where its foot sits in the coals.
// - Each stone is a lumpy, squashed round with a broken face, its own tone, specks, soot on the
//   face toward the fire and earth at its foot.
//
// Its two materials, `Wood` (with a normal map and the cracks' emissive map, so they flicker
// with the fire, `play::props`) and `Stone`, keep the model contract: origin the middle of its
// base, front +Z, inside its catalogue `dims` (assets/data/builds.ron).

import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { encodePng } from "../sandbox/png.ts";
import { fbm, hash, HOTTEST, noise, random, smooth } from "./coals.ts";

/// The triangles it may have, its coal bed's included: a hero prop's, as a near tree's
/// (art-style.md's budgets, D427).
export const BUDGET = 10_000;
/// The footprint's half width and the height it is made within, metres; the catalogue's
/// `dims` are what it comes to (the contract's test holds them to the file).
export const HALF = 0.65;
export const TALL = 0.55;
const LOGS = 7;
const STONES = 11;
/// The ring of stones: the radius of their middles.
const RING_R = 0.52;
/// The wood atlas, texels a metre on it, and its side; the stone atlas's tiles.
const D = 1000;
const W = 2048;
const TILE = 512;

type V3 = [number, number, number];
const add = (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const mul = (a: V3, k: number): V3 => [a[0] * k, a[1] * k, a[2] * k];
const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a: V3): V3 => mul(a, 1 / (Math.hypot(...a) || 1));
const mix = (a: number[], b: number[], t: number) => a.map((x, i) => x + (b[i] - x) * t);
const clamp = (x: number) => Math.min(Math.max(x, 0), 1);
const frac = (x: number) => x - Math.floor(x);

export type Mesh = { pos: number[]; nrm: number[]; uv: number[]; idx: number[] };
const mesh = (): Mesh => ({ pos: [], nrm: [], uv: [], idx: [] });

// A run of a log's cross-section: bark round its outside or a split face, as points in its
// plane (metres, its pith at the origin) and how far round the section each is.
type Run = { bark: boolean; pts: [number, number][]; s: number[] };
type Log = { kind: number; R: number; foot: V3; axis: V3; e1: V3; e2: V3; out: V3; L: number; runs: Run[]; P: number; centre: [number, number]; side: [number, number]; caps: [number, number][]; box: number[]; seed: number; from?: number };
type Stone = { at: V3; size: V3; turn: number; tone: number[]; seed: number; cut: V3; from?: number };

function arc(R: number, from: number, to: number, n: number): [number, number][] {
  return Array.from({ length: n }, (_, i) => {
    const a = from + ((to - from) * i) / (n - 1);
    return [R * Math.cos(a), R * Math.sin(a)] as [number, number];
  });
}
function line(a: [number, number], b: [number, number], n: number): [number, number][] {
  return Array.from({ length: n }, (_, i) => [a[0] + ((b[0] - a[0]) * i) / (n - 1), a[1] + ((b[1] - a[1]) * i) / (n - 1)] as [number, number]);
}
// A section's runs: a half (kind 0), a quarter (1) or a whole round (2).
function section(kind: number, R: number): Run[] {
  const runs: { bark: boolean; pts: [number, number][] }[] =
    kind === 0 ? [{ bark: true, pts: arc(R, 0, Math.PI, 13) }, { bark: false, pts: line([-R, 0], [R, 0], 6) }]
    : kind === 1 ? [{ bark: true, pts: arc(R, 0, Math.PI / 2, 8) }, { bark: false, pts: line([0, R], [0, 0], 4) }, { bark: false, pts: line([0, 0], [R, 0], 4) }]
    : [{ bark: true, pts: arc(R, 0, 2 * Math.PI, 17) }];
  let s = 0;
  return runs.map((r) => {
    const ss = r.pts.map((p, i) => (i ? (s += Math.hypot(p[0] - r.pts[i - 1][0], p[1] - r.pts[i - 1][1])) : s));
    return { ...r, s: ss };
  });
}

/** The stones' tones, sRGB 0-255: warm to neutral grey-brown field stone, granite and sandstone
 * greys of mid albedo; paler, they passed the tonemapper's shoulder by firelight and read
 * salmon-pink (fire round 21). */
// Grey-brown field stones, from blue-grey granite to brown sandstone, darker than the beige
// of before (fire round 25: identical beige ovals in the firelight).
// Darker again by a quarter (fire round 31): by firelight they had the hue RDR2's ring has (22
// against 21-27) but stood brighter (sRGB red 215-237 against 140-176), pale at the shoulder, so
// the salmon was their albedo, not the light's colour; a greener light only moved their hue.
// Cool grey, not brown (campfire fill round): by firelight the brown tones' hue stood at 20, the
// groundlight check's floor, and read salmon, and stood brighter (sRGB 233,128,71) than RDR2's
// mismatched grey rock, lit #B26D36-#D39558 (hue 27-30); a neutral grey moved it only to 21.
// Brown again (campfire net round): with e9e5b733's rounded stones they scored higher from the
// camp's eye than the grey, which read pink-lit.
export const STONE_TONES: readonly (readonly number[])[] = [[70, 64, 54], [62, 59, 53], [78, 68, 54], [56, 53, 48], [68, 62, 52], [82, 76, 66], [64, 56, 46]];

/** Where its logs and stones lie, from a seed. */
export function layout(seed = 1): { logs: Log[]; stones: Stone[] } {
  const rnd = random(seed);
  const kinds = [0, 1, 0, 2, 1, 0, 1];
  const logs: Log[] = kinds.slice(0, LOGS).map((kind, i) => {
    const R = kind === 2 ? 0.055 + 0.01 * rnd() : 0.085 + 0.025 * rnd();
    const phi = (i / LOGS) * 2 * Math.PI + 0.25 * (rnd() - 0.5);
    const rf = 0.33 + 0.04 * rnd();
    const out: V3 = [Math.cos(phi), 0, Math.sin(phi)];
    const foot: V3 = [rf * out[0], R * 1.0, rf * out[2]];
    // Tops past the middle and turned on a little, so the logs cross rather than meet in a point.
    const pt = phi + 0.5 + 0.3 * rnd(), rt = 0.06 + 0.04 * rnd();
    const top: V3 = [rt * Math.cos(pt), 0.33 + 0.06 * rnd(), rt * Math.sin(pt)];
    const axis = norm(sub(top, foot));
    const L = Math.hypot(...sub(top, foot)) + 0.05;
    // Its section's plane, turned about its axis at random, so split faces show every way.
    const h = norm(cross(axis, [0, 1, 0]));
    const v = cross(h, axis);
    const roll = rnd() * 2 * Math.PI;
    const e1 = add(mul(h, Math.cos(roll)), mul(v, Math.sin(roll)));
    const e2 = cross(e1, axis);
    const runs = section(kind, R);
    const all = runs.flatMap((r) => r.pts);
    const centre: [number, number] = [all.reduce((a, p) => a + p[0], 0) / all.length, all.reduce((a, p) => a + p[1], 0) / all.length];
    const xs = all.map((p) => p[0]), ys = all.map((p) => p[1]);
    const box = [Math.min(...xs), Math.min(...ys), Math.max(...xs), Math.max(...ys)];
    return { kind, R, foot, axis, e1, e2, out, L, runs, P: runs.at(-1)!.s.at(-1)!, centre, side: [0, 0], caps: [[0, 0], [0, 0]], box, seed: seed * 31 + i };
  });
  pack(logs);
  const stones: Stone[] = [];
  // Each stone a tone of the field stone's, lighter or darker by one scale for all its channels:
  // a scale each tinted them lavender and blue (fire round 20). Three draws still, so the ring
  // is laid as it was.
  const toned = (tone: readonly number[], k: number[]) => tone.map((c) => c * (0.9 + (0.2 * (k[0] + k[1] + k[2])) / 3));
  for (let i = 0; i < STONES; i++) {
    const phi = (i / STONES) * 2 * Math.PI + 0.22 * (rnd() - 0.5);
    const long = 0.08 + 0.075 * rnd();
    const size: V3 = [long, 0.048 + 0.06 * rnd(), Math.min(0.06 + 0.05 * rnd(), HALF - RING_R - 0.005)];
    const r = RING_R + 0.02 * (rnd() - 0.5);
    stones.push({ at: [r * Math.cos(phi), size[1] * 0.25, r * Math.sin(phi)], size, turn: phi + 0.6 * (rnd() - 0.5), tone: toned(STONE_TONES[Math.floor(rnd() * STONE_TONES.length)], [rnd(), rnd(), rnd()]), seed: seed * 17 + i, cut: norm([rnd() - 0.5, 0.6 + 0.4 * rnd(), rnd() - 0.5]) });
  }
  return { logs, stones };
}

// Lays each log's side and two end caps out on the wood atlas in shelves.
function pack(logs: Log[]) {
  let [x, y, row] = [0, 0, 0];
  const place = (w: number, h: number): [number, number] => {
    if (x + w > W) [x, y, row] = [0, y + row, 0];
    const at: [number, number] = [x, y];
    [x, row] = [x + w, Math.max(row, h)];
    if (y + h > W) throw new Error("the wood atlas is full; lower its texels a metre");
    return at;
  };
  for (const l of logs) l.side = place(Math.ceil(l.P * D) + 4, Math.ceil(l.L * D) + 4);
  for (const l of logs) l.caps = [0, 1].map(() => place(Math.ceil((l.box[2] - l.box[0]) * D) + 4, Math.ceil((l.box[3] - l.box[1]) * D) + 4));
}

// A log at a point of its section (x, y in its plane) and t along it, as built: tapered, bent
// a little and its ends rounded over.
function logPoint(l: Log, x: number, y: number, t: number, scale: number): V3 {
  const taper = (1 - 0.07 * (t / l.L)) * scale;
  const wob = 1 + 0.05 * (noise(x * 25 + y * 25 + l.seed, t * 6, 3) - 0.5);
  const [cx, cy] = [(x - l.centre[0]) * taper * wob, (y - l.centre[1]) * taper * wob];
  const bend = 0.012 * Math.sin((Math.PI * t) / l.L);
  return add(add(l.foot, mul(l.axis, t)), add(mul(l.e1, cx), mul(l.e2, cy + bend)));
}

function addLog(m: Mesh, l: Log) {
  l.from = m.pos.length / 3;
  const ts = [0, 0.012, ...Array.from({ length: 10 }, (_, i) => 0.012 + ((l.L - 0.024) * (i + 1)) / 11), l.L - 0.012, l.L];
  const scales = ts.map((_, i) => (i === 0 || i === ts.length - 1 ? 0.9 : 1));
  const middle = (t: number) => add(l.foot, mul(l.axis, t));
  for (const run of l.runs) {
    const first = m.pos.length / 3;
    ts.forEach((t, j) => {
      run.pts.forEach(([x, y], k) => {
        m.pos.push(...logPoint(l, x, y, t, scales[j]));
        m.uv.push((l.side[0] + 2 + run.s[k] * D) / W, (l.side[1] + 2 + t * D) / W);
      });
    });
    const n = run.pts.length;
    for (let j = 0; j + 1 < ts.length; j++) {
      for (let k = 0; k + 1 < n; k++) {
        const [a, b, c, d] = [j * n + k, j * n + k + 1, (j + 1) * n + k, (j + 1) * n + k + 1].map((i) => first + i);
        face(m, a, b, d, middle(ts[j]));
        face(m, a, d, c, middle(ts[j]));
      }
    }
  }
  // The ends: a fan each from the middle of the section, in its own patch of the atlas.
  [0, ts.length - 1].forEach((j, end) => {
    const t = ts[j];
    const ring = l.runs.flatMap((r) => r.pts).filter((p, k, all) => Math.hypot(p[0] - all[(k + 1) % all.length][0], p[1] - all[(k + 1) % all.length][1]) > 1e-9);
    const first = m.pos.length / 3;
    const capUv = ([x, y]: [number, number]) => [(l.caps[end][0] + 2 + (x - l.box[0]) * D) / W, (l.caps[end][1] + 2 + (y - l.box[1]) * D) / W];
    m.pos.push(...logPoint(l, l.centre[0], l.centre[1], t, 1));
    m.uv.push(...capUv(l.centre));
    ring.forEach((p) => {
      m.pos.push(...logPoint(l, p[0], p[1], t, scales[j]));
      m.uv.push(...capUv(p));
    });
    const beyond = end ? middle(t + 1) : middle(t - 1);
    for (let k = 0; k < ring.length; k++) face(m, first, first + 1 + k, first + 1 + ((k + 1) % ring.length), beyond, true);
  });
}

// A triangle wound to face away from a point inside (or, `toward`, toward a point outside).
function face(m: Mesh, a: number, b: number, c: number, ref: V3, toward = false) {
  const p = (i: number): V3 => [m.pos[i * 3], m.pos[i * 3 + 1], m.pos[i * 3 + 2]];
  const nrm = cross(sub(p(b), p(a)), sub(p(c), p(a)));
  const away = dot(nrm, sub(p(a), ref));
  if (toward ? away > 0 : away < 0) m.idx.push(a, c, b);
  else m.idx.push(a, b, c);
}

// The unit icosahedron subdivided twice: 320 faces.
function icosphere(): { v: V3[]; f: number[][] } {
  const t = (1 + Math.sqrt(5)) / 2;
  let v: V3[] = [[-1, t, 0], [1, t, 0], [-1, -t, 0], [1, -t, 0], [0, -1, t], [0, 1, t], [0, -1, -t], [0, 1, -t], [t, 0, -1], [t, 0, 1], [-t, 0, -1], [-t, 0, 1]].map((p) => norm(p as V3));
  let f = [[0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11], [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8], [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9], [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1]];
  for (let s = 0; s < 2; s++) {
    const mid = new Map<string, number>();
    const m = (a: number, b: number) => {
      const k = a < b ? `${a},${b}` : `${b},${a}`;
      if (!mid.has(k)) mid.set(k, v.push(norm(mul(add(v[a], v[b]), 0.5))) - 1);
      return mid.get(k)!;
    };
    f = f.flatMap(([a, b, c]) => {
      const [ab, bc, ca] = [m(a, b), m(b, c), m(c, a)];
      return [[a, ab, ca], [b, bc, ab], [c, ca, bc], [ab, bc, ca]];
    });
    v = [...v];
  }
  return { v, f };
}
const SPHERE = icosphere();

// Octahedral texture coordinates of a direction, folded by the given signs below the equator,
// so a triangle across a fold keeps one side's mapping.
function oct(d: V3, sx: number, sz: number): [number, number] {
  const l1 = Math.abs(d[0]) + Math.abs(d[1]) + Math.abs(d[2]);
  let [x, z] = [d[0] / l1, d[2] / l1];
  if (d[1] < 0) [x, z] = [(1 - Math.abs(z)) * sx, (1 - Math.abs(x)) * sz];
  return [x * 0.5 + 0.5, z * 0.5 + 0.5];
}
function unoct(u: number, v: number): V3 {
  let [x, z] = [u * 2 - 1, v * 2 - 1];
  const y = 1 - Math.abs(x) - Math.abs(z);
  if (y < 0) [x, z] = [(1 - Math.abs(z)) * Math.sign(x || 1), (1 - Math.abs(x)) * Math.sign(z || 1)];
  return norm([x, y, z]);
}
// 3D value noise, 0 to 1: a stone painted by direction has no seams or streaks.
function n3(p: V3, s: number): number {
  const i = p.map(Math.floor), f = p.map((v, k) => smooth(0, 1, v - i[k]));
  const at = (a: number, b: number, c: number) => hash(i[0] + a, i[1] + b, i[2] + c + s * 7919);
  const lerp = (a: number, b: number, t: number) => a + (b - a) * t;
  const plane = (c: number) => lerp(lerp(at(0, 0, c), at(1, 0, c), f[0]), lerp(at(0, 1, c), at(1, 1, c), f[0]), f[1]);
  return lerp(plane(0), plane(1), f[2]);
}

// A stone's surface in its own frame, for a direction from its middle.
function stoneLocal(st: Stone, d: V3): V3 {
  const r = 1 + 0.34 * (n3(mul(d, 1.3), st.seed) - 0.5) + 0.1 * (n3(mul(d, 3.5), st.seed + 5) - 0.5);
  let p = mul(d, r);
  // Broken faces: what passes a plane is pressed back onto it, its edge rounded; a second across
  // the first, so no two stones share an egg's outline. Rounded field stones, as RDR2's ring:
  // cut to angular rocks, each its own scale and depth, they scored lower from the camp's eye
  // than e9e5b733's (campfire net round).
  const over = dot(p, st.cut) - 0.62;
  if (over > 0) p = sub(p, mul(st.cut, over * 0.85));
  const cut2 = norm([-st.cut[2] + 0.6 * (hash(st.seed, 3, 1) - 0.5), 0.2, st.cut[0]]);
  const over2 = dot(p, cut2) - (0.55 + 0.25 * hash(st.seed, 4, 1));
  if (over2 > 0) p = sub(p, mul(cut2, over2 * 0.8));
  return [p[0] * st.size[0], p[1] * st.size[1], p[2] * st.size[2]];
}
// Its frame in the fire's: its long side round the ring, its depth along the radius.
function stoneWorld(st: Stone, p: V3): V3 {
  const [c, s] = [Math.cos(st.turn), Math.sin(st.turn)];
  const tan: V3 = [-s, 0, c], rad: V3 = [c, 0, s];
  return add(st.at, add(add(mul(tan, p[0]), [0, p[1], 0]), mul(rad, p[2])));
}

function addStone(m: Mesh, st: Stone, tile: number) {
  st.from = m.idx.length / 3;
  const pts = SPHERE.v.map((d) => stoneWorld(st, stoneLocal(st, d)));
  // Cut flush at its base, where the contract puts its origin: what is sunk lies under the
  // ground it stands on, never under its origin (play::contract).
  const ground = 0;
  // Smooth normals on the stone as shaped, then a vertex a corner, for its folded mapping.
  const nrm: V3[] = pts.map(() => [0, 0, 0]);
  for (const [a, b, c] of SPHERE.f) {
    const n = cross(sub(pts[b], pts[a]), sub(pts[c], pts[a]));
    for (const i of [a, b, c]) nrm[i] = add(nrm[i], n);
  }
  const [tx, ty] = [(tile % (W / TILE)) * TILE, Math.floor(tile / (W / TILE)) * TILE];
  for (const f of SPHERE.f) {
    if (f.every((i) => pts[i][1] < ground)) continue;
    const mid = norm(f.reduce((a, i) => add(a, SPHERE.v[i]), [0, 0, 0] as V3));
    const first = m.pos.length / 3;
    for (const i of f) {
      const p = pts[i];
      m.pos.push(p[0], Math.max(p[1], ground), p[2]);
      // Outward, whichever way the sphere's faces wind (`face` turns the triangle outward too).
      const n = norm(nrm[i]);
      m.nrm.push(...(dot(n, sub(pts[i], st.at)) < 0 ? mul(n, -1) : n));
      const [u, v] = oct(SPHERE.v[i], Math.sign(mid[0] || 1), Math.sign(mid[2] || 1));
      m.uv.push((tx + 4 + u * (TILE - 8)) / W, (ty + 4 + v * (TILE - 8)) / W);
    }
    face(m, first, first + 1, first + 2, st.at);
  }
}

// Vertex normals from the faces round each (a log's runs and ends are their own vertices, so
// it is split at the crease between bark and split face and at its ends).
function normals(m: Mesh) {
  const n = new Float64Array(m.pos.length);
  for (let i = 0; i < m.idx.length; i += 3) {
    const [a, b, c] = [m.idx[i], m.idx[i + 1], m.idx[i + 2]];
    const p = (k: number): V3 => [m.pos[k * 3], m.pos[k * 3 + 1], m.pos[k * 3 + 2]];
    const f = cross(sub(p(b), p(a)), sub(p(c), p(a)));
    for (const k of [a, b, c]) for (let j = 0; j < 3; j++) n[k * 3 + j] += f[j];
  }
  m.nrm = [];
  for (let k = 0; k < n.length; k += 3) m.nrm.push(...norm([n[k], n[k + 1], n[k + 2]]));
}

// Tangents for the normal map, along +u, their sign so that the bitangent points up the
// image (toward -v), as glTF's normal maps are drawn (+Y up).
function tangents(m: Mesh): number[] {
  const t = new Float64Array(m.pos.length), b = new Float64Array(m.pos.length);
  for (let i = 0; i < m.idx.length; i += 3) {
    const ids = [m.idx[i], m.idx[i + 1], m.idx[i + 2]];
    const p = ids.map((k): V3 => [m.pos[k * 3], m.pos[k * 3 + 1], m.pos[k * 3 + 2]]);
    const w = ids.map((k) => [m.uv[k * 2], m.uv[k * 2 + 1]]);
    const [e1, e2] = [sub(p[1], p[0]), sub(p[2], p[0])];
    const [du1, dv1, du2, dv2] = [w[1][0] - w[0][0], w[1][1] - w[0][1], w[2][0] - w[0][0], w[2][1] - w[0][1]];
    const r = du1 * dv2 - du2 * dv1 || 1e-12;
    const T = mul(sub(mul(e1, dv2), mul(e2, dv1)), 1 / r), B = mul(sub(mul(e2, du1), mul(e1, du2)), 1 / r);
    for (const k of ids) for (let j = 0; j < 3; j++) [t[k * 3 + j], b[k * 3 + j]] = [t[k * 3 + j] + T[j], b[k * 3 + j] + B[j]];
  }
  const out: number[] = [];
  for (let k = 0; k < m.pos.length; k += 3) {
    const n: V3 = [m.nrm[k], m.nrm[k + 1], m.nrm[k + 2]];
    let T: V3 = [t[k], t[k + 1], t[k + 2]];
    T = norm(sub(T, mul(n, dot(n, T))));
    const up: V3 = [-b[k], -b[k + 1], -b[k + 2]];
    out.push(...T, dot(cross(n, T), up) < 0 ? -1 : 1);
  }
  return out;
}

/** Its logs and stones as two meshes. */
export function hearth(seed = 1): { wood: Mesh; stone: Mesh; logs: Log[]; stones: Stone[] } {
  const { logs, stones } = layout(seed);
  const wood = mesh(), stone = mesh();
  for (const l of logs) addLog(wood, l);
  normals(wood);
  stones.forEach((st, i) => addStone(stone, st, i));
  return { wood, stone, logs, stones };
}

// 2D cellular noise: the distances to the nearest two cells' points, and the nearest's hash.
function cells(x: number, y: number, s: number): [number, number, number] {
  const [xi, yi] = [Math.floor(x), Math.floor(y)];
  let [d1, d2, id] = [9, 9, 0];
  for (let j = -1; j <= 1; j++) {
    for (let i = -1; i <= 1; i++) {
      const h = hash(xi + i, yi + j, s);
      const [px, py] = [xi + i + 0.15 + 0.7 * h, yi + j + 0.15 + 0.7 * hash(xi + i, yi + j, s + 1)];
      const d = Math.hypot(x - px, y - py);
      if (d < d1) [d1, d2, id] = [d, d1, h];
      else if (d < d2) d2 = d;
    }
  }
  return [d1, d2, id];
}

// A texel of wood: colour (sRGB), glow (sRGB), roughness and height (0-1).
type Texel = [number[], number[], number, number];
const DARK = [0, 0, 0];
// Bark in plates: cells long along the log and warped, their furrows of varied width and
// depth, a coarse layer of deep furrows over a fine one, so it is broken, never corduroy.
function bark(s: number, t: number, seed: number): Texel {
  const ws = 0.008 * (fbm(s * 50, t * 14, seed + 30) - 0.5), wt = 0.03 * (fbm(s * 25, t * 6, seed + 31) - 0.5);
  const furrow = (cs: number, ct: number, k: number, w0: number, w1: number) => {
    const [d1, d2, id] = cells((s + ws) / cs, (t + wt) / ct, seed + k);
    const width = w0 + w1 * fbm(s * 30, t * 5, seed + k + 1);
    return [1 - smooth(0, width, d2 - d1), id];
  };
  const [deep, plate] = furrow(0.017, 0.085, 32, 0.05, 0.2);
  const [fine] = furrow(0.008, 0.035, 34, 0.04, 0.14);
  const flake = smooth(0.6, 0.72, fbm(s * 170, t * 40, seed + 36));
  const top = 0.6 + 0.25 * plate + 0.15 * fbm(s * 140, t * 70, seed + 1) - 0.12 * flake;
  const h = top * (1 - 0.85 * deep) * (1 - 0.45 * fine);
  const tone = 0.85 + 0.3 * fbm(s * 9, t * 4, seed + 2);
  let c = mix([26, 21, 17], [84, 71, 57], smooth(0.05, 0.7, h)).map((x) => x * tone * (0.9 + 0.2 * plate));
  // Weathered grey on the plates' tops.
  c = mix(c, [112, 108, 99], 0.4 * smooth(0.55, 0.75, fbm(s * 14, t * 9, seed + 3)) * smooth(0.45, 0.75, h));
  return [c, DARK, 0.92, h];
}
// Split wood: weathered grey-brown, its fibres along it, long darker streaks, its growth
// rings as stripes at their distance from the pith, a few drying checks along the grain.
function splitWood(s: number, t: number, r: number, R: number, seed: number): Texel {
  const fibre = fbm(s * 420, t * 9, seed + 4);
  const streak = fbm(s * 70, t * 1.6, seed + 40);
  const ring = smooth(0.62, 0.92, frac(r / 0.0055 + 0.8 * fbm(s * 40, t * 3, seed + 5)));
  const sap = smooth(0.78, 0.9, r / R);
  const tone = 0.86 + 0.28 * fbm(s * 6, t * 3, seed + 6);
  let c = mix([136, 114, 90], [98, 79, 60], 0.55 * ring + 0.35 * (1 - fibre)).map((x) => x * tone);
  c = mix(c, [146, 130, 110], 0.4 * sap);
  c = mix(c, [76, 64, 54], 0.55 * smooth(0.55, 0.78, streak));
  // Weathered grey and smoked darker, as firewood stacked and burning is.
  c = mix(c, [114, 110, 102], 0.3 + 0.3 * fbm(s * 12, t * 2, seed + 41)).map((x) => x * 0.86);
  const u = s / 0.026 + 0.6 * (fbm(s * 10, t * 4, seed + 42) - 0.5), col = Math.floor(u);
  const check = (1 - smooth(0.02, 0.06, Math.abs(frac(u) - 0.5))) * smooth(0.58, 0.72, noise(hash(col, seed, 43) * 97, t * 6, seed + 44));
  c = mix(c, [40, 32, 26], 0.85 * check);
  return [c, DARK, 0.82, 0.55 + 0.3 * fibre - 0.25 * ring - 0.12 * streak - 0.45 * check];
}
function endGrain(x: number, y: number, r: number, R: number, bark: boolean, seed: number): Texel {
  const a = Math.atan2(y, x);
  const ring = smooth(0.66, 0.95, frac(r / 0.0048 + 1.2 * fbm(x * 30, y * 30, seed + 7)));
  // Radial checks from the pith, as drying wood splits.
  let check = 0;
  for (let k = 0; k < 4; k++) {
    const ca = hash(k, seed, 9) * 2 * Math.PI, reach = (0.4 + 0.5 * hash(k, seed, 10)) * R;
    const off = Math.abs(Math.sin(a - ca)) * r;
    if (Math.cos(a - ca) > 0 && r < reach) check = Math.max(check, 1 - smooth(0.0004, 0.0016 * (1 - r / reach) + 0.0005, off));
  }
  const tone = 0.9 + 0.2 * fbm(x * 12, y * 12, seed + 8);
  let c = mix([172, 132, 88], [118, 82, 50], ring).map((v) => v * tone);
  c = mix(c, [44, 31, 22], check);
  let h = 0.6 - 0.15 * ring - 0.5 * check;
  if (bark && r > R * 0.93) [c, h] = [mix([44, 35, 28], [70, 58, 46], fbm(a * 20, r * 300, seed)), 0.8];
  return [c, DARK, 0.86, h];
}
// Checked char along the grain, as wood burns to alligator char: fissures along it at a
// across it (warped, of varied spacing), shorter cross-checks between them staggered column
// to column, some missing. Its crack (0-1, deepest the fissures), how far inside a block
// (0 at its rim, 1 inside), and the block's hash. `open` widens the cracks as it burns deeper;
// `stretch` is metres a unit of a, where a is not in metres (an end's angle).
function checks(a: number, b: number, open: number, seed: number, stretch: number): [number, number, number] {
  const SA = 0.017;
  const u = a / SA + 1.4 * (fbm(a * 20, b * 16, seed + 50) - 0.5) + 0.5 * (fbm(a * 60, b * 45, seed + 49) - 0.5), col = Math.floor(u);
  const dl = Math.min(frac(u), 1 - frac(u)) * SA * stretch;
  // Blocks about as long as wide, crossed nearly everywhere, as alligator char is: at 2.4 to
  // 5.4 cm long, half uncrossed, its glowing fissures ran along the grain as bristles.
  const run = 0.016 + 0.014 * hash(col, seed, 51);
  const v = b / run + hash(col, seed, 52) + 0.5 * (fbm(a * 40, b * 12, seed + 53) - 0.5), row = Math.floor(v);
  const dt = Math.min(frac(v), 1 - frac(v)) * run;
  const crossed = hash(col, Math.round(v), 54) > 0.04 + 0.2 * noise(a * 30, b * 8, seed + 48) ? 1 : 0;
  // Fewer fissures along the grain than across it: at seven columns in ten, a log's glowing
  // seams ran its length as bristles (blind judge, campfire tongue round: "straw or brush").
  const along = smooth(0.4, 0.55, noise(col * 7.3, b * 5, seed + 55));
  const wl = (0.0007 + 0.0016 * fbm(a * 12, b * 3, seed + 56)) * (0.5 + 1.5 * open);
  const wt = (0.0005 + 0.001 * hash(col, row, 57)) * (0.5 + 1.5 * open);
  const crack = Math.max(along * (1 - smooth(wl * 0.4, wl, dl)), 0.75 * crossed * (1 - smooth(wt * 0.4, wt, dt)));
  const inside = smooth(0, 0.005, Math.min(along > 0.5 ? dl : 1, crossed ? dt : 1));
  return [crack, inside, hash(col, row, 58)];
}
// Char over any wood by how hot it got (a across the grain, b along it; `groove` its bark's
// furrows, which burn and glow first): scorched brown, then black checked char whose cracks
// glow ember orange, deepest and hottest brightest; their albedo stays dark (the glow is
// emissive, `play::props`). Its whole crack net glows, on every face: only the deepest runs lit,
// the logs read dark from the camp's eye, above (campfire net round: e9e5b733's bright net on
// every face scored higher with every judge, twice).
function charred(w: Texel, heat: number, a: number, b: number, seed: number, groove = 0, stretch = 1): Texel {
  const [c0, , r0, h0] = w;
  // Its front ragged, in tongues along the grain, never a band.
  const jit = 0.42 * (fbm(a * 14, b * 6, seed + 9) - 0.5) + 0.2 * (fbm(a * 55, b * 22, seed + 19) - 0.5);
  const x = heat + jit;
  const scorch = smooth(0.1, 0.3, x), char = smooth(0.27, 0.4, x);
  let c = mix(c0, mix(c0.map((v) => v * 0.5), [52, 34, 24], 0.6), scorch);
  if (char <= 0) return [c, DARK, r0, h0];
  const hot = smooth(0.4, 0.82, x);
  // How far toward the coals: the cracks burn hotter there, opening only a little wider
  // (fire round 32: opened 2.2 times, the feet's fissures stood as even pale stripes, "like a
  // barcode"); the bark's furrows glow dull, never as deep as a fissure.
  const feet = smooth(0.55, 1.25, x);
  const [crack, inside, id] = checks(a, b, hot * (0.6 + 0.5 * feet), seed + 11, stretch);
  const deep = Math.max(crack, 0.55 * groove * smooth(0.35, 0.7, x));
  let block = mix([15, 13, 12], [40, 36, 33], 0.2 * id + 0.8 * fbm(a * 70, b * 30, seed));
  // Grey-white ash on the blocks, in patches, from where it chars through to where it burns
  // hottest: only on the hottest it read as no ash at all (blind judge, campfire ember round).
  // Little where it glows hottest, whose firelit ash read tan (blind judge, campfire glow round).
  const ash = 0.35 * smooth(0.6, 0.95, x) * (1 - 0.6 * smooth(0.85, 1.1, x)) * smooth(0.58, 0.74, fbm(a * 60, b * 25, seed + 59));
  block = mix(block, [124, 120, 114], ash);
  c = mix(c, mix(block.map((v) => v * (0.72 + 0.4 * inside)), [30, 13, 8], deep), char);
  const leak = 0.3 * (1 - inside) * hot;
  // A fissure's heat: its deepest middle and toward the coals the hottest, dull orange-red at
  // its lips up the log, through ember orange to yellow-white in the deepest seams at the feet
  // (fire round 31: an even-width pink-red neon mesh, its red near pure, which the night's
  // grade takes to pink).
  // Some runs of a seam burn hot and some dull, in patches along it, so its yellow is few and
  // irregular; never past the ember's orange, whose yellow read tan by firelight (campfire glow round).
  const vary = smooth(0.3, 0.8, fbm(a * 5, b * 2.2, seed + 62));
  const t = Math.min(0.5, deep * deep * hot * (0.55 + 0.6 * feet) * (0.35 + 0.9 * vary));
  const g = Math.min(1, deep + leak) * hot * char * (0.5 + 0.7 * fbm(a * 14, b * 4, seed + 60)) * (0.8 + 0.3 * id) * (0.55 + 0.7 * t);
  const tone = t < 0.5 ? mix([196, 64, 8], HOTTEST, t * 2) : mix(HOTTEST, [255, 206, 120], t * 2 - 1);
  // Where it lies in the coals its whole char glows, not its cracks alone: a broad warm gradient
  // over its blocks, brightest at the hottest, broken by the char's mottle (fire round 31: dark
  // logs standing on the bed).
  // A soft orange gradient rising from the coals, its mottle broad (fire round 32).
  // Unbroken where it lies in the coals, its mottle only dimming it (campfire fill round: the
  // glowing blocks read as a lattice; the judges ask a continuous glow on the undersides and
  // contacts), redder where it is coolest.
  const bed = smooth(0.5, 1.05, x) * (0.5 + 0.5 * smooth(0.35, 0.65, fbm(a * 4, b * 1.6, seed + 61))) * (1 - 0.35 * inside);
  const bedTone = mix([200, 52, 8], [255, 104, 24], smooth(0.8, 1.25, x));
  const glow = tone.map((v, i) => v * g + bedTone[i] * bed);
  // Charcoal is matte: its blocks' faces about 0.86 for their faint sheen, its fissures and
  // ash rougher still (fire round 37: at 0.6 the char took the fire's light as a gloss, and
  // read as glossy fresh-cut wood).
  const rc = Math.min(1, 0.86 + 0.06 * (1 - inside) + 0.08 * deep + 0.1 * ash);
  return [c, glow, r0 + (rc - r0) * char, h0 + (0.42 + 0.3 * inside - 0.45 * deep - h0) * char];
}

// A log's section at s round it: the point, its outward normal in the plane, and bark or not.
function at(l: Log, s: number): [[number, number], [number, number], boolean] {
  for (const run of l.runs) {
    if (s > run.s.at(-1)! && run !== l.runs.at(-1)) continue;
    let k = 0;
    while (k + 2 < run.s.length && run.s[k + 1] < s) k++;
    const [a, b] = [run.pts[k], run.pts[k + 1]];
    const f = clamp((s - run.s[k]) / (run.s[k + 1] - run.s[k] || 1));
    const [dx, dy] = [b[0] - a[0], b[1] - a[1]], len = Math.hypot(dx, dy) || 1;
    return [[a[0] + dx * f, a[1] + dy * f], [dy / len, -dx / len], run.bark];
  }
  throw new Error("past the section");
}
// How hot a log is at t along it on a face of this normal: its underside, toward the middle
// and down, licked by the flames all along; its inner end over the flames and its foot in the
// coals most; its upper outer face raw but for its top.
// Burnt black from its middle to its inner end, its cracks glowing on its burnt faces, and its
// bark kept only on its cool outer end, as a teepee's logs burn from the apex down: charred
// all over with glowing seams along the grain at its foot too, its ends read as orange-striped
// brushes (blind judges twice, campfire cut round 3). Its foot's underside, in the coals, glows.
function heat(l: Log, t: number, n: V3): number {
  const inward = -dot(n, l.out), down = -n[1], f = t / l.L;
  const under = clamp(0.5 + 0.55 * inward + 0.35 * down);
  // Its foot hot on the faces toward the coals and under, not on top: hot on every face, the
  // feet read as a glowing waffle grid from above (campfire view round).
  // Its upper half hot on every face, its upper faces too, with hot edges toward the gaps: from
  // the camp's eye, above, the eye sees mostly the logs' upper faces, and a glow only low and
  // where they cross read as black sticks over a yellow grid at their feet (campfire history
  // panel: e9e5b733's upper glow 4.75, the low band 4.00, every judge).
  const foot = smooth(0.3, 0.0, f) * under * under * 0.95;
  const top = smooth(0.3, 0.85, f) * (0.85 + 0.35 * under);
  // Its underside licked hot enough that its cracks glow there, seen low between the logs.
  const lick = under * under * (0.72 + 0.3 * f);
  // Smoke and heat scorch it all over and char it black from about its middle up (fire round
  // 25: its raw upper faces read as pale planks); its outer end charred in patches through its
  // scorched bark (blind judge, campfire ember round: brown bark read as brush, not burnt logs).
  const all = 0.34 + 0.3 * smooth(0.2, 0.55, f);
  // Its sides toward its neighbours, which see the coals and the flames through the gaps
  // between the logs, glow along their edges, more toward the inner end.
  const across = Math.abs(dot(n, norm(cross([0, 1, 0], l.out))));
  const rim = smooth(0.3, 0.85, across) * (0.62 + 0.25 * f);
  return Math.max(top, foot, lick, all, rim);
}

/** Edge padding, as a baker dilates its islands: each unpainted texel within `reach` (here the
 * whole gutter) of an island takes its nearest painted neighbour's values, so no mip averages the
 * gutter into an island's edge (fire round 37: the white gutter drew pale lines along each log's
 * seams from a few metres, and its height a ridge in the normals). */
function bleed(maps: Uint8Array[], height: Float32Array, painted: Uint8Array, reach: number) {
  let front: number[] = [];
  for (let i = 0; i < W * W; i++) if (painted[i]) front.push(i);
  for (let step = 0; step < reach && front.length; step++) {
    const next: number[] = [];
    for (const i of front) {
      const [x, y] = [i % W, Math.floor(i / W)];
      for (const [dx, dy] of [[1, 0], [-1, 0], [0, 1], [0, -1]]) {
        const [u, v] = [x + dx, y + dy];
        if (u < 0 || v < 0 || u >= W || v >= W || painted[v * W + u]) continue;
        const j = v * W + u;
        painted[j] = 1;
        for (const m of maps) m.copyWithin(j * 4, i * 4, i * 4 + 4);
        height[j] = height[i];
        next.push(j);
      }
    }
    front = next;
  }
}

/** The wood atlas's colour, emissive, metal-roughness and normal maps, RGBA a byte each. */
export function woodMaps(logs: Log[]): Uint8Array[] {
  const out = [0, 1, 2, 3].map(() => new Uint8Array(W * W * 4).fill(255));
  out[1].fill(0);
  const height = new Float32Array(W * W).fill(0.5);
  const painted = new Uint8Array(W * W);
  const put = (x: number, y: number, [c, g, r, h]: Texel) => {
    const o = (y * W + x) * 4;
    for (let i = 0; i < 3; i++) [out[0][o + i], out[1][o + i]] = [Math.round(Math.min(255, c[i])), Math.round(Math.min(255, g[i]))];
    out[1][o + 3] = 255;
    out[2].set([0, Math.round(r * 255), 0], o);
    height[y * W + x] = h;
    painted[y * W + x] = 1;
  };
  for (const l of logs) {
    // Its side: s round it, t along it.
    for (let y = 0; y < Math.ceil(l.L * D) + 4; y++) {
      for (let x = 0; x < Math.ceil(l.P * D) + 4; x++) {
        const s = Math.min(Math.max((x - 2 + 0.5) / D, 0), l.P), t = Math.min(Math.max((y - 2 + 0.5) / D, 0), l.L);
        const [p, n2, isBark] = at(l, s);
        const n: V3 = add(mul(l.e1, n2[0]), mul(l.e2, n2[1]));
        const w = isBark ? bark(s, t, l.seed) : splitWood(s, t, Math.hypot(p[0], p[1]), l.R, l.seed);
        put(l.side[0] + x, l.side[1] + y, charred(w, heat(l, t, n), s, t, l.seed, isBark ? smooth(0.3, 0.08, w[3]) : 0));
      }
    }
    // Its ends: the foot's in the coals and the top's over the flames, both charred.
    l.caps.forEach(([cx, cy], end) => {
      for (let y = 0; y < Math.ceil((l.box[3] - l.box[1]) * D) + 4; y++) {
        for (let x = 0; x < Math.ceil((l.box[2] - l.box[0]) * D) + 4; x++) {
          const [px, py] = [l.box[0] + (x - 2 + 0.5) / D, l.box[1] + (y - 2 + 0.5) / D];
          const r = Math.hypot(px, py);
          const w = endGrain(px, py, r, l.R, true, l.seed + end);
          // The foot's end, out of the fire, only scorched round its bark (`heat`); the inner
          // end over the flames charred black, its cracks glowing.
          const hot = end ? 0.68 + 0.16 * fbm(px * 40, py * 40, l.seed) : 0.12 + 0.2 * smooth(0.6, 1, r / l.R);
          // Its char split along the radial checks and round the rings, as a log's end burns.
          put(cx + x, cy + y, charred(w, hot, Math.atan2(py, px) * 0.024, r, l.seed + 13, 0, Math.max(r, 0.008) / 0.024));
        }
      }
    });
  }
  bleed([out[0], out[1], out[2]], height, painted, W);
  // The normal map from the height: relief of about 3 mm a unit, +Y up the image.
  const k = 3;
  for (let y = 1; y < W - 1; y++) {
    for (let x = 1; x < W - 1; x++) {
      const i = y * W + x;
      const n = norm([-k * (height[i + 1] - height[i - 1]) * 0.5, -k * (height[i - W] - height[i + W]) * 0.5, 1]);
      out[3].set([0, 1, 2].map((j) => Math.round((n[j] * 0.5 + 0.5) * 255)), i * 4);
    }
  }
  return out;
}

/** The stone atlas's colour and metal-roughness maps: a tile a stone, painted by direction. */
export function stoneMaps(stones: Stone[]): Uint8Array[] {
  const out = [0, 1].map(() => new Uint8Array(W * W * 4).fill(255));
  stones.forEach((st, tile) => {
    const [tx, ty] = [(tile % (W / TILE)) * TILE, Math.floor(tile / (W / TILE)) * TILE];
    for (let y = 0; y < TILE; y++) {
      for (let x = 0; x < TILE; x++) {
        const d = unoct((x - 4 + 0.5) / (TILE - 8), (y - 4 + 0.5) / (TILE - 8));
        const p = mul(d, 1);
        const mottle = n3(mul(p, 4), st.seed + 20), grain = n3(mul(p, 70), st.seed + 21), vein = n3(mul(p, 9), st.seed + 22);
        let c = st.tone.map((v) => v * (0.82 + 0.36 * mottle));
        c = mix(c, [58, 56, 54], smooth(0.7, 0.78, grain) * 0.8);
        c = mix(c, [146, 140, 130], smooth(0.3, 0.22, grain) * 0.45);
        c = mix(c, c.map((v) => v * 1.15), smooth(0.48, 0.52, vein) * (1 - smooth(0.52, 0.56, vein)));
        // Lichen on what faces up, away from the fire's heat.
        c = mix(c, [132, 138, 112], 0.55 * smooth(0.2, 0.5, d[1]) * smooth(0.6, 0.68, n3(mul(p, 6), st.seed + 23)) * smooth(-0.2, 0.3, d[2]));
        // Black soot on the face toward the fire (its -z, inward), most at its top, and over
        // its top from that side, broken by its mottle; earth at its foot. Sooted only low, its
        // firelit tops read salmon from the camp's eye, above (campfire history panel: e9e5b733's
        // sooted ring 4.75, the low soot 4.00, every judge).
        const inner = smooth(-0.05, -0.65, d[2]) * (0.6 + 0.4 * smooth(-0.3, 0.5, d[1]));
        const top = smooth(0.35, 0.8, d[1]) * smooth(0.5, -0.4, d[2]) * 0.7;
        const soot = Math.min(1, Math.max(inner, top) * (0.65 + 0.5 * smooth(0.3, 0.7, n3(mul(p, 11), st.seed + 24))));
        c = mix(c, [22, 20, 19], 0.85 * soot);
        // The smoke's stain over its upper half, broken, darkest toward the fire.
        c = mix(c, [40, 37, 34], 0.35 * smooth(-0.1, 0.6, d[1]) * smooth(0.6, -0.5, d[2]) * smooth(0.25, 0.65, n3(mul(p, 5), st.seed + 25)));
        c = mix(c, [78, 64, 48], 0.7 * smooth(-0.05, -0.45, d[1]));
        const o = ((ty + y) * W + tx + x) * 4;
        for (let i = 0; i < 3; i++) out[0][o + i] = Math.round(Math.min(255, c[i]));
        out[1].set([0, Math.round((0.84 - 0.12 * soot + 0.1 * grain) * 255), 0], o);
      }
    }
  });
  return out;
}

// A map as a JPEG (quality 90) at a side, or a PNG, through ImageMagick.
function image(rgba: Uint8Array, side: number, jpeg: boolean): Buffer {
  const dir = mkdtempSync(join(tmpdir(), "hearth-"));
  try {
    const png = join(dir, "in.png"), to = join(dir, jpeg ? "out.jpg" : "out.png");
    writeFileSync(png, encodePng({ width: W, height: W, rgba }));
    execFileSync("magick", [png, "-alpha", "off", "-resize", `${side}x${side}`, ...(jpeg ? ["-quality", "90"] : []), to]);
    return readFileSync(to);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

/** The campfire, without its coal bed, as a glTF binary. */
export function campfire(seed = 1): Buffer {
  const { wood, stone, logs, stones } = hearth(seed);
  const wm = woodMaps(logs), sm = stoneMaps(stones);
  const j: any = { asset: { version: "2.0", generator: "terra tools/props/hearth.ts" }, scene: 0, scenes: [{ nodes: [0] }], nodes: [{ mesh: 0, name: "Campfire" }], meshes: [{ primitives: [] }], materials: [], textures: [], images: [], samplers: [{}], accessors: [], bufferViews: [], buffers: [{ byteLength: 0 }], extensionsUsed: ["KHR_materials_emissive_strength"] };
  const parts: Buffer[] = [];
  let len = 0;
  const view = (data: Buffer, target?: number) => {
    const pad = (4 - (len % 4)) % 4;
    if (pad) parts.push(Buffer.alloc(pad));
    len += pad;
    parts.push(data);
    j.bufferViews.push({ buffer: 0, byteOffset: len, byteLength: data.length, ...(target ? { target } : {}) });
    len += data.length;
    return j.bufferViews.length - 1;
  };
  const acc = (data: number[], type: string, n: number) => {
    const a: any = { bufferView: view(Buffer.from(new Float32Array(data).buffer), 34962), componentType: 5126, count: data.length / n, type };
    if (type === "VEC3") {
      a.min = [0, 1, 2].map((i) => data.reduce((m, v, k) => (k % 3 === i ? Math.min(m, v) : m), Infinity));
      a.max = [0, 1, 2].map((i) => data.reduce((m, v, k) => (k % 3 === i ? Math.max(m, v) : m), -Infinity));
    }
    return j.accessors.push(a) - 1;
  };
  const tex = (data: Buffer, mime: string) => {
    j.images.push({ mimeType: mime, bufferView: view(data) });
    return { index: j.textures.push({ sampler: 0, source: j.images.length - 1 }) - 1 };
  };
  const prim = (m: Mesh, material: number, tan?: number[]) => {
    const attributes: any = { POSITION: acc(m.pos, "VEC3", 3), NORMAL: acc(m.nrm, "VEC3", 3), TEXCOORD_0: acc(m.uv, "VEC2", 2) };
    if (tan) attributes.TANGENT = acc(tan, "VEC4", 4);
    const indices = j.accessors.push({ bufferView: view(Buffer.from(new Uint32Array(m.idx).buffer), 34963), componentType: 5125, count: m.idx.length, type: "SCALAR" }) - 1;
    j.meshes[0].primitives.push({ attributes, indices, material });
  };
  j.materials.push({
    name: "Wood",
    pbrMetallicRoughness: { baseColorTexture: tex(image(wm[0], W, true), "image/jpeg"), metallicRoughnessTexture: tex(image(wm[2], W / 2, true), "image/jpeg"), metallicFactor: 0 },
    normalTexture: tex(image(wm[3], W, true), "image/jpeg"),
    emissiveTexture: tex(image(wm[1], W / 2, false), "image/png"),
    emissiveFactor: [1, 1, 1],
    extensions: { KHR_materials_emissive_strength: { emissiveStrength: 3 } },
  });
  j.materials.push({ name: "Stone", pbrMetallicRoughness: { baseColorTexture: tex(image(sm[0], W / 2, true), "image/jpeg"), metallicRoughnessTexture: tex(image(sm[1], W / 4, true), "image/jpeg"), metallicFactor: 0 } });
  prim(wood, 0, tangents(wood));
  prim(stone, 1);
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
