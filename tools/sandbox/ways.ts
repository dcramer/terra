// Which ways collide.ts may walk at a thing (D267): the rule for a clear way, kept apart from
// the running game so it can be tested. Node 22.18 or later; no dependencies.

/// The walker's body (terra_build::Walker).
export const BODY = 0.3;

/// A spot on a way, as `lush.footprint` measures it: how far it is from the nearest solid mesh
/// taller than a step (`solid`), from the nearest of anything but the thing walked at
/// (`other`), and from the thing as drawn (`seen`); null where there is none.
export type Spot = { solid: number | null; solid_of: string | null; other: number | null; other_of: string | null; seen: number | null };

/// How far off a way starts, and the spots measured along it toward the thing; and for a thing
/// to be stepped over, the spots past it the walker must reach.
export const APPROACH = [4, 3.5, 3, 2.5, 2, 1.5, 1, 0.5];
export const BEYOND = [0.5, 1, 1.5];

/// Whether a way is clear: on the way to the thing, every spot more than a stride from it as
/// drawn is clear of anything solid by a body and a little, and the start a body and a half
/// from it; past it, for a thing to be stepped over, the body clears anything else solid,
/// so the walker is not asked to step over a log into the trunk it lies against.
export function clear(approach: Spot[], beyond: Spot[] = []): boolean {
  const near = (s: Spot) => s.seen !== null && s.seen < 0.8;
  const free = (d: number | null) => d === null || d > BODY + 0.15;
  return (
    approach.every((s) => near(s) || free(s.solid)) &&
    (approach[0].seen === null || approach[0].seen > BODY + 0.5) &&
    beyond.every((s) => free(s.other))
  );
}
