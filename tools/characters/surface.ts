// How deep a point is inside a triangle mesh that may cut through itself: its distance to the
// nearest point of the surface (Ericson's closest point on a triangle, Real-Time Collision
// Detection 5.1.5), positive where the mesh's winding number there is over a half, as libigl's
// signed_distance signs by the winding number (Jacobson, Kavan and Sorkine-Hornung, "Robust
// inside-outside segmentation using generalized winding numbers", SIGGRAPH 2013), which holds
// where a mesh cuts through itself. The mesh is closed (its holes capped), so its winding
// number is the signed count of a ray's crossings, along the cells the ray passes, a few dozen
// triangles where the solid angles of all of them cost 30 times as much. A skinned body cuts
// through itself where a joint bends: in the female template's hip
// crease in Walk_Loop the inner thigh's skin pokes out through the front of the thigh, facing in,
// so a sign taken from the nearest surface's normal (a vertex's, or Bærentzen's pseudonormal)
// read a thumb 8 cm clear of the hip as 8 cm inside it. The skin of the move check (skin.ts)
// measures with it. Node 22.18 or later; no dependencies.

import { type V3, dot, len, sub } from "./glb.ts";

/** Positive inside, metres, and the part of the corner nearest the surface point. */
export type Depth = { depth: number; part: string };

const add = (a: V3, b: V3, k = 1): V3 => [a[0] + k * b[0], a[1] + k * b[1], a[2] + k * b[2]];

/** The closest point of triangle `a b c` to `p` (Ericson 5.1.5). */
export function closestOnTriangle(p: V3, a: V3, b: V3, c: V3): V3 {
  const ab = sub(b, a);
  const ac = sub(c, a);
  const ap = sub(p, a);
  const d1 = dot(ab, ap);
  const d2 = dot(ac, ap);
  if (d1 <= 0 && d2 <= 0) return a;
  const bp = sub(p, b);
  const d3 = dot(ab, bp);
  const d4 = dot(ac, bp);
  if (d3 >= 0 && d4 <= d3) return b;
  const vc = d1 * d4 - d3 * d2;
  if (vc <= 0 && d1 >= 0 && d3 <= 0) return add(a, ab, d1 / (d1 - d3));
  const cp = sub(p, c);
  const d5 = dot(ab, cp);
  const d6 = dot(ac, cp);
  if (d6 >= 0 && d5 <= d6) return c;
  const vb = d5 * d2 - d1 * d6;
  if (vb <= 0 && d2 >= 0 && d6 <= 0) return add(a, ac, d2 / (d2 - d6));
  const va = d3 * d6 - d5 * d4;
  if (va <= 0 && d4 - d3 >= 0 && d5 - d6 >= 0) return add(b, sub(c, b), (d4 - d3) / (d4 - d3 + (d5 - d6)));
  const k = 1 / (va + vb + vc);
  return add(add(a, ab, vb * k), ac, vc * k);
}

/** A mesh's topology, kept across poses: its triangles (corner indices, three a triangle,
 * counter-clockwise seen from outside), each corner's part, and the caps that close its holes
 * (`[i, j, hole]`: the triangle from corner `j` to `i` to the middle of hole `hole`). */
export type Topology = { tris: Uint32Array; parts: string[]; caps: Uint32Array; holes: number[][] };

/** The topology of `tris` over `parts.length` corners. An edge on one triangle only is on a hole's
 * rim; each hole, its rim's corners joined, is capped by a fan from their middle, turned against
 * the triangle beside it, so the mesh is closed and its winding number whole. */
export function topology(tris: ArrayLike<number>, parts: string[]): Topology {
  const n = parts.length;
  const uses = new Map<number, number>();
  for (let t = 0; t < tris.length; t += 3)
    for (let e = 0; e < 3; e++) {
      const [i, j] = [tris[t + e], tris[t + ((e + 1) % 3)]];
      const key = Math.min(i, j) * n + Math.max(i, j);
      uses.set(key, (uses.get(key) ?? 0) + 1);
    }
  const root = Int32Array.from({ length: n }, (_, i) => i);
  const find = (i: number): number => (root[i] === i ? i : (root[i] = find(root[i])));
  const rim: [number, number][] = [];
  for (let t = 0; t < tris.length; t += 3)
    for (let e = 0; e < 3; e++) {
      const [i, j] = [tris[t + e], tris[t + ((e + 1) % 3)]];
      if (uses.get(Math.min(i, j) * n + Math.max(i, j)) !== 1) continue;
      rim.push([i, j]);
      root[find(i)] = find(j);
    }
  const hole = new Map<number, number>();
  const holes: number[][] = [];
  const caps: number[] = [];
  for (const [i, j] of rim) {
    const r = find(i);
    if (!hole.has(r)) hole.set(r, holes.push([]) - 1);
    caps.push(i, j, hole.get(r)!);
  }
  for (const [i] of rim) holes[hole.get(find(i))!].push(i);
  return { tris: Uint32Array.from(tris), parts, caps: Uint32Array.from(caps), holes };
}

/** Whether the ray from `q` along +X crosses triangle `a b c`, which way and where: `[+1, x]`
 * out through a face turned to +X at `x`, `[-1, x]` in, `[0, x]` not at all. Its edges' signs
 * in the YZ plane decide; an edge the ray meets exactly counts for one of its two triangles
 * only (by the edge's direction), so a ray along a shared edge is counted once. */
function crossing(q: V3, a: V3, b: V3, c: V3): [number, number] {
  const edge = (u: V3, v: V3) => {
    const e = (u[1] - q[1]) * (v[2] - q[2]) - (u[2] - q[2]) * (v[1] - q[1]);
    return e !== 0 ? e : u[1] < v[1] || (u[1] === v[1] && u[2] < v[2]) ? Number.MIN_VALUE : -Number.MIN_VALUE;
  };
  // Edge-on to the ray: no crossing (its neighbours count it).
  const area = (b[1] - a[1]) * (c[2] - a[2]) - (b[2] - a[2]) * (c[1] - a[1]);
  if (area === 0) return [0, q[0]];
  const [ea, eb, ec] = [edge(b, c), edge(c, a), edge(a, b)];
  if (!((ea > 0 && eb > 0 && ec > 0) || (ea < 0 && eb < 0 && ec < 0))) return [0, q[0]];
  // The crossing by its barycentric weights, each corner's the edge across from it, kept
  // within the triangle's own span against rounding.
  const x = Math.min(Math.max((ea * a[0] + eb * b[0] + ec * c[0]) / (ea + eb + ec), Math.min(a[0], b[0], c[0])), Math.max(a[0], b[0], c[0]));
  return x > q[0] ? [Math.sign(area), x] : [0, x];
}

/** How deep a point is inside the mesh `top` with its corners at `at` (x, y, z a corner), or
 * `{ depth: -1, part: "" }` with no corner within two cells. Cells of `cell` metres grid it. */
export function surface(top: Topology, at: ArrayLike<number>, cell = 0.1): (q: V3) => Depth {
  const { tris, parts, caps, holes } = top;
  const P = (i: number): V3 => [at[i * 3], at[i * 3 + 1], at[i * 3 + 2]];
  const middles: V3[] = holes.map((h) => h.reduce((m, i) => add(m, P(i), 1 / h.length), [0, 0, 0] as V3));
  // The corners gridded, for the nearest one, whose distance bounds the search; the triangles
  // gridded by the cells their bounds cover.
  const key = (x: number, y: number, z: number) => `${x},${y},${z}`;
  const corners = new Map<string, number[]>();
  const used = new Uint8Array(parts.length);
  for (const i of tris) used[i] = 1;
  for (let i = 0; i < parts.length; i++) {
    if (!used[i]) continue;
    const k = key(Math.floor(at[i * 3] / cell), Math.floor(at[i * 3 + 1] / cell), Math.floor(at[i * 3 + 2] / cell));
    if (!corners.has(k)) corners.set(k, []);
    corners.get(k)!.push(i);
  }
  const faces = new Map<string, number[]>();
  let far = -Infinity;
  for (let i = 0; i < at.length; i += 3) far = Math.max(far, Math.floor(at[i] / cell));
  for (let t = 0; t < tris.length; t += 3) {
    const lo = [0, 1, 2].map((j) => Math.floor(Math.min(at[tris[t] * 3 + j], at[tris[t + 1] * 3 + j], at[tris[t + 2] * 3 + j]) / cell));
    const hi = [0, 1, 2].map((j) => Math.floor(Math.max(at[tris[t] * 3 + j], at[tris[t + 1] * 3 + j], at[tris[t + 2] * 3 + j]) / cell));
    for (let x = lo[0]; x <= hi[0]; x++)
      for (let y = lo[1]; y <= hi[1]; y++)
        for (let z = lo[2]; z <= hi[2]; z++) {
          const k = key(x, y, z);
          if (!faces.has(k)) faces.set(k, []);
          faces.get(k)!.push(t);
        }
  }
  // The winding number at `q`: of a closed mesh, the signed count of a ray's crossings, the ray
  // along +X through the row of cells it passes, each triangle counted in the cell its crossing
  // is in; and every cap's.
  const winding = (q: V3): number => {
    let sum = 0;
    const [y, z] = [Math.floor(q[1] / cell), Math.floor(q[2] / cell)];
    for (let x = Math.floor(q[0] / cell); x <= far; x++)
      for (const t of faces.get(key(x, y, z)) ?? []) {
        const [s, along] = crossing(q, P(tris[t]), P(tris[t + 1]), P(tris[t + 2]));
        // Counted in the cell its crossing is in only: the same triangle lies in several.
        if (s !== 0 && Math.floor(along / cell) === x) sum += s;
      }
    for (let c = 0; c < caps.length; c += 3) sum += crossing(q, P(caps[c + 1]), P(caps[c]), middles[caps[c + 2]])[0];
    return sum;
  };
  return (q: V3): Depth => {
    const c = q.map((x) => Math.floor(x / cell));
    let reach = Infinity;
    for (let i = -2; i <= 2; i++)
      for (let j = -2; j <= 2; j++)
        for (let k = -2; k <= 2; k++) for (const v of corners.get(key(c[0] + i, c[1] + j, c[2] + k)) ?? []) reach = Math.min(reach, len(sub(P(v), q)));
    if (reach === Infinity) return { depth: -1, part: "" };
    // The nearest surface point is no farther than the nearest corner: only triangles in the
    // cells that sphere touches can hold it.
    const lo = q.map((x) => Math.floor((x - reach) / cell));
    const hi = q.map((x) => Math.floor((x + reach) / cell));
    let best = { d: Infinity, part: "" };
    const seen = new Set<number>();
    for (let x = lo[0]; x <= hi[0]; x++)
      for (let y = lo[1]; y <= hi[1]; y++)
        for (let z = lo[2]; z <= hi[2]; z++)
          for (const t of faces.get(key(x, y, z)) ?? []) {
            if (seen.has(t)) continue;
            seen.add(t);
            const p = closestOnTriangle(q, P(tris[t]), P(tris[t + 1]), P(tris[t + 2]));
            const d = len(sub(p, q));
            if (d >= best.d) continue;
            // The part: the triangle's corner nearest the surface point.
            let near = 0;
            for (let e = 1; e < 3; e++) if (len(sub(P(tris[t + e]), p)) < len(sub(P(tris[t + near]), p))) near = e;
            best = { d, part: parts[tris[t + near]] };
          }
    return { depth: winding(q) > 0.5 ? best.d : -best.d, part: best.part };
  };
}
