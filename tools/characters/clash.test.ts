import assert from "node:assert/strict";
import { test } from "node:test";
import { type Ball, type Board, type Mesh, board, meshOf, sliced, through } from "./clash.ts";
import { Glb, type V3 } from "./glb.ts";

// Meshes made of boxes and cylinders, so their sizes are known.
function join(...parts: Mesh[]): Mesh {
  const points: V3[] = [];
  const tris: number[] = [];
  for (const m of parts) {
    const base = points.length;
    points.push(...m.points);
    for (let i = 0; i < m.tris.length; i++) tris.push(m.tris[i] + base);
  }
  return { points, tris };
}
function box(lo: V3, hi: V3): Mesh {
  const points: V3[] = [];
  for (let i = 0; i < 8; i++) points.push([i & 1 ? hi[0] : lo[0], i & 2 ? hi[1] : lo[1], i & 4 ? hi[2] : lo[2]]);
  const quads = [[0, 1, 3, 2], [4, 5, 7, 6], [0, 1, 5, 4], [2, 3, 7, 6], [0, 2, 6, 4], [1, 3, 7, 5]];
  return { points, tris: quads.flatMap(([a, b, c, d]) => [a, b, c, a, c, d]) };
}
/** A cylinder about z, centred on (x, y), from z0 to z1. */
function cylinder(radius: number, z0: number, z1: number, x = 0, y = 0, segs = 48): Mesh {
  const points: V3[] = [[x, y, z0], [x, y, z1]];
  for (let i = 0; i < segs; i++) {
    const a = (2 * Math.PI * i) / segs;
    points.push([x + radius * Math.cos(a), y + radius * Math.sin(a), z0], [x + radius * Math.cos(a), y + radius * Math.sin(a), z1]);
  }
  const tris: number[] = [];
  for (let i = 0; i < segs; i++) {
    const [a0, a1, b0, b1] = [2 + 2 * i, 3 + 2 * i, 2 + 2 * ((i + 1) % segs), 3 + 2 * ((i + 1) % segs)];
    tris.push(0, a0, b0, 1, a1, b1, a0, b0, b1, a0, b1, a1);
  }
  return { points, tris };
}

// A board 0.6 m across, 3 cm thick (z -0.02 to 0.01), with a boss in front and a grip behind.
const shield = join(cylinder(0.3, -0.02, 0.01), cylinder(0.08, -0.06, -0.02), box([-0.1, 0.12, 0.01], [0.1, 0.16, 0.04]));
// A sword along +y: a blade 0.8 m long, 5 cm wide, 6 mm thick; a guard 18 cm across; a grip.
const sword = join(box([-0.003, 0.02, -0.025], [0.003, 0.82, 0.025]), box([-0.012, -0.02, -0.09], [0.012, 0.02, 0.09]), box([-0.012, -0.12, -0.015], [0.012, -0.02, 0.015]));

test("a board is measured as its disc, leaving out the boss in front and the grip behind", () => {
  const b = board(shield);
  assert.ok(Math.abs(b.radius - 0.3) < 1e-6, `radius ${b.radius}`);
  assert.ok(Math.abs(b.half - 0.015) < 1e-6, `half ${b.half}`);
  assert.ok(Math.abs(b.centre[2] + 0.005) < 1e-6 && Math.abs(b.normal[2]) === 1, `centre ${b.centre}, normal ${b.normal}`);
});

test("a sword is sliced into blade, guard and hilt, as thick as each is", () => {
  const balls = sliced(sword, [0, 0.82, 0]);
  const of = (part: string) => balls.filter((b) => b.part === part);
  const ys = (part: string) => of(part).map((b) => b.p[1]);
  assert.ok(Math.min(...ys("blade")) > 0.02 && Math.max(...ys("blade")) > 0.81, "the blade runs from the guard to the tip");
  assert.ok(Math.min(...ys("guard")) > -0.02 && Math.max(...ys("guard")) < 0.02, "the guard is its band");
  assert.ok(Math.max(...ys("hilt")) < -0.02 && Math.min(...ys("hilt")) < -0.11, "the hilt is below it");
  assert.ok(of("blade").every((b) => Math.abs(b.r - 0.003) < 1e-6), "the blade is 6 mm thick");
  // Its balls reach its edges and no further.
  const reach = Math.max(...of("blade").map((b) => Math.abs(b.p[2]) + b.r));
  assert.ok(Math.abs(reach - 0.025) < 1e-6, `blade reaches ${reach}`);
});

// A line of balls of 3 mm from a to b, a centimetre apart.
const line = (a: V3, b: V3, part = "blade"): Ball[] => {
  const n = Math.ceil(Math.hypot(b[0] - a[0], b[1] - a[1], b[2] - a[2]) / 0.01);
  return [...Array(n + 1).keys()].map((i) => ({ p: a.map((x, j) => x + ((b[j] - x) * i) / n) as V3, r: 0.003, part }));
};
const disc: Board = { centre: [0, 0, 0], normal: [0, 0, 1], radius: 0.3, half: 0.015 };

test("a blade through the board's middle reads as deep as it came out of the nearer face", () => {
  // In from behind 0.6 m, out the front 0.2 m: clearing it is 0.2 m back, and the board and blade's thickness.
  const { cm, part } = through(line([0, 0, -0.6], [0, 0, 0.2]), disc);
  assert.ok(Math.abs(cm - 21.8) < 0.01, `${cm} cm`);
  assert.equal(part, "blade");
});

test("a tip through near the rim reads by the way out past the rim", () => {
  // 3 cm out of the front face, 5 cm in from the rim: past the rim is the shorter way out.
  const { cm } = through(line([0.25, 0, -0.5], [0.25, 0, 0.045]), disc);
  assert.ok(Math.abs(cm - 5.3) < 0.01, `${cm} cm`);
});

test("a blade clear of the board reads its gap, in front of it or past its rim", () => {
  const before = through(line([-0.2, 0, -0.1], [0.2, 0, -0.1]), disc);
  assert.ok(Math.abs(before.cm + 8.2) < 0.01, `in front: ${before.cm} cm`);
  const beside = through(line([0.4, 0, -0.5], [0.4, 0, 0.5]), disc);
  assert.ok(Math.abs(beside.cm + 9.7) < 0.01, `past the rim: ${beside.cm} cm`);
});

test("the round shield's board is the model's disc, between its boss and its grip", () => {
  // The model is 0.8 m across by its bounds, the boss at its front (-z) 8.5 cm out, the grip at its back 3.5 cm.
  const b = board(meshOf(new Glb("assets/models/held/round_shield.glb")));
  assert.ok(b.radius > 0.38 && b.radius < 0.42, `radius ${b.radius}`);
  assert.ok(b.half > 0.005 && b.half < 0.02, `half ${b.half}`);
  assert.ok(b.centre[2] - b.half > -0.085 && b.centre[2] + b.half < 0.035, `faces ${b.centre[2] - b.half} to ${b.centre[2] + b.half}`);
});
