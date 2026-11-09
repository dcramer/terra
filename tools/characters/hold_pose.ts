// How a hand holds a look at one pose, in numbers (holds.ts, the hold report): the wrist's bend
// off the forearm, where the handle points, the elbow, and how near the look comes to the ground.
// moves.ts calls it on the pose the game composes (carry pose, swing, fist, arms kept), so the
// report reads the same body the clearances are measured on; nothing here poses a body.
//
// Node 22.18 or later; no dependencies.

import { type Pose, type V3, dot, len, place, rot, sub } from "./glb.ts";
import { type HandAxes, bendOf, handAxes, wristPose } from "./wrist.ts";

const deg = (rad: number) => (rad * 180) / Math.PI;
const unit = (v: V3): V3 => {
  const l = len(v) || 1;
  return [v[0] / l, v[1] / l, v[2] / l];
};
const clamp = (x: number) => Math.max(-1, Math.min(1, x));

/** A hand's axes and its bend at rest, which every bend is measured from (wrist.ts). */
export type WristRest = { axes: HandAxes; rest: { flex: number; dev: number }; feet: [number, number][] };

/** The joints the soles are read from, and the ground under a pose: the model's y = 0, where
 * the game stands the body, or lower by as far as a foot goes below where it stands at rest.
 * Never higher: both feet off the ground in a sprint's stride or a jump lift nothing, the
 * ground stays where it was (read off the lowest foot alone, a sprint's flight raised it 7 cm
 * and failed looks that were clear). */
const FEET = ["foot_l", "foot_r", "ball_l", "ball_r"];
export const groundUnder = (pose: Pose[], feet: [number, number][]) => Math.min(0, ...feet.map(([i, y]) => pose[i].t[1] - y));

/** The wrist's rest on a body at its rest pose: `s` the side, `at` a bone's index. */
export function wristRest(rest: Pose[], at: (n: string) => number, s: "l" | "r"): WristRest {
  const hand = rest[at(`hand_${s}`)];
  const palmOut = rot(rest[at(`socket.hand.${s.toUpperCase()}`)].r, [-1, 0, 0]);
  const axes = handAxes(hand.r, hand.t, rest[at(`middle_01_${s}`)].t, rest[at(`thumb_01_${s}`)].t, palmOut);
  const w = wristPose(rest, at, s);
  return { axes, rest: bendOf(w.elbow, w.wrist, w.handR, axes), feet: FEET.map((n) => [at(n), rest[at(n)].t[1]]) };
}

/** The measures of one pose, each one where more is worse, so the worst of a clip is its
 * greatest. Degrees: the wrist's flexion, extension, radial and ulnar deviation from rest; the
 * handle (+Y of `frame`) below and above level, and out from the body's middle (the model faces
 * +Z, +X its left: a right hand's out is -X); the elbow's bend. Centimetres: how far the look's
 * lowest point is under the ground (`groundUnder`; negative, that far above it). */
export const HOLD_KEYS = ["wrist_flex_deg", "wrist_ext_deg", "wrist_radial_deg", "wrist_ulnar_deg", "below_deg", "above_deg", "out_deg", "elbow_deg", "ground_cm"] as const;
export type HoldKey = (typeof HOLD_KEYS)[number];

/** How the hand on side `s` holds a look whose `points` are in `frame` (its socket's), at `pose`. */
export function holdPose(pose: Pose[], at: (n: string) => number, s: "l" | "r", wrist: WristRest, frame: Pose, points: V3[]): Record<HoldKey, number> {
  const w = wristPose(pose, at, s);
  const b = bendOf(w.elbow, w.wrist, w.handR, wrist.axes);
  const [flex, dev] = [b.flex - wrist.rest.flex, b.dev - wrist.rest.dev];
  const way = unit(rot(frame.r, [0, 1, 0]));
  const up = deg(Math.asin(clamp(way[1])));
  const upper = unit(sub(w.elbow, pose[at(`upperarm_${s}`)].t));
  const fore = unit(sub(w.wrist, w.elbow));
  let low = Infinity;
  for (const q of points) low = Math.min(low, place(frame, q)[1]);
  return {
    wrist_flex_deg: flex,
    wrist_ext_deg: -flex,
    wrist_radial_deg: dev,
    wrist_ulnar_deg: -dev,
    below_deg: -up,
    above_deg: up,
    out_deg: deg(Math.atan2(s === "r" ? -way[0] : way[0], way[2])),
    elbow_deg: deg(Math.acos(clamp(dot(upper, fore)))),
    ground_cm: (groundUnder(pose, wrist.feet) - low) * 100,
  };
}

/** The worst of each measure over poses, and where: `see` a pose's measures with what to call it. */
export class HoldWorst {
  readonly worst = new Map<HoldKey, { v: number; where: string }>();
  see(m: Record<HoldKey, number>, where: string) {
    for (const k of HOLD_KEYS) if (m[k] > (this.worst.get(k)?.v ?? -Infinity)) this.worst.set(k, { v: m[k], where });
  }
}
