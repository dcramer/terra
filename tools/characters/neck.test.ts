import { test } from "node:test";
import assert from "node:assert/strict";
import { fileURLToPath } from "node:url";
import { Glb } from "./glb.ts";
import type { Piece } from "./intact.ts";
import { MANNEQUIN, openEdge } from "./layers.ts";
import { passes, seam, skinOf } from "./neck.ts";

const UBC = fileURLToPath(new URL("../../art/characters/quaternius/universal-base-characters/", import.meta.url));
const HEAD = `${UBC}Regular_Male_OnlyHead.gltf`;
const head = skinOf(HEAD);
const full = skinOf(`${UBC}Regular_Male_FullBody.gltf`);
const skel = new Glb(HEAD);
const clips = new Glb(MANNEQUIN);
const key = (p: Float64Array, i: number) => `${Math.round(p[3 * i] * 1e4)},${Math.round(p[3 * i + 1] * 1e4)},${Math.round(p[3 * i + 2] * 1e4)}`;
const tri = (p: Piece, k: number) => [0, 1, 2].map((c) => key(p.pos, p.tris[k + c])).sort().join("|");

/** `p` with only the triangles `keep` names, its vertices the ones they use. */
function cut(p: Piece, keep: (k: number) => boolean): Piece {
  const tris: number[] = [];
  for (let k = 0; k < p.tris.length; k += 3) if (keep(k)) tris.push(p.tris[k], p.tris[k + 1], p.tris[k + 2]);
  const used = [...new Set(tris)].sort((a, b) => a - b);
  const to = new Map(used.map((v, n) => [v, n]));
  return {
    ...p,
    count: used.length,
    pos: Float64Array.from(used.flatMap((v) => [p.pos[3 * v], p.pos[3 * v + 1], p.pos[3 * v + 2]])),
    nrm: undefined,
    uvs: [],
    weights: used.map((v) => p.weights[v]),
    tris: Uint32Array.from(tris.map((v) => to.get(v)!)),
  };
}

// Their full body cut where their head is (as cut_slots.py cuts): the torso and all below it,
// whose neck edge is the head's ring, twin for twin.
const headTris = new Set(Array.from({ length: head.tris.length / 3 }, (_, k) => tri(head, 3 * k)));
const exact = cut(full, (k) => !headTris.has(tri(full, k)));
const ring = new Set(openEdge(head).map((r) => key(head.pos, r)));

test("their full body as shipped, uncut, and an exact cut at their head's ring both pass: no gap, no overlap, through the clips", () => {
  for (const torso of [full, exact]) {
    const s = seam(head, torso, skel, clips, 2);
    assert.ok(passes(s), `gap ${s.gap} at ${s.gapAt}, overlap ${s.overlap} at ${s.overlapAt}`);
    assert.equal(s.ring, 32);
  }
});

test("a torso whose neck ring is moved 4 mm fails on its gap", () => {
  for (const dy of [-0.004, 0.004]) {
    const moved = { ...exact, pos: Float64Array.from(exact.pos) };
    let n = 0;
    for (let i = 0; i < moved.count; i++) if (ring.has(key(exact.pos, i))) (moved.pos[3 * i + 1] += dy, n++);
    assert.ok(n >= 32, `${n} torso vertices on the head's ring`);
    const s = seam(head, moved, skel, clips, 2);
    assert.equal(passes(s), false);
    assert.ok(Math.abs(s.gap - 0.004) < 0.0015, `gap ${s.gap}`);
  }
});

test("a torso that runs on under the head (a band of the head's faces kept on it) fails on its overlap", () => {
  const band = new Set<string>();
  for (let k = 0; k < head.tris.length; k += 3) if ([0, 1, 2].some((c) => ring.has(key(head.pos, head.tris[k + c])))) band.add(tri(head, k));
  const over = cut(full, (k) => !headTris.has(tri(full, k)) || band.has(tri(full, k)));
  const s = seam(head, over, skel, clips, 2);
  assert.equal(s.gap, 0);
  assert.equal(passes(s), false);
  assert.ok(s.overlap > 0.002, `overlap ${s.overlap}`);
});

test("today's bare torso, cut by face height under their UpperBody, stops short of the head's ring", () => {
  const torso = skinOf(fileURLToPath(new URL("../../assets/models/character/male/male_bare_torso.gltf", import.meta.url)));
  const s = seam(head, torso, skel, undefined);
  assert.ok(s.gap > 0.1, `gap ${s.gap}`);
});
