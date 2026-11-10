#!/usr/bin/env node
// A clip from key poses on our skeleton, no generator in between (D588, docs/keyed-moves.md): the
// same keys give the same clip, every key starts from the guard, and a foot not told to move does
// not move.
//
//   node tools/characters/key.ts KEYS.json OUT.glb
//
// OUT.glb is the body with that one clip, a take as made_clips.py reads one: an entry of
// art/animation/made.json with `"keys"` is made so by the build.
//
// KEYS.json is { name, body, guard, end, keys: [{ t, ...controls }] }. `body` is the file it is
// solved on (the mannequin unless it says; a blow keyed against the fight's guard names
// fight.glb). `guard` is the pose every key starts from: a clip of the body (its first frame), or
// a pose file beside the keys (`*.json`, { bones: { NAME: { r, t? } } }, each bone's local turn
// and place in the body's nodes), so a clip keyed from another need not be made after it. A
// control a key leaves out holds its last value. All places are [right, up, forward] in metres
// from the model's origin, all directions the same axes; degrees are yaw + to his left, pitch +
// forward, roll + to his right.
//   pelvis {move, yaw, pitch, roll}      from the guard, in the model
//   chest  {yaw, pitch, roll}            over the pelvis, spread up the three spine bones
//   head   {yaw, pitch}                  from the guard, in the model
//   hand_r {at, blade, edge, elbow}      wrist place, tip's way, leading edge's way, elbow's way
//   hand_l {at, elbow}                   an offset; else it rides half the chest's turn
//   clav_r / clav_l {up, forward}        the shoulder lifted and pushed forward
//   foot_r / foot_l {at, yaw, heel}      the ball moved, turned, the heel raised over the ball
//   arm_r / arm_l {forward, out, elbow}  with `hang`: empty arms hang as `rest_arm` sets them, swayed
//                                        by these degrees; hand_r and hand_l are unused
// `rest_arm` {out, forward, turn_in, elbow, palm} poses both empty arms afresh from the bind pose,
// nothing kept from the guard, the same both sides: the upper arm degrees out from straight down
// and forward of it, turned in about its length (the forearm swings toward the thigh's front),
// the elbow bent, the palm rolled from facing the thigh (+ toward the front). `fingers` is the
// degrees each finger curls over its three joints, the little finger most (the resting cascade);
// `thumb` the degrees its last joint bends.
// `fists` keys two empty hands by place, for an unarmed blow on any body of our skeleton (a
// creature's own: the troll's smash): hand_r and hand_l are each {at, elbow}, the wrist's place and
// the elbow's way, starting where the guard holds them; the hand rides its forearm as the guard
// has it, and grip {closed} takes its fingers from the guard's (0) to `fingers` curled (1). No
// blade, no shield, no hand socket needed.
// `step_lift` is how high a stepping foot rises at mid-step, metres (0.05 unless said; a 4 m body lifts more).
// `loop` makes the curve run on through the seam; the last key must equal the first.
// `start` {pelvis, chest, head, stance, turn_out, knees} sets the start pose afresh (below, where it is made).

import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { apply, dot, Glb, len, qinv, qmul, rot, sub, type Pose, type Q, type V3 } from "./glb.ts";
import { glbBytes } from "./glb_clips.ts";

export const FPS = 60;
export const MANNEQUIN = "assets/models/character/mannequin.glb";
const add = (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const scale = (a: V3, k: number): V3 => [a[0] * k, a[1] * k, a[2] * k];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a: V3): V3 => scale(a, 1 / (len(a) || 1));
const deg = (r: number) => (r * 180) / Math.PI;
const rad = (d: number) => (d * Math.PI) / 180;
// Our axes to the model's: he faces +Z, his right is -X.
const model = ([r, u, f]: number[]): V3 => [-r, u, f];
const about = (axis: V3, d: number): Q => {
  const h = (d * Math.PI) / 360;
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

/// Monotone cubic through the keys (Fritsch-Carlson): eases in and out of holds and never
/// overshoots, so a pose keyed is the pose reached. A loop's ends are one key, so its speed runs
/// on through the seam.
export function curve(ts: number[], ys: number[], loop = false): (t: number) => number {
  const n = ts.length;
  const d = ts.slice(1).map((t, i) => (ys[i + 1] - ys[i]) / (t - ts[i]));
  const m = ys.map((_, i) => {
    const end = i === 0 || i === n - 1;
    if (end && !loop) return 0;
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

export type Keyed = {
  name: string;
  /// Every frame's local pose of every node, at FPS.
  samples: Pose[][];
  /// The nodes the clip turns, and those of them it also places.
  tracked: number[];
  placed: Set<number>;
  keys: number;
  /// Each limb reached for past its full length, as a share of it.
  over: string[];
};

/// The guard every key starts from: a clip's first frame, or a pose file's bones.
type Part = { t?: V3; r?: Q; s?: V3 };
function guardOf(g: Glb, guard: string, dir: string): Map<string, Part> {
  if (guard.endsWith(".json")) {
    const file = JSON.parse(readFileSync(join(dir, guard), "utf8"));
    return new Map(Object.entries(file.bones as Record<string, Part>));
  }
  const clip = g.clip(guard);
  if (!clip) throw new Error(`key: no clip ${guard} in the body; name a clip it holds (${g.clipNames().slice(0, 6).join(", ")}, ...) or a pose file beside the keys (\`*.json\`)`);
  return new Map([...clip.tracks].map(([n, tr]) => [n, { r: tr.r?.(0) as Q | undefined, t: tr.t?.(0) as V3 | undefined, s: tr.s?.(0) as V3 | undefined }]));
}

/// The clip `spec` keys, solved on `g`; `dir` is where the keys file is (a pose file's place).
export function keyClip(spec: any, g: Glb, dir = "."): Keyed {
  const nodes = g.gltf.nodes;
  const id = (n: string) => {
    const i = g.named(n);
    if (i < 0) throw new Error(`key: no bone ${n} in ${spec.body ?? MANNEQUIN}; keys are solved on our skeleton`);
    return i;
  };
  const has = (n: string) => g.named(n) >= 0;
  const guard = guardOf(g, spec.guard, dir);
  const base: Pose[] = nodes.map((n, i) => {
    const at = n.name ? guard.get(n.name) : undefined;
    const rest = g.rest(i);
    return { t: at?.t ?? rest.t, r: at?.r ?? rest.r, s: at?.s ?? rest.s };
  });
  function fk(l: Pose[]): Pose[] {
    const w: Pose[] = [];
    const at = (i: number): Pose => (w[i] ??= g.parentOf[i] === undefined ? l[i] : apply(at(g.parentOf[i]), l[i]));
    l.forEach((_, i) => at(i));
    return w;
  }
  function setWorld(l: Pose[], i: number, r: Q, w: Pose[]) {
    l[i] = { ...l[i], r: qmul(qinv(w[g.parentOf[i]].r), r) };
  }
  function bend(l: Pose[], i: number, q: Q, w: Pose[]) {
    const p = w[g.parentOf[i]].r;
    l[i] = { ...l[i], r: qmul(qmul(qmul(qinv(p), q), p), l[i].r) };
  }
  const perp = (v: V3, along: V3): V3 => sub(v, scale(along, dot(v, along)));

  // A new start pose (`start`): the guard's first frame with the chest, head and feet set afresh,
  // so a loop needn't inherit a turned head or a crooked stance from the clip it began on. Its
  // hips, chest and head are degrees turned from the bind pose, in the model, a field left out
  // keeping the guard's (the hips' roll levelled); the arms keep the way they hung in the guard.
  const spread: Record<string, V3> = { r: [0, 0, 0], l: [0, 0, 0] };
  const turnOut: Record<string, number> = { r: 0, l: 0 };
  const heelUp: Record<string, number> = { r: 0, l: 0 };
  let rise = 0;
  const bind = fk(nodes.map((_, i) => g.rest(i)));
  if (spec.start) {
    const s = spec.start;
    const before = fk(base);
    /// A world turn from the bind pose as yaw, pitch and roll, the inverse of `euler`.
    const angles = (q: Q) => {
      const [x, y, f] = [rot(q, [1, 0, 0]), rot(q, [0, 1, 0]), rot(q, [0, 0, 1])];
      return { yaw: deg(Math.atan2(f[0], f[2])), pitch: deg(-Math.asin(Math.max(-1, Math.min(1, f[1])))), roll: deg(Math.atan2(x[1], y[1])) };
    };
    /// The turn that takes bone `i` from where it stands to the angles `want` names.
    const toward = (i: number, want: Record<string, number>): Q => {
      const w = fk(base)[i].r;
      const a = { ...angles(qmul(w, qinv(bind[i].r))), ...want };
      return qmul(qmul(euler(a.yaw, a.pitch, a.roll), bind[i].r), qinv(w));
    };
    if (s.pelvis) bend(base, id("pelvis"), toward(id("pelvis"), s.pelvis), fk(base));
    if (s.chest) {
      const c = toward(id("spine_03"), s.chest);
      for (const name of ["spine_01", "spine_02", "spine_03"]) bend(base, id(name), slerpId(c, 1 / 3), fk(base));
    }
    if (s.head) {
      bend(base, id("neck_01"), slerpId(toward(id("Head"), s.head), 0.5), fk(base));
      bend(base, id("Head"), toward(id("Head"), s.head), fk(base));
    }
    for (const side of ["r", "l"]) setWorld(base, id(`upperarm_${side}`), before[id(`upperarm_${side}`)].r, fk(base));
    // The stance: the balls of the feet this far apart, metres, centred under the hips, level front
    // to back (the guard's stagger dropped), measured after the hips are set, and flat on the floor:
    // each ball at the bind pose's height and each sole at its pitch (the guard held the right ball
    // 1 cm up, so that knee bent 23 to the left's 15, and both heels 6 degrees down into the floor).
    if (s.stance) {
      const now = fk(base);
      const middle = (now[id("thigh_r")].t[0] + now[id("thigh_l")].t[0]) / 2;
      const ahead = (now[id("ball_r")].t[2] + now[id("ball_l")].t[2]) / 2;
      const pitch = (w: Pose[], side: string) => {
        const d = sub(w[id(`foot_${side}`)].t, w[id(`ball_${side}`)].t);
        return deg(Math.atan2(d[1], Math.hypot(d[0], d[2])));
      };
      // He faces +Z, so his right is -X.
      for (const [side, sign] of [["r", -1], ["l", 1]] as const) {
        const ball = now[id(`ball_${side}`)].t;
        spread[side] = [middle + (sign * s.stance) / 2 - ball[0], bind[id(`ball_${side}`)].t[1] - ball[1], ahead - ball[2]];
        heelUp[side] = pitch(bind, side) - pitch(now, side);
      }
    }
    // The turn-out: each foot's toes this many degrees out from straight ahead, both alike, along
    // the ankle-to-ball line (the bind's runs with the sole's long axis, 13 to 15 out, while the
    // ball bone's own axis points straight ahead). A person standing at ease turns each about 7
    // (McIlroy and Maki 1997, 14 between the feet).
    if (s.turn_out !== undefined) {
      const now = fk(base);
      for (const [side, out] of [["r", -1], ["l", 1]] as const) {
        const d = sub(now[id(`ball_${side}`)].t, now[id(`foot_${side}`)].t);
        // He faces +Z and his left is +X; yaw is + to his left, so the right foot turns out by -.
        const turned = deg(Math.atan2(out * d[0], d[2]));
        turnOut[side] = out * (s.turn_out - turned);
      }
    }
  }
  const G = fk(base);

  // Every key's controls, each carrying the last value of what it leaves out. The sword hand's
  // start is the guard's grip, read from the hand's socket, which only a body with a held hand has.
  type Controls = Record<string, Record<string, number | number[]>>;
  const grip = (side: "r" | "l") => {
    if (spec.hang) return { at: [0, 0, 0], blade: [0, 1, 0], edge: [0, 0, 1] };
    const wrist = G[id(`hand_${side}`)].t;
    if (spec.fists) return { at: [-wrist[0], wrist[1], wrist[2]], blade: [0, 1, 0], edge: [0, 0, 1] };
    const socket = G[id(`socket.hand.${side.toUpperCase()}`)];
    return { at: [-wrist[0], wrist[1], wrist[2]], blade: rot(socket.r, [0, 1, 0]).map((v, i) => (i ? v : -v)), edge: rot(socket.r, [0, 0, -1]).map((v, i) => (i ? v : -v)) };
  };
  const start: Controls = {
    pelvis: { move: [0, 0, 0], yaw: 0, pitch: 0, roll: 0 },
    chest: { yaw: 0, pitch: 0, roll: 0 },
    head: { yaw: 0, pitch: 0 },
    hand_r: { ...grip("r"), elbow: [0, 0, 0] },
    hand_l: { at: spec.fists ? grip("l").at : [0, 0, 0], elbow: [0, 0, 0] },
    clav_r: { up: 0, forward: 0 },
    clav_l: { up: 0, forward: 0 },
    arm_r: { forward: 0, out: 0, elbow: 0 },
    arm_l: { forward: 0, out: 0, elbow: 0 },
    grip: { closed: 0 },
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

  const ts = keys.map((k) => k.t);
  const curves = new Map<string, (t: number) => number>();
  const value = (part: string, field: string, i: number, t: number): number => {
    const name = `${part}.${field}.${i}`;
    let f = curves.get(name);
    if (!f) {
      f = curve(ts, keys.map((k) => {
        const v = k.c[part][field];
        return Array.isArray(v) ? v[i] : v;
      }), !!spec.loop);
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
    if (Math.hypot(b[0] - a[0], b[2] - a[2]) > 1e-4) p[1] += (spec.step_lift ?? 0.05) * Math.sin(Math.PI * u);
    return p;
  }

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
  const stretchedAt: Record<string, number> = {};
  let solving = 0;
  function reach(l: Pose[], k: Chain, target: V3, pole: V3, name: string) {
    let w = fk(l);
    const S = w[k.a].t;
    const full = (k.la + k.lb) * 0.995;
    let d = len(sub(target, S));
    if (d / (k.la + k.lb) > (stretched[name] ?? 0)) [stretched[name], stretchedAt[name]] = [d / (k.la + k.lb), solving];
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
  const chestG = G[id("spine_03")];
  const handLInChest = { t: rot(qinv(chestG.r), sub(G[id("hand_l")].t, chestG.t)), r: qmul(qinv(chestG.r), G[id("hand_l")].r) };
  const forearmAxis = norm(g.rest(id("hand_r")).t);

  // The resting arm (`rest_arm`, `fingers`), built from the bind pose so nothing of the guard's arm
  // survives. In the bind pose the arm lies out to the side, palm down, and its elbow bends toward
  // the front; the same hinge carried down the side gives the palm facing the thigh, thumb forward.
  const FINGERS: [string, number][] = [["index", 0.8], ["middle", 1], ["ring", 1.1], ["pinky", 1.25]];
  const JOINT_SHARE = [0.35, 0.45, 0.2];
  function restArm(l: Pose[], side: "r" | "l", turned: Q) {
    const a = { out: 0, forward: 0, turn_in: 0, elbow: 0, palm: 0, ...spec.rest_arm };
    const sign = side === "r" ? -1 : 1;
    const b = (n: string) => id(`${n}_${side}`);
    const f = rot(turned, [0, 0, 1]);
    const ahead = norm([f[0], 0, f[2]]);
    const outward = scale(cross([0, 1, 0], ahead), sign);
    // The upper arm's way, and the way its elbow bends: ahead, turned in toward the thigh's front.
    const D = norm(add(add([0, -1, 0], scale(outward, Math.tan(rad(a.out)))), scale(ahead, Math.tan(rad(a.forward)))));
    const front = norm(perp(ahead, D));
    const inward = norm(perp(scale(outward, -1), D));
    const F = norm(add(scale(front, Math.cos(rad(a.turn_in))), scale(inward, Math.sin(rad(a.turn_in)))));
    const A0 = norm(sub(bind[b("lowerarm")].t, bind[b("upperarm")].t));
    const F0 = norm(perp([0, 0, 1], A0));
    const R = qmul(basis(D, F, cross(D, F)), qinv(basis(A0, F0, cross(A0, F0))));
    setWorld(l, b("upperarm"), qmul(R, bind[b("upperarm")].r), fk(l));
    // The bind pose's elbow is already a little bent; `elbow` is the whole bend.
    const fore0 = norm(sub(bind[b("hand")].t, bind[b("lowerarm")].t));
    const bent0 = deg(Math.acos(Math.max(-1, Math.min(1, dot(A0, fore0)))));
    const E = qmul(about(cross(D, F), a.elbow - bent0), R);
    setWorld(l, b("lowerarm"), qmul(E, bind[b("lowerarm")].r), fk(l));
    // The palm rolls with the forearm, half of it up the twist bone, as a forearm rolls (a body
    // without twist bones, the mannequin, gets them from twist.py as each body is exported).
    const roll = -sign * a.palm;
    setWorld(l, b("hand"), qmul(about(rot(E, A0), roll), qmul(E, bind[b("hand")].r)), fk(l));
    if (has(`lowerarm_twist_01_${side}`)) {
      for (const n of ["upperarm_twist_01", "lowerarm_twist_01"]) l[b(n)] = { ...l[b(n)], r: g.rest(b(n)).r };
      l[b("lowerarm_twist_01")].r = qmul(l[b("lowerarm_twist_01")].r, about(g.rest(b("hand")).t, roll * 0.5));
    }
    curl(l, side);
  }
  /// The fingers from flat, each curled toward the palm over its three joints (`fingers`), and
  /// the thumb's last joint bent (`thumb`).
  function curl(l: Pose[], side: "r" | "l") {
    const b = (n: string) => id(`${n}_${side}`);
    if (spec.fingers === undefined) return;
    for (const [finger] of FINGERS) {
      for (let j = 0; j < 3; j++) {
        const bone = b(`${finger}_0${j + 1}`);
        l[bone] = { ...l[bone], r: g.rest(bone).r };
      }
    }
    for (const [finger, k] of FINGERS) {
      for (let j = 0; j < 3; j++) {
        const w = fk(l);
        const at = (n: string) => w[b(n)].t;
        const along = norm(sub(at(`middle_01`), at("hand")));
        const thumbward = norm(sub(at("index_01"), at("pinky_01")));
        const palm = side === "r" ? cross(thumbward, along) : cross(along, thumbward);
        const bone = b(`${finger}_0${j + 1}`);
        const next = j < 2 ? b(`${finger}_0${j + 2}`) : b(`${finger}_04_leaf`);
        const dir = norm(sub(w[next].t, w[bone].t));
        bend(l, bone, about(cross(dir, palm), spec.fingers * k * JOINT_SHARE[j]), w);
      }
    }
    // The thumb keeps the guard's base and middle joint, which lay its tip against the side of the
    // index finger as a resting hand's does, but the guard holds its last joint straight (3 to 8
    // degrees); `thumb` bends it this much, in the plane of the middle joint's bend.
    if (spec.thumb === undefined) return;
    const w = fk(l);
    const at = (n: string) => w[b(n)].t;
    const [d1, d2, d3] = [sub(at("thumb_02"), at("thumb_01")), sub(at("thumb_03"), at("thumb_02")), sub(at("thumb_04_leaf"), at("thumb_03"))];
    const bent = deg(Math.acos(Math.max(-1, Math.min(1, dot(norm(d2), norm(d3))))));
    bend(l, b("thumb_03"), about(cross(d1, d2), spec.thumb - bent), w);
  }

  function pose(t: number): Pose[] {
    const l = base.map((p) => ({ ...p }));
    // The hips, then the chest over them.
    let w = fk(l);
    const pel = id("pelvis");
    const moved = add(add(G[pel].t, [0, rise, 0]), model(vec("pelvis", "move", t)));
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
    if (spec.hang) {
      // Empty arms hang as `rest_arm` poses them, a little sway laid on at the shoulder and the elbow.
      for (const side of ["r", "l"] as const) {
        const sign = side === "r" ? -1 : 1;
        const across = rot(turned, [1, 0, 0]);
        if (spec.rest_arm) restArm(l, side, turned);
        w = fk(l);
        bend(l, id(`upperarm_${side}`), qmul(about(across, -num(`arm_${side}`, "forward", t)), about(rot(turned, [0, 0, 1]), sign * num(`arm_${side}`, "out", t))), w);
        w = fk(l);
        bend(l, id(`lowerarm_${side}`), about(across, -num(`arm_${side}`, "elbow", t)), w);
      }
    } else if (spec.fists) {
      // Two empty fists, each wrist where its key puts it, the hand carried on the forearm as
      // the guard holds it and its fingers closed.
      for (const side of ["r", "l"] as const) {
        const v = vec(`hand_${side}`, "elbow", t);
        reach(l, arm[side], model(vec(`hand_${side}`, "at", t)), len(v) > 1e-3 ? model(v) : rot(turned, arm[side].pole), `arm_${side}`);
        // The fingers go from the guard's to closed by `grip.closed`, so the guard key is the guard.
        const k = num("grip", "closed", t);
        if (k <= 0 || spec.fingers === undefined) continue;
        const closed = l.map((p) => ({ ...p }));
        curl(closed, side);
        for (const bone of [...FINGERS.flatMap(([f]) => [1, 2, 3].map((j) => `${f}_0${j}_${side}`)), `thumb_03_${side}`]) {
          const [a, b] = [l[id(bone)].r, closed[id(bone)].r];
          const sign = dot([a[0], a[1], a[2]], [b[0], b[1], b[2]]) + a[3] * b[3] < 0 ? -1 : 1;
          const m = a.map((x, i) => x + (b[i] * sign - x) * Math.min(1, k));
          const n = Math.hypot(...m);
          l[id(bone)] = { ...l[id(bone)], r: m.map((x) => x / n) as Q };
        }
      }
    } else {
      // The sword hand goes to its place and points the blade; the shield rides the chest.
      const hint = (part: string, k: Chain): V3 => {
        const v = vec(part, "elbow", t);
        return len(v) > 1e-3 ? model(v) : rot(turned, k.pole);
      };
      reach(l, arm.r, model(vec("hand_r", "at", t)), hint("hand_r", arm.r), "arm_r");
      w = fk(l);
      const blade = norm(model(vec("hand_r", "blade", t)));
      const leading = norm(perp(model(vec("hand_r", "edge", t)), blade));
      const socket = basis(cross(blade, scale(leading, -1)), blade, scale(leading, -1));
      const socketInHand = qmul(qinv(G[id("hand_r")].r), G[id("socket.hand.R")].r);
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
      const yaw = about([0, 1, 0], num(part, "yaw", t) + turnOut[side]);
      const ballG = G[id(`ball_${side}`)];
      const footG = G[id(`foot_${side}`)];
      const ball = add(add(ballG.t, spread[side]), model(footAt(side, t)));
      const along = rot(yaw, sub(footG.t, ballG.t));
      const sideAxis = rot(yaw, norm(cross(sub(footG.t, ballG.t), [0, 1, 0])));
      const heel = about(sideAxis, -(num(part, "heel", t) + heelUp[side]) * (dot(cross(sideAxis, along), [0, 1, 0]) > 0 ? -1 : 1));
      const ankle = add(ball, rot(heel, along));
      reach(l, leg[side], ankle, leg[side].pole, `leg_${side}`);
      w = fk(l);
      setWorld(l, id(`foot_${side}`), qmul(qmul(heel, yaw), footG.r), w);
      w = fk(l);
      setWorld(l, id(`ball_${side}`), qmul(yaw, ballG.r), w);
    }
    return l;
  }

  // The knees (`start.knees`): with the soles flat the guard's hips stand low enough to bend both
  // knees 26 degrees, a crouch; the hips rise (or drop) until the start's knees bend this much.
  if (spec.start?.knees !== undefined) {
    const bent = () => {
      const w = fk(pose(0));
      const at = (n: string) => w[id(n)].t;
      const knee = (s: string) => deg(Math.acos(Math.max(-1, Math.min(1, dot(norm(sub(at(`calf_${s}`), at(`thigh_${s}`))), norm(sub(at(`foot_${s}`), at(`calf_${s}`))))))));
      return (knee("r") + knee("l")) / 2;
    };
    let [lo, hi] = [-0.05, 0.05];
    for (let k = 0; k < 30; k++) {
      rise = (lo + hi) / 2;
      if (bent() > spec.start.knees) lo = rise;
      else hi = rise;
    }
    for (const k of Object.keys(stretched)) delete stretched[k];
  }

  const frames = Math.round(spec.end * FPS) + 1;
  const samples = Array.from({ length: frames }, (_, i) => pose((solving = i / FPS)));
  const tracked = [...guard.keys()].map((n) => g.named(n)).filter((i) => i >= 0);
  const placed = new Set(tracked.filter((i) => guard.get(nodes[i].name!)?.t));
  const over = Object.entries(stretched).filter(([, v]) => v > 1).map(([k, v]) => `${k} ${(v * 100).toFixed(0)}% at ${stretchedAt[k].toFixed(2)} s`);
  return { name: spec.name, samples, tracked, placed, keys: keys.length, over };
}

/// The body with only the keyed clip: a take, as made_clips.py reads one.
export function takeBytes(g: Glb, k: Keyed): Uint8Array {
  const frames = k.samples.length;
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
  const input = accessor(Float32Array.from({ length: frames }, (_, i) => i / FPS), "SCALAR", frames, true);
  const channels: any[] = [];
  const samplers: any[] = [];
  for (const node of k.tracked) {
    for (const [path, key, size, type] of [["rotation", "r", 4, "VEC4"], ["translation", "t", 3, "VEC3"]] as const) {
      if (path === "translation" && !k.placed.has(node)) continue;
      const data = new Float32Array(frames * size);
      k.samples.forEach((s, i) => data.set(s[node][key], i * size));
      samplers.push({ input, output: accessor(data, type, frames), interpolation: "LINEAR" });
      channels.push({ sampler: samplers.length - 1, target: { node, path } });
    }
  }
  json.animations = [{ name: k.name, channels, samplers }];
  json.buffers[0].byteLength = bin.length + ((4 - (bin.length % 4)) % 4);
  return glbBytes(json, bin);
}

if (import.meta.main) {
  const [keysFile, out] = process.argv.slice(2);
  if (!out) throw new Error("key: give KEYS.json OUT.glb (art/animation/keyed/ holds the keys)");
  const spec = JSON.parse(readFileSync(keysFile, "utf8"));
  const g = new Glb(spec.body ?? MANNEQUIN);
  const k = keyClip(spec, g, dirname(keysFile));
  writeFileSync(out, takeBytes(g, k));
  console.log(`✓ ${k.name}: ${k.keys} keys, ${spec.end} s, ${k.samples.length} frames${k.over.length ? `; ✗ past full reach: ${k.over.join(", ")}` : ""}`);
}
