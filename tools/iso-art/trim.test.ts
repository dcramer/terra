import assert from "node:assert/strict";
import { test } from "node:test";
import { type Glb, accessor, addTyped } from "./gltf.ts";
import { type Colour, isLeaf, trim } from "./trim.ts";

const BARK: [number, number, number] = [120, 90, 50];
const LEAF: [number, number, number] = [110, 140, 40];
const colour: Colour = (u) => (u < 0.5 ? BARK : LEAF);

/** Quads 0.3 m wide, within one tuft: bark at x 0 (u 0.1); a leaf quad at x 2.05 (u 0.6) whose two triangles share no
 * index, only points, as at a UV seam; a second leaf quad at x 5.25 (u 0.9). */
function tree(): Glb {
  const glb: Glb = { doc: { asset: { version: "2.0" }, nodes: [{ mesh: 0 }], meshes: [{ primitives: [{ attributes: {} }] }] }, bin: Buffer.alloc(0) };
  const pos: number[][] = [], uv: number[][] = [], idx: number[] = [];
  const quad = (x: number, u: number, seam: boolean) => {
    const corner = (dx: number, dy: number) => { pos.push([x + 0.3 * dx, 0.3 * dy, 0]); uv.push([u, 0.5]); return pos.length - 1; };
    const [a, b, c, d] = [corner(0, 0), corner(1, 0), corner(1, 1), corner(0, 1)];
    idx.push(a, b, c);
    if (seam) idx.push(corner(0, 0), corner(1, 1), corner(0, 1));
    else idx.push(a, c, d);
  };
  quad(0, 0.1, false);
  quad(2.05, 0.6, true);
  quad(5.25, 0.9, false);
  const prim = glb.doc.meshes[0].primitives[0];
  prim.attributes.POSITION = addTyped(glb, Float32Array.from(pos.flat()), "VEC3", { min: [0, 0, 0], max: [5.55, 0.3, 0] });
  prim.attributes.TEXCOORD_0 = addTyped(glb, Float32Array.from(uv.flat()), "VEC2");
  prim.indices = addTyped(glb, Uint16Array.from(idx), "SCALAR");
  return glb;
}

test("bark is not leaf; yellow-green and green leaves are", () => {
  assert.equal(isLeaf(BARK), false);
  assert.equal(isLeaf([128, 128, 128]), false, "a grey is no leaf");
  assert.equal(isLeaf(LEAF), true);
  assert.equal(isLeaf([60, 130, 50]), true);
});

test("trim drops the leaves, keeps the bark and every point, and a seam does not split a cluster", () => {
  const glb = tree();
  const points = glb.doc.accessors[glb.doc.meshes[0].primitives[0].attributes.POSITION].count;
  const s = trim(glb, colour, 0);
  assert.deepEqual([s.before, s.leaf, s.tufts, s.kept, s.after], [6, 4, 2, 0, 2]);
  const prim = glb.doc.meshes[0].primitives[0];
  const left = accessor(glb, prim.indices).flat();
  assert.deepEqual(left, [0, 1, 2, 0, 2, 3], "only the bark quad's triangles are left");
  assert.equal(glb.doc.accessors[prim.attributes.POSITION].count, points, "the points stay, so the skin and clips need no mapping");
  const all = trim(tree(), colour, 1);
  assert.deepEqual([all.after, all.kept], [6, 2], "keep 1 keeps every tuft");
});
