// Reading a binary glTF (.glb) and posing its skeleton through a clip, for the character
// tools that measure clips without Blender or the game (phases.ts, moves.ts): accessors,
// the nodes' rest and animated transforms, and linear blend skinning.
//
// Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";

export type V3 = [number, number, number];
export type Q = [number, number, number, number];
export type Pose = { t: V3; r: Q; s: V3 };

export const qmul = (a: Q, b: Q): Q => [
  a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
  a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
  a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
  a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
];
export const qinv = (q: Q): Q => [-q[0], -q[1], -q[2], q[3]];
export const rot = (q: Q, v: V3): V3 => {
  const p = qmul(qmul(q, [v[0], v[1], v[2], 0]), qinv(q));
  return [p[0], p[1], p[2]];
};
/// A child's pose under its parent's.
export const apply = (parent: Pose, local: Pose): Pose => {
  const scaled: V3 = [local.t[0] * parent.s[0], local.t[1] * parent.s[1], local.t[2] * parent.s[2]];
  const moved = rot(parent.r, scaled);
  return {
    t: [parent.t[0] + moved[0], parent.t[1] + moved[1], parent.t[2] + moved[2]],
    r: qmul(parent.r, local.r),
    s: [parent.s[0] * local.s[0], parent.s[1] * local.s[1], parent.s[2] * local.s[2]],
  };
};
/// A point in a pose's frame, in its parent's.
export const place = (pose: Pose, p: V3): V3 => apply(pose, { t: p, r: [0, 0, 0, 1], s: [1, 1, 1] }).t;
export const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
export const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
export const len = (a: V3) => Math.hypot(a[0], a[1], a[2]);
/// A rotation of `degrees` about x, then y, then z, as glam's `from_rotation_x * y * z`.
export const turn = ([x, y, z]: V3): Q => {
  const about = (axis: number, deg: number): Q => {
    const h = (deg * Math.PI) / 360;
    const q: Q = [0, 0, 0, Math.cos(h)];
    q[axis] = Math.sin(h);
    return q;
  };
  return qmul(qmul(about(0, x), about(1, y)), about(2, z));
};

type Node = { name?: string; children?: number[]; translation?: V3; rotation?: Q; scale?: V3; mesh?: number; skin?: number };

export class Glb {
  readonly gltf: {
    nodes: Node[];
    meshes?: { primitives: { attributes: Record<string, number>; targets?: Record<string, number>[] }[]; extras?: { targetNames?: string[] } }[];
    skins?: { joints: number[]; inverseBindMatrices: number }[];
    animations?: { name: string; channels: { sampler: number; target: { node: number; path: string } }[]; samplers: { input: number; output: number }[] }[];
    accessors: { bufferView: number; byteOffset?: number; componentType: number; count: number; type: string; normalized?: boolean; max?: number[] }[];
    bufferViews: { byteOffset?: number; byteStride?: number }[];
  };
  readonly parentOf: number[] = [];
  private readonly bytes: Buffer;
  private readonly bin: number;

  readonly file: string;

  constructor(file: string) {
    this.file = file;
    this.bytes = readFileSync(file);
    const jsonLen = this.bytes.readUInt32LE(12);
    this.gltf = JSON.parse(this.bytes.subarray(20, 20 + jsonLen).toString());
    this.bin = 20 + jsonLen + 8;
    this.gltf.nodes.forEach((n, i) => n.children?.forEach((c) => (this.parentOf[c] = i)));
  }

  /// An accessor's values, flat, as numbers: floats as they are, normalized integers scaled
  /// to 0..1, other integers as they are.
  read(index: number): Float64Array {
    const acc = this.gltf.accessors[index];
    const view = this.gltf.bufferViews[acc.bufferView];
    const width = ({ SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 } as Record<string, number>)[acc.type];
    const size = ({ 5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4 } as Record<number, number>)[acc.componentType];
    const stride = view.byteStride ?? width * size;
    const start = this.bin + (view.byteOffset ?? 0) + (acc.byteOffset ?? 0);
    const out = new Float64Array(acc.count * width);
    const b = this.bytes;
    const one = (at: number): number => {
      switch (acc.componentType) {
        case 5126: return b.readFloatLE(at);
        case 5121: return acc.normalized ? b.readUInt8(at) / 255 : b.readUInt8(at);
        case 5123: return acc.normalized ? b.readUInt16LE(at) / 65535 : b.readUInt16LE(at);
        case 5125: return b.readUInt32LE(at);
        case 5120: return b.readInt8(at);
        default: return b.readInt16LE(at);
      }
    };
    for (let i = 0; i < acc.count; i++) for (let j = 0; j < width; j++) out[i * width + j] = one(start + i * stride + j * size);
    return out;
  }

  named(name: string): number {
    return this.gltf.nodes.findIndex((n) => n.name === name);
  }

  rest(node: number): Pose {
    const n = this.gltf.nodes[node];
    return { t: n.translation ?? [0, 0, 0], r: n.rotation ?? [0, 0, 0, 1], s: n.scale ?? [1, 1, 1] };
  }

  clipNames(): string[] {
    return (this.gltf.animations ?? []).map((a) => a.name);
  }

  /// A clip's tracks by node name, so a clip in one file can pose a body in another with the
  /// same skeleton; and its length in seconds.
  clip(name: string): Clip | undefined {
    const anim = this.gltf.animations?.find((a) => a.name === name);
    if (!anim) return undefined;
    const tracks = new Map<string, Partial<Record<keyof Pose, (t: number) => number[]>>>();
    let end = 0;
    for (const ch of anim.channels) {
      const path = ({ translation: "t", rotation: "r", scale: "s" } as Record<string, keyof Pose>)[ch.target.path];
      const nodeName = this.gltf.nodes[ch.target.node].name;
      if (!path || !nodeName) continue;
      const s = anim.samplers[ch.sampler];
      if (!tracks.has(nodeName)) tracks.set(nodeName, {});
      tracks.get(nodeName)![path] = sampler(this.read(s.input), this.read(s.output), path === "r" ? 4 : 3);
      end = Math.max(end, this.gltf.accessors[s.input].max?.[0] ?? 0);
    }
    return { name, end, tracks };
  }

  /// `under` with the bones of `roots` and every bone below them held at `over`'s first pose,
  /// in their own frames: the hold layer laid over a locomotion clip by a bone mask, as the
  /// game lays it at run time (terra_walker::figure, D458). Unreal's Layered Blend per Bone at
  /// full weight; a held bone `over` has no track for is at rest, as `posed` reads any.
  layered(under: Clip, over: Clip, roots: readonly string[] = HOLD_MASK): Clip {
    const held = new Set<string>();
    const below = (i: number) => {
      const name = this.gltf.nodes[i].name;
      if (name) held.add(name);
      this.gltf.nodes[i].children?.forEach(below);
    };
    this.gltf.nodes.forEach((n, i) => n.name && roots.includes(n.name) && below(i));
    const tracks = new Map(under.tracks);
    for (const name of held) {
      const track = over.tracks.get(name);
      if (!track) {
        tracks.delete(name);
        continue;
      }
      const first = Object.fromEntries(Object.entries(track).map(([k, f]) => [k, () => f(0)]));
      tracks.set(name, first);
    }
    return { name: `${under.name}+${over.name}`, end: under.end, tracks };
  }
}

/// The bones a stance's hold lays its pose on, with every bone below them: both arms from the
/// clavicles (D458; the twin of terra_walker::figure::HOLD_MASK).
export const HOLD_MASK: readonly string[] = ["clavicle_l", "clavicle_r"];

export type Clip = { name: string; end: number; tracks: Map<string, Partial<Record<keyof Pose, (t: number) => number[]>>> };

function sampler(times: Float64Array, values: Float64Array, width: number) {
  return (t: number): number[] => {
    let i = 0;
    while (i < times.length - 2 && times[i + 1] < t) i++;
    const a = Array.from(values.subarray(i * width, i * width + width));
    if (times.length < 2) return a;
    const span = times[i + 1] - times[i] || 1;
    const k = Math.min(1, Math.max(0, (t - times[i]) / span));
    const b = Array.from(values.subarray((i + 1) * width, (i + 1) * width + width));
    if (width === 4) {
      const sign = a.reduce((sum, x, j) => sum + x * b[j], 0) < 0 ? -1 : 1;
      const mixed = a.map((x, j) => x + (b[j] * sign - x) * k);
      const l = Math.hypot(...mixed);
      return mixed.map((x) => x / l);
    }
    return a.map((x, j) => x + (b[j] - x) * k);
  };
}

/// Every node of `body` posed by `clip` at `t`, in the model's space; `bends` turns named
/// bones about the model's axes at their joints over the clip, as the game's staged bends do
/// (`terra_creatures::staged::pose::bent`).
export function posed(body: Glb, clip: Clip | undefined, t: number, bends: Map<string, Q> = new Map()): Pose[] {
  const out: Pose[] = [];
  const at = (node: number): Pose => {
    if (out[node]) return out[node];
    const n = body.gltf.nodes[node];
    const track = n.name ? clip?.tracks.get(n.name) : undefined;
    const rest = body.rest(node);
    const local: Pose = {
      t: (track?.t?.(t) as V3) ?? rest.t,
      r: (track?.r?.(t) as Q) ?? rest.r,
      s: (track?.s?.(t) as V3) ?? rest.s,
    };
    const parent = body.parentOf[node] === undefined ? undefined : at(body.parentOf[node]);
    const bend = n.name ? bends.get(n.name) : undefined;
    if (bend && parent) local.r = qmul(qmul(qmul(qinv(parent.r), bend), parent.r), local.r);
    out[node] = parent ? apply(parent, local) : local;
    return out[node];
  };
  body.gltf.nodes.forEach((_, i) => at(i));
  return out;
}

/// 4x4 column-major matrix of a pose.
export function matrix(p: Pose): number[] {
  const [x, y, z, w] = p.r;
  const [sx, sy, sz] = p.s;
  return [
    (1 - 2 * (y * y + z * z)) * sx, 2 * (x * y + z * w) * sx, 2 * (x * z - y * w) * sx, 0,
    2 * (x * y - z * w) * sy, (1 - 2 * (x * x + z * z)) * sy, 2 * (y * z + x * w) * sy, 0,
    2 * (x * z + y * w) * sz, 2 * (y * z - x * w) * sz, (1 - 2 * (x * x + y * y)) * sz, 0,
    p.t[0], p.t[1], p.t[2], 1,
  ];
}

export function mmul(a: number[], b: number[] | Float64Array, bAt = 0): number[] {
  const o = new Array(16).fill(0);
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) for (let k = 0; k < 4; k++) o[c * 4 + r] += a[k * 4 + r] * b[bAt + c * 4 + k];
  return o;
}
