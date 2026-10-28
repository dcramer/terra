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

/** A closed box from corner lo to corner hi, faces outward: 12 triangles. */
export function box(lo: [number, number, number], hi: [number, number, number]): Mesh {
  const c = (i: number) => [i & 1 ? hi[0] : lo[0], i & 2 ? hi[1] : lo[1], i & 4 ? hi[2] : lo[2]];
  const positions = [...Array(8).keys()].flatMap(c);
  // Each face's corners counter-clockwise seen from outside: -X, +X, -Y, +Y, -Z, +Z.
  const faces = [[0, 4, 6, 2], [1, 3, 7, 5], [0, 1, 5, 4], [2, 6, 7, 3], [0, 2, 3, 1], [4, 5, 7, 6]];
  const indices = faces.flatMap(([a, b, c, d]) => [a, b, c, a, c, d]);
  return { positions, indices, materials: ["Metal"], textures: [], scaled: [] };
}

/** A glb of one mesh on one node, with the node's scale: a made-up part's model. */
export function glbOf(m: Mesh, scale = [1, 1, 1], name = "head"): Buffer {
  const pos = Buffer.from(new Float32Array(m.positions).buffer);
  const idx = Buffer.from(new Uint32Array(m.indices).buffer);
  const lo = [0, 1, 2].map((k) => Math.min(...m.positions.filter((_, i) => i % 3 === k)));
  const hi = [0, 1, 2].map((k) => Math.max(...m.positions.filter((_, i) => i % 3 === k)));
  const gltf = {
    asset: { version: "2.0" },
    scenes: [{ nodes: [0] }],
    nodes: [{ name, mesh: 0, scale }],
    meshes: [{ primitives: [{ attributes: { POSITION: 0 }, indices: 1, material: 0 }] }],
    materials: [{ name: m.materials[0] }],
    buffers: [{ byteLength: pos.length + idx.length }],
    bufferViews: [{ buffer: 0, byteOffset: 0, byteLength: pos.length }, { buffer: 0, byteOffset: pos.length, byteLength: idx.length }],
    accessors: [
      { bufferView: 0, componentType: 5126, count: m.positions.length / 3, type: "VEC3", min: lo, max: hi },
      { bufferView: 1, componentType: 5125, count: m.indices.length, type: "SCALAR" },
    ],
  };
  let json = Buffer.from(JSON.stringify(gltf));
  json = Buffer.concat([json, Buffer.alloc((4 - (json.length % 4)) % 4, 0x20)]);
  const bin = Buffer.concat([pos, idx]);
  const head = Buffer.alloc(12);
  head.writeUInt32LE(0x46546c67, 0);
  head.writeUInt32LE(2, 4);
  head.writeUInt32LE(12 + 8 + json.length + 8 + bin.length, 8);
  const chunk = (b: Buffer, t: number) => {
    const h = Buffer.alloc(8);
    h.writeUInt32LE(b.length, 0);
    h.writeUInt32LE(t, 4);
    return Buffer.concat([h, b]);
  };
  return Buffer.concat([head, chunk(json, 0x4e4f534a), chunk(bin, 0x004e4942)]);
}

export const EYE = 0.035; // the eye across, the reference axe's grip (axe.json)
export const HEAD_H = 0.06;
// The box head's own volume, worked by hand: its 6 cm square less its eye, a 32-gon of radius r
// (area N/2 r^2 sin(2 pi/N)), times its height.
export const HEAD_M3 = HEAD_H * (0.06 * 0.06 - (N / 2) * (EYE / 2) ** 2 * Math.sin((2 * Math.PI) / N));
export const boxHead =(eye = EYE, top = HEAD_H) => tube(square(0.03), circle(eye / 2), 0, top);
export const collarMesh = (top = 0) => tube(circle(0.022), circle(EYE / 2), -0.02, top, "Leather");
// The collar's, the same way: two 32-gons, 2.2 cm and the eye's radius, 2 cm high; in leather.
const gon = (r: number) => (N / 2) * r ** 2 * Math.sin((2 * Math.PI) / N);
export const COLLAR_M3 = 0.02 * (gon(0.022) - gon(EYE / 2));
export const LEATHER = 860;

const n = (value: number, source = "made up for the part checker's tests") => ({ value, source });

export const head = (over: Record<string, any> = {}) => ({
  part: "test_box_head",
  type: "axe_head",
  about: "a made-up box head with a round eye",
  model: "models/parts/test_box_head.glb",
  volume_m3: n(HEAD_M3),
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
  density_kg_m3: n(LEATHER),
  mass_kg: n(Number((COLLAR_M3 * LEATHER).toFixed(4))),
  centre_m: [0, -0.01, 0],
  bounds_m: [[-0.022, -0.02, -0.022], [0.022, 0, 0.022]],
  cover_m: [-0.02, 0],
  points: [{ name: "bore", type: "bore", at: [0, 0, 0], dir: [0, 1, 0], size: EYE }],
  ...over,
});

export const haft = (over: Record<string, any> = {}) => ({
  part: "test_haft",
  type: "haft",
  about: "a made-up ash haft, short enough that the box head's 1.25 kg of iron swings inside the axe's inertia",
  kinds: ["axe"],
  length_m: n(0.62),
  grip_from_end_m: n(0.08),
  grip_diameter_m: n(0.035),
  density_kg_m3: n(670),
  ...over,
});
