// The shield arm fitted to each body after the clips (D554): every body shape plays the one
// fight.glb, whose shield arm hold.py's `shield` op solved on the base body, so a body whose arm
// differs (the female template's shoulder sits 5.4 cm in, her forearm is 2 cm shorter) carries
// the board centimetres off its pose's place against the chest. After the clips, the shoulder
// blade, upper arm and forearm are turned the least way that puts the strapped board back on
// its target of art/animation/shield_targets.json (the near targets blended by inverse distance,
// so a crossfade between two poses' places never snaps): wholly where the base body's arm, as
// the clip has it, holds the board within NEAR of a target, and not at all from FAR, so a pose
// with the board elsewhere (the block's raise) is left as its clip has it and every shape of one
// pose is pulled alike.
// It is the run-time correction shipped games lay over a clip for a body's proportions (Unreal's
// IK Rig retargeting and Control Rig hand IK): the bake gets the board close, this takes up the
// centimetres. The solve is hold.py's (a damped Gauss-Newton over each bone's turn in its own
// frame, 3 degrees weighing as 1 cm). Twin of terra_walker::shield_arm; change both.
// Node 22.18 or later; no dependencies.

import { type Glb, type Pose, type Q, type V3, apply, place, qinv, qmul, rot, sub } from "./glb.ts";
import { type Held, along } from "./held.ts";
import { type Place, placeOff, readBoard, readTargets } from "./shield_place.ts";

/** The bones turned, from the shoulder blade down, and what a turn of each costs per radian
 * against a centimetre off: the shoulder blade moves least, so it shrugs only when the arm
 * cannot reach. hold.py's COST. */
export const ARM = ["clavicle_l", "upperarm_l", "lowerarm_l"] as const;
export const COST = [0.4, 0.05, 0.05];
/** 3 degrees off the board's face or up weigh as 1 cm off its centre (the move check's bars). */
export const PER_RADIAN = 180 / (3 * Math.PI);
/** How far off its nearest target (cm, or degrees / 3, the larger) the base body's board is
 * pulled wholly onto it, and from how far not at all: Low_Guard's bake is 0.7 off, a strap moved
 * 2 cm about 3; the block's raise is 55 off. */
export const NEAR = 5;
export const FAR = 10;
/** The power of the off in the targets' blend (`between`): 4 keeps a strap moved 2 cm off the
 * guard's target within 0.2 cm of it with the strafes' 7 cm away. */
const SHEPARD = 4;
const STEPS = 12;

/** How far one place is from another in one number: cm, or degrees / 3, the larger. */
export const offBy = (a: Place, b: Place) => {
  const o = placeOff(a, b);
  return Math.max(o.cm, o.deg / 3);
};

/** The turn of a rotation vector (radians about its own way). */
function turnOf(v: number[]): Q {
  const a = Math.hypot(v[0], v[1], v[2]);
  if (a < 1e-12) return [0, 0, 0, 1];
  const s = Math.sin(a / 2) / a;
  return [v[0] * s, v[1] * s, v[2] * s, Math.cos(a / 2)];
}

/** A node's own pose under its parent's, from both in the body. */
function localOf(parent: Pose, world: Pose): Pose {
  const t = rot(qinv(parent.r), sub(world.t, parent.t));
  return { t: [t[0] / parent.s[0], t[1] / parent.s[1], t[2] / parent.s[2]], r: qmul(qinv(parent.r), world.r), s: [world.s[0] / parent.s[0], world.s[1] / parent.s[1], world.s[2] / parent.s[2]] };
}

/** `a` x = `b` for a square `a`, by elimination with partial pivoting. */
export function solveLinear(a: number[][], b: number[]): number[] {
  const n = b.length;
  const m = a.map((row, i) => [...row, b[i]]);
  for (let c = 0; c < n; c++) {
    let p = c;
    for (let r = c + 1; r < n; r++) if (Math.abs(m[r][c]) > Math.abs(m[p][c])) p = r;
    [m[c], m[p]] = [m[p], m[c]];
    for (let r = c + 1; r < n; r++) {
      const k = m[r][c] / m[c][c];
      for (let j = c; j <= n; j++) m[r][j] -= k * m[c][j];
    }
  }
  const x = new Array(n).fill(0);
  for (let r = n - 1; r >= 0; r--) {
    let s = m[r][n];
    for (let j = r + 1; j < n; j++) s -= m[r][j] * x[j];
    x[r] = s / m[r][r];
  }
  return x;
}

/** The arm as the solve sees it: the shoulder blade's parent in the body, the three bones' own
 * poses, and the strap's frame in the forearm's (held.ts `frameOf`, the true forearm at the fist). */
export type Arm = { parent: Pose; locals: Pose[]; strap: Pose };

/** The strapped board (`board`, in the strap's frame: centre in metres from the fist, face, up)
 * in the body with the bones turned by `x` (a rotation vector a bone, in its own frame). */
export function boardAt(arm: Arm, board: Place, x: number[] = new Array(9).fill(0)): Place {
  let w = arm.parent;
  arm.locals.forEach((l, k) => (w = apply(w, { ...l, r: qmul(l.r, turnOf(x.slice(3 * k, 3 * k + 3))) })));
  const f = { t: place(w, arm.strap.t), r: qmul(w.r, arm.strap.r) };
  const c = rot(f.r, board.centre);
  return { centre: [f.t[0] + c[0], f.t[1] + c[1], f.t[2] + c[2]], face: rot(f.r, board.face), up: rot(f.r, board.up) };
}

/** The bones' turns (a rotation vector a bone, in its own frame) that put the board on `want`
 * (in the body), the least way: hold.py's solve. */
export function solveArm(arm: Arm, board: Place, want: Place): number[] {
  const residual = (x: number[]) => {
    const b = boardAt(arm, board, x);
    const r: number[] = [];
    for (let j = 0; j < 3; j++) r.push(100 * (b.centre[j] - want.centre[j]));
    for (let j = 0; j < 3; j++) r.push(PER_RADIAN * (b.face[j] - want.face[j]));
    for (let j = 0; j < 3; j++) r.push(PER_RADIAN * (b.up[j] - want.up[j]));
    for (let j = 0; j < 9; j++) r.push(COST[Math.floor(j / 3)] * x[j]);
    return r;
  };
  let x = new Array(9).fill(0);
  for (let step = 0; step < STEPS; step++) {
    const r = residual(x);
    const J = r.map(() => new Array(9).fill(0));
    for (let j = 0; j < 9; j++) {
      const e = [...x];
      e[j] += 1e-6;
      const rj = residual(e);
      for (let i = 0; i < r.length; i++) J[i][j] = (rj[i] - r[i]) / 1e-6;
    }
    const A = Array.from({ length: 9 }, (_, a) => Array.from({ length: 9 }, (_, b) => J.reduce((s, row) => s + row[a] * row[b], 0) + (a === b ? 1e-6 : 0)));
    const g = Array.from({ length: 9 }, (_, a) => -J.reduce((s, row, i) => s + row[a] * r[i], 0));
    const d = solveLinear(A, g);
    x = x.map((v, j) => v + d[j]);
    if (Math.hypot(...d) < 1e-7) break;
  }
  return x;
}

/** How wholly a board `off` (offBy) from its target is pulled onto it: 1 within NEAR, 0 from FAR. */
export function weightOf(off: number): number {
  const s = Math.min(1, Math.max(0, (off - NEAR) / (FAR - NEAR)));
  return 1 - s * s * (3 - 2 * s);
}

/** The targets blended by Shepard's inverse distance weighting (each by 1 / off^SHEPARD), so a
 * board on one target goes to it and a board crossfading between two near ones (the guard's and
 * the strafes', 7 cm apart, both in the pull) is pulled along between them instead of snapping
 * from one to the other halfway. */
export function between(wants: Place[], offs: number[]): Place {
  const by = offs.map((d) => 1 / Math.max(d, 1e-6) ** SHEPARD);
  const all = by.reduce((a, b) => a + b, 0);
  const sum = (f: (p: Place) => V3): V3 => {
    const s: V3 = [0, 0, 0];
    wants.forEach((p, i) => f(p).forEach((c, j) => (s[j] += (c * by[i]) / all)));
    return s;
  };
  const unit = (v: V3): V3 => {
    const n = Math.hypot(...v);
    return [v[0] / n, v[1] / n, v[2] / n];
  };
  const face = unit(sum((p) => p.face));
  const up = sum((p) => p.up);
  const d = up[0] * face[0] + up[1] * face[1] + up[2] * face[2];
  return { centre: sum((p) => p.centre), face, up: unit([up[0] - face[0] * d, up[1] - face[1] * d, up[2] - face[2] * d]) };
}

/** The strap's frame (held.ts `frameOf`: the true forearm's, at the fist) in the forearm's own,
 * from the forearm's, the hand's and the fist's poses (in any one frame). */
function strapIn(lower: Pose, hand: Pose, fist: Pose): Pose {
  const wrist = rot(qinv(lower.r), sub(hand.t, lower.t));
  const t = rot(qinv(lower.r), sub(fist.t, lower.t));
  return { t: [t[0] / lower.s[0], t[1] / lower.s[1], t[2] / lower.s[2]], r: along(wrist), s: [1, 1, 1] };
}

/** A fitter for `body` with `shield` strapped: a pose of every node and the body's shape (its
 * joints moved from the base's, skin.ts) in, the pose with the shield arm fitted out (the same
 * pose if the shield's strap is an older one, at a place on the bone, which the game of then
 * did not fit). How far off its target the board is, which says how wholly it is pulled on, is
 * read on the base body's arm (the clip's own joints), so every shape of a pose is pulled alike.
 * `board` and `targets` default to shield_targets.json's. */
export function fitter(body: Glb, rest: Pose[], shield: Held, o: { board?: Place; targets?: Place[] } = {}) {
  const board = o.board ?? readBoard();
  const targets = o.targets ?? Object.values(readTargets());
  const named = (n: string) => body.named(n);
  const parentOf = (i: number) => body.parentOf[i];
  const bones = ARM.map(named);
  const [spine, hand, fist] = ["spine_03", "hand_l", shield.strap?.fist ?? "socket.hand.L"].map(named);
  // The arm a chain from the chest down to the fist.
  const chain = [spine, ...bones, hand, fist];
  chain.slice(1).forEach((i, k) => {
    if (parentOf(i) !== chain[k]) throw new Error(`${body.gltf.nodes[i].name}'s parent is not ${body.gltf.nodes[chain[k]].name}`);
  });
  // Every node below the shoulder blade, each after its parent.
  const below: number[] = [];
  const walk = (i: number) => (below.push(i), (body.gltf.nodes[i].children ?? []).forEach(walk));
  walk(bones[0]);
  return (pose: Pose[], moved: Map<number, V3> = new Map()): Pose[] => {
    if (!shield.strap?.fist || !board || !targets.length) return pose;
    const C = { t: pose[spine].t, r: qmul(pose[spine].r, qinv(rest[spine].r)) };
    const inBody = (p: Place): Place => ({ centre: place({ ...C, s: [1, 1, 1] }, p.centre), face: rot(C.r, p.face), up: rot(C.r, p.up) });
    const wants = targets.map(inBody);
    const parent = pose[parentOf(bones[0])];
    const locals = bones.map((b) => localOf(pose[parentOf(b)], pose[b]));
    const arm: Arm = { parent, locals, strap: strapIn(pose[bones[2]], pose[hand], pose[fist]) };
    // The base body's arm: each joint where the clip has it, without the shape's move.
    const base = (i: number, p: Pose): Pose => {
      const l = localOf(pose[parentOf(i)], pose[i]);
      const by = moved.get(i) ?? [0, 0, 0];
      return apply(p, { ...l, t: [l.t[0] - by[0], l.t[1] - by[1], l.t[2] - by[2]] });
    };
    const spineAt = pose[spine];
    let at = spineAt;
    const baseLocals = bones.map((b) => {
      const w = base(b, at);
      const l = localOf(at, w);
      at = w;
      return l;
    });
    const baseHand = base(hand, at);
    const baseArm: Arm = { parent: spineAt, locals: baseLocals, strap: strapIn(at, baseHand, base(fist, baseHand)) };
    // The base body's board, its chest at this body's (the board's place is in the chest's frame).
    const offs = wants.map((w) => offBy(boardAt(baseArm, board), w));
    const w = weightOf(Math.min(...offs));
    if (w <= 0) return pose;
    const x = solveArm(arm, board, between(wants, offs)).map((v) => v * w);
    const out = [...pose];
    const turned = new Map(bones.map((b, j) => [b, { ...locals[j], r: qmul(locals[j].r, turnOf(x.slice(3 * j, 3 * j + 3))) }]));
    for (const i of below) out[i] = apply(out[parentOf(i)], turned.get(i) ?? localOf(pose[parentOf(i)], pose[i]));
    return out;
  };
}
