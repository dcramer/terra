// The wrist and the edge of a hand that swings what it holds (D565 step 3, docs/assembled-items.md
// "Through the animations"; research/2026-10-28-assembled-items-animation.md): how far the hand
// bends off its forearm, how far it turns in one key of the take, and which side of the held look
// leads its tip's motion. Pure: poses in, numbers out; moves.ts measures them on the composed pose.
//
// The bend is the hand's line (the wrist to the middle knuckle) against the forearm's (the elbow
// to the wrist), split into flexion (toward the palm) and extension, radial deviation (toward the
// thumb) and ulnar, each relative to the body's rest, where the hand is straight. Twist about the
// forearm (pronation) is the forearm's, not the wrist's, and is not counted in the bend; a key's
// turn counts the whole of the hand's turn in the forearm's frame.

import { type Pose, type Q, type V3, dot, len, qinv, qmul, rot, sub } from "./glb.ts";

/** A person's wrist (AAOS ranges, adult averages): degrees of flexion, extension, radial and
 * ulnar deviation, and the most a hand turns off its forearm in one key at 24 a second. */
export const WRIST = { flex: 80, ext: 70, radial: 20, ulnar: 35, key: 45, fps: 24 } as const;
/** The share of a blow's tip motion across the handle its working edge must lead at speed
 * (attachment.md: a one-sided head's -Z), and the most a blade's flat may. */
export const EDGE = { lead: 0.7, flat: 0.5 } as const;

const unit = (v: V3): V3 => {
  const l = len(v) || 1;
  return [v[0] / l, v[1] / l, v[2] / l];
};
const deg = (rad: number) => (rad * 180) / Math.PI;
const without = (v: V3, n: V3): V3 => {
  const d = dot(v, n);
  return [v[0] - d * n[0], v[1] - d * n[1], v[2] - d * n[2]];
};

/** The hand's own axes in its bone's frame: along it (to the middle knuckle), out of the palm,
 * across it toward the thumb; each unit and square to the others. */
export type HandAxes = { along: V3; palm: V3; thumb: V3 };

/** A hand's axes from its rest pose: the bone's rotation, where the wrist, middle knuckle and
 * thumb are, and a direction out of the palm (the hand socket's -X, attachment.md). */
export function handAxes(handR: Q, wrist: V3, knuckle: V3, thumb: V3, palmOut: V3): HandAxes {
  const back = qinv(handR);
  const along = unit(rot(back, sub(knuckle, wrist)));
  const palm = unit(without(rot(back, palmOut), along));
  const across = unit(without(without(rot(back, sub(thumb, wrist)), along), palm));
  return { along, palm, thumb: across };
}

/** The bend of a hand off its forearm, degrees: `flex` + toward the palm (- extension), `dev` +
 * toward the thumb (radial, - ulnar). */
export function bendOf(elbow: V3, wrist: V3, handR: Q, axes: HandAxes): { flex: number; dev: number } {
  const f = unit(sub(wrist, elbow));
  const [h, n, a] = [rot(handR, axes.along), rot(handR, axes.palm), rot(handR, axes.thumb)];
  // The forearm seen from the hand: a hand flexed toward its palm sees the forearm toward its back.
  // Flexion in the plane of the hand's line and its palm, deviation out of that plane, so a bend
  // past 90° stays one bend.
  const [fh, fn, fa] = [dot(f, h), dot(f, n), dot(f, a)];
  return { flex: deg(Math.atan2(-fn, fh)), dev: deg(Math.atan2(-fa, Math.hypot(fh, fn))) };
}

/** The angle of the turn between two rotations, degrees. */
export function turnBetween(a: Q, b: Q): number {
  const d = Math.abs(a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3]);
  return deg(2 * Math.acos(Math.min(1, d)));
}

/** One pose of the wrist: the elbow's and wrist's places and the forearm's and hand's rotations. */
export type WristPose = { elbow: V3; wrist: V3; foreR: Q; handR: Q };

export type WristWorst = { flex: number; ext: number; radial: number; ulnar: number; key: number; over: number; keysOver: number; where: Record<string, string> };

/** The worst of a wrist over a take's keys (poses at WRIST.fps, in order), relative to its rest
 * bend: each angle at its most, how far the worst is past its limit (`over`, degrees, negative
 * within them all) and how many keys are past one. `when` names a key. */
export function wristWorst(keys: WristPose[], axes: HandAxes, rest: { flex: number; dev: number }, when: (i: number) => string): WristWorst {
  const w: WristWorst = { flex: -Infinity, ext: -Infinity, radial: -Infinity, ulnar: -Infinity, key: 0, over: -Infinity, keysOver: 0, where: {} };
  const most = (k: "flex" | "ext" | "radial" | "ulnar" | "key", v: number, i: number) => {
    if (v > w[k]) {
      w[k] = v;
      w.where[k] = when(i);
    }
  };
  let last: Q | undefined;
  keys.forEach((p, i) => {
    const b = bendOf(p.elbow, p.wrist, p.handR, axes);
    const [flex, dev] = [b.flex - rest.flex, b.dev - rest.dev];
    const inFore = qmul(qinv(p.foreR), p.handR);
    const key = last ? turnBetween(last, inFore) : 0;
    last = inFore;
    most("flex", flex, i);
    most("ext", -flex, i);
    most("radial", dev, i);
    most("ulnar", -dev, i);
    most("key", key, i);
    const over = Math.max(flex - WRIST.flex, -flex - WRIST.ext, dev - WRIST.radial, -dev - WRIST.ulnar, key - WRIST.key);
    if (over > 0) w.keysOver++;
    w.over = Math.max(w.over, over);
  });
  return w;
}

/** Which side of a held look leads its tip's motion across its handle (+Y), over a blow's active
 * frames: each step's motion in the look's frame (`r`, its rotation then), weighted by how far it
 * moves across the handle, counting only steps at half the window's top speed or more. `lead`
 * is the share along -Z (the working edge, attachment.md), `flat` the share along ±X. */
export function edgeLead(steps: { moved: V3; r: Q }[]): { lead: number; flat: number } {
  const across = steps.map((s) => rot(qinv(s.r), s.moved)).map(([x, , z]) => ({ x, z, m: Math.hypot(x, z) }));
  const top = Math.max(0, ...across.map((a) => a.m));
  const fast = across.filter((a) => a.m >= top / 2 && a.m > 0);
  const total = fast.reduce((s, a) => s + a.m, 0) || 1;
  return { lead: fast.reduce((s, a) => s - a.z, 0) / total, flat: fast.reduce((s, a) => s + Math.abs(a.x), 0) / total };
}

/** A wrist's pose from a posed body: `n` the nodes' poses, `at` a bone's index. */
export const wristPose = (pose: Pose[], at: (n: string) => number, s: "l" | "r"): WristPose => ({
  elbow: pose[at(`lowerarm_${s}`)].t,
  wrist: pose[at(`hand_${s}`)].t,
  foreR: pose[at(`lowerarm_${s}`)].r,
  handR: pose[at(`hand_${s}`)].r,
});
