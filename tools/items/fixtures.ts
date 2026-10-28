// Made-up parts for the part checker's tests (D565 step 2): a box head with a round eye, a collar,
// a code-made haft, each built from numbers so the right answer is known without the checker.
// The Rust assembler's tests use the same numbers (terra_items::assembly's tests).

import type { Mesh } from "./mesh.ts";

const N = 32; // sides of the round eye: off a true circle by r (1 - cos(pi/32)), under 0.1 mm here

type Ring = [number, number][];
const circle = (r: number): Ring => [...Array(N).keys()].map((i) => [r * Math.cos((2 * Math.PI * i) / N), r * Math.sin((2 * Math.PI * i) / N)]);
// A square of half-side h, met by the same rays as the circle, so the two rings pair vertex by vertex.
const square = (h: number): Ring =>
  circle(1).map(([c, s]) => {
    const k = h / Math.max(Math.abs(c), Math.abs(s));
    return [c * k, s * k];
  });

/** A closed tube: an outer ring round an inner one, from y0 up to y1, faces outward. */
export function tube(outer: Ring, inner: Ring, y0: number, y1: number, material = "Metal"): Mesh {
  const positions: number[] = [];
  const indices: number[] = [];
  const at = (ring: Ring, i: number, y: number) => (positions.push(ring[i % N][0], y, ring[i % N][1]), positions.length / 3 - 1);
  const quad = (a: number, b: number, c: number, d: number) => indices.push(a, b, c, a, c, d);
  for (let i = 0; i < N; i++) {
    const [o0, o1, i0, i1] = [at(outer, i, y0), at(outer, i + 1, y0), at(inner, i, y0), at(inner, i + 1, y0)];
    const [O0, O1, I0, I1] = [at(outer, i, y1), at(outer, i + 1, y1), at(inner, i, y1), at(inner, i + 1, y1)];
    quad(o0, O0, O1, o1); // outer wall
    quad(i0, i1, I1, I0); // inner wall, facing the line
    quad(O0, I0, I1, O1); // top, +Y
    quad(o0, o1, i1, i0); // bottom, -Y
  }
  return { positions, indices, materials: [material], textures: [], scaled: [] };
}

export const EYE = 0.035; // the eye across, the reference axe's grip (axe.json)
export const HEAD_H = 0.06;
export const boxHead = (eye = EYE, top = HEAD_H) => tube(square(0.03), circle(eye / 2), 0, top);
export const collarMesh = (top = 0) => tube(circle(0.022), circle(EYE / 2), -0.02, top, "Leather");

const n = (value: number, source = "made up for the part checker's tests") => ({ value, source });

export const head = (over: Record<string, any> = {}) => ({
  part: "test_box_head",
  type: "axe_head",
  about: "a made-up box head with a round eye",
  model: "models/parts/test_box_head.glb",
  mass_kg: n(0.9),
  centre_m: [0, 0.03, 0],
  bounds_m: [[-0.03, 0, -0.03], [0.03, HEAD_H, 0.03]],
  points: [
    { name: "eye.lower", type: "eye", at: [0, 0, 0], dir: [0, 1, 0], size: EYE },
    { name: "eye.upper", type: "eye", at: [0, HEAD_H, 0], dir: [0, 1, 0], size: EYE },
    { name: "edge.lower", type: "edge", at: [0, 0.01, -0.03], dir: [0, 0, -1] },
    { name: "edge.upper", type: "edge", at: [0, 0.05, -0.03], dir: [0, 0, -1] },
  ],
  ...over,
});

export const collar = (over: Record<string, any> = {}) => ({
  part: "test_collar",
  type: "collar",
  about: "a made-up leather collar under the head",
  kinds: ["axe"],
  model: "models/parts/test_collar.glb",
  mass_kg: n(0.01),
  centre_m: [0, -0.01, 0],
  bounds_m: [[-0.022, -0.02, -0.022], [0.022, 0, 0.022]],
  cover_m: [-0.02, 0],
  points: [{ name: "bore", type: "bore", at: [0, 0, 0], dir: [0, 1, 0], size: EYE }],
  ...over,
});

export const haft = (over: Record<string, any> = {}) => ({
  part: "test_haft",
  type: "haft",
  about: "a made-up ash haft, the reference axe's",
  kinds: ["axe"],
  length_m: n(0.68),
  grip_from_end_m: n(0.08),
  grip_diameter_m: n(0.035),
  density_kg_m3: n(670),
  ...over,
});
