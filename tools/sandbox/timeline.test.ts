import assert from "node:assert/strict";
import { test } from "node:test";
import { bars, joined, lay, type Piece, smoothed } from "./timeline.ts";

const piece = (name: string, seconds: number, overlap: number): Piece => ({ name, seconds, overlap, act: 0 });

test("pieces overlap the one before by their transition, and the cut is that much shorter", () => {
  // 3 + 5 + 4 seconds, the second and third each fading in over a second: 10 s, worked by hand.
  const { starts, seconds } = lay([piece("card", 3, 0), piece("a", 5, 1), piece("b", 4, 1)]);
  assert.deepEqual(starts, [0, 2, 6]);
  assert.equal(seconds, 10);
});

test("a transition never takes more than half of either piece it joins", () => {
  const { starts } = lay([piece("short", 1, 0), piece("long", 6, 3)]);
  assert.equal(starts[1], 0.5);
});

test("a smoothed route goes through its spots, a spot every step, turning a little at each", () => {
  // A right-angle corner: 40 m east, then 40 m south.
  const spots: [number, number][] = [
    [0, 0],
    [40, 0],
    [40, 40],
  ];
  const route = smoothed(spots, 4);
  assert.deepEqual(route[0], [0, 0]);
  assert.deepEqual(route[route.length - 1], [40, 40]);
  // Near the corner it passes, within a step.
  assert.ok(route.some(([e, s]) => Math.hypot(e - 40, s) < 4));
  // About 80 m in steps of 4: some twenty spots, none more than a step and a little apart.
  assert.ok(route.length >= 18 && route.length <= 24, `${route.length}`);
  const heading = (i: number) => Math.atan2(route[i + 1][1] - route[i][1], route[i + 1][0] - route[i][0]);
  for (let i = 0; i < route.length - 2; i++) {
    const gap = Math.hypot(route[i + 1][0] - route[i][0], route[i + 1][1] - route[i][1]);
    assert.ok(gap <= 4.6, `gap ${gap} at ${i}`);
    // The 90-degree corner spread over several spots: none turns half of it.
    const turn = Math.abs(heading(i + 1) - heading(i));
    assert.ok(turn < Math.PI / 4, `turned ${((turn * 180) / Math.PI).toFixed(0)} degrees at ${i}`);
  }
});

test("the filter fades each piece into the next at its start, overlapping as laid", () => {
  const filter = joined([piece("card", 3, 0), piece("a", 5, 1), piece("b", 4, 0.5)], 30, "1920x1080");
  assert.match(filter, /\[p0\]\[p1\]xfade=transition=fade:duration=1\.000:offset=2\.000\[x1\]/);
  assert.match(filter, /\[x1\]\[p2\]xfade=transition=fade:duration=0\.500:offset=6\.500\[cut\]/);
  assert.equal(filter.match(/xfade/g)?.length, 2);
});

test("a letterbox at 2.39:1 on 1080p leaves 804 lines between its bars", () => {
  assert.equal(1080 - 2 * bars(1920, 1080, 2.39), 804);
  assert.equal(bars(1920, 1080, 16 / 9), 0);
  assert.equal(bars(1920, 1080, 4 / 3), 0);
});
