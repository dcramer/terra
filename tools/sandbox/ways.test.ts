import assert from "node:assert/strict";
import { test } from "node:test";
import { APPROACH, BEYOND, clear, type Spot } from "./ways.ts";

/// A spot this far from anything solid and from the thing as drawn, the solid being `of`.
const spot = (solid: number | null, seen: number | null, of = "Oak", own = false): Spot => ({
  solid,
  solid_of: solid === null ? null : of,
  other: own ? null : solid,
  other_of: own || solid === null ? null : of,
  seen,
});

/// Open ground up to a thing, as the approach to a log's low end measured across it.
const open = APPROACH.map((d) => spot(2.5, d - 0.3));

test("an open way to a low thing and on past it is clear", () => {
  assert.equal(clear(open, BEYOND.map(() => spot(2.0, 0.5))), true);
});

test("a low log end lying against an oak is not clear past it, though its approach is", () => {
  // As measured at a log's low end in the lush forest (2026-10-14): open ground to it, and
  // across it the oak's root flare from 0.3 m past where it was aimed at.
  const beyond = [spot(0.0, 0.24), spot(0.0, 0.7), spot(0.0, 1.17)];
  assert.equal(clear(open), true);
  assert.equal(clear(open, beyond), false);
});

test("past a low thing, its own taller parts do not close the way", () => {
  // A log's thick end, beside the line across its thin end, is the thing itself.
  const beyond = BEYOND.map(() => spot(0.2, 0.3, "Log", true));
  assert.equal(clear(open, beyond), true);
});

test("something solid on the way there, away from the thing, closes it", () => {
  const way = open.map((s, i) => (i === 2 ? spot(0.1, 2.2, "Boulder") : s));
  assert.equal(clear(way), false);
  // Near the thing as drawn, what is solid is the thing.
  const near = open.map((s, i) => (i === open.length - 1 ? spot(0.1, 0.2, "Log") : s));
  assert.equal(clear(near), true);
});

test("a start in or against the thing is not clear", () => {
  const start = [spot(2.5, 0.6), ...open.slice(1)];
  assert.equal(clear(start), false);
});
