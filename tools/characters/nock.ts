#!/usr/bin/env node
// A bow's nock measured on its look (docs/attachment.md "Second grip", the `nock` extra): the
// middle of its string at rest, the String material's vertices halfway along its length, in the
// look's own space, metres. fit.ts checks a bow's data against it; a person never reads it off a
// render.
//
//   node tools/characters/nock.ts assets/models/held/bow.glb    # [x, y, z]
//
// Node 22.18 or later; no dependencies.

import { Glb, place, posed, type V3 } from "./glb.ts";

/** How far either side of the string's middle its vertices are taken, m. */
const SLAB_M = 0.01;

/** Every vertex of a material whose name starts with `material`, in the look's space. */
export function materialPoints(file: string, material: string): V3[] {
  const g = new Glb(file);
  const gl = g.gltf as unknown as { nodes: { mesh?: number }[]; meshes: { primitives: { attributes: Record<string, number>; material?: number }[] }[]; materials?: { name?: string }[] };
  const world = posed(g, undefined, 0);
  const out: V3[] = [];
  gl.nodes.forEach((n, i) => {
    if (n.mesh === undefined) return;
    for (const p of gl.meshes[n.mesh].primitives) {
      const name = p.material === undefined ? "" : (gl.materials?.[p.material]?.name ?? "");
      if (!name.toLowerCase().startsWith(material.toLowerCase())) continue;
      const v = g.read(p.attributes.POSITION);
      for (let k = 0; k < v.length; k += 3) out.push(place(world[i], [v[k], v[k + 1], v[k + 2]]));
    }
  });
  return out;
}

/** The middle of a string: its vertices within SLAB_M of halfway along its length (Y), averaged. */
export function stringMiddle(points: V3[]): V3 | undefined {
  if (!points.length) return undefined;
  const ys = points.map((p) => p[1]);
  const mid = (Math.min(...ys) + Math.max(...ys)) / 2;
  const near = points.filter((p) => Math.abs(p[1] - mid) <= SLAB_M);
  const at = near.length ? near : [points.reduce((a, b) => (Math.abs(b[1] - mid) < Math.abs(a[1] - mid) ? b : a))];
  const m = [0, 1, 2].map((j) => at.reduce((s, p) => s + p[j], 0) / at.length);
  return m.map((v) => Math.round(v * 1000) / 1000) as V3;
}

/** A bow's nock on its look: its String's middle, or undefined if it has no String. */
export const nockOf = (file: string) => stringMiddle(materialPoints(file, "String"));

if (import.meta.main) {
  const file = process.argv[2];
  if (!file) {
    console.error("usage: node tools/characters/nock.ts LOOK.glb");
    process.exit(2);
  }
  const at = nockOf(file);
  if (!at) {
    console.log(`✗ ${file}: no String material to measure a nock on`);
    process.exit(1);
  }
  console.log(`✓ nock [${at.join(", ")}]`);
}
