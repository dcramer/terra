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
// measures with it. The nearest triangle and corner are found down trees of boxes refit to each
// pose (`surface`, as a ray tracer refits a skinned mesh's), answering as the plain search through
// the cells around does (`reference`) to the bit: a move check takes 30 s, not 6.5 min.
// Node 22.18 or later; no dependencies.

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

// `crossing` and `closestOnTriangle` on bare numbers, for the queries' inner loops, their
// arithmetic in the same order so every answer is the same to the bit: they write here.
let crossed = 0;
let along = 0;
const near = new Float64Array(3);

/** `crossing`'s edge sign: the edge `u v` against the ray at `q` in the YZ plane. */
function edgeAt(q1: number, q2: number, u1: number, u2: number, v1: number, v2: number): number {
  const e = (u1 - q1) * (v2 - q2) - (u2 - q2) * (v1 - q1);
  return e !== 0 ? e : u1 < v1 || (u1 === v1 && u2 < v2) ? Number.MIN_VALUE : -Number.MIN_VALUE;
}

/** `crossing` from the ray at `q` across triangle `a b c`, into `crossed` and `along`. */
function crossingAt(q0: number, q1: number, q2: number, a0: number, a1: number, a2: number, b0: number, b1: number, b2: number, c0: number, c1: number, c2: number): void {
  const area = (b1 - a1) * (c2 - a2) - (b2 - a2) * (c1 - a1);
  crossed = 0;
  along = q0;
  if (area === 0) return;
  const ea = edgeAt(q1, q2, b1, b2, c1, c2);
  const eb = edgeAt(q1, q2, c1, c2, a1, a2);
  const ec = edgeAt(q1, q2, a1, a2, b1, b2);
  if (!((ea > 0 && eb > 0 && ec > 0) || (ea < 0 && eb < 0 && ec < 0))) return;
  along = Math.min(Math.max((ea * a0 + eb * b0 + ec * c0) / (ea + eb + ec), Math.min(a0, b0, c0)), Math.max(a0, b0, c0));
  if (along > q0) crossed = Math.sign(area);
}

/** `closestOnTriangle` from `p` to the triangle of corners `ia ib ic` of `at`, into `near`. */
function closestAt(p0: number, p1: number, p2: number, at: ArrayLike<number>, ia: number, ib: number, ic: number): void {
  const a0 = at[ia * 3], a1 = at[ia * 3 + 1], a2 = at[ia * 3 + 2];
  const b0 = at[ib * 3], b1 = at[ib * 3 + 1], b2 = at[ib * 3 + 2];
  const c0 = at[ic * 3], c1 = at[ic * 3 + 1], c2 = at[ic * 3 + 2];
  const ab0 = b0 - a0, ab1 = b1 - a1, ab2 = b2 - a2;
  const ac0 = c0 - a0, ac1 = c1 - a1, ac2 = c2 - a2;
  const ap0 = p0 - a0, ap1 = p1 - a1, ap2 = p2 - a2;
  const d1 = ab0 * ap0 + ab1 * ap1 + ab2 * ap2;
  const d2 = ac0 * ap0 + ac1 * ap1 + ac2 * ap2;
  if (d1 <= 0 && d2 <= 0) return set(a0, a1, a2);
  const bp0 = p0 - b0, bp1 = p1 - b1, bp2 = p2 - b2;
  const d3 = ab0 * bp0 + ab1 * bp1 + ab2 * bp2;
  const d4 = ac0 * bp0 + ac1 * bp1 + ac2 * bp2;
  if (d3 >= 0 && d4 <= d3) return set(b0, b1, b2);
  const vc = d1 * d4 - d3 * d2;
  if (vc <= 0 && d1 >= 0 && d3 <= 0) {
    const k = d1 / (d1 - d3);
    return set(a0 + k * ab0, a1 + k * ab1, a2 + k * ab2);
  }
  const cp0 = p0 - c0, cp1 = p1 - c1, cp2 = p2 - c2;
  const d5 = ab0 * cp0 + ab1 * cp1 + ab2 * cp2;
  const d6 = ac0 * cp0 + ac1 * cp1 + ac2 * cp2;
  if (d6 >= 0 && d5 <= d6) return set(c0, c1, c2);
  const vb = d5 * d2 - d1 * d6;
  if (vb <= 0 && d2 >= 0 && d6 <= 0) {
    const k = d2 / (d2 - d6);
    return set(a0 + k * ac0, a1 + k * ac1, a2 + k * ac2);
  }
  const va = d3 * d6 - d5 * d4;
  if (va <= 0 && d4 - d3 >= 0 && d5 - d6 >= 0) {
    const k = (d4 - d3) / (d4 - d3 + (d5 - d6));
    return set(b0 + k * (c0 - b0), b1 + k * (c1 - b1), b2 + k * (c2 - b2));
  }
  const k = 1 / (va + vb + vc);
  const u = vb * k, w = vc * k;
  set(a0 + u * ab0 + w * ac0, a1 + u * ab1 + w * ac1, a2 + u * ab2 + w * ac2);
}

/** `near` made `x y z`. */
function set(x: number, y: number, z: number): void {
  near[0] = x;
  near[1] = y;
  near[2] = z;
}

/** Items (`count` of them) filed by the cells of a box of cells from `lo` (x, y, z) `dims`
 * wide, each in every cell of its own box (`box`, six an item: its first cell's x, y, z, its
 * last's), in order: cell `c`'s are `items[first[c]]` up to `items[first[c + 1]]`. */
function bin(count: number, lo: number[], dims: number[], box: Int32Array) {
  const first = new Int32Array(dims[0] * dims[1] * dims[2] + 1);
  const [lx, ly, lz, ny, nz] = [lo[0], lo[1], lo[2], dims[1], dims[2]];
  // Counted (no `items` yet), then filed, each pass over every item's cells.
  const pass = (items?: Int32Array, fill?: Int32Array) => {
    for (let i = 0; i < count; i++)
      for (let x = box[i * 6]; x <= box[i * 6 + 3]; x++)
        for (let y = box[i * 6 + 1]; y <= box[i * 6 + 4]; y++)
          for (let z = box[i * 6 + 2]; z <= box[i * 6 + 5]; z++) {
            const c = ((x - lx) * ny + (y - ly)) * nz + (z - lz);
            if (items && fill) items[fill[c]++] = i;
            else first[c + 1]++;
          }
  };
  pass();
  for (let c = 1; c < first.length; c++) first[c] += first[c - 1];
  const items = new Int32Array(first[first.length - 1]);
  pass(items, first.slice());
  return { first, items };
}

/** How deep a point is inside the mesh `top` with its corners at `at` (x, y, z a corner), or
 * `{ depth: -1, part: "" }` with no corner within two cells. Cells of `cell` metres grid it.
 *
 * `reference`'s answers to the bit (skin.test.ts holds it on the body's poses), many times as
 * fast: the nearest triangle found down a tree of boxes (`Tree`), not through every triangle in
 * the cells around; the cells (for the nearest corner and the winding number's ray) in flat
 * arrays, not a map keyed by strings; no arrays made in a query's loops. Of triangles as near as
 * each other, the one `reference` would meet first is kept, so even its ties go the same way. */
export function surface(top: Topology, at: ArrayLike<number>, cell = 0.1): (q: V3) => Depth {
  const { tris, parts, caps, holes } = top;
  const P = (i: number): V3 => [at[i * 3], at[i * 3 + 1], at[i * 3 + 2]];
  const middles = holes.map((h) => h.reduce((m, i) => add(m, P(i), 1 / h.length), [0, 0, 0] as V3));
  const count = tris.length / 3;
  if (count === 0) return () => ({ depth: -1, part: "" });
  // Each triangle's bounds, metres and cells, and the box of cells they cover.
  const bounds = new Float64Array(count * 6);
  const cells = new Int32Array(count * 6);
  const lo = [Infinity, Infinity, Infinity];
  const hi = [-Infinity, -Infinity, -Infinity];
  for (let t = 0; t < count; t++)
    for (let j = 0; j < 3; j++) {
      const a = at[tris[t * 3] * 3 + j], b = at[tris[t * 3 + 1] * 3 + j], c = at[tris[t * 3 + 2] * 3 + j];
      bounds[t * 6 + j] = Math.min(a, b, c);
      bounds[t * 6 + 3 + j] = Math.max(a, b, c);
      cells[t * 6 + j] = Math.floor(bounds[t * 6 + j] / cell);
      cells[t * 6 + 3 + j] = Math.floor(bounds[t * 6 + 3 + j] / cell);
      lo[j] = Math.min(lo[j], cells[t * 6 + j]);
      hi[j] = Math.max(hi[j], cells[t * 6 + 3 + j]);
    }
  const dims = [0, 1, 2].map((j) => hi[j] - lo[j] + 1);
  const index = (x: number, y: number, z: number) => ((x - lo[0]) * dims[1] + (y - lo[1])) * dims[2] + (z - lo[2]);
  const faces = bin(count, lo, dims, cells);
  // The corners on a triangle, their places and cells, for the nearest one, whose distance
  // bounds the search.
  const usedList = trees.get(top)?.used ?? usedOf(top);
  const spots = new Float64Array(usedList.length * 6);
  const spotCells = new Int32Array(usedList.length * 3);
  for (let n = 0; n < usedList.length; n++)
    for (let j = 0; j < 3; j++) {
      spots[n * 6 + j] = spots[n * 6 + 3 + j] = at[usedList[n] * 3 + j];
      spotCells[n * 3 + j] = Math.floor(spots[n * 6 + j] / cell);
    }
  if (!trees.has(top)) trees.set(top, { faces: build(bounds), corners: build(spots), used: usedList });
  const tree = trees.get(top)!;
  const [faceBox, cornerBox] = [refit(tree.faces, bounds), refit(tree.corners, spots)];
  // The winding number at `q`, as `reference` counts it.
  const winding = (q0: number, q1: number, q2: number): number => {
    let sum = 0;
    const [y, z] = [Math.floor(q1 / cell), Math.floor(q2 / cell)];
    if (y >= lo[1] && y <= hi[1] && z >= lo[2] && z <= hi[2])
      for (let x = Math.max(Math.floor(q0 / cell), lo[0]); x <= hi[0]; x++) {
        const c = index(x, y, z);
        for (let k = faces.first[c]; k < faces.first[c + 1]; k++) {
          const t = faces.items[k];
          // Wholly behind the ray's start: `crossing` would clamp its crossing there, no count.
          if (bounds[t * 6 + 3] <= q0) continue;
          const a = tris[t * 3] * 3, b = tris[t * 3 + 1] * 3, d = tris[t * 3 + 2] * 3;
          crossingAt(q0, q1, q2, at[a], at[a + 1], at[a + 2], at[b], at[b + 1], at[b + 2], at[d], at[d + 1], at[d + 2]);
          if (crossed !== 0 && Math.floor(along / cell) === x) sum += crossed;
        }
      }
    for (let c = 0; c < caps.length; c += 3) {
      const i = caps[c + 1] * 3, j = caps[c] * 3, m = middles[caps[c + 2]];
      crossingAt(q0, q1, q2, at[i], at[i + 1], at[i + 2], at[j], at[j + 1], at[j + 2], m[0], m[1], m[2]);
      sum += crossed;
    }
    return sum;
  };
  return (q: V3): Depth => {
    const [q0, q1, q2] = q;
    const c = q.map((x) => Math.floor(x / cell));
    // The nearest corner in the cells within two of `q`'s, as `reference` finds it, down the
    // corners' tree: a box past those cells (with a nanometre for rounding) or farther than the
    // nearest yet passed by.
    let reach = Infinity;
    const block = [0, 1, 2].flatMap((j) => [(c[j] - 2) * cell - 1e-9, (c[j] + 3) * cell + 1e-9]);
    const outside = (b: number) => cornerBox[b] > block[1] || cornerBox[b + 3] < block[0] || cornerBox[b + 1] > block[3] || cornerBox[b + 4] < block[2] || cornerBox[b + 2] > block[5] || cornerBox[b + 5] < block[4];
    down(tree.corners, cornerBox, q0, q1, q2, () => reach + 1e-9, (n) => {
      if (Math.abs(spotCells[n * 3] - c[0]) > 2 || Math.abs(spotCells[n * 3 + 1] - c[1]) > 2 || Math.abs(spotCells[n * 3 + 2] - c[2]) > 2) return;
      reach = Math.min(reach, Math.hypot(spots[n * 6] - q0, spots[n * 6 + 1] - q1, spots[n * 6 + 2] - q2));
    }, outside);
    if (reach === Infinity) return { depth: -1, part: "" };
    // The first cell `reference` would meet a triangle in: its search box's first cell
    // (`from`, the cells of `q` less `reach`) or the triangle's, the later on each axis.
    const from0 = Math.floor((q0 - reach) / cell), from1 = Math.floor((q1 - reach) / cell), from2 = Math.floor((q2 - reach) / cell);
    const before = (t: number, u: number) => {
      const tx = Math.max(from0, cells[t * 6]), ux = Math.max(from0, cells[u * 6]);
      if (tx !== ux) return tx < ux;
      const ty = Math.max(from1, cells[t * 6 + 1]), uy = Math.max(from1, cells[u * 6 + 1]);
      if (ty !== uy) return ty < uy;
      const tz = Math.max(from2, cells[t * 6 + 2]), uz = Math.max(from2, cells[u * 6 + 2]);
      return tz !== uz ? tz < uz : t < u;
    };
    // The nearest triangle down the tree, a box farther than the nearest yet (or the nearest
    // corner, which no nearest point is beyond) passed by: with a nanometre for rounding, so a
    // triangle whose distance could tie is never passed.
    let best = Infinity;
    let won = -1;
    const p: V3 = [0, 0, 0];
    down(tree.faces, faceBox, q0, q1, q2, () => Math.min(best, reach) + 1e-9, (t, limit) => {
      if (boxDistance2(bounds, t, q0, q1, q2) > limit * limit) return;
      closestAt(q0, q1, q2, at, tris[t * 3], tris[t * 3 + 1], tris[t * 3 + 2]);
      const d = Math.hypot(near[0] - q0, near[1] - q1, near[2] - q2);
      // `reference` keeps the first it meets of those as near as the nearest.
      if (d > best || (d === best && !before(t, won))) return;
      [best, won] = [d, t];
      [p[0], p[1], p[2]] = near;
    });
    // The part: the nearest triangle's corner nearest the surface point.
    let e0 = 0;
    for (let e = 1; e < 3; e++) if (len(sub(P(tris[won * 3 + e]), p)) < len(sub(P(tris[won * 3 + e0]), p))) e0 = e;
    return { depth: winding(q0, q1, q2) > 0.5 ? best : -best, part: parts[tris[won * 3 + e0]] };
  };
}

const stack = new Int32Array(256);

/** Down `tree` (its boxes `box`) from `q`, the nearer child first, to each item of every leaf
 * whose box is within `limit()` of `q` (asked again at each node, as the search closes in) and
 * not `pass`ed: `visit(item, limit)`. */
function down(tree: Tree, box: Float64Array, q0: number, q1: number, q2: number, limit: () => number, visit: (item: number, limit: number) => void, pass = (_b: number) => false): void {
  let sp = 0;
  stack[sp++] = 0;
  while (sp > 0) {
    const k = stack[--sp];
    const most = limit();
    if (pass(k * 6) || boxDistance2(box, k, q0, q1, q2) > most * most) continue;
    if (tree.size[k] > 0) {
      for (let i = tree.start[k]; i < tree.start[k] + tree.size[k]; i++) visit(tree.order[i], most);
      continue;
    }
    const [l, r] = [k + 1, tree.right[k]];
    if (boxDistance2(box, l, q0, q1, q2) <= boxDistance2(box, r, q0, q1, q2)) (stack[sp++] = r), (stack[sp++] = l);
    else (stack[sp++] = l), (stack[sp++] = r);
  }
}

/** The square of the distance from `q` to box `k` of `boxes` (six a box: its least x, y, z,
 * its most), nought inside it. */
function boxDistance2(boxes: Float64Array, k: number, q0: number, q1: number, q2: number): number {
  const dx = Math.max(boxes[k * 6] - q0, 0, q0 - boxes[k * 6 + 3]);
  const dy = Math.max(boxes[k * 6 + 1] - q1, 0, q1 - boxes[k * 6 + 4]);
  const dz = Math.max(boxes[k * 6 + 2] - q2, 0, q2 - boxes[k * 6 + 5]);
  return dx * dx + dy * dy + dz * dz;
}

/** A bounding volume hierarchy over items with bounds (six an item, as `boxDistance2` reads
 * them), nodes depth first, each node's left child just after it: a leaf (`size` over nought)
 * holds `order[start]` up to `start + size`; a branch's right child is `right`. Built on the
 * first pose a topology is measured at, split at the middle item along the longest axis; every
 * pose refits its boxes (`refit`), as a ray tracer refits a skinned mesh's tree each frame
 * rather than rebuilding it. */
type Tree = { order: Int32Array; start: Int32Array; size: Int32Array; right: Int32Array };
/** Each topology's trees, of its triangles and of its corners on a triangle (`used`). */
const trees = new WeakMap<Topology, { faces: Tree; corners: Tree; used: Int32Array }>();

/** The corners on a triangle of `top`, in order. */
function usedOf(top: Topology): Int32Array {
  const used = new Uint8Array(top.parts.length);
  for (const i of top.tris) used[i] = 1;
  return Int32Array.from({ length: used.length }, (_, i) => i).filter((i) => used[i] === 1);
}

function build(bounds: Float64Array): Tree {
  const count = bounds.length / 6;
  const order = Int32Array.from({ length: count }, (_, i) => i);
  const middle = (t: number, j: number) => bounds[t * 6 + j] + bounds[t * 6 + 3 + j];
  const [start, size, right]: number[][] = [[], [], []];
  const node = (from: number, to: number): number => {
    const k = start.push(from) - 1;
    size.push(0);
    right.push(-1);
    if (to - from <= 4) return (size[k] = to - from), k;
    const lo = [Infinity, Infinity, Infinity];
    const hi = [-Infinity, -Infinity, -Infinity];
    for (let i = from; i < to; i++)
      for (let j = 0; j < 3; j++) (lo[j] = Math.min(lo[j], middle(order[i], j))), (hi[j] = Math.max(hi[j], middle(order[i], j)));
    const axis = [0, 1, 2].reduce((a, j) => (hi[j] - lo[j] > hi[a] - lo[a] ? j : a), 0);
    order.subarray(from, to).sort((a, b) => middle(a, axis) - middle(b, axis));
    const half = (from + to) >> 1;
    node(from, half);
    right[k] = node(half, to);
    return k;
  };
  node(0, count);
  return { order, start: Int32Array.from(start), size: Int32Array.from(size), right: Int32Array.from(right) };
}

/** The tree's boxes around its items' `bounds` at a pose, children before parents. */
function refit(tree: Tree, bounds: Float64Array): Float64Array {
  const box = new Float64Array(tree.start.length * 6);
  for (let k = tree.start.length - 1; k >= 0; k--) {
    const leaf = tree.size[k] > 0;
    // A leaf's items, or a branch's two children.
    const [from, n] = leaf ? [bounds, tree.size[k]] : [box, 2];
    for (let m = 0; m < n; m++) {
      const i = (leaf ? tree.order[tree.start[k] + m] : m === 0 ? k + 1 : tree.right[k]) * 6;
      for (let j = 0; j < 3; j++) {
        box[k * 6 + j] = m === 0 ? from[i + j] : Math.min(box[k * 6 + j], from[i + j]);
        box[k * 6 + 3 + j] = m === 0 ? from[i + 3 + j] : Math.max(box[k * 6 + 3 + j], from[i + 3 + j]);
      }
    }
  }
  return box;
}

/** How deep a point is inside the mesh `top` with its corners at `at`, as `surface` answers, the
 * plain way it was first written (39c90245): the sure answer `surface`'s test holds it to. */
export function reference(top: Topology, at: ArrayLike<number>, cell = 0.1): (q: V3) => Depth {
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
