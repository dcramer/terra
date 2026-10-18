#!/usr/bin/env node
// Measures a blow's phases from its clip (docs/movement.md, "Moves as data"): the held
// weapon's tip followed through the skeleton frame by frame, in the body's own space (x to its
// left, y up, z ahead, from the hips' place at the start), and when it sweeps forward of the
// body fast enough to strike: the active frames a move's data says
// (assets/data/moves/*.ron). Prints, a line a clip, the span its tip is more than --ahead
// metres in front of the hips and moving faster than --speed m/s, its fastest moment in it and
// where it is then; --trace prints every frame.
//
//   node tools/characters/phases.ts assets/models/character/fight.glb Fight_Slash Fight_Backhand
//   node tools/characters/phases.ts <file.glb> <clip>… [--socket socket.hand.R] [--tip 0.83] [--speed 4] [--ahead 0.5] [--trace]
//
// Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";

type V3 = [number, number, number];
type Q = [number, number, number, number];
const args = process.argv.slice(2);
const opt = (name: string, fallback: string) => {
  const i = args.indexOf(name);
  return i >= 0 ? args.splice(i, 2)[1] : fallback;
};
const trace = args.includes("--trace") ? (args.splice(args.indexOf("--trace"), 1), true) : false;
const socket = opt("--socket", "socket.hand.R");
const tipM = Number(opt("--tip", "0.83"));
const fast = Number(opt("--speed", "4"));
const reach = Number(opt("--ahead", "0.5"));
const [file, ...clips] = args;
if (!file || clips.length === 0) {
  console.error("usage: phases.ts <file.glb> <clip>… [--socket name] [--tip m] [--speed m/s] [--trace]");
  process.exit(1);
}

const glb = readFileSync(file);
const jsonLen = glb.readUInt32LE(12);
const gltf = JSON.parse(glb.subarray(20, 20 + jsonLen).toString());
const binStart = 20 + jsonLen + 8;
const floats = (index: number): Float32Array => {
  const acc = gltf.accessors[index];
  const view = gltf.bufferViews[acc.bufferView];
  const width = ({ SCALAR: 1, VEC3: 3, VEC4: 4 } as Record<string, number>)[acc.type];
  const start = binStart + (view.byteOffset ?? 0) + (acc.byteOffset ?? 0);
  const out = new Float32Array(acc.count * width);
  for (let i = 0; i < out.length; i++) out[i] = glb.readFloatLE(start + i * 4);
  return out;
};

const qmul = (a: Q, b: Q): Q => [
  a[3] * b[0] + a[0] * b[3] + a[1] * b[2] - a[2] * b[1],
  a[3] * b[1] - a[0] * b[2] + a[1] * b[3] + a[2] * b[0],
  a[3] * b[2] + a[0] * b[1] - a[1] * b[0] + a[2] * b[3],
  a[3] * b[3] - a[0] * b[0] - a[1] * b[1] - a[2] * b[2],
];
const rot = (q: Q, v: V3): V3 => {
  const p = qmul(qmul(q, [v[0], v[1], v[2], 0]), [-q[0], -q[1], -q[2], q[3]]);
  return [p[0], p[1], p[2]];
};
type Pose = { t: V3; r: Q; s: V3 };
const apply = (parent: Pose, local: Pose): Pose => {
  const scaled: V3 = [local.t[0] * parent.s[0], local.t[1] * parent.s[1], local.t[2] * parent.s[2]];
  const moved = rot(parent.r, scaled);
  return {
    t: [parent.t[0] + moved[0], parent.t[1] + moved[1], parent.t[2] + moved[2]],
    r: qmul(parent.r, local.r),
    s: [parent.s[0] * local.s[0], parent.s[1] * local.s[1], parent.s[2] * local.s[2]],
  };
};

const parentOf: number[] = [];
gltf.nodes.forEach((n: { children?: number[] }, i: number) => n.children?.forEach((c) => (parentOf[c] = i)));
const named = (name: string) => gltf.nodes.findIndex((n: { name?: string }) => n.name === name);
const sock = named(socket);
const hips = named("pelvis");
if (sock < 0 || hips < 0) throw new Error(`no ${sock < 0 ? socket : "pelvis"} in ${file}`);

type Anim = { samplers: { input: number; output: number }[] };
function sampler(anim: Anim, s: number, width: number) {
  const times = floats(anim.samplers[s].input);
  const values = floats(anim.samplers[s].output);
  return (t: number): number[] => {
    let i = 0;
    while (i < times.length - 2 && times[i + 1] < t) i++;
    const span = times[i + 1] - times[i] || 1;
    const k = Math.min(1, Math.max(0, (t - times[i]) / span));
    const a = Array.from(values.subarray(i * width, i * width + width));
    const b = Array.from(values.subarray((i + 1) * width, (i + 1) * width + width));
    if (times.length < 2) return a;
    if (width === 4) {
      const sign = a.reduce((sum, x, j) => sum + x * b[j], 0) < 0 ? -1 : 1;
      const mixed = a.map((x, j) => x + (b[j] * sign - x) * k);
      const len = Math.hypot(...mixed);
      return mixed.map((x) => x / len);
    }
    return a.map((x, j) => x + (b[j] - x) * k);
  };
}

for (const clip of clips) {
  const anim = gltf.animations.find((a: { name: string }) => a.name === clip);
  if (!anim) {
    console.log(`${clip}: no such clip in ${file}`);
    continue;
  }
  const tracks = new Map<number, Record<string, (t: number) => number[]>>();
  let end = 0;
  for (const ch of anim.channels) {
    const width = ch.target.path === "rotation" ? 4 : 3;
    if (!tracks.has(ch.target.node)) tracks.set(ch.target.node, {});
    tracks.get(ch.target.node)![ch.target.path] = sampler(anim, ch.sampler, width);
    end = Math.max(end, gltf.accessors[anim.samplers[ch.sampler].input].max[0]);
  }
  const world = (node: number, t: number): Pose => {
    const n = gltf.nodes[node];
    const tr = tracks.get(node) ?? {};
    const local: Pose = {
      t: (tr.translation?.(t) ?? n.translation ?? [0, 0, 0]) as V3,
      r: (tr.rotation?.(t) ?? n.rotation ?? [0, 0, 0, 1]) as Q,
      s: (tr.scale?.(t) ?? n.scale ?? [1, 1, 1]) as V3,
    };
    return parentOf[node] === undefined ? local : apply(world(parentOf[node], t), local);
  };
  const start = world(hips, 0).t;
  const step = 1 / 60;
  const frames: { t: number; tip: V3; speed: number }[] = [];
  for (let t = 0; t <= end + 1e-6; t += step) {
    const hand = world(sock, t);
    const out = rot(hand.r, [0, tipM * hand.s[1], 0]);
    const tip = out.map((x, j) => x + hand.t[j] - start[j]) as V3;
    const prev = frames.at(-1);
    const moved = prev ? Math.hypot(...tip.map((x, j) => x - prev.tip[j])) : 0;
    frames.push({ t, tip, speed: moved / step });
  }
  const f = (v: number) => v.toFixed(2);
  if (trace) for (const fr of frames) console.log(`${f(fr.t)} tip ${fr.tip.map(f).join(" ")} speed ${f(fr.speed)} m/s`);
  const striking = frames.filter((fr) => fr.tip[2] > reach && fr.speed > fast);
  if (striking.length === 0) {
    console.log(`${clip} (${f(end)} s): the tip never moves faster than ${fast} m/s more than ${reach} m ahead`);
    continue;
  }
  // The one sweep through the fastest moment, not every quick frame after it.
  const fastest = striking.reduce((a, b) => (b.speed > a.speed ? b : a));
  const strikes = (i: number) => frames[i].tip[2] > reach && frames[i].speed > fast;
  let from = frames.indexOf(fastest);
  let to = from;
  while (from > 0 && strikes(from - 1)) from--;
  while (to < frames.length - 1 && strikes(to + 1)) to++;
  const [left, up, ahead] = fastest.tip;
  console.log(
    `${clip} (${f(end)} s): striking ${f(frames[from].t)}-${f(frames[to].t)} s; fastest ${f(fastest.speed)} m/s at ${f(fastest.t)} s, ${f(ahead)} m ahead, ${f(up)} m up, ${f(left)} m left`,
  );
}
