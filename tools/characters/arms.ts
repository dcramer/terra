// The hanging hands kept off the hips (D535), the twin of terra_walker::arms: after a clip has
// posed a body, an upper arm whose hand hangs by the hip closer to the body's middle than `OUT`
// times the hip joint's own distance from it is turned out at the shoulder, about the body's
// forward axis, just enough to clear it (ALS's arm spacing), what the hand holds counted by its
// reach as the hand's own points are. The move check lays it over the
// walker's clips (moves.ts --arms), so it measures the pose the game shows. Node 22.18 or
// later; no dependencies.

import { type Glb, type Pose, type Q, type V3, place } from "./glb.ts";

/** terra_walker::arms's numbers: keep the hand OUT hip joints' distances from the middle; a hand
 * wholly by the hip up to LOW over the hip joint and not at all from HIGH, wholly within NEAR
 * ahead or behind it and not at all from FAR; MOST degrees at most; what the hand holds HELD
 * metres further out than the hand. */
export const OUT = 2.0;
export const HELD = 0.04;
const [LOW, HIGH, NEAR, FAR] = [0.12, 0.22, 0.2, 0.32];
const MOST = 25.0;

/** terra_gen::smoothstep. */
const smoothstep = (from: number, to: number, x: number) => {
  const t = Math.min(1, Math.max(0, (x - from) / (to - from)));
  return t * t * (3 - 2 * t);
};

/** How far, radians, to turn out an arm (side +1 the left, -1 the right) hanging from
 * `shoulder` whose hand's points are `points`, beside a hip joint at `hip` on a pelvis at
 * `middle` (terra_walker::arms::turn_out). */
export function turnOut(side: number, shoulder: V3, points: V3[], hip: V3, middle: V3): number {
  const hipHalf = side * (hip[0] - middle[0]);
  if (hipHalf <= 0) return 0;
  let most = 0;
  for (const p of points) {
    const short = OUT * hipHalf - side * (p[0] - middle[0]);
    if (short <= 0) continue;
    const by = smoothstep(HIGH, LOW, p[1] - hip[1]) * smoothstep(FAR, NEAR, Math.abs(p[2] - hip[2]));
    const drop = Math.max(shoulder[1] - p[1], 0.15);
    most = Math.max(most, (short * by) / drop);
  }
  return side * Math.min(most, (MOST * Math.PI) / 180);
}

/** How many slices along its handle a held look's reach takes (terra_walker::arms::SLICES). */
export const SLICES = 8;

/** A held look's reach (terra_walker::arms::reach): its points in its own frame (the grip at the
 * origin, the handle along +Y) cut into `SLICES` slices along +Y, and of each the points
 * farthest along -X, +X, -Z and +Z, so that a few points bound what the hand holds. */
export function reach(points: V3[]): V3[] {
  if (!points.length) return [];
  const lo = Math.min(...points.map((p) => p[1]));
  const span = Math.max(...points.map((p) => p[1])) - lo;
  const slices: (V3 | undefined)[][] = Array.from({ length: SLICES }, () => [undefined, undefined, undefined, undefined]);
  for (const p of points) {
    const k = span > 0 ? Math.min(SLICES - 1, Math.floor(((p[1] - lo) / span) * SLICES)) : 0;
    const s = slices[k];
    if (!s[0] || p[0] < s[0][0]) s[0] = p;
    if (!s[1] || p[0] > s[1][0]) s[1] = p;
    if (!s[2] || p[2] < s[2][2]) s[2] = p;
    if (!s[3] || p[2] > s[3][2]) s[3] = p;
  }
  return slices.flat().filter((p): p is V3 => p !== undefined);
}

/** What a hand holds, for keptOff: its socket's name and its reach in the socket's frame, as
 * the game poses it. */
export type Holding = { socket: string; points: V3[] };

/** The bends that keep a posed body's hanging hands, and what they hold, off its hips, each
 * upper arm's turn about the body's Z, for `posed`'s bends (terra_walker::arms::keep_off_hips). */
export function keptOff(body: Glb, pose: Pose[], holding: Holding[] = []): Map<string, Q> {
  const at = (name: string): V3 => pose[body.gltf.nodes.findIndex((n) => n.name === name)].t;
  const bends = new Map<string, Q>();
  for (const [side, s, socket] of [[1, "l", "socket.hand.L"], [-1, "r", "socket.hand.R"]] as const) {
    const [wrist, knuckle] = [at(`hand_${s}`), at(`middle_01_${s}`)];
    const points: V3[] = [wrist.map((x, j) => (x + knuckle[j]) / 2) as V3, at(`middle_02_${s}`), at(`index_02_${s}`), at(`thumb_02_${s}`)];
    for (const h of holding) {
      if (h.socket !== socket) continue;
      const S = pose[body.gltf.nodes.findIndex((n) => n.name === socket)];
      // Kept HELD further out than the hand: counted that much nearer the middle.
      for (const q of h.points) {
        const w = place(S, q);
        points.push([w[0] - side * HELD, w[1], w[2]]);
      }
    }
    const angle = turnOut(side, at(`upperarm_${s}`), points, at(`thigh_${s}`), at("pelvis"));
    if (angle !== 0) bends.set(`upperarm_${s}`, [0, 0, Math.sin(angle / 2), Math.cos(angle / 2)]);
  }
  return bends;
}
