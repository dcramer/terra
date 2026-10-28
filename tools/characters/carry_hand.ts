// A tool carried by the rule tips forward and down with the hand and the arm, not by sliding its
// handle across the palm (D564): the twin of terra_walker::carry_hand, which the move check lays
// over the carried clips (moves.ts --carry). The look turns in the fist no more than OBLIQUE, the
// hand turns the rest at the wrist about the socket's X, and while it is carried the forearm is
// kept FOREARM_DOWN below level by opening the elbow in its own bend's plane. Change both.
import { type Q, type V3, dot, len, rot } from "./glb.ts";

/** The most a carried look turns in the fist, degrees (carry_hand::OBLIQUE). */
export const OBLIQUE = 15;
/** How far below level a carried tool's forearm is kept at least, degrees (carry_hand::FOREARM_DOWN). */
export const FOREARM_DOWN = 45;
/** The least an elbow opened for it stays bent, degrees. */
const ELBOW_MOST_OPEN = 10;

/** The share of a carried tilt the look takes in the fist, degrees (Held::in_fist). */
export const inFist = (tilt: number): number => Math.max(-OBLIQUE, Math.min(OBLIQUE, tilt));
/** The share the wrist takes, degrees (Held::at_wrist). */
export const atWrist = (tilt: number): number => tilt - inFist(tilt);

const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
/** The turn of `rad` about unit `a`. */
export const about = (a: V3, rad: number): Q => {
  const s = Math.sin(rad / 2);
  return [a[0] * s, a[1] * s, a[2] * s, Math.cos(rad / 2)];
};

/** How far, radians, to open an elbow whose upper arm runs `upper` and forearm `fore` (+y up) for
 * the forearm to point FOREARM_DOWN below level, no further than ELBOW_MOST_OPEN short of
 * straight, and the axis to turn the forearm about by minus that (carry_hand::elbow_open). */
export function elbowOpen(upper: V3, fore: V3): { axis: V3; open: number } | undefined {
  const low = (-FOREARM_DOWN * Math.PI) / 180;
  const rise = (v: V3) => Math.asin(Math.max(-1, Math.min(1, v[1] / len(v))));
  if (rise(fore) <= low) return undefined;
  const n = cross(upper, fore);
  if (len(n) < 1e-6) return undefined;
  const axis = n.map((x) => x / len(n)) as V3;
  const bent = Math.acos(Math.max(-1, Math.min(1, dot(upper, fore) / (len(upper) * len(fore)))));
  const most = Math.max(0, bent - (ELBOW_MOST_OPEN * Math.PI) / 180);
  const step = (0.5 * Math.PI) / 180;
  let open = 0;
  while (open < most && rise(rot(about(axis, -open), fore)) > low) open += step;
  return { axis, open: Math.min(open, most) };
}

/** The hand's turn at the wrist after its clip's, in its own frame: `deg` about the socket's X,
 * the socket turned `socket` in the hand. */
export const wristTurn = (socket: Q, deg: number): Q => about(rot(socket, [1, 0, 0]), (deg * Math.PI) / 180);
