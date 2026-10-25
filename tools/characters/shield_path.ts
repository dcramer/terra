// Where a fighter's shield goes through a blow and between chained blows (the move check,
// moves.ts; the faults named in research/2026-10-25-shield-prior-art.md): pure numbers on
// points and directions the move check takes off the posed body, so each is tested on a case
// whose answer is known.
//
//   roundTrip   the shield's way in and out again as the next blow of a chain starts: the
//               outgoing clip's shield against the incoming's, crossfaded over the game's move
//               blend, the farthest the shown shield gets from where it was at the cancel
//   faceOff     how far the board's painted face is turned from the chest's forward, flattened

import { type V3, dot, len, sub } from "./glb.ts";

/** The game's crossfade between moves (terra_creatures::melee, `show`'s 110 ms). */
export const MOVE_BLEND_S = 0.11;
/** How long after a chained blow starts its shield is watched for a round trip. */
export const ROUND_TRIP_S = 0.3;

/**
 * The farthest the shown shield gets, over `span` seconds from a move change, from where it was
 * at the change: `out(u)` and `into(u)` the shield's middle `u` seconds after it in the outgoing
 * and incoming clips (each in the frame it is measured in), the incoming's weight rising from 0
 * to 1 over `blend` seconds as Bevy's animation transitions fade (linearly), the shown shield
 * their weighted mean. Metres, with the second it is farthest.
 */
export function roundTrip(out: (u: number) => V3, into: (u: number) => V3, blend = MOVE_BLEND_S, span = ROUND_TRIP_S, step = 1 / 120): { m: number; at: number } {
  const start = out(0);
  let worst = { m: 0, at: 0 };
  for (let u = 0; u <= span + 1e-9; u += step) {
    const w = blend > 0 ? Math.min(1, u / blend) : 1;
    const [a, b] = [out(u), into(u)];
    const shown = a.map((x, j) => x * (1 - w) + b[j] * w) as V3;
    const m = len(sub(shown, start));
    if (m > worst.m) worst = { m, at: u };
  }
  return worst;
}

/** Degrees between the board's face (the way its painted front looks) and the chest's forward
 * laid flat (its up part taken out): 0 facing ahead, 90 to a side or up, 180 back. */
export function faceOff(face: V3, forward: V3, up: V3 = [0, 1, 0]): number {
  const k = dot(forward, up);
  const flat = forward.map((x, j) => x - up[j] * k) as V3;
  const c = dot(face, flat) / (len(face) * len(flat) || 1);
  return (Math.acos(Math.max(-1, Math.min(1, c))) * 180) / Math.PI;
}
