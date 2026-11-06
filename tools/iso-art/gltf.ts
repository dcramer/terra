// Reading a GLB as far as the checks need, with no 3D library: its accessors, its images'
// sizes, its node hierarchy posed by a clip at a time, and the skinned mesh's points in that
// pose. Writing one back after a change to its JSON alone (the root node `export` adds).
// glTF 2.0's own rules (https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html): matrices are
// column-major, a skinned mesh's points are sum(w * world(joint) * inverseBind * p) and its own
// node's transform is ignored, a quaternion is [x, y, z, w].

import { gltfOf } from "../tripo/glb.ts";

export type Vec3 = [number, number, number];
export type Quat = [number, number, number, number];
export type Mat4 = number[]; // 16, column-major

export type Glb = { doc: any; bin: Buffer };

const JSON_CHUNK = 0x4e4f534a;
const BIN_CHUNK = 0x004e4942;

export function readGlb(bytes: Buffer): Glb {
  const doc = gltfOf(bytes);
  if (!doc) throw new Error("not a GLB (no glTF header); give the file `export` wrote");
  const jsonLength = bytes.readUInt32LE(12);
  let at = 20 + jsonLength;
  let bin = Buffer.alloc(0);
  while (at + 8 <= bytes.length) {
    const length = bytes.readUInt32LE(at);
    if (bytes.readUInt32LE(at + 4) === BIN_CHUNK) bin = bytes.subarray(at + 8, at + 8 + length);
    at += 8 + length;
  }
  if ((doc.extensionsRequired ?? []).length) {
    throw new Error(`the GLB needs ${doc.extensionsRequired.join(", ")} (Studio's compressed download); take a plain one from \`tripo.ts export --format glb\``);
  }
  return { doc, bin };
}

export function writeGlb({ doc, bin }: Glb): Buffer {
  const pad = (b: Buffer, fill: number) => Buffer.concat([b, Buffer.alloc((4 - (b.length % 4)) % 4, fill)]);
  const json = pad(Buffer.from(JSON.stringify(doc)), 0x20);
  const body = pad(bin, 0);
  const header = Buffer.alloc(12);
  header.write("glTF", 0, "latin1");
  header.writeUInt32LE(2, 4);
  header.writeUInt32LE(12 + 8 + json.length + (body.length ? 8 + body.length : 0), 8);
  const chunk = (type: number, data: Buffer) => {
    const h = Buffer.alloc(8);
    h.writeUInt32LE(data.length, 0);
    h.writeUInt32LE(type, 4);
    return Buffer.concat([h, data]);
  };
  return Buffer.concat([header, chunk(JSON_CHUNK, json), ...(body.length ? [chunk(BIN_CHUNK, body)] : [])]);
}

const WIDTH: Record<string, number> = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 };
const COMPONENT: Record<number, [number, (b: Buffer, o: number) => number, number]> = {
  5120: [1, (b, o) => b.readInt8(o), 127],
  5121: [1, (b, o) => b.readUInt8(o), 255],
  5122: [2, (b, o) => b.readInt16LE(o), 32767],
  5123: [2, (b, o) => b.readUInt16LE(o), 65535],
  5125: [4, (b, o) => b.readUInt32LE(o), 1],
  5126: [4, (b, o) => b.readFloatLE(o), 1],
};

/** An accessor's elements, each an array of its components. */
export function accessor({ doc, bin }: Glb, index: number): number[][] {
  const a = doc.accessors[index];
  if (a.sparse) throw new Error(`accessor ${index} is sparse, which this reader does not take`);
  const width = WIDTH[a.type];
  const [size, read, max] = COMPONENT[a.componentType];
  const out: number[][] = [];
  if (a.bufferView === undefined) {
    for (let i = 0; i < a.count; i++) out.push(new Array(width).fill(0));
    return out;
  }
  const view = doc.bufferViews[a.bufferView];
  const stride = view.byteStride ?? size * width;
  const base = (view.byteOffset ?? 0) + (a.byteOffset ?? 0);
  for (let i = 0; i < a.count; i++) {
    const e: number[] = [];
    for (let c = 0; c < width; c++) {
      const v = read(bin, base + i * stride + c * size);
      e.push(a.normalized ? Math.max(v / max, -1) : v);
    }
    out.push(e);
  }
  return out;
}

/** The pixel size of each image, read from its PNG or JPEG header. */
export function imageSizes(glb: Glb): { name: string; width: number; height: number }[] {
  return (glb.doc.images ?? []).map((img: any, i: number) => {
    const name = img.name ?? img.uri ?? `image ${i}`;
    if (img.bufferView === undefined) return { name, width: 0, height: 0 };
    const view = glb.doc.bufferViews[img.bufferView];
    const b = glb.bin.subarray(view.byteOffset ?? 0, (view.byteOffset ?? 0) + view.byteLength);
    return { name, ...pixelSize(b) };
  });
}

export function pixelSize(b: Buffer): { width: number; height: number } {
  if (b.length > 24 && b.readUInt32BE(0) === 0x89504e47) return { width: b.readUInt32BE(16), height: b.readUInt32BE(20) };
  if (b.length > 4 && b[0] === 0xff && b[1] === 0xd8) {
    let at = 2;
    while (at + 9 < b.length) {
      if (b[at] !== 0xff) {
        at++;
        continue;
      }
      const marker = b[at + 1];
      const length = b.readUInt16BE(at + 2);
      // Start of frame: baseline, extended, progressive or lossless.
      if (marker >= 0xc0 && marker <= 0xc3) return { width: b.readUInt16BE(at + 7), height: b.readUInt16BE(at + 5) };
      at += 2 + length;
    }
  }
  return { width: 0, height: 0 };
}

// ---- transforms ----

export function compose(t: Vec3, r: Quat, s: Vec3): Mat4 {
  const [x, y, z, w] = r;
  const m = [
    1 - 2 * (y * y + z * z), 2 * (x * y + z * w), 2 * (x * z - y * w), 0,
    2 * (x * y - z * w), 1 - 2 * (x * x + z * z), 2 * (y * z + x * w), 0,
    2 * (x * z + y * w), 2 * (y * z - x * w), 1 - 2 * (x * x + y * y), 0,
    t[0], t[1], t[2], 1,
  ];
  for (let c = 0; c < 3; c++) for (let r2 = 0; r2 < 3; r2++) m[c * 4 + r2] *= s[c];
  return m;
}

export function multiply(a: Mat4, b: Mat4): Mat4 {
  const out = new Array(16).fill(0);
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) for (let k = 0; k < 4; k++) out[c * 4 + r] += a[k * 4 + r] * b[c * 4 + k];
  return out;
}

export function apply(m: Mat4, p: number[]): Vec3 {
  return [
    m[0] * p[0] + m[4] * p[1] + m[8] * p[2] + m[12],
    m[1] * p[0] + m[5] * p[1] + m[9] * p[2] + m[13],
    m[2] * p[0] + m[6] * p[1] + m[10] * p[2] + m[14],
  ];
}

function slerp(a: number[], b: number[], t: number): number[] {
  let dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];
  const sign = dot < 0 ? -1 : 1;
  dot *= sign;
  const out = a.map((v, i) => v * (1 - t) + sign * b[i] * t); // nlerp: close enough between keys
  const len = Math.hypot(...out) || 1;
  return out.map((v) => v / len);
}

/** A clip read once: each channel's node, path, times and values. */
export type Clip = { name: string; duration: number; channels: { node: number; path: string; times: number[]; values: number[][]; step: boolean }[] };

export function clips(glb: Glb): Clip[] {
  return (glb.doc.animations ?? []).map((anim: any, i: number) => {
    const channels = (anim.channels ?? [])
      .filter((ch: any) => ch.target?.node !== undefined && ch.target.path !== "weights")
      .map((ch: any) => {
        const s = anim.samplers[ch.sampler];
        const times = accessor(glb, s.input).map((e) => e[0]);
        let values = accessor(glb, s.output);
        // A cubic spline keeps an in-tangent, the value and an out-tangent per key: take the value.
        if (s.interpolation === "CUBICSPLINE") values = values.filter((_, k) => k % 3 === 1);
        return { node: ch.target.node, path: ch.target.path, times, values, step: s.interpolation === "STEP" };
      });
    const duration = Math.max(0, ...channels.map((c: any) => c.times.at(-1) ?? 0));
    return { name: anim.name ?? `animation ${i}`, duration, channels };
  });
}

function sample(ch: Clip["channels"][number], t: number): number[] {
  const { times, values } = ch;
  if (t <= times[0]) return values[0];
  if (t >= times.at(-1)!) return values.at(-1)!;
  let k = 0;
  while (times[k + 1] < t) k++;
  if (ch.step) return values[k];
  const f = (t - times[k]) / (times[k + 1] - times[k] || 1);
  if (ch.path === "rotation") return slerp(values[k], values[k + 1], f);
  return values[k].map((v, i) => v * (1 - f) + values[k + 1][i] * f);
}

/** Every node's world matrix, at time `t` of a clip (or at rest without one). */
export function pose(doc: any, clip?: Clip, t = 0): Mat4[] {
  const nodes: any[] = doc.nodes ?? [];
  const trs = nodes.map((n) => ({ t: (n.translation ?? [0, 0, 0]) as number[], r: (n.rotation ?? [0, 0, 0, 1]) as number[], s: (n.scale ?? [1, 1, 1]) as number[] }));
  for (const ch of clip?.channels ?? []) {
    const v = sample(ch, t);
    if (ch.path === "translation") trs[ch.node].t = v;
    else if (ch.path === "rotation") trs[ch.node].r = v;
    else if (ch.path === "scale") trs[ch.node].s = v;
  }
  const local = nodes.map((n, i) => (n.matrix && !clip?.channels.some((c) => c.node === i) ? n.matrix : compose(trs[i].t as Vec3, trs[i].r as Quat, trs[i].s as Vec3)));
  const parent = new Array(nodes.length).fill(-1);
  nodes.forEach((n, i) => (n.children ?? []).forEach((c: number) => (parent[c] = i)));
  const world: Mat4[] = new Array(nodes.length);
  const at = (i: number): Mat4 => (world[i] ??= parent[i] < 0 ? local[i] : multiply(at(parent[i]), local[i]));
  nodes.forEach((_, i) => at(i));
  return world;
}

/** Where a node is in the world. */
export const origin = (m: Mat4): Vec3 => [m[12], m[13], m[14]];

/** The meshes' points in a pose, skinned where a mesh has a skin. `every` keeps one point in
 * that many, for speed where a sample of the shape is enough. */
export function points(glb: Glb, world: Mat4[], every = 1): Vec3[] {
  const out: Vec3[] = [];
  const { doc } = glb;
  for (const [ni, node] of (doc.nodes ?? []).entries()) {
    if (node.mesh === undefined) continue;
    const skin = node.skin !== undefined ? doc.skins[node.skin] : undefined;
    const joints = skin ? skinMatrices(glb, skin, world) : [];
    for (const prim of doc.meshes[node.mesh].primitives ?? []) {
      const pos = cached(glb, prim.attributes.POSITION);
      const js = skin ? cached(glb, prim.attributes.JOINTS_0) : [];
      const ws = skin ? cached(glb, prim.attributes.WEIGHTS_0) : [];
      for (let v = 0; v < pos.length; v += every) {
        if (!skin) {
          out.push(apply(world[ni], pos[v]));
          continue;
        }
        const p: Vec3 = [0, 0, 0];
        for (let k = 0; k < 4; k++) {
          const w = ws[v][k];
          if (!w) continue;
          const q = apply(joints[js[v][k]], pos[v]);
          p[0] += w * q[0];
          p[1] += w * q[1];
          p[2] += w * q[2];
        }
        out.push(p);
      }
    }
  }
  return out;
}

const cache = new WeakMap<Glb, Map<number, number[][]>>();
function cached(glb: Glb, index: number): number[][] {
  let m = cache.get(glb);
  if (!m) cache.set(glb, (m = new Map()));
  if (!m.has(index)) m.set(index, accessor(glb, index));
  return m.get(index)!;
}

function skinMatrices(glb: Glb, skin: any, world: Mat4[]): Mat4[] {
  const ibm = skin.inverseBindMatrices !== undefined ? cached(glb, skin.inverseBindMatrices) : [];
  const identity = compose([0, 0, 0], [0, 0, 0, 1], [1, 1, 1]);
  return skin.joints.map((j: number, i: number) => multiply(world[j], ibm[i] ?? identity));
}

export type Box = { min: Vec3; max: Vec3 };
export function bounds(ps: Vec3[]): Box {
  const min: Vec3 = [Infinity, Infinity, Infinity];
  const max: Vec3 = [-Infinity, -Infinity, -Infinity];
  for (const p of ps) for (let i = 0; i < 3; i++) {
    min[i] = Math.min(min[i], p[i]);
    max[i] = Math.max(max[i], p[i]);
  }
  return { min, max };
}
