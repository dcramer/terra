#!/usr/bin/env node
// Puts named clips from one binary glTF into another and changes nothing else (D563): the
// target's nodes, meshes, skins and other clips keep their bytes, so remaking a clip moves no
// float of the body. made_clips.py makes its clips in Blender and writes them in with this,
// rather than exporting the mannequin through Blender again, whose glTF round trip moves the
// rest pose a little each time (leaf bones' rolls, float drift).
//
//   node tools/characters/glb_clips.ts TARGET.glb SOURCE.glb CLIP...
//
// A clip given as `NEW=A+B` is the source's A and then its B joined into one clip named NEW
// (`joinClips`: a blow and its recovery played as one move).
//
// Each clip's channels go to the target's node of the same name. Where the source's rest
// frame for a node differs from the target's (a bone Blender rolled), the keys are carried into
// the target's frame, so the clip poses the body as it did in the source. A clip the target has
// is replaced where it stands in its list; a new one is added at the end. Data no longer read is
// dropped, so the file does not grow, and the same clips written again give the same bytes.
//
// Node 22.18 or later; no dependencies.

import { readFileSync, writeFileSync } from "node:fs";
import { apply, Glb, type Pose, type Q, qinv, qmul, rot, type V3 } from "./glb.ts";

type Accessor = { bufferView?: number; byteOffset?: number; componentType: number; count: number; type: string; min?: number[]; max?: number[]; normalized?: boolean; sparse?: unknown };
type View = { buffer: number; byteOffset?: number; byteLength: number; byteStride?: number; target?: number };
type Anim = { name: string; channels: { sampler: number; target: { node: number; path: string } }[]; samplers: { input: number; output: number; interpolation?: string }[] };
type Gltf = {
  nodes: { name?: string; children?: number[] }[];
  meshes?: { primitives: { attributes: Record<string, number>; indices?: number; targets?: Record<string, number>[] }[] }[];
  skins?: { inverseBindMatrices?: number }[];
  animations?: Anim[];
  accessors: Accessor[];
  bufferViews: View[];
  buffers: { byteLength: number }[];
  images?: { bufferView?: number }[];
};

const WIDTH: Record<string, number> = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 };

function chunks(bytes: Buffer): { json: Gltf; bin: Buffer } {
  const jsonLen = bytes.readUInt32LE(12);
  const json = JSON.parse(bytes.subarray(20, 20 + jsonLen).toString());
  const binLen = bytes.readUInt32LE(20 + jsonLen);
  return { json, bin: bytes.subarray(28 + jsonLen, 28 + jsonLen + binLen) };
}

/** A glb from its JSON and binary chunk, padded as the format asks. */
export function glbBytes(json: unknown, bin: Buffer): Buffer {
  let j = Buffer.from(JSON.stringify(json));
  if (j.length % 4) j = Buffer.concat([j, Buffer.alloc(4 - (j.length % 4), 0x20)]);
  const b = bin.length % 4 ? Buffer.concat([bin, Buffer.alloc(4 - (bin.length % 4))]) : bin;
  const head = Buffer.alloc(12);
  head.write("glTF", 0);
  head.writeUInt32LE(2, 4);
  head.writeUInt32LE(12 + 8 + j.length + 8 + b.length, 8);
  const chunk = (len: number, kind: number) => {
    const c = Buffer.alloc(8);
    c.writeUInt32LE(len, 0);
    c.writeUInt32LE(kind, 4);
    return c;
  };
  return Buffer.concat([head, chunk(j.length, 0x4e4f534a), j, chunk(b.length, 0x004e4942), b]);
}

/** Every node's rest in the model's space. */
function worldRest(g: Glb): Pose[] {
  const out: Pose[] = [];
  const at = (i: number): Pose => {
    if (out[i]) return out[i];
    const p = g.parentOf[i];
    out[i] = p === undefined ? g.rest(i) : apply(at(p), g.rest(i));
    return out[i];
  };
  g.gltf.nodes.forEach((_, i) => at(i));
  return out;
}

/** No turn but float32's rounding (a millionth of a radian): the keys are copied, not recomputed. */
const near = (q: Q) => Math.hypot(q[0], q[1], q[2]) < 1e-6 * Math.hypot(...q);

/** How the source's rest stood from the target's: bones whose frames are turned ("name degrees"), and the most a joint is apart (m). */
export type Report = { turned: string[]; apart: number };

/** `target` with `names` taken from `source`; throws when a clip or a node it moves is missing. */
export function putClips(target: Buffer, source: Buffer, names: string[], report: Report = { turned: [], apart: 0 }): Buffer {
  const t = chunks(target);
  const s = chunks(source);
  const g = t.json;
  if (g.buffers.length !== 1 || g.accessors.some((a) => a.sparse)) throw new Error("glb_clips: one buffer and no sparse accessors only");
  const tg = new Glb(target);
  const sg = new Glb(source);
  const tw = worldRest(tg);
  const sw = worldRest(sg);
  const byName = new Map(g.nodes.map((n, i) => [n.name, i]));
  // How the source's frame for each target node is turned from the target's (identity unless Blender rolled the bone).
  const frame = new Map<number, Q>();
  sg.gltf.nodes.forEach((n, si) => {
    const ti = byName.get(n.name);
    if (ti === undefined) return;
    const d = qmul(qinv(tw[ti].r), sw[si].r);
    frame.set(ti, d[3] < 0 ? (d.map((x) => -x) as Q) : d);
    const off = Math.hypot(...tw[ti].t.map((x, k) => x - sw[si].t[k]));
    report.apart = Math.max(report.apart, off);
    if (!near(frame.get(ti)!)) report.turned.push(`${n.name} ${((2 * Math.acos(Math.min(1, Math.abs(frame.get(ti)![3]) / Math.hypot(...d))) * 180) / Math.PI).toFixed(1)}`);
  });

  // The new data: each source accessor a clip reads, copied once (inputs are shared), values carried into the target's frames.
  const extra: Buffer[] = [];
  const newAcc: Accessor[] = [];
  const newViews: View[] = [];
  const add = (acc: Accessor, data: Buffer) => {
    newViews.push({ buffer: 0, byteLength: data.length });
    extra.push(data.length % 4 ? Buffer.concat([data, Buffer.alloc(4 - (data.length % 4))]) : data);
    const { bufferView: _v, byteOffset: _o, ...rest } = acc;
    newAcc.push({ ...rest, bufferView: -newViews.length });
    return -newAcc.length;
  };
  const raw = (i: number): Buffer => {
    const a = sg.gltf.accessors[i] as Accessor;
    const v = s.json.bufferViews[a.bufferView!];
    const size = { 5120: 1, 5121: 1, 5122: 2, 5123: 2, 5125: 4, 5126: 4 }[a.componentType as 5126]!;
    const w = WIDTH[a.type] * size;
    const stride = v.byteStride ?? w;
    const start = (v.byteOffset ?? 0) + (a.byteOffset ?? 0);
    const out = Buffer.alloc(a.count * w);
    for (let k = 0; k < a.count; k++) s.bin.copy(out, k * w, start + k * stride, start + k * stride + w);
    return out;
  };
  const copied = new Map<number, number>();
  const anims: Anim[] = [];
  for (const name of names) {
    const a = s.json.animations?.find((x) => x.name === name);
    if (!a) throw new Error(`glb_clips: no clip ${name} in the source`);
    const anim: Anim = { name, channels: [], samplers: [] };
    for (const ch of a.channels) {
      const node = byName.get(sg.gltf.nodes[ch.target.node].name);
      if (node === undefined) throw new Error(`glb_clips: ${name} moves ${sg.gltf.nodes[ch.target.node].name}, which the target has not`);
      const sm = a.samplers[ch.sampler];
      const parent = tg.parentOf[node];
      const dn = frame.get(node) ?? [0, 0, 0, 1];
      const dp: Q = parent === undefined ? [0, 0, 0, 1] : (frame.get(parent) ?? [0, 0, 0, 1]);
      if (!copied.has(sm.input)) copied.set(sm.input, add(sg.gltf.accessors[sm.input] as Accessor, raw(sm.input)));
      let output: number;
      const turned = ch.target.path === "rotation" ? !(near(dp) && near(dn)) : ch.target.path === "translation" && !near(dp);
      if (!turned) output = add(sg.gltf.accessors[sm.output] as Accessor, raw(sm.output));
      else {
        // local in the target's frames = D_parent * local in the source's * D_node^-1 (a rotation; translations by D_parent alone).
        const vals = sg.read(sm.output);
        const out = Buffer.alloc(vals.length * 4);
        const w = ch.target.path === "rotation" ? 4 : 3;
        for (let k = 0; k < vals.length / w; k++) {
          const v = Array.from(vals.subarray(k * w, k * w + w));
          const r = w === 4 ? qmul(qmul(dp, v as Q), qinv(dn)) : rot(dp, v as V3);
          r.forEach((x, j) => out.writeFloatLE(x, (k * w + j) * 4));
        }
        const { min: _min, max: _max, ...acc } = sg.gltf.accessors[sm.output] as Accessor;
        output = add(acc, out);
      }
      anim.samplers.push({ input: copied.get(sm.input)!, output, interpolation: sm.interpolation });
      anim.channels.push({ sampler: anim.samplers.length - 1, target: { node, path: ch.target.path } });
    }
    anims.push(anim);
  }
  const animations = [...(g.animations ?? [])];
  for (const a of anims) {
    const at = animations.findIndex((x) => x.name === a.name);
    if (at < 0) animations.push(a);
    else animations[at] = a;
  }

  // What is still read: kept in its old order, the new after it; the rest dropped.
  const usedAcc = new Set<number>();
  for (const m of g.meshes ?? []) for (const p of m.primitives) [...Object.values(p.attributes), p.indices, ...(p.targets ?? []).flatMap((x) => Object.values(x))].forEach((i) => i !== undefined && usedAcc.add(i));
  for (const sk of g.skins ?? []) if (sk.inverseBindMatrices !== undefined) usedAcc.add(sk.inverseBindMatrices);
  for (const a of animations) for (const sm of a.samplers) [sm.input, sm.output].forEach((i) => i >= 0 && usedAcc.add(i));
  const accOrder = g.accessors.map((_, i) => i).filter((i) => usedAcc.has(i));
  const accMap = new Map<number, number>(accOrder.map((i, k) => [i, k]));
  newAcc.forEach((_, k) => accMap.set(-(k + 1), accOrder.length + k));
  const usedView = new Set<number>(accOrder.map((i) => g.accessors[i].bufferView!).filter((v) => v !== undefined));
  for (const im of g.images ?? []) if (im.bufferView !== undefined) usedView.add(im.bufferView);
  const viewOrder = g.bufferViews.map((_, i) => i).filter((i) => usedView.has(i));
  const viewMap = new Map<number, number>(viewOrder.map((i, k) => [i, k]));
  const parts: Buffer[] = [];
  let offset = 0;
  const views: View[] = [];
  const place = (v: View, data: Buffer) => {
    const pad = (4 - (offset % 4)) % 4;
    if (pad) parts.push(Buffer.alloc(pad));
    offset += pad;
    views.push({ ...v, byteOffset: offset });
    parts.push(data);
    offset += data.length;
  };
  for (const i of viewOrder) {
    const v = g.bufferViews[i];
    place(v, t.bin.subarray(v.byteOffset ?? 0, (v.byteOffset ?? 0) + v.byteLength));
  }
  newViews.forEach((v, k) => {
    viewMap.set(-(k + 1), viewOrder.length + k);
    place(v, extra[k].subarray(0, v.byteLength));
  });
  const accessors = [...accOrder.map((i) => g.accessors[i]), ...newAcc].map((a) => ({ ...a, bufferView: a.bufferView === undefined ? undefined : viewMap.get(a.bufferView) }));
  const remap = (i: number) => accMap.get(i)!;
  const json: Gltf = {
    ...g,
    accessors,
    bufferViews: views,
    buffers: [{ ...g.buffers[0], byteLength: offset + ((4 - (offset % 4)) % 4) }],
    meshes: g.meshes?.map((m) => ({
      ...m,
      primitives: m.primitives.map((p) => ({
        ...p,
        attributes: Object.fromEntries(Object.entries(p.attributes).map(([k, v]) => [k, remap(v)])),
        ...(p.indices !== undefined ? { indices: remap(p.indices) } : {}),
        ...(p.targets ? { targets: p.targets.map((x) => Object.fromEntries(Object.entries(x).map(([k, v]) => [k, remap(v)]))) } : {}),
      })),
    })),
    skins: g.skins?.map((sk) => (sk.inverseBindMatrices === undefined ? sk : { ...sk, inverseBindMatrices: remap(sk.inverseBindMatrices) })),
    images: g.images?.map((im) => (im.bufferView === undefined ? im : { ...im, bufferView: viewMap.get(im.bufferView) })),
    animations: animations.map((a) => ({ ...a, samplers: a.samplers.map((sm) => ({ ...sm, input: remap(sm.input), output: remap(sm.output) })) })),
  };
  if (!g.meshes) delete json.meshes;
  if (!g.skins) delete json.skins;
  if (!g.images) delete json.images;
  return glbBytes(json, Buffer.concat(parts));
}

/**
 * `source` with a clip `name` added that plays `parts` one after another (a blow and then its
 * recovery, as a montage plays its sections): each channel's keys of every part, the next part's
 * shifted by the lengths before it, a key landing on the time of the one before dropped. Every
 * part must move what the first moves, keyed linearly or stepped. A part that is a number is a
 * pause of that many seconds on the next part's first pose: a lead-in, where a blow taken from
 * another stance is eased into from the guard (hold.py's `out`) before it plays.
 */
export function joinClips(source: Buffer, name: string, parts: string[]): Buffer {
  const { json, bin } = chunks(source);
  const g = new Glb(source);
  const pauses = new Map<Anim, number>();
  let pause = 0;
  const anims: Anim[] = [];
  for (const p of parts) {
    if (/^\d+(\.\d+)?$/.test(p)) {
      pause += Number(p);
      continue;
    }
    const a = json.animations?.find((x) => x.name === p);
    if (!a) throw new Error(`glb_clips: no clip ${p} in the source to join into ${name}`);
    if (pause) pauses.set(a, pause);
    pause = 0;
    anims.push(a);
  }
  if (pause) throw new Error(`glb_clips: ${name} ends on a pause; a pause holds the first pose of the part after it`);
  const length = (a: Anim) => Math.max(...a.samplers.map((s) => g.read(s.input).at(-1) ?? 0));
  const data: Buffer[] = [bin.length % 4 ? Buffer.concat([bin, Buffer.alloc(4 - (bin.length % 4))]) : bin];
  let offset = data[0].length;
  const accessors = [...json.accessors];
  const bufferViews = [...json.bufferViews];
  const floats = (values: number[], type: string, bounds: boolean) => {
    const b = Buffer.alloc(values.length * 4);
    values.forEach((x, k) => b.writeFloatLE(x, k * 4));
    bufferViews.push({ buffer: 0, byteOffset: offset, byteLength: b.length });
    data.push(b);
    offset += b.length;
    accessors.push({ bufferView: bufferViews.length - 1, componentType: 5126, count: values.length / WIDTH[type], type, ...(bounds ? { min: [Math.min(...values)], max: [Math.max(...values)] } : {}) });
    return accessors.length - 1;
  };
  const joined: Anim = { name, channels: [], samplers: [] };
  for (const ch of anims[0].channels) {
    const times: number[] = [];
    const values: number[] = [];
    let at = 0;
    let type = "";
    let interpolation: string | undefined;
    for (const a of anims) {
      const c = a.channels.find((x) => x.target.node === ch.target.node && x.target.path === ch.target.path);
      const what = `${json.nodes[ch.target.node].name} ${ch.target.path}`;
      if (!c) throw new Error(`glb_clips: ${a.name} does not move ${what}, which ${anims[0].name} does; join clips that key the same channels`);
      const sm = a.samplers[c.sampler];
      if (sm.interpolation === "CUBICSPLINE") throw new Error(`glb_clips: ${a.name}'s ${what} is a cubic spline; only linear or stepped keys join`);
      interpolation = sm.interpolation;
      type = json.accessors[sm.output].type;
      const t = g.read(sm.input);
      const v = g.read(sm.output);
      const w = WIDTH[type];
      const held = pauses.get(a);
      if (held) {
        if (!(times.length && at <= times[times.length - 1] + 1e-5)) {
          times.push(at);
          for (let j = 0; j < w; j++) values.push(v[j]);
        }
        at += held;
      }
      for (let k = 0; k < t.length; k++) {
        if (times.length && at + t[k] <= times[times.length - 1] + 1e-5) continue;
        times.push(at + t[k]);
        for (let j = 0; j < w; j++) values.push(v[k * w + j]);
      }
      at += length(a);
    }
    joined.samplers.push({ input: floats(times, "SCALAR", true), output: floats(values, type, false), interpolation });
    joined.channels.push({ sampler: joined.samplers.length - 1, target: ch.target });
  }
  const out = { ...json, accessors, bufferViews, buffers: [{ ...json.buffers[0], byteLength: offset }], animations: [...(json.animations ?? []).filter((a) => a.name !== name), joined] };
  return glbBytes(out, Buffer.concat(data));
}

/** The file with data nothing reads dropped from its buffer. */
export const compact = (bytes: Buffer): Buffer => putClips(bytes, bytes, []);

/**
 * Only the skeleton and the named clips of `source`: no mesh, skin, material or image, every
 * other clip dropped, the nodes and their rest kept, so `putClips` reads it as it reads the
 * whole pack. A bought library is cut to what the game plays this way and that is committed.
 */
export function onlyClips(source: Buffer, names: string[]): Buffer {
  const { json, bin } = chunks(source) as { json: Gltf & Record<string, unknown>; bin: Buffer };
  const missing = names.filter((n) => !json.animations?.some((a) => a.name === n));
  if (missing.length) throw new Error(`glb_clips: no clip ${missing.join(", ")} in the source; name its clips as the pack does`);
  const { meshes: _m, skins: _s, materials: _ma, textures: _t, images: _i, samplers: _sa, ...rest } = json;
  const nodes = json.nodes.map((n) => {
    const { mesh: _nm, skin: _ns, ...node } = n as Record<string, unknown>;
    return node;
  });
  const animations = (json.animations ?? []).filter((a) => names.includes(a.name));
  return compact(glbBytes({ ...rest, nodes, animations }, bin));
}

/** `onlyClips` of the named clips, each a clip of `source` or NEW=A+B, the parts joined first. */
export function cutClips(source: Buffer, names: string[]): Buffer {
  let from = source;
  const kept = names.map((n) => {
    const [name, parts] = n.split("=");
    if (parts) from = joinClips(from, name, parts.split("+"));
    return name;
  });
  return onlyClips(from, kept);
}

if (import.meta.main) {
  if (process.argv[2] === "--only") {
    const [out, source, ...names] = process.argv.slice(3);
    if (!out || !source || !names.length) {
      console.log("usage: node tools/characters/glb_clips.ts --only OUT.glb SOURCE.glb CLIP|NEW=A+B...");
      process.exit(2);
    }
    const bytes = cutClips(readFileSync(source), names);
    writeFileSync(out, bytes);
    console.log(`CLIPS kept ${names.length} of ${source} in ${out}, ${(bytes.length / 1024).toFixed(0)} KB, the skeleton and no mesh`);
    process.exit(0);
  }
  const [target, source, ...given] = process.argv.slice(2);
  if (!target || !source || !given.length) {
    console.log("usage: node tools/characters/glb_clips.ts TARGET.glb SOURCE.glb CLIP|NEW=A+B...\n       node tools/characters/glb_clips.ts --only OUT.glb SOURCE.glb CLIP|NEW=A+B...");
    process.exit(2);
  }
  const before = readFileSync(target);
  const report: Report = { turned: [], apart: 0 };
  let from = readFileSync(source);
  const names = given.map((n) => {
    const [name, parts] = n.split("=");
    if (parts) from = joinClips(from, name, parts.split("+"));
    return name;
  });
  const out = putClips(before, from, names, report);
  if (!out.equals(before)) writeFileSync(target, out);
  const turned = report.turned.length ? `; ${report.turned.length} bones' frames turned in the source, keys carried into the target's (${report.turned.slice(0, 4).join(", ")} deg)` : "";
  console.log(`CLIPS put ${names.length} into ${target}${out.equals(before) ? ", the same bytes as it had" : ""}; the source's joints within ${(report.apart * 1000).toFixed(2)} mm of the target's${turned}`);
}
