import { test } from "node:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { Glb, type Pose, posed } from "./glb.ts";
import type { Piece } from "./intact.ts";
import { cover, type Frame, Grid, inPlace, layered, retargeted, type Run, said, skinned, topEdge } from "./layers.ts";

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

test("a grid walk measures as trying every triangle does", () => {
  const [inner, outer] = [tube(0.15, 0.2, 0.6), tube(0.12, 0.3, 0.8)];
  const all = Array.from({ length: inner.count }, (_, i) => i);
  const vi = skinned(inner, still);
  const vo = skinned(outer, still);
  const grid = new Grid(vo, outer.tris);
  const buf: number[] = [];
  const f = cover(inner, vi, vo, outer.tris, (o, d, l) => grid.along(o, d, l + 0.12, buf), still, bones, all, topEdge(inner));
  const brute = wear(inner, outer);
  assert.equal(f.poking, brute.poking);
  assert.ok(Math.abs(f.worst - brute.worst) < 1e-9, `${f.worst} against ${brute.worst}`);
  assert.equal(f.uncovered, brute.uncovered);
});

test("a travelling clip played in place leaves no skin weighted to the root behind", () => {
  // Their full body weights its heels a little to the root; a travelling clip carries the pelvis
  // 2 m on while the root bone stays. The boot follows the pelvis alone, the heel 3% the root.
  const on = (p: Piece, w: [string, number][]): Piece => ({
    ...p,
    weights: p.weights.map(() => new Map(w)),
    bind: new Map([
      ["pelvis", I],
      ["root", I],
    ]),
  });
  const heel = on(tube(0.1, 0.2, 0.6), [
    ["pelvis", 0.97],
    ["root", 0.03],
  ]);
  const boot = on(tube(0.12, 0.1, 0.8), [["pelvis", 1]]);
  const pose = (t: [number, number, number]): Pose => ({ t, r: [0, 0, 0, 1], s: [1, 1, 1] });
  const rest = new Map([
    ["root", pose([0, 0, 0])],
    ["pelvis", pose([0, 1, 0])],
  ]);
  const away = () => new Map([...rest, ["pelvis", pose([2, 1, 0.5])]]);
  const all = Array.from({ length: heel.count }, (_, i) => i);
  const tris = Array.from({ length: boot.tris.length / 3 }, (_, k) => 3 * k);
  const worn = (joints: Map<string, Pose>) => layered(heel, boot, joints, new Map(), all, topEdge(heel), tris);
  // Left behind, the heel lags 6 cm, through the boot by about 4.
  assert.ok(worn(away()).worst > 0.03, `travelling: ${worn(away()).worst}`);
  const f = worn(inPlace(away(), rest));
  assert.equal(f.worst, 0);
  assert.equal(f.uncovered, 0);
  // Only the ground plane is carried: the root keeps its height.
  assert.deepEqual(inPlace(away(), rest).get("root")!.t, [2, 0, 0.5]);
});

test("the terminal's line fails a planted poke-through and names its clip, the numbers in the log (D388)", () => {
  const rest = wear(tube(0.1, 0.2, 0.6), tube(0.12, 0.3, 0.8));
  const run = (poked: Frame): Run => {
    const clips = new Map([["Walk", { poke: rest.worst, open: rest.uncovered }], ["Swing", { poke: poked.worst, open: poked.uncovered }]]);
    return { rest, worst: { ...poked, clip: "Swing", t: 0.5 }, open: { ...rest, clip: "rest", t: 0 }, frames: 24, poked: poked.poking ? 1 : 0, holed: 0, clips };
  };
  const ok = said("coat over shirt", run(rest));
  assert.ok(ok.ok);
  assert.equal(ok.line.split("\n").length, 1);
  assert.match(ok.line, /^✓ layers: coat over shirt: /);
  // The planted fault: a fuller shirt in one clip, 3 cm through the coat.
  const bad = said("coat over shirt", run(wear(tube(0.15, 0.2, 0.6), tube(0.12, 0.3, 0.8))));
  assert.ok(!bad.ok);
  assert.equal(bad.line.split("\n").length, 1);
  assert.match(bad.line, /^✗ layers: coat over shirt: 1 of 2 clips .*: Swing 30\/32/);
  assert.match(bad.log, /worst 30\.0 mm, 224 vertices past 2 mm, Swing@0\.50/);
});
