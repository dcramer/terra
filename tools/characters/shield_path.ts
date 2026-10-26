// Where a fighter's shield goes through a blow and between chained blows (the move check,
// moves.ts; the faults named in research/2026-10-25-shield-prior-art.md): pure numbers on
// points and directions the move check takes off the posed body, so each is tested on a case
// whose answer is known.
//
//   roundTrip   the shield's way in and out again as the next blow of a chain starts: the
//               outgoing clip's shield against the incoming's, crossfaded over the game's move
//               blend, the farthest the shown shield strays from the straight way between where
//               it was at the press and where it is at the span's end (a shield that stays
//               put, or goes straight out for the blow, makes none)
//   faceOff     how far the board's painted face is turned from the chest's forward, flattened
//   gripOff     how far the shield's grip is from the fist that should close on it, and how
//               far its bar is turned from the line across the palm (held.shield.grip_hand,
//               grip_across_palm; research/2026-10-25-shield-prior-art.md fault 3)

import { type V3, dot, len, sub } from "./glb.ts";

/** The game's crossfade between moves (terra_creatures::melee, `show`'s 110 ms). */
export const MOVE_BLEND_S = 0.11;
/** How long after a chained blow starts its shield is watched for a round trip. */
export const ROUND_TRIP_S = 0.3;

/**
 * How far the shown shield goes in and comes back out, over `span` seconds from a move change:
 * `out(u)` and `into(u)` the shield's middle `u` seconds after it in the outgoing and incoming
 * clips (each in the frame it is measured in), the incoming's weight rising from 0 to 1 over
 * `blend` seconds as Bevy's animation transitions fade (linearly), the shown shield their
 * weighted mean. The farthest the shown shield gets from the straight way between where it was
 * at the change and where it is at the span's end: a shield held still, or moved straight out
 * for the blow, makes none; one that dips in and comes back out makes the dip. Metres, with the
 * second it is farthest.
 */
export function roundTrip(out: (u: number) => V3, into: (u: number) => V3, blend = MOVE_BLEND_S, span = ROUND_TRIP_S, step = 1 / 120): { m: number; at: number } {
  const shown = (u: number): V3 => {
    const w = blend > 0 ? Math.min(1, u / blend) : 1;
    const [a, b] = [out(u), into(u)];
    return a.map((x, j) => x * (1 - w) + b[j] * w) as V3;
  };
  const [start, end] = [shown(0), shown(span)];
  const way = sub(end, start);
  const long = dot(way, way);
  let worst = { m: 0, at: 0 };
  for (let u = 0; u <= span + 1e-9; u += step) {
    const p = sub(shown(u), start);
    const k = long > 0 ? Math.max(0, Math.min(1, dot(p, way) / long)) : 0;
    const m = len(sub(p, way.map((x) => x * k) as V3));
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

/**
 * The shield's grip against the fist (research/2026-10-25-shield-prior-art.md fault 3): `fist`
 * the middle of the closed fist (socket.hand.L's origin), `knuckles` two points on the line
 * across the palm (the index and little finger's first joints), `grip` the shield's grip (its
 * look's origin, as it is hung; a strapped shield's hand strap) and `bar` the way its handle runs
 * (the look's +Y; a strapped look's hand strap, across its board, +X). The grip's
 * distance from the fist, cm, and the degrees between the bar and the line across the palm, as
 * lines (0 across the palm either way round, 90 along the forearm).
 */
export function gripOff(fist: V3, knuckles: [V3, V3], grip: V3, bar: V3): { cm: number; deg: number } {
  const across = sub(knuckles[1], knuckles[0]);
  const c = Math.abs(dot(across, bar)) / (len(across) * len(bar) || 1);
  return { cm: len(sub(grip, fist)) * 100, deg: (Math.acos(Math.min(1, c)) * 180) / Math.PI };
}
