// A part's triangles in its own frame, and the measures the part checker takes on them: a slice
// across the line, rays out from the line in that slice, a point's distance to the surface, and
// faces of two parts that lie in one plane. Read from a glb by tools/characters/glb.ts's reader.

import { readFileSync } from "node:fs";
import { apply, Glb, type Pose, place, type V3 } from "../characters/glb.ts";

export type Mesh = {
  positions: number[]; // x, y, z a vertex, in the part's frame
  indices: number[]; // three a triangle
  materials: string[]; // a primitive's material name each
  textures: { name: string; px: number | undefined }[]; // longest side, undefined when not a PNG
  scaled: string[]; // nodes carrying a scale other than 1
};

export function readMesh(file: string): Mesh {
  const bytes = readFileSync(file);
  const glb = new Glb(bytes);
  const g: any = glb.gltf;
  const out: Mesh = { positions: [], indices: [], materials: [], textures: [], scaled: [] };
  const world = (i: number): Pose => {
    const n = g.nodes[i];
    const local: Pose = { t: n.translation ?? [0, 0, 0], r: n.rotation ?? [0, 0, 0, 1], s: n.scale ?? [1, 1, 1] };
    const parent = glb.parentOf[i];
    return parent === undefined ? local : apply(world(parent), local);
  };
  g.nodes.forEach((n: any, i: number) => {
    if (n.matrix) throw new Error(`${file}: node ${n.name ?? i} has a matrix; export TRS (D303)`);
    if (n.scale && n.scale.some((s: number) => Math.abs(s - 1) > 1e-6)) out.scaled.push(n.name ?? `node ${i}`);
    if (n.mesh === undefined) return;
    const pose = world(i);
    for (const p of g.meshes[n.mesh].primitives) {
      if ((p.mode ?? 4) !== 4) continue;
      const pos = glb.read(p.attributes.POSITION);
      const base = out.positions.length / 3;
      for (let v = 0; v < pos.length; v += 3) out.positions.push(...place(pose, [pos[v], pos[v + 1], pos[v + 2]]));
      const idx = p.indices === undefined ? [...Array(pos.length / 3).keys()] : [...glb.read(p.indices)];
      for (const k of idx) out.indices.push(base + k);
      out.materials.push(p.material === undefined ? "" : (g.materials?.[p.material]?.name ?? ""));
    }
  });
  const jsonLen = bytes.readUInt32LE(12);
  const bin = 20 + jsonLen + 8;
  for (const [i, img] of (g.images ?? []).entries()) {
    const view = img.bufferView === undefined ? undefined : g.bufferViews[img.bufferView];
    const at = view ? bin + (view.byteOffset ?? 0) : -1;
    const png = at >= 0 && bytes.readUInt32BE(at) === 0x89504e47;
    out.textures.push({ name: img.name ?? `image ${i}`, px: png ? Math.max(bytes.readUInt32BE(at + 16), bytes.readUInt32BE(at + 20)) : undefined });
  }
  return out;
}

export const vertex = (m: Mesh, i: number): V3 => [m.positions[3 * i], m.positions[3 * i + 1], m.positions[3 * i + 2]];
export const triangles = (m: Mesh): [V3, V3, V3][] => {
  const out: [V3, V3, V3][] = [];
  for (let t = 0; t < m.indices.length; t += 3) out.push([vertex(m, m.indices[t]), vertex(m, m.indices[t + 1]), vertex(m, m.indices[t + 2])]);
  return out;
};

export function bounds(m: Mesh): [V3, V3] {
  const lo: V3 = [Infinity, Infinity, Infinity];
  const hi: V3 = [-Infinity, -Infinity, -Infinity];
  for (let i = 0; i < m.positions.length; i++) {
    lo[i % 3] = Math.min(lo[i % 3], m.positions[i]);
    hi[i % 3] = Math.max(hi[i % 3], m.positions[i]);
  }
  return [lo, hi];
}

type Seg = [[number, number], [number, number]]; // two (x, z) points

/** Where the surface crosses the plane at height y: segments in (x, z). */
export function slice(m: Mesh, y: number): Seg[] {
  const out: Seg[] = [];
  for (const tri of triangles(m)) {
    const cross: [number, number][] = [];
    for (let e = 0; e < 3; e++) {
      const [a, b] = [tri[e], tri[(e + 1) % 3]];
      if ((a[1] - y) * (b[1] - y) < 0) {
        const t = (y - a[1]) / (b[1] - a[1]);
        cross.push([a[0] + t * (b[0] - a[0]), a[2] + t * (b[2] - a[2])]);
      }
    }
    if (cross.length === 2) out.push([cross[0], cross[1]]);
  }
  return out;
}

/** How far a ray from the line, along (dx, dz), runs before it meets the slice; Infinity if never. */
export function ray(segs: Seg[], dx: number, dz: number): number {
  let best = Infinity;
  for (const [[ax, az], [bx, bz]] of segs) {
    const [ex, ez] = [bx - ax, bz - az];
    const den = dx * ez - dz * ex;
    if (Math.abs(den) < 1e-12) continue;
    const t = (ax * ez - az * ex) / den;
    const s = (ax * dz - az * dx) / den;
    if (t > 0 && s >= 0 && s <= 1) best = Math.min(best, t);
  }
  return best;
}

const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a: V3): V3 => {
  const l = Math.hypot(...a) || 1;
  return [a[0] / l, a[1] / l, a[2] / l];
};
export const normal = (t: [V3, V3, V3]): V3 => norm(cross(sub(t[1], t[0]), sub(t[2], t[0])));

/** The nearest point of a triangle to p (Ericson, Real-Time Collision Detection, 5.1.5). */
function closest(p: V3, [a, b, c]: [V3, V3, V3]): V3 {
  const ab = sub(b, a), ac = sub(c, a), ap = sub(p, a);
  const d1 = dot(ab, ap), d2 = dot(ac, ap);
  if (d1 <= 0 && d2 <= 0) return a;
  const bp = sub(p, b), d3 = dot(ab, bp), d4 = dot(ac, bp);
  if (d3 >= 0 && d4 <= d3) return b;
  const vc = d1 * d4 - d3 * d2;
  if (vc <= 0 && d1 >= 0 && d3 <= 0) { const v = d1 / (d1 - d3); return [a[0] + v * ab[0], a[1] + v * ab[1], a[2] + v * ab[2]]; }
  const cp = sub(p, c), d5 = dot(ab, cp), d6 = dot(ac, cp);
  if (d6 >= 0 && d5 <= d6) return c;
  const vb = d5 * d2 - d1 * d6;
  if (vb <= 0 && d2 >= 0 && d6 <= 0) { const w = d2 / (d2 - d6); return [a[0] + w * ac[0], a[1] + w * ac[1], a[2] + w * ac[2]]; }
  const va = d3 * d6 - d5 * d4;
  if (va <= 0 && d4 - d3 >= 0 && d5 - d6 >= 0) {
    const w = (d4 - d3) / (d4 - d3 + (d5 - d6));
    return [b[0] + w * (c[0] - b[0]), b[1] + w * (c[1] - b[1]), b[2] + w * (c[2] - b[2])];
  }
  const den = 1 / (va + vb + vc), v = vb * den, w = vc * den;
  return [a[0] + ab[0] * v + ac[0] * w, a[1] + ab[1] * v + ac[1] * w, a[2] + ab[2] * v + ac[2] * w];
}

/** The distance from p to the surface. */
export const distance = (m: Mesh, p: V3) => Math.min(...triangles(m).map((t) => Math.hypot(...sub(p, closest(p, t)))));

// The inside test's ray: off every axis, so it does not run along a face or an edge of a part
// made square to its frame (Möller and Trumbore's ray-triangle test).
const SKEW: V3 = [0.9271, 0.2381, 0.2893];

/** Whether p is inside a closed surface: a ray from it crosses the surface an odd number of times. */
export function inside(m: Mesh, p: V3): boolean {
  let n = 0;
  for (const [a, b, c] of triangles(m)) {
    const e1 = sub(b, a), e2 = sub(c, a), h = cross(SKEW, e2), det = dot(e1, h);
    if (Math.abs(det) < 1e-12) continue;
    const s = sub(p, a), u = dot(s, h) / det;
    if (u < 0 || u > 1) continue;
    const q = cross(s, e1), v = dot(SKEW, q) / det;
    if (v < 0 || u + v > 1) continue;
    if (dot(e2, q) / det > 0) n++;
  }
  return n % 2 === 1;
}

/** The gap between two surfaces: the least distance from either's vertices to the other. */
export function gap(a: Mesh, b: Mesh): number {
  let best = Infinity;
  for (const [x, y] of [[a, b], [b, a]] as const) for (let i = 0; i < x.positions.length / 3; i++) best = Math.min(best, distance(y, vertex(x, i)));
  return best;
}

/** How deep a's vertices go into b, a closed surface: 0 when none is inside. */
export function buried(a: Mesh, b: Mesh): number {
  let worst = 0;
  for (let i = 0; i < a.positions.length / 3; i++) {
    const p = vertex(a, i);
    const d = distance(b, p);
    if (d > worst && inside(b, p)) worst = d;
  }
  return worst;
}

/** Faces of a and b facing the same way in one plane (within `within` m) and overlapping: what z-fights. */
export function sharedPlanes(a: Mesh, b: Mesh, within: number): number {
  const tb = triangles(b).map((t) => ({ t, n: normal(t) }));
  let n = 0;
  for (const ta of triangles(a)) {
    const na = normal(ta);
    for (const { t, n: nb } of tb) {
      if (dot(na, nb) < Math.cos((1 * Math.PI) / 180)) continue;
      if (t.some((p) => Math.abs(dot(sub(p, ta[0]), na)) > within)) continue;
      if (overlap(ta, t, na)) n++;
    }
  }
  return n;
}

// Two coplanar triangles overlap when no edge of either separates them (2D separating axes).
function overlap(a: [V3, V3, V3], b: [V3, V3, V3], n: V3): boolean {
  const u = norm(sub(a[1], a[0]));
  const v = cross(n, u);
  const flat = (t: [V3, V3, V3]) => t.map((p) => [dot(p, u), dot(p, v)] as [number, number]);
  const [pa, pb] = [flat(a), flat(b)];
  for (const poly of [pa, pb])
    for (let e = 0; e < 3; e++) {
      const [x0, y0] = poly[e], [x1, y1] = poly[(e + 1) % 3];
      const axis = [y0 - y1, x1 - x0];
      const span = (q: [number, number][]) => q.map(([x, y]) => x * axis[0] + y * axis[1]);
      const [sa, sb] = [span(pa), span(pb)];
      if (Math.max(...sa) <= Math.min(...sb) + 1e-9 || Math.max(...sb) <= Math.min(...sa) + 1e-9) return false;
    }
  return true;
}

/** The mesh moved by a rotation (3x3, rows) and a shift. */
export function moved(m: Mesh, r: number[][], t: V3): Mesh {
  const positions: number[] = [];
  for (let i = 0; i < m.positions.length / 3; i++) {
    const p = vertex(m, i);
    positions.push(...[0, 1, 2].map((k) => r[k][0] * p[0] + r[k][1] * p[1] + r[k][2] * p[2] + t[k]));
  }
  return { ...m, positions };
}

export const IDENTITY = [[1, 0, 0], [0, 1, 0], [0, 0, 1]];
