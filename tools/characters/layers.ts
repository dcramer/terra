#!/usr/bin/env node
// One piece worn over another through the clips: where the inner piece pokes out through the
// outer, and whether the outer still covers the inner's top edge, so the empty inside of a
// head-only body never shows (D599: their outfits are worn on the head alone, the pieces
// overlapping at the seams). A measurement, never a fix: their pieces stay as made.
//
//   node tools/characters/layers.ts OUTER.gltf INNER.gltf [--clips FILE.glb] [--frames N]
//
// Both pieces are posed on the outer file's skeleton by the clips of `--clips` (the mannequin's
// by default), carried across as retarget.py carries them (D542, in short): each bone's turn in
// the world from the clip skeleton's rest laid on the body's rest, the pelvis moved as the
// clip's, scaled by the two pelvises' heights; then skinned. For each vertex of the inner
// piece near the outer at rest, a ray from the nearest point on the bone it follows most out
// through the vertex: the outer met before the vertex and not after it is a poke-through, by
// the distance between; met beyond it, covered. The inner's top edge (its open boundary within
// 3 cm of its highest point) should be covered in every frame.
//
// Node 22.18 or later; no dependencies.

import { fileURLToPath } from "node:url";
import { type Clip, Glb, matrix, mmul, type Pose, posed, qinv, qmul, rot, type V3 } from "./glb.ts";
import { type Piece, pieces } from "./intact.ts";

export const MANNEQUIN = fileURLToPath(new URL("../../assets/models/character/mannequin.glb", import.meta.url));

/** Each node of `body` by name, in the model's space, posed by `clip` at `t` as made on `from`'s
 *  skeleton (no clip: at rest). */
export function retargeted(body: Glb, from: Glb, clip: Clip | undefined, t: number): Map<string, Pose> {
  const rest = posed(body, undefined, 0);
  const out = new Map<string, Pose>();
  if (!clip) {
    body.gltf.nodes.forEach((n, i) => n.name && out.set(n.name, rest[i]));
    return out;
  }
  const fromRest = posed(from, undefined, 0);
  const fromNow = posed(from, clip, t);
  const height = (g: Glb, p: Pose[]) => p[g.named("pelvis")]?.t[1] || 1;
  const k = height(body, rest) / height(from, fromRest);
  const world: Pose[] = [];
  const at = (i: number): Pose => {
    if (world[i]) return world[i];
    const n = body.gltf.nodes[i];
    const local = body.rest(i);
    const parent = body.parentOf[i] === undefined ? undefined : at(body.parentOf[i]);
    const j = n.name ? from.named(n.name) : -1;
    const r = j >= 0 ? qmul(qmul(fromNow[j].r, qinv(fromRest[j].r)), rest[i].r) : parent ? qmul(parent.r, local.r) : local.r;
    let p: V3 = parent ? add(parent.t, rot(parent.r, [local.t[0] * parent.s[0], local.t[1] * parent.s[1], local.t[2] * parent.s[2]])) : local.t;
    if (j >= 0 && (n.name === "pelvis" || n.name === "root")) p = add(rest[i].t, scale(sub3(fromNow[j].t, fromRest[j].t), k));
    world[i] = { t: p, r, s: rest[i].s };
    if (n.name) out.set(n.name, world[i]);
    return world[i];
  };
  body.gltf.nodes.forEach((_, i) => at(i));
  return out;
}

const add = (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const sub3 = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const scale = (a: V3, k: number): V3 => [a[0] * k, a[1] * k, a[2] * k];

/** A piece's vertices skinned by the joints' poses (linear blend skinning, as the game draws). */
export function skinned(p: Piece, joints: Map<string, Pose>): Float64Array {
  const m = new Map<string, number[]>();
  for (const [name, ibm] of p.bind) {
    const j = joints.get(name);
    if (j) m.set(name, mmul(matrix(j), ibm));
  }
  const out = new Float64Array(p.count * 3);
  for (let i = 0; i < p.count; i++) {
    const [x, y, z] = [p.pos[3 * i], p.pos[3 * i + 1], p.pos[3 * i + 2]];
    let total = 0;
    for (const [name, w] of p.weights[i]) {
      const a = m.get(name);
      if (!a) continue;
      total += w;
      out[3 * i] += w * (a[0] * x + a[4] * y + a[8] * z + a[12]);
      out[3 * i + 1] += w * (a[1] * x + a[5] * y + a[9] * z + a[13]);
      out[3 * i + 2] += w * (a[2] * x + a[6] * y + a[10] * z + a[14]);
    }
    if (total > 0) for (let c = 0; c < 3; c++) out[3 * i + c] /= total;
  }
  return out;
}

/** The distances along a ray from `o` (unit `d`) at which it crosses the triangles listed
 *  (Möller and Trumbore 1997), either face, edges counted to both sides of them: a ray along a
 *  mirrored piece's middle runs exactly through its seam, where float error misses both. */
const EDGE = 1e-6;
export function hits(o: V3, d: V3, pos: Float64Array, tris: Uint32Array, list: number[], out: number[]): number[] {
  out.length = 0;
  for (const k of list) {
    const [a, b, c] = [3 * tris[k], 3 * tris[k + 1], 3 * tris[k + 2]];
    const e1 = [pos[b] - pos[a], pos[b + 1] - pos[a + 1], pos[b + 2] - pos[a + 2]];
    const e2 = [pos[c] - pos[a], pos[c + 1] - pos[a + 1], pos[c + 2] - pos[a + 2]];
    const p = [d[1] * e2[2] - d[2] * e2[1], d[2] * e2[0] - d[0] * e2[2], d[0] * e2[1] - d[1] * e2[0]];
    const det = e1[0] * p[0] + e1[1] * p[1] + e1[2] * p[2];
    if (Math.abs(det) < 1e-12) continue;
    const s = [o[0] - pos[a], o[1] - pos[a + 1], o[2] - pos[a + 2]];
    const u = (s[0] * p[0] + s[1] * p[1] + s[2] * p[2]) / det;
    if (u < -EDGE || u > 1 + EDGE) continue;
    const q = [s[1] * e1[2] - s[2] * e1[1], s[2] * e1[0] - s[0] * e1[2], s[0] * e1[1] - s[1] * e1[0]];
    const v = (d[0] * q[0] + d[1] * q[1] + d[2] * q[2]) / det;
    if (v < -EDGE || u + v > 1 + EDGE) continue;
    const t = (e2[0] * q[0] + e2[1] * q[1] + e2[2] * q[2]) / det;
    if (t > 0) out.push(t);
  }
  return out;
}

/** Triangles (index of the first corner) filed by the cubes of `size` their bounds touch, so a
 *  ray tries only those in the cubes it passes (Amanatides and Woo 1987), not every one. */
export class Grid {
  private cells = new Map<number, number[]>();
  private seen: Uint32Array;
  private stamp = 0;
  private size: number;
  constructor(pos: Float64Array, tris: Uint32Array, keep: (k: number) => boolean = () => true, size = 0.02) {
    this.size = size;
    this.seen = new Uint32Array(tris.length / 3 + 1);
    const cube = (x: number) => Math.floor(x / size);
    for (let k = 0; k < tris.length; k += 3) {
      if (!keep(k)) continue;
      const lo = [Infinity, Infinity, Infinity];
      const hi = [-Infinity, -Infinity, -Infinity];
      for (let c = 0; c < 3; c++)
        for (let a = 0; a < 3; a++) ((lo[a] = Math.min(lo[a], pos[3 * tris[k + c] + a])), (hi[a] = Math.max(hi[a], pos[3 * tris[k + c] + a])));
      for (let x = cube(lo[0] - 1e-4); x <= cube(hi[0] + 1e-4); x++)
        for (let y = cube(lo[1] - 1e-4); y <= cube(hi[1] + 1e-4); y++)
          for (let z = cube(lo[2] - 1e-4); z <= cube(hi[2] + 1e-4); z++) {
            const id = Grid.id(x, y, z);
            const list = this.cells.get(id);
            if (list) list.push(k);
            else this.cells.set(id, [k]);
          }
    }
  }
  private static id = (x: number, y: number, z: number) => ((x + 4096) * 8192 + (y + 4096)) * 8192 + (z + 4096);
  /** The triangles in the cubes a ray from `o` along unit `d` passes within `length`. */
  along(o: V3, d: V3, length: number, out: number[] = []): number[] {
    out.length = 0;
    this.stamp++;
    const s = this.size;
    const i = [Math.floor(o[0] / s), Math.floor(o[1] / s), Math.floor(o[2] / s)];
    const step = d.map((x) => (x > 0 ? 1 : x < 0 ? -1 : 0));
    const next = [0, 1, 2].map((a) => (d[a] === 0 ? Infinity : ((i[a] + (step[a] > 0 ? 1 : 0)) * s - o[a]) / d[a]));
    const delta = [0, 1, 2].map((a) => (d[a] === 0 ? Infinity : s / Math.abs(d[a])));
    for (let t = 0; t <= length; ) {
      for (const k of this.cells.get(Grid.id(i[0], i[1], i[2])) ?? [])
        if (this.seen[k / 3] !== this.stamp) ((this.seen[k / 3] = this.stamp), out.push(k));
      const a = next[0] < next[1] ? (next[0] < next[2] ? 0 : 2) : next[1] < next[2] ? 1 : 2;
      t = next[a];
      next[a] += delta[a];
      i[a] += step[a];
    }
    return out;
  }
}

/** The vertices on a piece's open boundary within `band` of its highest point: its top edge. */
export function topEdge(p: Piece, band = 0.03): number[] {
  let top = -Infinity;
  for (let i = 0; i < p.count; i++) top = Math.max(top, p.pos[3 * i + 1]);
  return openEdge(p).filter((i) => p.pos[3 * i + 1] > top - band);
}

/** The vertices on a piece's open boundary (edges of one triangle), one per place. */
export function openEdge(p: Piece): number[] {
  const key = (i: number) => `${Math.round(p.pos[3 * i] * 1e4)},${Math.round(p.pos[3 * i + 1] * 1e4)},${Math.round(p.pos[3 * i + 2] * 1e4)}`;
  const keys = Array.from({ length: p.count }, (_, i) => key(i));
  const edges = new Map<string, number>();
  for (let k = 0; k < p.tris.length; k += 3)
    for (let e = 0; e < 3; e++) {
      const [a, b] = [keys[p.tris[k + e]], keys[p.tris[k + ((e + 1) % 3)]]];
      const id = a < b ? `${a}|${b}` : `${b}|${a}`;
      edges.set(id, (edges.get(id) ?? 0) + 1);
    }
  const open = new Set<string>();
  for (const [id, n] of edges) if (n === 1) id.split("|").forEach((k) => open.add(k));
  const seen = new Set<string>();
  const out: number[] = [];
  for (let i = 0; i < p.count; i++) if (open.has(keys[i]) && !seen.has(keys[i])) (seen.add(keys[i]), out.push(i));
  return out;
}

export type Frame = { worst: number; at: number; poking: number; uncovered: number; edge: number; holes: number[]; through: number[]; deep: [number, number][] };

/** One frame: how far the inner pokes through the outer (metres, and how many vertices past
 *  `slack`), and how many of the inner's top-edge vertices the outer does not cover. */
export function layered(
  inner: Piece,
  outer: Piece,
  joints: Map<string, Pose>,
  children: Map<string, string[]>,
  near: number[],
  edge: number[],
  outerTris: number[],
  slack = 0.002,
): Frame {
  return cover(inner, skinned(inner, joints), skinned(outer, joints), outer.tris, () => outerTris, joints, children, near, edge, slack);
}

/** `layered` on vertices skinned already (`vi` the inner's, `vo` the outer's), the outer's
 *  triangles to try along each ray given by `candidates` (a list, or a `Grid` walk). */
export function cover(
  inner: Piece,
  vi: Float64Array,
  vo: Float64Array,
  outerTris: Uint32Array,
  candidates: (o: V3, d: V3, length: number) => number[],
  joints: Map<string, Pose>,
  children: Map<string, string[]>,
  near: number[],
  edge: number[],
  slack = 0.002,
): Frame {
  const f: Frame = { worst: 0, at: -1, poking: 0, uncovered: 0, edge: edge.length, holes: [], through: [], deep: [] };
  const buf: number[] = [];
  const onEdge = new Set(edge);
  for (const i of new Set([...near, ...edge])) {
    const v: V3 = [vi[3 * i], vi[3 * i + 1], vi[3 * i + 2]];
    const a = anchor(inner, i, v, joints, children);
    const to = sub3(v, a);
    const L = Math.hypot(...to);
    if (L < 1e-6) continue;
    const d = scale(to, 1 / L);
    const ts = hits(a, d, vo, outerTris, candidates(a, d, L), buf);
    const last = ts.length ? Math.max(...ts) : -1;
    if (onEdge.has(i) && last <= L) (f.uncovered++, f.holes.push(i));
    // Nothing of the outer anywhere on the ray: a hole to see through, not only an edge showing.
    if (onEdge.has(i) && last < 0) f.through.push(i);
    if (last >= 0 && last < L) {
      const depth = L - last;
      if (depth > slack) (f.poking++, f.deep.push([i, depth]));
      if (depth > f.worst) [f.worst, f.at] = [depth, i];
    }
  }
  return f;
}

/** The nearest point to `v` on the bone the vertex follows most, from its joint to a child. */
export function anchor(p: Piece, i: number, v: V3, joints: Map<string, Pose>, children: Map<string, string[]>): V3 {
  const [bone] = [...p.weights[i]].reduce((a, b) => (b[1] > a[1] ? b : a), ["", -1]);
  const j = joints.get(bone)!.t;
  let best: V3 = j;
  let dist = Math.hypot(...sub3(v, j));
  for (const c of children.get(bone) ?? []) {
    const e = joints.get(c)?.t;
    if (!e) continue;
    const seg = sub3(e, j);
    const l2 = seg[0] ** 2 + seg[1] ** 2 + seg[2] ** 2 || 1;
    const k = Math.min(1, Math.max(0, (sub3(v, j)[0] * seg[0] + sub3(v, j)[1] * seg[1] + sub3(v, j)[2] * seg[2]) / l2));
    const q = add(j, scale(seg, k));
    const dq = Math.hypot(...sub3(v, q));
    if (dq < dist) [best, dist] = [q, dq];
  }
  return best;
}

/** Each vertex of `a` within `r` of a vertex of `b`, at rest. */
export function within(a: Piece, b: Piece, r: number): number[] {
  const out: number[] = [];
  for (let i = 0; i < a.count; i++)
    for (let j = 0; j < b.count; j++)
      if (Math.hypot(a.pos[3 * i] - b.pos[3 * j], a.pos[3 * i + 1] - b.pos[3 * j + 1], a.pos[3 * i + 2] - b.pos[3 * j + 2]) < r) {
        out.push(i);
        break;
      }
  return out;
}

/** The outer's triangles (index of the first corner) with a corner within `r` of a vertex listed. */
export function trianglesNear(outer: Piece, inner: Piece, list: number[], r: number): number[] {
  const close = new Uint8Array(outer.count);
  for (let j = 0; j < outer.count; j++)
    for (const i of list)
      if (Math.hypot(inner.pos[3 * i] - outer.pos[3 * j], inner.pos[3 * i + 1] - outer.pos[3 * j + 1], inner.pos[3 * i + 2] - outer.pos[3 * j + 2]) < r) {
        close[j] = 1;
        break;
      }
  const out: number[] = [];
  for (let k = 0; k < outer.tris.length; k += 3) if (close[outer.tris[k]] || close[outer.tris[k + 1]] || close[outer.tris[k + 2]]) out.push(k);
  return out;
}

/** Every piece of a file as one: a body piece with its belts, say. */
export function joined(ps: Piece[]): Piece {
  const count = ps.reduce((a, p) => a + p.count, 0);
  const pos = new Float64Array(3 * count);
  const tris: number[] = [];
  const weights: Map<string, number>[] = [];
  const bind = new Map<string, number[]>();
  let base = 0;
  for (const p of ps) {
    pos.set(p.pos, 3 * base);
    for (const t of p.tris) tris.push(base + t);
    weights.push(...p.weights);
    for (const [k, v] of p.bind) bind.set(k, v);
    base += p.count;
  }
  return { ...ps[0], node: ps.map((p) => p.node).join("+"), count, pos, nrm: undefined, uvs: [], weights, tris: Uint32Array.from(tris), bind };
}

export type Run = { rest: Frame; worst: Frame & { clip: string; t: number }; open: Frame & { clip: string; t: number }; frames: number; poked: number; holed: number; clips: Map<string, { poke: number; open: number }> };

/** Two files' pieces made ready to measure, and one frame measured: `one(clip, t)`. */
export function pair(outerFile: string, innerFile: string, clipsFile = MANNEQUIN) {
  const body = new Glb(outerFile);
  const from = new Glb(clipsFile);
  const outer = joined(pieces(body));
  const inner = joined(pieces(new Glb(innerFile)));
  const children = new Map<string, string[]>();
  body.gltf.nodes.forEach((n) => n.name && children.set(n.name, (n.children ?? []).map((c) => body.gltf.nodes[c].name ?? "")));
  const near = within(inner, outer, 0.08);
  const edge = topEdge(inner);
  const tris = trianglesNear(outer, inner, [...new Set([...near, ...edge])], 0.15);
  const one = (clip: Clip | undefined, t: number) => layered(inner, outer, retargeted(body, from, clip, t), children, near, edge, tris);
  return { body, from, outer, inner, one };
}

/** The inner under the outer through every clip of `clips`, `frames` moments each. */
export function measure(outerFile: string, innerFile: string, clipsFile = MANNEQUIN, frames = 12): Run {
  const { from, one } = pair(outerFile, innerFile, clipsFile);
  const rest = one(undefined, 0);
  const run: Run = { rest, worst: { ...rest, clip: "rest", t: 0 }, open: { ...rest, clip: "rest", t: 0 }, frames: 0, poked: 0, holed: 0, clips: new Map() };
  for (const name of from.clipNames()) {
    const clip = from.clip(name)!;
    const mine = { poke: 0, open: 0 };
    for (let k = 0; k < frames; k++) {
      const t = (clip.end * k) / frames;
      const f = one(clip, t);
      run.frames++;
      if (f.poking) run.poked++;
      if (f.uncovered > rest.uncovered) run.holed++;
      if (f.worst > run.worst.worst) run.worst = { ...f, clip: name, t };
      if (f.uncovered > run.open.uncovered) run.open = { ...f, clip: name, t };
      mine.poke = Math.max(mine.poke, f.worst);
      mine.open = Math.max(mine.open, f.uncovered);
    }
    run.clips.set(name, mine);
  }
  return run;
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const opt = (k: string) => (args.includes(k) ? args.splice(args.indexOf(k), 2)[1] : undefined);
  const clips = opt("--clips") ?? MANNEQUIN;
  const frames = Number(opt("--frames") ?? 12);
  if (args.length !== 2) {
    console.log("usage: node tools/characters/layers.ts OUTER.gltf INNER.gltf [--clips FILE.glb] [--frames N]");
    process.exit(2);
  }
  const r = measure(args[0], args[1], clips, frames);
  const name = (f: string) => f.split("/").pop()!.replace(/\.gltf$|\.glb$/, "");
  const mm = (m: number) => `${(m * 1000).toFixed(1)} mm`;
  console.log(
    `${name(args[0])} over ${name(args[1])}: at rest poke ${mm(r.rest.worst)}, top edge uncovered ${r.rest.uncovered}/${r.rest.edge}; ` +
      `through ${r.frames} frames of ${r.clips.size} clips: poke-through past 2 mm in ${r.poked} (worst ${mm(r.worst.worst)}, ${r.worst.poking} vertices past 2 mm, ${r.worst.clip}@${r.worst.t.toFixed(2)}), ` +
      `top edge more uncovered than at rest in ${r.holed} (worst ${r.open.uncovered}/${r.open.edge}, ${r.open.clip}@${r.open.t.toFixed(2)})`,
  );
  const bad = [...r.clips].filter(([, c]) => c.poke > 0.005 || c.open > r.rest.uncovered).sort((a, b) => b[1].poke - a[1].poke);
  console.log(`  clips past 5 mm or opening the top edge (worst poke mm/top-edge vertices uncovered): ${bad.map(([n, c]) => `${n} ${(c.poke * 1000).toFixed(0)}/${c.open}`).join(", ") || "none"}; the other ${r.clips.size - bad.length} clean`);
}
