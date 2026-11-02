#!/usr/bin/env node
// A clip from key poses on our skeleton, no generator in between: the same keys give the same
// clip, every key starts from the guard, and a foot not told to move does not move.
//
//   node spike/pose-sheet/key.ts KEYS.json OUT.glb
//
// KEYS.json is { name, guard, end, keys: [{ t, ...controls }] }. A control a key leaves out holds
// its last value. All places are [right, up, forward] in metres from the model's origin, all
// directions the same axes; degrees are yaw + to his left, pitch + forward, roll + to his right.
//   pelvis {move, yaw, pitch, roll}      from the guard, in the model
//   chest  {yaw, pitch, roll}            over the pelvis, spread up the three spine bones
//   head   {yaw, pitch}                  from the guard, in the model
//   hand_r {at, blade, edge, elbow}      wrist place, tip's way, leading edge's way, elbow's way
//   hand_l {at, elbow}                   an offset; else it rides half the chest's turn
//   clav_r / clav_l {up, forward}        the shoulder lifted and pushed forward
//   foot_r / foot_l {at, yaw, heel}      the ball moved, turned, the heel raised over the ball
//   arm_r / arm_l {forward, out, elbow}  with `hang`: empty arms ride the chest as the guard hangs
//                                        them, swayed by these degrees; hand_r and hand_l are unused
// `loop` makes the curve run on through the seam; the last key must equal the first.

import { readFileSync, writeFileSync } from "node:fs";
import { apply, dot, Glb, len, qinv, qmul, rot, sub, type Pose, type Q, type V3 } from "../../tools/characters/glb.ts";
import { glbBytes } from "../../tools/characters/glb_clips.ts";

const FPS = 60;
const body = "assets/models/character/fight.glb";
const add = (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const scale = (a: V3, k: number): V3 => [a[0] * k, a[1] * k, a[2] * k];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a: V3): V3 => scale(a, 1 / (len(a) || 1));
// Our axes to the model's: he faces +Z, his right is -X.
const model = ([r, u, f]: number[]): V3 => [-r, u, f];
const about = (axis: V3, deg: number): Q => {
  const h = (deg * Math.PI) / 360;
  const a = norm(axis);
  return [a[0] * Math.sin(h), a[1] * Math.sin(h), a[2] * Math.sin(h), Math.cos(h)];
};
const euler = (yaw = 0, pitch = 0, roll = 0): Q => qmul(about([0, 1, 0], yaw), qmul(about([1, 0, 0], pitch), about([0, 0, 1], roll)));
const slerpId = (q: Q, k: number): Q => {
  const w = Math.max(-1, Math.min(1, q[3]));
  const half = Math.acos(w);
  const s = Math.sin(half);
  if (s < 1e-6) return [0, 0, 0, 1];
  const f = Math.sin(half * k) / s;
  return [q[0] * f, q[1] * f, q[2] * f, Math.cos(half * k)];
};
/// A rotation whose x is `dir` and whose z is `dir` crossed with `ref`.
function frame(dir: V3, ref: V3): Q {
  const x = norm(dir);
  const z = norm(cross(x, ref));
  return basis(x, cross(z, x), z);
}
function basis(x: V3, y: V3, z: V3): Q {
  const t = x[0] + y[1] + z[2];
  if (t > 0) {
    const s = Math.sqrt(t + 1) * 2;
    return [(y[2] - z[1]) / s, (z[0] - x[2]) / s, (x[1] - y[0]) / s, s / 4];
  }
  if (x[0] > y[1] && x[0] > z[2]) {
    const s = Math.sqrt(1 + x[0] - y[1] - z[2]) * 2;
    return [s / 4, (y[0] + x[1]) / s, (z[0] + x[2]) / s, (y[2] - z[1]) / s];
  }
  if (y[1] > z[2]) {
    const s = Math.sqrt(1 + y[1] - x[0] - z[2]) * 2;
    return [(y[0] + x[1]) / s, s / 4, (z[1] + y[2]) / s, (z[0] - x[2]) / s];
  }
  const s = Math.sqrt(1 + z[2] - x[0] - y[1]) * 2;
  return [(z[0] + x[2]) / s, (z[1] + y[2]) / s, s / 4, (x[1] - y[0]) / s];
}

const spec = JSON.parse(readFileSync(process.argv[2], "utf8"));
const out = process.argv[3];
const g = new Glb(body);
const nodes = g.gltf.nodes;
const id = (n: string) => {
  const i = g.named(n);
  if (i < 0) throw new Error(`key: no bone ${n} in ${body}`);
  return i;
};
const guard = g.clip(spec.guard)!;
const base: Pose[] = nodes.map((n, i) => {
  const track = n.name ? guard.tracks.get(n.name) : undefined;
  const rest = g.rest(i);
  return { t: (track?.t?.(0) as V3) ?? rest.t, r: (track?.r?.(0) as Q) ?? rest.r, s: (track?.s?.(0) as V3) ?? rest.s };
});
function fk(l: Pose[]): Pose[] {
  const w: Pose[] = [];
  const at = (i: number): Pose => (w[i] ??= g.parentOf[i] === undefined ? l[i] : apply(at(g.parentOf[i]), l[i]));
  l.forEach((_, i) => at(i));
  return w;
}
const G = fk(base);

// Every key's controls, each carrying the last value of what it leaves out.
type Controls = Record<string, Record<string, number | number[]>>;
const grip = (side: "r" | "l") => {
  const wrist = G[id(`hand_${side}`)].t;
  const socket = G[id(`socket.hand.${side.toUpperCase()}`)];
  return { at: [-wrist[0], wrist[1], wrist[2]], blade: rot(socket.r, [0, 1, 0]).map((v, i) => (i ? v : -v)), edge: rot(socket.r, [0, 0, -1]).map((v, i) => (i ? v : -v)) };
};
const start: Controls = {
  pelvis: { move: [0, 0, 0], yaw: 0, pitch: 0, roll: 0 },
  chest: { yaw: 0, pitch: 0, roll: 0 },
  head: { yaw: 0, pitch: 0 },
  hand_r: { ...grip("r"), elbow: [0, 0, 0] },
  hand_l: { at: [0, 0, 0], elbow: [0, 0, 0] },
  clav_r: { up: 0, forward: 0 },
  clav_l: { up: 0, forward: 0 },
  arm_r: { forward: 0, out: 0, elbow: 0 },
  arm_l: { forward: 0, out: 0, elbow: 0 },
  foot_r: { at: [0, 0, 0], yaw: 0, heel: 0 },
  foot_l: { at: [0, 0, 0], yaw: 0, heel: 0 },
};
const keys: { t: number; c: Controls }[] = [];
let last = start;
for (const key of spec.keys) {
  const c: Controls = JSON.parse(JSON.stringify(key.guard ? start : last));
  for (const part of Object.keys(key)) {
    if (part === "t" || part === "guard" || part === "panel") continue;
    if (!c[part]) throw new Error(`key: t=${key.t}: no control ${part}; the header of key.ts lists them`);
    Object.assign(c[part], key[part]);
  }
  keys.push({ t: key.t, c });
  last = c;
}

/// Monotone cubic through the keys (Fritsch-Carlson): eases in and out of holds and never
/// overshoots, so a pose keyed is the pose reached.
function curve(ts: number[], ys: number[]): (t: number) => number {
  const n = ts.length;
  const d = ts.slice(1).map((t, i) => (ys[i + 1] - ys[i]) / (t - ts[i]));
  const m = ys.map((_, i) => {
    // A loop's ends are one key, so the speed runs on through the seam.
    const end = i === 0 || i === n - 1;
    if (end && !spec.loop) return 0;
    const [before, after] = end ? [n - 2, 0] : [i - 1, i];
    if (d[before] * d[after] <= 0) return 0;
    const h0 = ts[before + 1] - ts[before];
    const h1 = ts[after + 1] - ts[after];
    return (3 * (h0 + h1)) / ((2 * h1 + h0) / d[before] + (h1 + 2 * h0) / d[after]);
  });
  return (t) => {
    if (t <= ts[0]) return ys[0];
    if (t >= ts[n - 1]) return ys[n - 1];
    const i = Math.max(0, ts.findIndex((x) => x > t) - 1);
    const h = ts[i + 1] - ts[i];
    const u = (t - ts[i]) / h;
    const u2 = u * u;
    const u3 = u2 * u;
    return (2 * u3 - 3 * u2 + 1) * ys[i] + (u3 - 2 * u2 + u) * h * m[i] + (-2 * u3 + 3 * u2) * ys[i + 1] + (u3 - u2) * h * m[i + 1];
  };
}
const ts = keys.map((k) => k.t);
const curves = new Map<string, (t: number) => number>();
const value = (part: string, field: string, i: number, t: number): number => {
  const name = `${part}.${field}.${i}`;
  let f = curves.get(name);
  if (!f) {
    f = curve(ts, keys.map((k) => {
      const v = k.c[part][field];
      return Array.isArray(v) ? v[i] : v;
    }));
    curves.set(name, f);
  }
  return f(t);
};
const num = (p: string, f: string, t: number) => value(p, f, 0, t);
const vec = (p: string, f: string, t: number): V3 => [value(p, f, 0, t), value(p, f, 1, t), value(p, f, 2, t)];
/// A foot's way between two keys: stands, or lifts along an arc; never drifts.
function footAt(side: string, t: number): V3 {
  const part = `foot_${side}`;
  const i = Math.max(0, Math.min(keys.length - 2, ts.findIndex((x) => x > t) - 1));
  const a = keys[i].c[part].at as number[];
  const b = keys[i + 1].c[part].at as number[];
  const u = Math.min(1, Math.max(0, (t - ts[i]) / (ts[i + 1] - ts[i])));
  const s = u * u * (3 - 2 * u);
  const p: V3 = [a[0] + (b[0] - a[0]) * s, a[1] + (b[1] - a[1]) * s, a[2] + (b[2] - a[2]) * s];
  if (Math.hypot(b[0] - a[0], b[2] - a[2]) > 1e-4) p[1] += 0.05 * Math.sin(Math.PI * u);
  return p;
}

function setWorld(l: Pose[], i: number, r: Q, w: Pose[]) {
  l[i] = { ...l[i], r: qmul(qinv(w[g.parentOf[i]].r), r) };
}
function bend(l: Pose[], i: number, q: Q, w: Pose[]) {
  const p = w[g.parentOf[i]].r;
  l[i] = { ...l[i], r: qmul(qmul(qmul(qinv(p), q), p), l[i].r) };
}
const perp = (v: V3, along: V3): V3 => sub(v, scale(along, dot(v, along)));

type Chain = { a: number; b: number; c: number; la: number; lb: number; pole: V3; oa: Q; ob: Q };
function chain(a: string, b: string, c: string): Chain {
  const [ia, ib, ic] = [id(a), id(b), id(c)];
  const [A, B, C] = [G[ia], G[ib], G[ic]];
  const along = norm(sub(C.t, A.t));
  const pole = norm(perp(sub(B.t, A.t), along));
  const n = cross(along, pole);
  return { a: ia, b: ib, c: ic, la: len(sub(B.t, A.t)), lb: len(sub(C.t, B.t)), pole, oa: qmul(qinv(frame(sub(B.t, A.t), n)), A.r), ob: qmul(qinv(frame(sub(C.t, B.t), n)), B.r) };
}
const stretched: Record<string, number> = {};
function reach(l: Pose[], k: Chain, target: V3, pole: V3, name: string) {
  let w = fk(l);
  const S = w[k.a].t;
  const full = (k.la + k.lb) * 0.995;
  let d = len(sub(target, S));
  stretched[name] = Math.max(stretched[name] ?? 0, d / (k.la + k.lb));
  d = Math.min(d, full);
  const along = norm(sub(target, S));
  const T = add(S, scale(along, d));
  const p = norm(perp(pole, along));
  const cosA = (k.la * k.la + d * d - k.lb * k.lb) / (2 * k.la * d);
  const E = add(S, add(scale(along, k.la * cosA), scale(p, k.la * Math.sqrt(Math.max(0, 1 - cosA * cosA)))));
  const n = cross(along, p);
  setWorld(l, k.a, qmul(frame(sub(E, S), n), k.oa), w);
  w = fk(l);
  setWorld(l, k.b, qmul(frame(sub(T, E), n), k.ob), w);
}
const arm = { r: chain("upperarm_r", "lowerarm_r", "hand_r"), l: chain("upperarm_l", "lowerarm_l", "hand_l") };
const leg = { r: chain("thigh_r", "calf_r", "foot_r"), l: chain("thigh_l", "calf_l", "foot_l") };
const socketInHand = qmul(qinv(G[id("hand_r")].r), G[id("socket.hand.R")].r);
const chestG = G[id("spine_03")];
const handLInChest = { t: rot(qinv(chestG.r), sub(G[id("hand_l")].t, chestG.t)), r: qmul(qinv(chestG.r), G[id("hand_l")].r) };
const forearmAxis = norm(g.rest(id("hand_r")).t);

function pose(t: number): Pose[] {
  const l = base.map((p) => ({ ...p }));
  // The hips, then the chest over them.
  let w = fk(l);
  const pel = id("pelvis");
  const moved = add(G[pel].t, model(vec("pelvis", "move", t)));
  const parent = w[g.parentOf[pel]];
  l[pel].t = scale(rot(qinv(parent.r), sub(moved, parent.t)), 1 / parent.s[0]);
  setWorld(l, pel, qmul(euler(num("pelvis", "yaw", t), num("pelvis", "pitch", t), num("pelvis", "roll", t)), G[pel].r), w);
  const lean = euler(num("chest", "yaw", t), num("chest", "pitch", t), num("chest", "roll", t));
  for (const name of ["spine_01", "spine_02", "spine_03"]) {
    w = fk(l);
    bend(l, id(name), slerpId(lean, 1 / 3), w);
  }
  // The head looks where it is told, whatever the chest does.
  w = fk(l);
  const wanted = qmul(euler(num("head", "yaw", t), num("head", "pitch", t)), G[id("Head")].r);
  bend(l, id("neck_01"), slerpId(qmul(wanted, qinv(w[id("Head")].r)), 0.5), w);
  w = fk(l);
  bend(l, id("Head"), qmul(wanted, qinv(w[id("Head")].r)), w);
  // The shoulders.
  w = fk(l);
  const chest = w[id("spine_03")];
  const turned = qmul(chest.r, qinv(chestG.r));
  for (const side of ["r", "l"] as const) {
    const sign = side === "r" ? 1 : -1;
    const up = about(rot(turned, [0, 0, 1]), -sign * num(`clav_${side}`, "up", t));
    const forward = about(rot(turned, [0, 1, 0]), sign * num(`clav_${side}`, "forward", t));
    bend(l, id(`clavicle_${side}`), qmul(up, forward), w);
  }
  // Empty arms hang from the chest, a little sway laid on at the shoulder and the elbow.
  if (spec.hang) {
    for (const side of ["r", "l"] as const) {
      const sign = side === "r" ? -1 : 1;
      const across = rot(turned, [1, 0, 0]);
      w = fk(l);
      bend(l, id(`upperarm_${side}`), qmul(about(across, -num(`arm_${side}`, "forward", t)), about(rot(turned, [0, 0, 1]), sign * num(`arm_${side}`, "out", t))), w);
      w = fk(l);
      bend(l, id(`lowerarm_${side}`), about(across, -num(`arm_${side}`, "elbow", t)), w);
    }
  }
  // The sword hand goes to its place and points the blade; the shield rides the chest.
  if (!spec.hang) {
  const hint = (part: string, k: Chain): V3 => {
    const v = vec(part, "elbow", t);
    return len(v) > 1e-3 ? model(v) : rot(turned, k.pole);
  };
  reach(l, arm.r, model(vec("hand_r", "at", t)), hint("hand_r", arm.r), "arm_r");
  w = fk(l);
  const blade = norm(model(vec("hand_r", "blade", t)));
  const leading = norm(perp(model(vec("hand_r", "edge", t)), blade));
  const socket = basis(cross(blade, scale(leading, -1)), blade, scale(leading, -1));
  const handR = qmul(socket, qinv(socketInHand));
  setWorld(l, id("hand_r"), handR, w);
  // Half the wrist's turn goes up the forearm, as a forearm rolls.
  const rel = qmul(qinv(w[arm.r.b].r), handR);
  const relG = qmul(qinv(G[arm.r.b].r), G[id("hand_r")].r);
  const d = qmul(rel, qinv(relG));
  const twist = norm([forearmAxis[0], forearmAxis[1], forearmAxis[2]]);
  const proj = dot([d[0], d[1], d[2]], twist);
  const angle = (2 * Math.atan2(proj, d[3]) * 180) / Math.PI;
  const tw = id("lowerarm_twist_01_r");
  l[tw] = { ...l[tw], r: qmul(l[tw].r, about(twist, angle * 0.5)) };
  // The shield keeps half the chest's turn, so it stays out in front of him rather than edge on.
  const half = qmul(slerpId(turned, 0.5), chestG.r);
  const handTarget = add(chest.t, add(rot(half, handLInChest.t), model(vec("hand_l", "at", t))));
  reach(l, arm.l, handTarget, hint("hand_l", arm.l), "arm_l");
  w = fk(l);
  setWorld(l, id("hand_l"), qmul(half, handLInChest.r), w);
  }
  // The feet: planted where the keys put them, the leg solved up to them.
  for (const side of ["r", "l"] as const) {
    const part = `foot_${side}`;
    const yaw = about([0, 1, 0], num(part, "yaw", t));
    const ballG = G[id(`ball_${side}`)];
    const footG = G[id(`foot_${side}`)];
    const ball = add(ballG.t, model(footAt(side, t)));
    const along = rot(yaw, sub(footG.t, ballG.t));
    const side_axis = rot(yaw, norm(cross(sub(footG.t, ballG.t), [0, 1, 0])));
    const heel = about(side_axis, -num(part, "heel", t) * (dot(cross(side_axis, along), [0, 1, 0]) > 0 ? -1 : 1));
    const ankle = add(ball, rot(heel, along));
    reach(l, leg[side], ankle, footPole(side, leg[side]), `leg_${side}`);
    w = fk(l);
    setWorld(l, id(`foot_${side}`), qmul(qmul(heel, yaw), footG.r), w);
    w = fk(l);
    setWorld(l, id(`ball_${side}`), qmul(yaw, ballG.r), w);
  }
  return l;
}
const footPole = (_side: string, k: Chain): V3 => k.pole;

// Sample it and write it as a new clip in a copy of the body.
const frames = Math.round(spec.end * FPS) + 1;
const times = Float32Array.from({ length: frames }, (_, i) => i / FPS);
const samples = Array.from({ length: frames }, (_, i) => pose(i / FPS));
const json = JSON.parse(JSON.stringify(g.gltf)) as any;
json.buffers[0] = json.buffers[0] ?? {};
let bin: Buffer = Buffer.from(g.bytes.subarray(g.bin));
function accessor(data: Float32Array, type: string, count: number, stats = false) {
  const pad = (4 - (bin.length % 4)) % 4;
  bin = Buffer.concat([bin, Buffer.alloc(pad), Buffer.from(data.buffer, data.byteOffset, data.byteLength)]);
  json.bufferViews.push({ buffer: 0, byteOffset: bin.length - data.byteLength, byteLength: data.byteLength });
  const a: any = { bufferView: json.bufferViews.length - 1, componentType: 5126, count, type };
  if (stats) Object.assign(a, { min: [data[0]], max: [data[data.length - 1]] });
  json.accessors.push(a);
  return json.accessors.length - 1;
}
const input = accessor(times, "SCALAR", frames, true);
const channels: any[] = [];
const samplers: any[] = [];
const tracked = [...guard.tracks.keys()].map((n) => g.named(n)).filter((i) => i >= 0);
for (const node of tracked) {
  for (const [path, key, size, type] of [["rotation", "r", 4, "VEC4"], ["translation", "t", 3, "VEC3"]] as const) {
    if (path === "translation" && !guard.tracks.get(nodes[node].name!)?.t) continue;
    const data = new Float32Array(frames * size);
    samples.forEach((s, i) => data.set(s[node][key], i * size));
    samplers.push({ input, output: accessor(data, type, frames), interpolation: "LINEAR" });
    channels.push({ sampler: samplers.length - 1, target: { node, path } });
  }
}
json.animations = json.animations.filter((a: any) => a.name !== spec.name);
json.animations.push({ name: spec.name, channels, samplers });
json.buffers[0].byteLength = bin.length + ((4 - (bin.length % 4)) % 4);
writeFileSync(out, glbBytes(json, bin));
const over = Object.entries(stretched).filter(([, v]) => v > 1).map(([k, v]) => `${k} ${(v * 100).toFixed(0)}%`);
console.log(`✓ ${spec.name}: ${keys.length} keys, ${spec.end} s, ${frames} frames${over.length ? `; ✗ past full reach: ${over.join(", ")}` : ""}`);
