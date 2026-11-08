// Trimming a creature's leaves (iso-art, `trim`): a tree creature's crown seen from the ARPG
// camera's 17 m hides its trunk and limbs, so it reads as a bush (iso-forest-art j1: the forest's
// tree-monster's top 30% of height is 54 to 78% leaf, treant-b's 46 to 64%). The triangles whose
// base colour is leaf (a green to yellow-green hue, saturated) are dropped from the index list,
// all but a share of whole leaf clusters (`keep`), so the crown thins to a few tufts on bare
// limbs, as a game artist cuts a tree's card count for a top-down LOD. Only the indices change:
// the points, skin, skeleton and clips stay byte for byte, so nothing needs mapping (decimate's
// splice does). The texture is read by ffmpeg (JPEG or PNG to raw RGB), the one decoder at hand.

import { spawnSync } from "node:child_process";
import { type Glb, accessor, addTyped, compact, points, pose } from "./gltf.ts";

const ELEMENT_ARRAY_BUFFER = 34963;

/** The base colour at a UV: [r, g, b] in 0..255. */
export type Colour = (u: number, v: number) => [number, number, number];

/** Leaf: hue 60 to 160 degrees (yellow-green to green), saturation over 0.3. Tripo's bark on the
 * forest's three tree creatures sits at 30 to 60 degrees; their leaves at 60 to 90. */
export function isLeaf([r, g, b]: [number, number, number]): boolean {
  const mx = Math.max(r, g, b), mn = Math.min(r, g, b);
  if (mx === 0 || (mx - mn) / mx <= 0.3) return false;
  const d = mx - mn;
  const h = 60 * (mx === r ? ((g - b) / d + 6) % 6 : mx === g ? (b - r) / d + 2 : (r - g) / d + 4);
  return h > 60 && h < 160;
}

/** The GLB's base-colour texture as a lookup, decoded at `size` square by ffmpeg. */
export function baseColour(glb: Glb, size = 512): Colour {
  const { doc, bin } = glb;
  const tex = doc.materials?.[0]?.pbrMetallicRoughness?.baseColorTexture;
  if (!tex) throw new Error("trim reads leaves by the base-colour texture, and this GLB's first material has none: trim a textured Tripo export");
  const bv = doc.bufferViews[doc.images[doc.textures[tex.index].source].bufferView];
  const bytes = bin.subarray(bv.byteOffset ?? 0, (bv.byteOffset ?? 0) + bv.byteLength);
  const r = spawnSync("ffmpeg", ["-loglevel", "error", "-i", "pipe:0", "-vf", `scale=${size}:${size}`, "-f", "rawvideo", "-pix_fmt", "rgb24", "pipe:1"], { input: bytes, maxBuffer: size * size * 3 + 1024 });
  if (r.status !== 0 || r.stdout.length !== size * size * 3) throw new Error(`ffmpeg could not decode the base-colour texture (${String(r.stderr).trim() || `exit ${r.status}`}): install ffmpeg, which loop.ts and the films use too`);
  const px = r.stdout;
  const at = (t: number) => Math.min(size - 1, Math.max(0, Math.floor((t - Math.floor(t)) * size)));
  return (u, v) => {
    const o = (at(v) * size + at(u)) * 3;
    return [px[o], px[o + 1], px[o + 2]];
  };
}

/** A tuft's side in metres: about a leaf spray's, 47 px at the closest camera (117 px a metre). */
const TUFT = 0.4;

/** A string's hash in 0..1 (FNV-1a, then murmur's finaliser): a tuft keeps or goes on every run. */
function hash(s: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 0x01000193) >>> 0;
  h = Math.imul(h ^ (h >>> 16), 0x85ebca6b) >>> 0;
  h = Math.imul(h ^ (h >>> 13), 0xc2b2ae35) >>> 0;
  return ((h ^ (h >>> 16)) >>> 0) / 4294967296;
}

export type Trim = { before: number; after: number; leaf: number; tufts: number; kept: number };

/** Drops the leaf triangles of the GLB's first mesh but a `keep` share of its tufts: a cluster
 * (leaf triangles joined by a shared point, 1 mm apart or nearer, so a UV seam does not split
 * one) cut by a TUFT grid. */
export function trim(glb: Glb, colour: Colour, keep = 0): Trim {
  const prim = glb.doc.meshes[0].primitives[0];
  const uv = accessor(glb, prim.attributes.TEXCOORD_0);
  const idx = accessor(glb, prim.indices).flat();
  const at = points(glb, pose(glb.doc));
  const n = idx.length / 3;
  const leaf: boolean[] = [];
  for (let t = 0; t < n; t++) {
    const [a, b, c] = [idx[3 * t], idx[3 * t + 1], idx[3 * t + 2]];
    leaf.push(isLeaf(colour((uv[a][0] + uv[b][0] + uv[c][0]) / 3, (uv[a][1] + uv[b][1] + uv[c][1]) / 3)));
  }
  // Clusters: union-find over the points of leaf triangles, a point keyed by its millimetre.
  const key = (i: number) => at[i].map((x) => Math.round(x * 1000)).join(",");
  const id = new Map<string, number>();
  const parent: number[] = [];
  const find = (x: number): number => (parent[x] === x ? x : (parent[x] = find(parent[x])));
  const node = (i: number) => {
    const k = key(i);
    let v = id.get(k);
    if (v === undefined) id.set(k, (v = parent.push(parent.length) - 1));
    return v;
  };
  for (let t = 0; t < n; t++) {
    if (!leaf[t]) continue;
    const [a, b, c] = [0, 1, 2].map((j) => find(node(idx[3 * t + j])));
    parent[b] = a;
    parent[find(c)] = a;
  }
  // A tuft: a cluster's part in one TUFT cell of the rest pose, so a crown Tripo fused into one
  // cluster (the tree-monster's: 12k leaf triangles, most in one) thins in patches, not all or none.
  const tufts = new Set<string>();
  const kept = new Set<string>();
  const out: number[] = [];
  for (let t = 0; t < n; t++) {
    if (leaf[t]) {
      const tri = [0, 1, 2].map((j) => at[idx[3 * t + j]]);
      const cell = [0, 1, 2].map((k) => Math.floor((tri[0][k] + tri[1][k] + tri[2][k]) / 3 / TUFT));
      const tuft = `${find(node(idx[3 * t]))}:${cell.join(",")}`;
      tufts.add(tuft);
      if (hash(tuft) >= keep) continue;
      kept.add(tuft);
    }
    out.push(idx[3 * t], idx[3 * t + 1], idx[3 * t + 2]);
  }
  const count = leaf.filter(Boolean).length;
  prim.indices = addTyped(glb, at.length < 65536 ? Uint16Array.from(out) : Uint32Array.from(out), "SCALAR", { target: ELEMENT_ARRAY_BUFFER });
  compact(glb);
  return { before: n, after: out.length / 3, leaf: count, tufts: tufts.size, kept: kept.size };
}
