import { test } from "node:test";
import assert from "node:assert/strict";
import type { Piece } from "./intact.ts";
import { joined } from "./layers.ts";
import { hiddenPatches, regions } from "./slots.ts";

const I = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];

/** An open tube about the y axis at `x`, every vertex on the bone `a`. */
function tube(x: number, r: number, y0: number, y1: number, segs = 16, rings = 4): Piece {
  const pos: number[] = [];
  const tris: number[] = [];
  for (let j = 0; j <= rings; j++)
    for (let k = 0; k < segs; k++) {
      const a = (2 * Math.PI * k) / segs;
      pos.push(x + r * Math.cos(a), y0 + ((y1 - y0) * j) / rings, r * Math.sin(a));
    }
  for (let j = 0; j < rings; j++)
    for (let k = 0; k < segs; k++) {
      const [a, b, c, d] = [j * segs + k, j * segs + ((k + 1) % segs), (j + 1) * segs + k, (j + 1) * segs + ((k + 1) % segs)];
      tris.push(a, b, d, a, d, c);
    }
  const count = pos.length / 3;
  return { file: "test", node: "tube", mesh: "tube", count, pos: Float64Array.from(pos), uvs: [], weights: Array.from({ length: count }, () => new Map([["a", 1]])), tris: Uint32Array.from(tris), bind: new Map([["a", I]]) };
}

// A body of a torso and an arm, four bands each, a band a patch (its vertices split at the
// borders, as a patched body's are): the torso's patches 0-3, the arm's 4-7.
const bands = (x: number, r: number) => [0, 1, 2, 3].map((j) => tube(x, r, 1.0 + 0.125 * j, 1.125 + 0.125 * j, 16, 1));
const torsoBands = bands(0, 0.15);
const armBands = bands(0.3, 0.04);
const torso = joined(torsoBands);
const armSkin = joined(armBands);
const body = joined([...torsoBands, ...armBands]);
const patch = Int32Array.from([...torsoBands, ...armBands].flatMap((b, p) => Array(b.count).fill(p)));

test("a flush sleeve hides the whole arm under it, and nothing of the torso", () => {
  // Their sleeve lies on the skin: the arms piece's surface is the arm's own, 0 mm off. A ray
  // from 2 mm out meets nothing, so our gear's rule hid none of it; the region hides it all.
  const by = regions(body, patch, [["arms", armSkin], ["body", torso]]);
  const arm = new Set([...Array(armSkin.count).keys()].map((i) => patch[torso.count + i]));
  const hid = hiddenPatches(by, ["arms"]);
  assert.deepEqual([...hid].sort(), [...arm].sort());
  for (let i = 0; i < torso.count; i++) assert.ok(!hid.has(patch[i]), `torso patch ${patch[i]} hidden by the arms`);
  assert.equal(hiddenPatches(by, []).size, 0);
});

test("a patch across two regions is refused", () => {
  const across = Int32Array.from(patch, (p) => (p === 4 ? 0 : p));
  assert.throws(() => regions(body, across, [["arms", armSkin], ["body", torso]]), /spans the/);
});

test("a face in no slot's piece is refused", () => {
  assert.throws(() => regions(body, patch, [["body", torso]]), /in no slot's piece/);
});
