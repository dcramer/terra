// Mirrored clips: a clip made from another by reflecting it across the body's middle, its left
// and right bones swapped, as Unreal's Mirror Data Table and Godot's skeleton mirror do: each
// bone's pose in the model's space reflected, the left one's taken by the right, and each bone
// kept in its own rest frame (`C`, its rest reflected onto its partner's), then read back into
// its parent's. So a stop planted on the left foot gives the one planted on the right (D455,
// controller criterion 21d). Which clips: art/animation/mirrors.json, {"made": "from"}.
//
//   node tools/characters/mirror.ts FILE.glb...   (then markers.ts bakes the new clips)

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { Glb, type Pose, type Q, type V3 } from "./glb.ts";
import { compact } from "./glb_clips.ts";

const ROOT = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
export const MIRRORS = resolve(ROOT, "art/animation/mirrors.json");

/** An affine map: `m` row-major 3x3 (rotation and scale, or a reflection), then `t`. */
type Affine = { m: number[]; t: V3 };

const mat = (q: Q, s: V3): number[] => {
  const [x, y, z, w] = q;
  const r = [
    1 - 2 * (y * y + z * z), 2 * (x * y - z * w), 2 * (x * z + y * w),
    2 * (x * y + z * w), 1 - 2 * (x * x + z * z), 2 * (y * z - x * w),
    2 * (x * z - y * w), 2 * (y * z + x * w), 1 - 2 * (x * x + y * y),
  ];
  return r.map((v, i) => v * s[i % 3]);
};
const mm = (a: number[], b: number[]) =>
  [0, 1, 2].flatMap((r) => [0, 1, 2].map((c) => a[r * 3] * b[c] + a[r * 3 + 1] * b[3 + c] + a[r * 3 + 2] * b[6 + c]));
const mv = (a: number[], v: V3): V3 => [0, 1, 2].map((r) => a[r * 3] * v[0] + a[r * 3 + 1] * v[1] + a[r * 3 + 2] * v[2]) as V3;
const then = (a: Affine, b: Affine): Affine => {
  const t = mv(a.m, b.t);
  return { m: mm(a.m, b.m), t: [t[0] + a.t[0], t[1] + a.t[1], t[2] + a.t[2]] };
};
const inverse = ({ m, t }: Affine): Affine => {
  const [a, b, c, d, e, f, g, h, i] = m;
  const det = a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g);
  const n = [e * i - f * h, c * h - b * i, b * f - c * e, f * g - d * i, a * i - c * g, c * d - a * f, d * h - e * g, b * g - a * h, a * e - b * d].map((v) => v / det);
  const u = mv(n, t);
  return { m: n, t: [-u[0], -u[1], -u[2]] };
};
const affine = (p: Pose): Affine => ({ m: mat(p.r, p.s), t: [...p.t] as V3 });

/** A proper affine map back to translation, rotation and scale. */
export function decompose({ m, t }: Affine): Pose {
  const s: V3 = [0, 1, 2].map((c) => Math.hypot(m[c], m[3 + c], m[6 + c])) as V3;
  const r = m.map((v, i) => v / s[i % 3]);
  const tr = r[0] + r[4] + r[8];
  let q: Q;
  if (tr > 0) {
    const k = Math.sqrt(tr + 1) * 2;
    q = [(r[7] - r[5]) / k, (r[2] - r[6]) / k, (r[3] - r[1]) / k, k / 4];
  } else if (r[0] > r[4] && r[0] > r[8]) {
    const k = Math.sqrt(1 + r[0] - r[4] - r[8]) * 2;
    q = [k / 4, (r[1] + r[3]) / k, (r[2] + r[6]) / k, (r[7] - r[5]) / k];
  } else if (r[4] > r[8]) {
    const k = Math.sqrt(1 + r[4] - r[0] - r[8]) * 2;
    q = [(r[1] + r[3]) / k, k / 4, (r[5] + r[7]) / k, (r[2] - r[6]) / k];
  } else {
    const k = Math.sqrt(1 + r[8] - r[0] - r[4]) * 2;
    q = [(r[2] + r[6]) / k, (r[5] + r[7]) / k, k / 4, (r[3] - r[1]) / k];
  }
  return { t, r: q, s };
}

/** A bone's partner across the body's middle: `_l` for `_r` and back; itself if none. */
export const partner = (name: string, names: Set<string>): string => {
  const other = name.replace(/_l$/, "\0").replace(/_r$/, "_l").replace("\0", "_r");
  return names.has(other) ? other : name;
};

/** `from`'s tracks mirrored at time `t`: each animated node's local pose. */
function mirroredAt(body: Glb, clip: NonNullable<ReturnType<Glb["clip"]>>, t: number, axis: number, middle: number): Pose[] {
  const nodes = body.gltf.nodes;
  const names = new Set(nodes.map((n) => n.name ?? ""));
  const index = new Map(nodes.map((n, i) => [n.name ?? "", i]));
  // Reflected across the plane midway between the thighs at rest.
  const S: Affine = {
    m: [1, 0, 0, 0, 1, 0, 0, 0, 1].map((v, i) => (i === axis * 4 ? -v : v)),
    t: [0, 1, 2].map((k) => (k === axis ? 2 * middle : 0)) as V3,
  };
  const world = (pose: (i: number) => Pose): Affine[] => {
    const out: Affine[] = [];
    const at = (i: number): Affine => {
      if (out[i]) return out[i];
      const p = body.parentOf[i];
      out[i] = p === undefined ? affine(pose(i)) : then(at(p), affine(pose(i)));
      return out[i];
    };
    nodes.forEach((_, i) => at(i));
    return out;
  };
  const rest = world((i) => body.rest(i));
  const now = world((i) => {
    const track = clip.tracks.get(nodes[i].name ?? "");
    const r = body.rest(i);
    return { t: (track?.t?.(t) as V3) ?? r.t, r: (track?.r?.(t) as Q) ?? r.r, s: (track?.s?.(t) as V3) ?? r.s };
  });
  const mirrored = nodes.map((n, i) => {
    const j = index.get(partner(n.name ?? "", names)) ?? i;
    const keep = then(inverse(rest[j]), then(S, rest[i]));
    return then(S, then(now[j], keep));
  });
  return nodes.map((_, i) => {
    const p = body.parentOf[i];
    return decompose(p === undefined ? mirrored[i] : then(inverse(mirrored[p]), mirrored[i]));
  });
}

/** `file`'s bytes with each clip of `mirrors` ({made: from}) whose source it has made again
 * as that source mirrored, keyed linearly on the source's longest keys; undefined if none. */
export function mirrored(file: string, mirrors: Record<string, string>): Buffer | undefined {
  const body = new Glb(file);
  const gltf = body.gltf as Glb["gltf"] & { buffers: { byteLength: number }[]; bufferViews: { buffer: number; byteOffset?: number; byteLength: number }[]; animations: NonNullable<Glb["gltf"]["animations"]> };
  const made = Object.entries(mirrors).filter(([, from]) => body.clip(from));
  if (!made.length) return undefined;
  const thighs = ["thigh_l", "thigh_r"].map((n) => body.named(n));
  if (thighs.some((i) => i < 0)) throw new Error(`${file}: no thigh_l and thigh_r to mirror across`);
  const restOf = (i: number): V3 => {
    let a: Affine = affine(body.rest(i));
    for (let p = body.parentOf[i]; p !== undefined; p = body.parentOf[p]) a = then(affine(body.rest(p)), a);
    return a.t;
  };
  const [l, r] = thighs.map(restOf);
  const apart = [0, 1, 2].map((k) => Math.abs(l[k] - r[k]));
  const axis = apart.indexOf(Math.max(...apart));
  const middle = (l[axis] + r[axis]) / 2;
  const bytes = readFileSync(file);
  const jsonLen = bytes.readUInt32LE(12);
  const binStart = 20 + jsonLen + 8;
  const binLen = bytes.readUInt32LE(20 + jsonLen);
  const extra: Buffer[] = [];
  let offset = binLen;
  const push = (values: number[], type: string, count: number, minMax = false): number => {
    const buf = Buffer.alloc(values.length * 4);
    values.forEach((v, i) => buf.writeFloatLE(v, i * 4));
    gltf.bufferViews.push({ buffer: 0, byteOffset: offset, byteLength: buf.length });
    extra.push(buf);
    offset += buf.length;
    const acc: Glb["gltf"]["accessors"][number] & { min?: number[] } = { bufferView: gltf.bufferViews.length - 1, componentType: 5126, count, type };
    if (minMax) Object.assign(acc, { min: [Math.min(...values)], max: [Math.max(...values)] });
    gltf.accessors.push(acc);
    return gltf.accessors.length - 1;
  };
  for (const [name, from] of made) {
    gltf.animations = gltf.animations.filter((a) => a.name !== name);
    const source = gltf.animations.find((a) => a.name === from)!;
    const clip = body.clip(from)!;
    const keys = source.samplers.map((s) => s.input).reduce((a, b) => (gltf.accessors[b].count > gltf.accessors[a].count ? b : a));
    const times = Array.from(body.read(keys));
    const poses = times.map((t) => mirroredAt(body, clip, t, axis, middle));
    const input = push(times, "SCALAR", times.length, true);
    const anim = { name, channels: [] as { sampler: number; target: { node: number; path: string } }[], samplers: [] as { input: number; output: number; interpolation: string }[] };
    for (const ch of source.channels) {
      const key = ({ translation: "t", rotation: "r", scale: "s" } as Record<string, keyof Pose>)[ch.target.path];
      if (!key) continue;
      let prev: Q | undefined;
      const values = poses.flatMap((p) => {
        const v = [...p[ch.target.node][key]];
        // Each rotation on the same side as the last, so keys blend the short way.
        if (key === "r") {
          if (prev && v.reduce((s, x, k) => s + x * prev![k], 0) < 0) v.forEach((x, k) => (v[k] = -x));
          prev = v as Q;
        }
        return v;
      });
      anim.samplers.push({ input, output: push(values, key === "r" ? "VEC4" : "VEC3", times.length), interpolation: "LINEAR" });
      anim.channels.push({ sampler: anim.samplers.length - 1, target: { node: ch.target.node, path: ch.target.path } });
    }
    gltf.animations.push(anim);
  }
  gltf.buffers[0].byteLength = offset;
  let json = Buffer.from(JSON.stringify(gltf));
  if (json.length % 4) json = Buffer.concat([json, Buffer.alloc(4 - (json.length % 4), 0x20)]);
  const bin = Buffer.concat([bytes.subarray(binStart, binStart + binLen), ...extra]);
  const head = Buffer.alloc(12);
  head.write("glTF", 0);
  head.writeUInt32LE(2, 4);
  head.writeUInt32LE(12 + 8 + json.length + 8 + bin.length, 8);
  const chunk = (len: number, kind: number) => {
    const c = Buffer.alloc(8);
    c.writeUInt32LE(len, 0);
    c.writeUInt32LE(kind, 4);
    return c;
  };
  // The clip it replaced dropped from the buffer, so mirroring again gives the same bytes (D563).
  return compact(Buffer.concat([head, chunk(json.length, 0x4e4f534a), json, chunk(bin.length, 0x004e4942), bin]));
}

/** Mirrors `file`'s listed clips in place; how many it made. */
export function mirrorFile(file: string): number {
  const mirrors: Record<string, string> = JSON.parse(readFileSync(MIRRORS, "utf8"));
  const out = mirrored(file, mirrors);
  if (!out) return 0;
  writeFileSync(file, out);
  return Object.values(mirrors).filter((from) => new Glb(file).clip(from)).length;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const files = process.argv.slice(2).filter((a) => !a.startsWith("--"));
  if (!files.length) {
    console.log("mirror.ts FILE.glb...   (the clips of art/animation/mirrors.json; then markers.ts)");
    process.exit(1);
  }
  for (const file of files) console.log(`mirror: ${file}, ${mirrorFile(file)} clips mirrored`);
}
