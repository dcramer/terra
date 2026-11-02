import { test } from "node:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { Glb, type Pose, posed } from "./glb.ts";
import type { Piece } from "./intact.ts";
import { layered, retargeted, topEdge } from "./layers.ts";

const I = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];

/** An open tube about the y axis, every vertex on the bone `a`, bound at rest. */
function tube(r: number, y0: number, y1: number, segs = 32, rings = 8): Piece {
  const pos: number[] = [];
  const tris: number[] = [];
  for (let j = 0; j <= rings; j++)
    for (let k = 0; k < segs; k++) {
      const a = (2 * Math.PI * k) / segs;
      pos.push(r * Math.cos(a), y0 + ((y1 - y0) * j) / rings, r * Math.sin(a));
    }
  for (let j = 0; j < rings; j++)
    for (let k = 0; k < segs; k++) {
      const [a, b, c, d] = [j * segs + k, j * segs + ((k + 1) % segs), (j + 1) * segs + k, (j + 1) * segs + ((k + 1) % segs)];
      tris.push(a, b, d, a, d, c);
    }
  const count = pos.length / 3;
  return {
    file: "test",
    node: "tube",
    mesh: "tube",
    count,
    pos: Float64Array.from(pos),
    uvs: [],
    weights: Array.from({ length: count }, () => new Map([["a", 1]])),
    tris: Uint32Array.from(tris),
    bind: new Map([
      ["a", I],
      ["b", I],
    ]),
  };
}

const still: Map<string, Pose> = new Map([
  ["a", { t: [0, 0, 0], r: [0, 0, 0, 1], s: [1, 1, 1] }],
  ["b", { t: [0, 1, 0], r: [0, 0, 0, 1], s: [1, 1, 1] }],
]);
const bones = new Map([["a", ["b"]]]);

/** The inner tube under the outer, at rest. */
function wear(inner: Piece, outer: Piece) {
  const all = Array.from({ length: inner.count }, (_, i) => i);
  const tris = Array.from({ length: outer.tris.length / 3 }, (_, k) => 3 * k);
  return layered(inner, outer, still, bones, all, topEdge(inner), tris);
}

test("a slimmer piece under a longer one neither pokes nor shows its top", () => {
  const f = wear(tube(0.1, 0.2, 0.6), tube(0.12, 0.3, 0.8));
  assert.equal(f.worst, 0);
  assert.equal(f.uncovered, 0);
  assert.equal(f.edge, 32);
});

test("a fuller piece under it pokes through by the difference in radius", () => {
  const f = wear(tube(0.15, 0.2, 0.6), tube(0.12, 0.3, 0.8));
  assert.ok(Math.abs(f.worst - 0.03) < 1e-3, `worst ${f.worst}`);
  // Its rings from 0.3 to 0.6 up, every 5 cm, seven of them, all round.
  assert.equal(f.poking, 7 * 32);
});

test("an outer piece too short leaves the inner's top edge uncovered", () => {
  const f = wear(tube(0.1, 0.2, 0.6), tube(0.12, 0.3, 0.55));
  assert.equal(f.uncovered, 32);
});

test("a clip carried onto its own skeleton is the clip", () => {
  const man = new Glb(fileURLToPath(new URL("../../assets/models/character/mannequin.glb", import.meta.url)));
  const clip = man.clip("Walk_Loop")!;
  const mine = retargeted(man, man, clip, 0.4);
  const theirs = posed(man, clip, 0.4);
  for (const name of ["pelvis", "hand_l", "foot_r", "Head"]) {
    const [a, b] = [mine.get(name)!.t, theirs[man.named(name)].t];
    assert.ok(Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]) < 0.002, `${name}: ${a} against ${b}`);
  }
});
