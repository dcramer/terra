// Held look against held look, for the move check (moves.ts's `clash` rows, D481): how far a
// fighter's sword goes through their own shield's board, and the board through their sword
// arm. The shapes are measured from the looks' meshes, not guessed:
//
//   board   a round shield's board: a disc across its thinnest axis, as wide as its rim, its
//           faces where rays across the board between a third and four fifths of the way out
//           meet it (the median of each face), so the boss in front and the grip behind it
//           are left out
//   sliced  a long look (a sword) as balls: a station every centimetre along its longest axis,
//           a row across its width there a centimetre apart, each as thick as the look is
//           there; its parts by width: the widest band the guard, the tip's side of it the
//           blade, the other side the hilt
//   through how deep balls go through a board: the least of the three pushes that would clear
//           them (out its front, out its back, out past its rim); negative, the gap, when
//           nothing touches it
//
// Node 22.18 or later; no dependencies.

import { type Glb, type V3, dot, len, place, sub } from "./glb.ts";

/** A sample of a shape: a ball at `p` of radius `r`, naming the part it is of. */
export type Ball = { p: V3; r: number; part: string };
/** A disc: its centre, its face's unit normal, its radius and half its thickness. */
export type Board = { centre: V3; normal: V3; radius: number; half: number };
/** A mesh's vertices and its triangles (three vertex indices each), in one frame. */
export type Mesh = { points: V3[]; tris: ArrayLike<number> };

/** A look's mesh (its first node with one, at that node's rest), in the look's frame. */
export function meshOf(g: Glb): Mesh {
  const node = g.gltf.nodes.findIndex((n) => n.mesh !== undefined);
  const prim = g.gltf.meshes![g.gltf.nodes[node].mesh!].primitives[0] as { attributes: Record<string, number>; indices?: number };
  const pose = g.rest(node);
  const p = g.read(prim.attributes.POSITION);
  const points: V3[] = [];
  for (let i = 0; i < p.length; i += 3) points.push(place(pose, [p[i], p[i + 1], p[i + 2]]));
  return { points, tris: prim.indices === undefined ? points.map((_, i) => i) : g.read(prim.indices) };
}

function bounds(points: V3[]) {
  const lo: V3 = [Infinity, Infinity, Infinity];
  const hi: V3 = [-Infinity, -Infinity, -Infinity];
  for (const q of points) for (let j = 0; j < 3; j++) (lo[j] = Math.min(lo[j], q[j])), (hi[j] = Math.max(hi[j], q[j]));
  return { lo, hi, size: hi.map((x, j) => x - lo[j]) as V3, mid: lo.map((x, j) => (x + hi[j]) / 2) as V3 };
}

/** Where a ray along `axis` through `at` (its other two coordinates) meets the mesh, sorted. */
export function hits(mesh: Mesh, axis: number, at: V3): number[] {
  const [a, b] = [0, 1, 2].filter((j) => j !== axis);
  const [u, w] = [at[a], at[b]];
  const out: number[] = [];
  const { points: v, tris } = mesh;
  for (let t = 0; t + 2 < tris.length; t += 3) {
    const [A, B, C] = [v[tris[t]], v[tris[t + 1]], v[tris[t + 2]]];
    const d = (B[a] - A[a]) * (C[b] - A[b]) - (C[a] - A[a]) * (B[b] - A[b]);
    if (Math.abs(d) < 1e-14) continue;
    const l1 = ((u - A[a]) * (C[b] - A[b]) - (C[a] - A[a]) * (w - A[b])) / d;
    const l2 = ((B[a] - A[a]) * (w - A[b]) - (u - A[a]) * (B[b] - A[b])) / d;
    if (l1 < 0 || l2 < 0 || l1 + l2 > 1) continue;
    out.push(A[axis] + l1 * (B[axis] - A[axis]) + l2 * (C[axis] - A[axis]));
  }
  return out.sort((x, y) => x - y);
}

const median = (xs: number[]) => {
  const s = [...xs].sort((x, y) => x - y);
  return s.length % 2 ? s[(s.length - 1) / 2] : (s[s.length / 2 - 1] + s[s.length / 2]) / 2;
};

/** A round shield's board, measured from its mesh (see the file's head). */
export function board(mesh: Mesh): Board {
  const { size, mid } = bounds(mesh.points);
  const thin = size.indexOf(Math.min(...size));
  const [a, b] = [0, 1, 2].filter((j) => j !== thin);
  const radius = Math.max(...mesh.points.map((p) => Math.hypot(p[a] - mid[a], p[b] - mid[b])));
  const fronts: number[] = [];
  const backs: number[] = [];
  for (let k = 0.3; k <= 0.8 + 1e-9; k += 0.1)
    for (let n = 0; n < 12; n++) {
      const at: V3 = [...mid];
      at[a] += k * radius * Math.cos((n * Math.PI) / 6);
      at[b] += k * radius * Math.sin((n * Math.PI) / 6);
      const h = hits(mesh, thin, at);
      if (h.length >= 2) fronts.push(h[0]), backs.push(h[h.length - 1]);
    }
  if (fronts.length < 36) throw new Error(`no board: ${fronts.length} of 72 rays across it meet two faces`);
  const [front, back] = [median(fronts), median(backs)];
  const centre: V3 = [...mid];
  centre[thin] = (front + back) / 2;
  const normal: V3 = [0, 0, 0];
  normal[thin] = 1;
  return { centre, normal, radius, half: (back - front) / 2 };
}

/** A long look as balls, a centimetre apart, with its parts (see the file's head). */
export function sliced(mesh: Mesh, tip: V3, step = 0.01): Ball[] {
  const { lo, hi, size, mid } = bounds(mesh.points);
  const long = size.indexOf(Math.max(...size));
  const thin = size.indexOf(Math.min(...size));
  const wide = 3 - long - thin;
  const stations: { at: number; w0: number; w1: number; t0: number; t1: number }[] = [];
  for (let s = lo[long] + step / 2; s < hi[long]; s += step) {
    const at: V3 = [...mid];
    at[long] = s;
    // Across it at its middle and a quarter of its thickness either side (a guard is thicker
    // than its blade, and its arms may not cross the middle).
    const across = [-0.25, 0, 0.25].flatMap((k) => {
      const off: V3 = [...at];
      off[thin] += k * size[thin];
      return hits(mesh, wide, off);
    }).sort((x, y) => x - y);
    const deep = hits(mesh, thin, at);
    if (across.length < 2 || deep.length < 2) continue;
    stations.push({ at: s, w0: across[0], w1: across[across.length - 1], t0: deep[0], t1: deep[deep.length - 1] });
  }
  const width = (k: number) => stations[k].w1 - stations[k].w0;
  let widest = 0;
  stations.forEach((_, k) => (width(k) > width(widest) ? (widest = k) : 0));
  // The guard: the stations about the widest that are half as wide again as the look mostly is.
  const wider = 1.5 * median(stations.map((_, k) => width(k)));
  let [g0, g1] = [widest, widest];
  while (g0 > 0 && width(g0 - 1) > wider) g0--;
  while (g1 < stations.length - 1 && width(g1 + 1) > wider) g1++;
  const tipUp = tip[long] > stations[widest].at;
  const balls: Ball[] = [];
  stations.forEach((st, k) => {
    const part = k >= g0 && k <= g1 ? "guard" : k > g1 === tipUp ? "blade" : "hilt";
    const r = (st.t1 - st.t0) / 2;
    const from = st.w0 + r;
    const to = Math.max(from, st.w1 - r);
    const n = Math.max(1, Math.ceil((to - from) / step));
    for (let i = 0; i <= n; i++) {
      const p: V3 = [...mid];
      p[long] = st.at;
      p[thin] = (st.t0 + st.t1) / 2;
      p[wide] = from + ((to - from) * i) / n;
      balls.push({ p, r, part });
    }
  });
  return balls;
}

/** How deep balls go through a board, cm (see the file's head), and the part deepest in it. */
export function through(balls: Ball[], disc: Board): { cm: number; part: string } {
  const n = disc.normal;
  let out = { push: -Infinity, part: "" };
  let back = { push: -Infinity, part: "" };
  let rim = { push: -Infinity, part: "" };
  let gap = { d: Infinity, part: "" };
  for (const b of balls) {
    const d = sub(b.p, disc.centre);
    const s = dot(d, n);
    const r = len(sub(d, [n[0] * s, n[1] * s, n[2] * s]));
    // The distance from the ball's centre to the board (a cylinder), less its radius.
    const [dr, dn] = [r - disc.radius, Math.abs(s) - disc.half];
    const dist = Math.hypot(Math.max(dr, 0), Math.max(dn, 0)) + Math.min(Math.max(dr, dn), 0) - b.r;
    if (dist < gap.d) gap = { d: dist, part: b.part };
    if (r >= disc.radius + b.r) continue;
    // Over the board: moving along its normal, it stays over it.
    const h = disc.half + b.r;
    if (h - s > out.push) out = { push: h - s, part: b.part };
    if (s + h > back.push) back = { push: s + h, part: b.part };
    if (Math.abs(s) < h && disc.radius + b.r - r > rim.push) rim = { push: disc.radius + b.r - r, part: b.part };
  }
  if (gap.d > 0) return { cm: -gap.d * 100, part: gap.part };
  const least = [out, back, rim].reduce((x, y) => (y.push < x.push ? y : x));
  return { cm: least.push * 100, part: least.part };
}
