// Cutting an asset to its triangle budget (iso-art, `decimate`): Blender's collapse decimation
// (tools/blender/decimate.py) makes the cut mesh, and only that mesh is spliced into the asset's
// own GLB, so its skeleton, inverse binds, clips, materials, textures and root node stay exactly
// as `export` wrote them. A Blender glTF round trip of a whole Tripo rig risks the bone frames a
// held prop is measured in (the knight's hand_r), the clip names and the root node; the mesh
// alone risks only its joint indices, remapped here by joint name, and its space: Blender writes a
// skinned mesh's points in the world at rest, so they are mapped through both files' binds.

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { type Glb, type Mat4, accessor, addTyped, apply, bounds, compact, invert, multiply, pose, skinMatrices } from "./gltf.ts";

const ARRAY_BUFFER = 34962, ELEMENT_ARRAY_BUFFER = 34963;

/** The mesh to cut: the one with the most triangles (Blender's importer adds bone shapes). */
function mainMesh(doc: any): number {
  const tris = (m: any) => m.primitives.reduce((n: number, p: any) => n + (p.indices !== undefined ? doc.accessors[p.indices].count : doc.accessors[p.attributes.POSITION].count) / 3, 0);
  return doc.meshes.map(tris).reduce((best: number, t: number, i: number, all: number[]) => (t > all[best] ? i : best), 0);
}

/** The joint names of the skin the node showing mesh `mesh` uses, or none if it is not skinned. */
function jointNames(doc: any, mesh: number): string[] | undefined {
  const node = doc.nodes.find((n: any) => n.mesh === mesh);
  if (node?.skin === undefined) return undefined;
  return doc.skins[node.skin].joints.map((j: number) => doc.nodes[j].name);
}

/** What the file does to its main mesh's points at rest: a skinned mesh's world(joint) *
 * inverseBind, the same for every joint when the rest pose is the bind pose (as Tripo's rig and
 * Blender's exporter write it, each in its own space: Tripo's carries export's scale, Blender's
 * is the identity); an unskinned mesh's node's world matrix. */
function bindOf(glb: Glb, mesh: number, label: string): Mat4 {
  const { doc } = glb;
  const ni = doc.nodes.findIndex((n: any) => n.mesh === mesh);
  const world = pose(doc);
  if (doc.nodes[ni].skin === undefined) return world[ni];
  const ms = skinMatrices(glb, doc.skins[doc.nodes[ni].skin], world);
  const size = Math.max(...ms[0].slice(0, 12).map(Math.abs));
  const off = Math.max(...ms.map((m) => Math.max(...m.map((v, k) => Math.abs(v - ms[0][k])))));
  if (off > 1e-3 * Math.max(1, size)) throw new Error(`the ${label}'s rest pose is not its bind pose (a joint's world * inverse bind differs from the first's by ${off.toFixed(4)}): its points are in no one space to map; cut the GLB \`export\` wrote, its skeleton at rest, and export the cut with no pose applied (tools/blender/decimate.py)`);
  return ms[0];
}

export type Splice = { before: number; after: number; drift: number };

/** Puts `cut`'s main mesh in place of `orig`'s, attribute by attribute: one primitive each, the
 * same attributes, joints by name, points mapped from the cut's bind space to the original's
 * (Blender writes a skinned mesh's points in the world at rest, Tripo's are export's scale
 * under it). `drift` is how far the mapped cut's bounds moved, a share of the original's
 * diagonal; over 2% the cut is of another model or pose, and it is refused. */
export function splice(orig: Glb, cut: Glb): Splice {
  const [om, cm] = [mainMesh(orig.doc), mainMesh(cut.doc)];
  const [op, cp] = [orig.doc.meshes[om].primitives, cut.doc.meshes[cm].primitives];
  if (op.length !== 1 || cp.length !== 1) throw new Error(`splice takes one primitive a mesh (the original has ${op.length}, the cut ${cp.length}): a mesh of several materials needs each primitive matched by material`);
  const [o, c] = [op[0], cp[0]];
  const missing = Object.keys(o.attributes).filter((k) => !(k in c.attributes));
  if (missing.length) throw new Error(`the cut mesh lacks ${missing.join(", ")}: export it from Blender with them (tools/blender/decimate.py)`);
  const before = orig.doc.accessors[o.indices].count / 3;
  const [names, cutNames] = [jointNames(orig.doc, om), jointNames(cut.doc, cm)];
  if (!!names !== !!cutNames) throw new Error(`the original is ${names ? "" : "not "}skinned and the cut ${cutNames ? "is" : "is not"}: cut the GLB \`export\` wrote, with its armature`);
  const remap = cutNames?.map((n) => {
    const i = names!.indexOf(n);
    if (i < 0) throw new Error(`the cut's joint ${n} is not in the original's skin: cut the GLB \`export\` wrote, not another`);
    return i;
  });
  // The cut's points into the original's space through the world both rest in: p_o = B_o^-1 B_c p_c
  // (j25 spliced them unmapped, 76-159% off); normals by the inverse transpose, tangents as points.
  const map = multiply(invert(bindOf(orig, om, "original")), bindOf(cut, cm, "cut"));
  const inv = invert(map);
  const unit = (v: number[]) => { const l = Math.hypot(...v) || 1; return v.map((x) => x / l); };
  const toNormal = (n: number[]) => unit([0, 1, 2].map((r) => inv[r * 4] * n[0] + inv[r * 4 + 1] * n[1] + inv[r * 4 + 2] * n[2]));
  const toVector = (t: number[]) => [...unit([0, 1, 2].map((r) => map[r] * t[0] + map[4 + r] * t[1] + map[8 + r] * t[2])), ...t.slice(3)];
  const read = (k: string) => {
    const rows = accessor(cut, c.attributes[k]);
    return k === "POSITION" ? rows.map((p) => apply(map, p)) : k === "NORMAL" ? rows.map(toNormal) : k === "TANGENT" ? rows.map(toVector) : rows;
  };
  const pos = read("POSITION");
  const [ob, nb] = [bounds(accessor(orig, o.attributes.POSITION) as any), bounds(pos as any)];
  const diag = Math.hypot(...ob.max.map((v, i) => v - ob.min[i]));
  const drift = Math.max(...[0, 1, 2].flatMap((i) => [Math.abs(nb.min[i] - ob.min[i]), Math.abs(nb.max[i] - ob.max[i])])) / diag;
  if (drift > 0.02) throw new Error(`the cut mesh's bounds, mapped into the original's bind space, are ${(drift * 100).toFixed(1)}% of its size from the original's: it is a cut of another model or export (or Blender's axes not undone); cut the GLB \`export\` wrote last (tools/blender/decimate.py) and splice that`);
  const attributes: Record<string, number> = {};
  for (const k of Object.keys(o.attributes)) {
    const rows = k === "POSITION" ? pos : read(k);
    const type = (["SCALAR", "VEC2", "VEC3", "VEC4"] as const)[rows[0].length - 1];
    if (k.startsWith("JOINTS_")) {
      const flat = rows.flat().map((j) => (remap ? remap[j] : j));
      attributes[k] = addTyped(orig, Math.max(...flat) < 256 ? Uint8Array.from(flat) : Uint16Array.from(flat), type, { target: ARRAY_BUFFER });
    } else {
      attributes[k] = addTyped(orig, Float32Array.from(rows.flat()), type, k === "POSITION" ? { min: nb.min, max: nb.max, target: ARRAY_BUFFER } : { target: ARRAY_BUFFER });
    }
  }
  const idx = accessor(cut, c.indices).flat();
  o.attributes = attributes;
  o.indices = addTyped(orig, pos.length < 65536 ? Uint16Array.from(idx) : Uint32Array.from(idx), "SCALAR", { target: ELEMENT_ARRAY_BUFFER });
  compact(orig);
  return { before, after: idx.length / 3, drift };
}

/** Runs tools/blender/decimate.py on `src`, writing the cut mesh to `out`; throws what to do if
 * this machine has no Blender of the pinned version. */
export function blenderCut(root: string, src: string, out: string, budget: number): void {
  const run = join(root, "tools", "blender", "run.sh");
  if (spawnSync(run, ["--find"]).status !== 0) {
    throw new Error(`no Blender ${readFileSync(join(root, "tools", "blender", "version"), "utf8").trim()} here (tools/blender/run.sh --find): run \`tools/blender/run.sh tools/blender/decimate.py ${src} OUT.glb ${budget}\` where there is one (the render worker, in a \`cargo xtask remote-render\` job), then \`decimate NAME OUT.glb\` here`);
  }
  const r = spawnSync(run, [join(root, "tools", "blender", "decimate.py"), src, out, String(budget)], { encoding: "utf8" });
  if (r.status !== 0) throw new Error(`tools/blender/decimate.py exited ${r.status}: ${(r.stdout + r.stderr).trim().split("\n").slice(-3).join(" / ")}`);
}
