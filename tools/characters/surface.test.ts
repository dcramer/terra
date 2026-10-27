// surface.ts against answers known another way: an L-shaped prism, whose inner corner is a
// crease like the hip's, at points whose distance to it is worked out by hand.
//   node --test tools/characters/surface.test.ts

import { test } from "node:test";
import assert from "node:assert/strict";
import { surface, topology } from "./surface.ts";
import { type V3 } from "./glb.ts";

/** The L (20 by 20 cm, 10 deep): the square [0, 0.2]² less [0.1, 0.2]², from z 0 to 0.1. */
function ell() {
  const ring: [number, number][] = [[0, 0], [0.2, 0], [0.2, 0.1], [0.1, 0.1], [0.1, 0.2], [0, 0.2]];
  const at: number[] = [];
  for (const z of [0, 0.1]) for (const [x, y] of ring) at.push(x, y, z);
  const n = ring.length;
  const tris: number[] = [];
  // The caps, fanned from the inner corner (3), which sees every other: the top counter-clockwise
  // from +z, the bottom the other way.
  for (let i = 4; i < 3 + n - 1; i++) {
    const [a, b] = [i % n, (i + 1) % n];
    tris.push(n + 3, n + a, n + b, 3, b, a);
  }
  // The walls, each edge of the ring counter-clockwise, so out is to its right.
  for (let i = 0; i < n; i++) {
    const j = (i + 1) % n;
    tris.push(i, j, n + j, i, n + j, n + i);
  }
  return { top: topology(tris, at.map((_, i) => (i % 3 === 0 ? "ell" : "")).filter(Boolean)), at };
}

test("the depth in and out of a crease is the distance to the surface, signed by side", () => {
  const { top, at } = ell();
  const inside = surface(top, at);
  const cases: [V3, number][] = [
    // Out in the notch, between its two walls: out by the distance to either.
    [[0.15, 0.15, 0.05], -0.05],
    [[0.12, 0.12, 0.05], -0.02],
    // Inside, by the inner edge: in by the distance to it.
    [[0.09, 0.09, 0.05], Math.hypot(0.01, 0.01)],
    // In the notch across from one wall, and inside behind it.
    [[0.13, 0.15, 0.05], -0.03],
    [[0.05, 0.15, 0.05], 0.05],
    // Beyond the end of an arm, and deep in its middle.
    [[0.3, 0.05, 0.05], -0.1],
    [[0.15, 0.05, 0.04], 0.04],
  ];
  for (const [q, want] of cases) {
    const got = inside(q).depth;
    assert.ok(Math.abs(got - want) < 1e-9, `${q}: ${got} not ${want}`);
  }
});

/** An axis-aligned box from `lo` to `hi` as twelve outward triangles, its corners from `base`;
 * `open` leaves out its top (+z). */
function box(lo: V3, hi: V3, base = 0, open = false) {
  const at: number[] = [];
  for (let i = 0; i < 8; i++) at.push(i & 1 ? hi[0] : lo[0], i & 2 ? hi[1] : lo[1], i & 4 ? hi[2] : lo[2]);
  // Each face's four corners counter-clockwise from outside.
  const quads = [[0, 2, 3, 1], [0, 1, 5, 4], [1, 3, 7, 5], [3, 2, 6, 7], [2, 0, 4, 6], ...(open ? [] : [[4, 5, 7, 6]])];
  const tris = quads.flatMap(([a, b, c, d]) => [a, b, c, a, c, d].map((i) => base + i));
  return { at, tris };
}

test("a mesh cutting through itself is inside wherever either part is", () => {
  // Two 20 cm boxes as one mesh, overlapping 10 cm in x: each one's faces run through the other,
  // as a bent body's skin runs through itself.
  const a = box([0, 0, 0], [0.2, 0.2, 0.2]);
  const b = box([0.1, 0, 0], [0.3, 0.2, 0.2], 8);
  const at = [...a.at, ...b.at];
  const inside = surface(topology([...a.tris, ...b.tris], Array(16).fill("box")), at);
  const cases: [V3, number][] = [
    // In both: as deep as the nearest face, b's buried one at x 0.1.
    [[0.12, 0.1, 0.1], 0.02],
    // In a only, by b's buried face; in b only, by a's.
    [[0.07, 0.1, 0.1], 0.03],
    [[0.24, 0.1, 0.1], 0.04],
    // Out beside the overlap, above it, and beyond b.
    [[0.15, 0.1, 0.25], -0.05],
    [[0.15, -0.02, 0.1], -0.02],
    [[0.35, 0.1, 0.1], -0.05],
  ];
  for (const [q, want] of cases) {
    const got = inside(q).depth;
    assert.ok(Math.abs(got - want) < 1e-9, `${q}: ${got} not ${want}`);
  }
});

test("a mesh with a hole is closed by its cap", () => {
  // A box with no top, as the torso has no arms: inside under the hole, outside above it, and
  // measured to the skin only, never the cap (under the hole, to the walls).
  const { at, tris } = box([0, 0, 0], [0.2, 0.2, 0.2], 0, true);
  const inside = surface(topology(tris, Array(8).fill("box")), at);
  const cases: [V3, number][] = [
    [[0.1, 0.1, 0.15], 0.1],
    [[0.1, 0.1, 0.05], 0.05],
    [[0.1, 0.1, 0.25], -Math.hypot(0.05, 0.1)],
    [[0.1, 0.1, 0.21], -Math.hypot(0.01, 0.1)],
  ];
  for (const [q, want] of cases) {
    const got = inside(q).depth;
    assert.ok(Math.abs(got - want) < 1e-9, `${q}: ${got} not ${want}`);
  }
});

test("a point moved straight out of the crease only ever comes out", () => {
  const { top, at } = ell();
  const inside = surface(top, at);
  let last = Infinity;
  for (let d = -0.04; d <= 0.08; d += 0.005) {
    const depth = inside([0.1 + d, 0.1 + d, 0.05]).depth;
    assert.ok(depth < last, `at ${d}: ${depth} after ${last}`);
    last = depth;
  }
});
