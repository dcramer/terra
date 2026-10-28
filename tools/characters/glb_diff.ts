#!/usr/bin/env node
// Where two binary glTF files differ, read at a glance (D71): the JSON's first differing
// paths, then each accessor whose values differ (what it is, how many of its values, by how
// much), and each image whose bytes differ. For finding why a build step's output is not the
// same bytes twice (D563), and what a rebuild moved.
//
//   node tools/characters/glb_diff.ts a.glb b.glb [--all]
//
// Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";
import { Glb } from "./glb.ts";

type Json = unknown;

/** The paths at which two JSON values differ, at most `limit`. */
export function jsonPaths(a: Json, b: Json, limit = 20, at = "$", out: string[] = []): string[] {
  if (out.length >= limit) return out;
  if (typeof a !== typeof b || Array.isArray(a) !== Array.isArray(b) || a === null || b === null || typeof a !== "object") {
    if (a !== b) out.push(`${at}: ${short(a)} -> ${short(b)}`);
    return out;
  }
  if (Array.isArray(a)) {
    const bb = b as Json[];
    if (a.length !== bb.length) out.push(`${at}: length ${a.length} -> ${bb.length}`);
    for (let i = 0; i < Math.min(a.length, bb.length); i++) jsonPaths(a[i], bb[i], limit, `${at}[${i}]`, out);
    return out;
  }
  const ao = a as Record<string, Json>;
  const bo = b as Record<string, Json>;
  for (const k of [...new Set([...Object.keys(ao), ...Object.keys(bo)])]) jsonPaths(ao[k], bo[k], limit, `${at}.${k}`, out);
  return out;
}

const short = (v: Json) => {
  const s = JSON.stringify(v) ?? "undefined";
  return s.length > 60 ? `${s.slice(0, 57)}...` : s;
};

type Gltf = {
  meshes?: { name?: string; primitives: { attributes: Record<string, number>; indices?: number; targets?: Record<string, number>[] }[] }[];
  skins?: { name?: string; inverseBindMatrices?: number }[];
  animations?: { name: string; samplers: { input: number; output: number }[] }[];
  accessors: unknown[];
  images?: { name?: string; bufferView?: number }[];
  bufferViews: { byteOffset?: number; byteLength: number }[];
};

/** What each accessor is, by the mesh, skin or clip that reads it. */
function uses(g: Gltf): Map<number, string> {
  const m = new Map<number, string>();
  const put = (i: number | undefined, what: string) => i !== undefined && !m.has(i) && m.set(i, what);
  g.meshes?.forEach((mesh, mi) =>
    mesh.primitives.forEach((p, pi) => {
      const name = `mesh ${mesh.name ?? mi}#${pi}`;
      put(p.indices, `${name} indices`);
      for (const [k, v] of Object.entries(p.attributes)) put(v, `${name} ${k}`);
      p.targets?.forEach((t, ti) => Object.entries(t).forEach(([k, v]) => put(v, `${name} target ${ti} ${k}`)));
    }),
  );
  g.skins?.forEach((s, si) => put(s.inverseBindMatrices, `skin ${s.name ?? si} inverseBindMatrices`));
  g.animations?.forEach((a) => a.samplers.forEach((s, si) => (put(s.input, `clip ${a.name} sampler ${si} times`), put(s.output, `clip ${a.name} sampler ${si} values`))));
  return m;
}

/** The two files' differences as lines; empty when their bytes are the same. */
export function glbDiff(fa: string, fb: string, all = false): string[] {
  const [ba, bb] = [readFileSync(fa), readFileSync(fb)];
  if (ba.equals(bb)) return [];
  const [a, b] = [new Glb(fa), new Glb(fb)];
  const [ga, gb] = [a.gltf as unknown as Gltf, b.gltf as unknown as Gltf];
  const lines: string[] = [];
  const paths = jsonPaths(stripViews(ga), stripViews(gb), all ? 1000 : 20);
  if (paths.length) lines.push(`JSON differs (buffer offsets and lengths aside) at ${paths.length}${paths.length >= 20 && !all ? "+" : ""} paths:`, ...paths.map((p) => `  ${p}`));
  const what = uses(ga);
  let differ = 0;
  const shown: string[] = [];
  for (let i = 0; i < Math.min(ga.accessors.length, gb.accessors.length); i++) {
    const [va, vb] = [a.read(i), b.read(i)];
    if (va.length !== vb.length) {
      differ++;
      shown.push(`  ${what.get(i) ?? `accessor ${i}`}: ${va.length} values -> ${vb.length}`);
      continue;
    }
    let n = 0;
    let max = 0;
    for (let j = 0; j < va.length; j++)
      if (va[j] !== vb[j]) {
        n++;
        max = Math.max(max, Math.abs(va[j] - vb[j]));
      }
    if (n) {
      differ++;
      shown.push(`  ${what.get(i) ?? `accessor ${i}`}: ${n} of ${va.length} values differ, by at most ${max.toPrecision(3)}`);
    }
  }
  if (differ) lines.push(`${differ} of ${ga.accessors.length} accessors differ:`, ...(all ? shown : shown.slice(0, 15)), ...(!all && shown.length > 15 ? [`  ... ${shown.length - 15} more (--all)`] : []));
  const bin = (buf: Buffer) => 20 + buf.readUInt32LE(12) + 8;
  ga.images?.forEach((img, i) => {
    const [va, vb] = [ga.bufferViews[img.bufferView ?? -1], gb.bufferViews[gb.images?.[i]?.bufferView ?? -1]];
    if (!va || !vb) return;
    const [ia, ib] = [ba.subarray(bin(ba) + (va.byteOffset ?? 0), bin(ba) + (va.byteOffset ?? 0) + va.byteLength), bb.subarray(bin(bb) + (vb.byteOffset ?? 0), bin(bb) + (vb.byteOffset ?? 0) + vb.byteLength)];
    if (!ia.equals(ib)) lines.push(`image ${img.name ?? i}: ${ia.length} bytes -> ${ib.length}, differ`);
  });
  if (!lines.length) lines.push("the same JSON and values; the bytes differ in layout (padding or buffer order)");
  return lines;
}

/** The JSON without where each view sits in the buffer, which moves whenever any data before it grows. */
function stripViews(g: Gltf): Json {
  return { ...g, bufferViews: g.bufferViews.map(({ byteOffset: _o, ...v }) => v), buffers: undefined };
}

if (import.meta.main) {
  const args = process.argv.slice(2);
  const files = args.filter((a) => !a.startsWith("--"));
  if (files.length !== 2) {
    console.log("usage: node tools/characters/glb_diff.ts a.glb b.glb [--all]");
    process.exit(2);
  }
  const lines = glbDiff(files[0], files[1], args.includes("--all"));
  console.log(lines.length ? `✗ ${files[0]} and ${files[1]} differ\n${lines.join("\n")}` : `✓ the same bytes`);
  process.exit(lines.length ? 1 : 0);
}
