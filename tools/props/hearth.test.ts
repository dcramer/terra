import assert from "node:assert/strict";
import { test } from "node:test";
import { bed } from "./coals.ts";
import { BUDGET, HALF, hearth, TALL } from "./hearth.ts";

const { wood, stone, logs, stones } = hearth();
const tris = (m: { idx: number[] }) => m.idx.length / 3;

test("a hero campfire keeps to its budget, its coal bed's included, and spends most of it", () => {
  const all = tris(wood) + tris(stone) + bed().idx.length / 3;
  assert.ok(all <= BUDGET && all >= BUDGET * 0.6, `${all} triangles`);
});

test("it stands inside its catalogue footprint and height, on the ground", () => {
  for (const m of [wood, stone]) {
    for (let i = 0; i < m.pos.length; i += 3) {
      assert.ok(Math.hypot(m.pos[i], m.pos[i + 2]) <= HALF, `${m.pos[i]}, ${m.pos[i + 2]} past the footprint`);
      assert.ok(m.pos[i + 1] <= TALL && m.pos[i + 1] >= -0.03, `height ${m.pos[i + 1]}`);
    }
  }
});

test("its logs lean in over the middle from inside the ring of stones", () => {
  const ring = Math.min(...stones.map((s) => Math.hypot(s.at[0], s.at[2])));
  for (const l of logs) {
    const top = l.foot.map((v, i) => v + l.axis[i] * l.L);
    assert.ok(Math.hypot(l.foot[0], l.foot[2]) < ring, "foot inside the stones");
    assert.ok(Math.hypot(top[0], top[2]) < 0.15 && top[1] > 0.3, "top over the middle");
  }
});

test("every face of it faces out: its logs from their axes, its stones from their middles", () => {
  const p = (m: { pos: number[] }, k: number) => [m.pos[k * 3], m.pos[k * 3 + 1], m.pos[k * 3 + 2]];
  const sub = (a: number[], b: number[]) => a.map((v, i) => v - b[i]);
  const cross = (a: number[], b: number[]) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const dot = (a: number[], b: number[]) => a.reduce((s, v, i) => s + v * b[i], 0);
  let inward = 0;
  for (let i = 0; i < stone.idx.length; i += 3) {
    const [a, b, c] = [0, 1, 2].map((k) => p(stone, stone.idx[i + k]));
    const near = stones.findLast((s) => s.from! <= i / 3)!;
    if (dot(cross(sub(b, a), sub(c, a)), sub(a, near.at)) < 0) inward++;
  }
  assert.equal(inward, 0, `${inward} stone faces turned in`);
  // And their vertex normals, which light them: turned in, a stone is black under the sun.
  for (let i = 0; i < stone.idx.length; i++) {
    const k = stone.idx[i], near = stones.findLast((s) => s.from! <= Math.floor(i / 3))!;
    assert.ok(dot(stone.nrm.slice(k * 3, k * 3 + 3), sub(p(stone, k), near.at)) > 0, "a stone's normal turned in");
  }
  // A log's vertex normals point away from its axis or along it at its ends.
  for (let k = 0; k < wood.pos.length / 3; k++) {
    const at = p(wood, k), n = wood.nrm.slice(k * 3, k * 3 + 3);
    const l = logs.findLast((x) => x.from! <= k)!;
    const r = sub(at, l.foot), t = dot(r, l.axis);
    const off = t <= 0.001 ? l.axis.map((v) => -v) : t >= l.L - 0.001 ? l.axis : sub(r, l.axis.map((v) => v * t));
    assert.ok(dot(n, off) > -1e-6, "a log's normal turned in");
  }
});
