// Carrying Uthana's motion onto our body, without the network (D533): the body as it is
// uploaded (`bare`), the take given the body's skin (`withSkin`) and its feet put on our ground
// (`lifted`, by `lift`), and the gate every take passes (`against`): it must stand as the same
// motion does on Uthana's own reference character, or it is refused.
//
// Node 22.18 or later; no dependencies.

import { apply, Glb, posed, qinv, rot, type Clip, type Pose, type Q, type V3 } from "../characters/glb.ts";

export class Refused extends Error {}

type Node = { name?: string; children?: number[]; translation?: V3; rotation?: Q; scale?: V3 };
type Gltf = {
  nodes: Node[];
  animations?: { channels: { sampler: number; target: { node: number; path: string } }[]; samplers: { output: number }[] }[];
  skins?: { joints: number[]; inverseBindMatrices?: number; skeleton?: number }[];
  accessors: { bufferView?: number; byteOffset?: number; count: number }[];
  bufferViews: { buffer: number; byteOffset?: number; byteLength: number; byteStride?: number }[];
  buffers: { byteLength: number }[];
};

function chunks(glb: Buffer): { json: Gltf; bin: Buffer } {
  const jsonLen = glb.readUInt32LE(12);
  const binLen = glb.readUInt32LE(20 + jsonLen);
  return { json: JSON.parse(glb.subarray(20, 20 + jsonLen).toString()), bin: glb.subarray(28 + jsonLen, 28 + jsonLen + binLen) };
}

/** A GLB from its JSON and binary chunk, each padded to four bytes (the JSON with spaces). */
export function glbOf(json: object, bin: Buffer): Buffer {
  const text = Buffer.from(JSON.stringify(json));
  const j = Buffer.concat([text, Buffer.alloc((4 - (text.length % 4)) % 4, 0x20)]);
  const b = Buffer.concat([bin, Buffer.alloc((4 - (bin.length % 4)) % 4)]);
  const head = Buffer.alloc(12);
  head.writeUInt32LE(0x46546c67, 0);
  head.writeUInt32LE(2, 4);
  head.writeUInt32LE(12 + 8 + j.length + 8 + b.length, 8);
  const chunk = (data: Buffer, type: number) => {
    const h = Buffer.alloc(8);
    h.writeUInt32LE(data.length, 0);
    h.writeUInt32LE(type, 4);
    return Buffer.concat([h, data]);
  };
  return Buffer.concat([head, chunk(j, 0x4e4f534a), chunk(b, 0x004e4942)]);
}

/** The body as Uthana is given it: without its animations. Uploaded with base.glb's 28 clips,
 * Uthana retargeted onto a skeleton it read wrongly (2026-10-26: the trial idle's pelvis 14.5
 * times too low and its trunk 30 degrees over); the same body without them takes the motion
 * as Uthana's own Manny does, whatever the Rig's scale or the root's rest turn. */
export function bare(body: Buffer): Buffer {
  const { json, bin } = chunks(body);
  delete json.animations;
  return glbOf(json, bin);
}

/** A downloaded clip with `body`'s skin added: Uthana's GLB is our skeleton's nodes and the
 * motion, with no skin, so an importer reads it as loose nodes (Blender as empties); with the
 * body's joints (matched by name) and inverse bind matrices it reads as the body's skeleton,
 * as a Kimodo take's BVH does. A clip that has a skin is left as it is. */
export function withSkin(clip: Buffer, body: Buffer): Buffer {
  const t = chunks(clip);
  const s = chunks(body);
  if (t.json.skins?.length) return clip;
  const skin = s.json.skins?.[0];
  if (!skin || skin.inverseBindMatrices === undefined) throw new Refused("the body has no skin to give the clip");
  const at = new Map(t.json.nodes.map((n, i) => [n.name, i]));
  const node = (i: number) => {
    const name = s.json.nodes[i].name;
    const found = at.get(name);
    if (found === undefined) throw new Refused(`the clip has no node ${name}: it is not on this body's skeleton`);
    return found;
  };
  const acc = s.json.accessors[skin.inverseBindMatrices];
  const view = s.json.bufferViews[acc.bufferView!];
  const from = (view.byteOffset ?? 0) + (acc.byteOffset ?? 0);
  const matrices = s.bin.subarray(from, from + acc.count * 64);
  const offset = t.bin.length + ((4 - (t.bin.length % 4)) % 4);
  const bin = Buffer.concat([t.bin, Buffer.alloc(offset - t.bin.length), matrices]);
  t.json.bufferViews.push({ buffer: 0, byteOffset: offset, byteLength: matrices.length });
  t.json.accessors.push({ bufferView: t.json.bufferViews.length - 1, componentType: 5126, count: acc.count, type: "MAT4" } as Gltf["accessors"][number]);
  t.json.skins = [{ joints: skin.joints.map(node), inverseBindMatrices: t.json.accessors.length - 1, ...(skin.skeleton !== undefined ? { skeleton: node(skin.skeleton) } : {}) }];
  t.json.buffers[0].byteLength = bin.length;
  return glbOf(t.json, bin);
}

const BALLS = ["ball_l", "ball_r"];

/** How high a body's forefeet stand at rest, the lower of its balls (m). */
export function ballHeight(body: Glb): number {
  const rest = posed(body, undefined, 0);
  return Math.min(...BALLS.map((b) => rest[body.named(b)].t[1]));
}

/** How far a take on `body` must be raised for its feet to stand on our ground: Uthana keeps the
 * forefeet at the height its reference character's stand at (2026-10-26: our balls at 1.1 cm in
 * the trial idle and Manny's at 1.0, where ours stand at 3.9 at rest and Manny's at 0.8), so
 * our soles sank 2.8 cm. The difference of the two rests, each read from its own file. */
export function lift(body: Glb, reference: Glb): number {
  return ballHeight(body) - ballHeight(reference);
}

/** A clip with its root raised `by` metres in the world, its translation keys moved in the
 * root's parent's frame (the Rig's, scaled). */
export function lifted(clip: Buffer, by: number): Buffer {
  const { json, bin } = chunks(clip);
  const root = json.nodes.findIndex((n) => n.name === "root");
  const parentOf: number[] = [];
  json.nodes.forEach((n, i) => n.children?.forEach((c) => (parentOf[c] = i)));
  // The root's parent's world rest: its chain of rests from the top.
  const chain: number[] = [];
  for (let i = parentOf[root]; i !== undefined; i = parentOf[i]) chain.unshift(i);
  const frame = chain.reduce<Pose>((p, i) => apply(p, { t: json.nodes[i].translation ?? [0, 0, 0], r: json.nodes[i].rotation ?? [0, 0, 0, 1], s: json.nodes[i].scale ?? [1, 1, 1] }), { t: [0, 0, 0], r: [0, 0, 0, 1], s: [1, 1, 1] });
  // [0, by, 0] in the world, in the parent's frame: turned back and unscaled.
  const up = rot(qinv(frame.r), [0, by, 0]).map((v, i) => v / frame.s[i]);
  const channel = json.animations?.[0]?.channels.find((c) => c.target.node === root && c.target.path === "translation");
  if (!channel) throw new Refused("the take keys no translation on its root: it is not Uthana's");
  const out = Buffer.from(bin);
  const acc = json.accessors[json.animations![0].samplers[channel.sampler].output];
  const view = json.bufferViews[acc.bufferView!];
  const start = (view.byteOffset ?? 0) + (acc.byteOffset ?? 0);
  for (let i = 0; i < acc.count; i++) {
    for (let j = 0; j < 3; j++) {
      const k = start + i * (view.byteStride ?? 12) + j * 4;
      out.writeFloatLE(out.readFloatLE(k) + up[j], k);
    }
  }
  return glbOf(json, out);
}

/** What a take may differ from the same motion on the reference character, at worst: the
 * pelvis's height as a share of its rest (in our metres) while the reference stands on a foot,
 * and at any time (`hips`, looser: off the ground Uthana scales the hips' path by its own
 * measure, 2026-10-26: our pelvis 0.925 times Manny's height plus 6.7 cm over the three trial
 * clips, within 0.9 cm, so a swim's is 6 cm off its share); the trunk's direction (degrees);
 * the lower forefoot's height over its rest while the reference's is on the ground (m). */
export const BOUNDS = { pelvis: 0.03, hips: 0.1, lean: 5, feet: 0.02 };

export type Stand = { pelvis: number; hips: number; lean: number; feet: number };

const angle = (a: V3, b: V3) => (Math.acos(Math.max(-1, Math.min(1, (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]) / Math.hypot(...a) / Math.hypot(...b)))) * 180) / Math.PI;

/** How far a take on `body` stands from the same motion on Uthana's reference character, at
 * worst over the take: the measure of a carry, whatever the motion is (a swim's trunk lies
 * flat and its feet never touch the ground, on both). Every bone's world direction is kept by
 * a right carry; a wrong one shows first in the hips (too low), the trunk (leaning) and the
 * feet (in or over the ground). */
export function against(body: Glb, take: Clip, reference: Glb, refTake: Clip): Stand {
  const one = (g: Glb, clip: Clip | undefined, t: number) => {
    const p = posed(g, clip, t);
    const at = (n: string) => p[g.named(n)].t;
    const pelvis = at("pelvis");
    const neck = at("neck_01");
    return { hips: pelvis[1], trunk: [neck[0] - pelvis[0], neck[1] - pelvis[1], neck[2] - pelvis[2]] as V3, ball: Math.min(...BALLS.map((b) => at(b)[1])) };
  };
  const ours = one(body, undefined, 0);
  const theirs = one(reference, undefined, 0);
  const askew = angle(ours.trunk, theirs.trunk);
  const worst: Stand = { pelvis: 0, hips: 0, lean: 0, feet: 0 };
  const big = (k: keyof Stand, v: number) => {
    if (Math.abs(v) > Math.abs(worst[k])) worst[k] = v;
  };
  const end = Math.min(take.end, refTake.end);
  for (let t = 0; t <= end + 1e-6; t += 1 / 30) {
    const a = one(body, take, t);
    const b = one(reference, refTake, t);
    const hips = (a.hips / ours.hips - b.hips / theirs.hips) * ours.hips;
    big("hips", hips);
    big("lean", Math.max(0, angle(a.trunk, b.trunk) - askew));
    // The reference on a foot: ours stands at its height, on its own feet.
    if (b.ball - theirs.ball < BOUNDS.feet) {
      big("pelvis", hips);
      big("feet", a.ball - ours.ball - (b.ball - theirs.ball));
    }
  }
  return worst;
}

/** The ways a take's stand is out of `BOUNDS`, each with its numbers; none for a take that stands. */
export function faults(s: Stand): string[] {
  const out: string[] = [];
  const cm = (m: number) => `${(m * 100).toFixed(1)} cm`;
  if (Math.abs(s.pelvis) > BOUNDS.pelvis) out.push(`standing, the pelvis ${cm(s.pelvis)} off the reference's height (bound ${cm(BOUNDS.pelvis)})`);
  if (Math.abs(s.hips) > BOUNDS.hips) out.push(`the pelvis ${cm(s.hips)} off the reference's height (bound ${cm(BOUNDS.hips)})`);
  if (s.lean > BOUNDS.lean) out.push(`the trunk ${s.lean.toFixed(1)} deg off the reference's (bound ${BOUNDS.lean})`);
  if (Math.abs(s.feet) > BOUNDS.feet) out.push(`the feet ${cm(s.feet)} ${s.feet < 0 ? "into" : "over"} the ground where the reference's stand (bound ${cm(BOUNDS.feet)})`);
  return out;
}
