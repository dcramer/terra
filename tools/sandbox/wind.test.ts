// The still-camera wind measure (wind.ts) on made-up films whose answers are known: a picture
// sliding as one block, tiles each moving on their own, nothing moving, a sway and a buzz.
// `node --test tools/sandbox/wind.test.ts`.

import assert from "node:assert/strict";
import { test } from "node:test";
import { HEIGHT, judge, measure, WIDTH } from "./wind.ts";

/// A fixed texture of noise, by a hash of the pixel, wider than the picture so it can shift.
const texture = (x: number, y: number) => {
  let h = (Math.imul(x + 1000, 0x27d4eb2d) ^ Math.imul(y + 1000, 0x165667b1)) >>> 0;
  h = Math.imul(h ^ (h >>> 15), 0x2c1b3c6d) >>> 0;
  return (h ^ (h >>> 13)) & 0xff;
};
/// A frame whose pixel at (x, y) shows the texture shifted by `shift` of that pixel's tile.
function frame(shift: (x: number, y: number) => [number, number]): Uint8Array {
  const f = new Uint8Array(WIDTH * HEIGHT);
  for (let y = 0; y < HEIGHT; y++) {
    for (let x = 0; x < WIDTH; x++) {
      const [sx, sy] = shift(x, y);
      f[y * WIDTH + x] = texture(x - sx, y - sy);
    }
  }
  return f;
}
/// A made-up pseudo-random integer shift in -1..1 for a step and a key, so a step's change
/// (-2..2) stays inside the measure's reach.
/// Mixed past the multiply, since a product's remainder by 3 follows its key in step.
const jiggle = (t: number, k: number) => {
  let h = Math.imul(t * 7919 + k * 104729, 0x9e3779b1) >>> 0;
  h = Math.imul(h ^ (h >>> 16), 0x85ebca6b) >>> 0;
  h = (h ^ (h >>> 13)) >>> 0;
  return (h % 3) - 1;
};

test("a picture sliding as one block is all shared motion, neighbours alike, and fails", () => {
  const frames = Array.from({ length: 31 }, (_, t) => frame(() => [jiggle(Math.floor(t / 3), 1), jiggle(Math.floor(t / 3), 2)]));
  const m = measure(frames);
  assert.ok(m.moving > 20, `${m.moving} tiles moving`);
  assert.ok(m.shared > 0.95, `shared ${m.shared}`);
  assert.ok(m.neighbour > 0.95, `neighbour r ${m.neighbour}`);
  // Shared, neighbours alike, and (a new place each step) a buzz.
  assert.equal(judge(m).length, 3);
});

test("tiles each moving on their own share little motion, neighbours unlike, and pass those", () => {
  // Each 32-pixel tile (the measure's, from 3 px in) shifts by its own amount each step.
  const tileOf = (x: number, y: number) => Math.floor((x - 3) / 32) + 100 * Math.floor((y - 3) / 32);
  const frames = Array.from({ length: 61 }, (_, t) =>
    frame((x, y) => {
      const k = tileOf(x, y);
      const s = Math.floor(t / 3);
      return [jiggle(s, 3 * k + 1), jiggle(s, 3 * k + 2)];
    }),
  );
  const m = measure(frames);
  assert.ok(m.moving > 20, `${m.moving} tiles moving`);
  assert.ok(m.shared < 0.15, `shared ${m.shared}`);
  assert.ok(Math.abs(m.neighbour) < 0.3, `neighbour r ${m.neighbour}`);
  // Jitter, a new place every step, goes no further in a second: it fails as a buzz alone.
  assert.ok(m.slow < 1.5, `second against step ${m.slow}`);
  assert.deepEqual(
    judge(m).map((f) => f.split(" ")[0]),
    ["second"],
  );
});

test("a still picture has nothing moving and fails as no wind to judge", () => {
  const still = frame(() => [0, 0]);
  const m = measure(Array.from({ length: 10 }, () => still));
  assert.equal(m.moving, 0);
  assert.equal(judge(m).length, 1);
  assert.match(judge(m)[0], /nothing moved/);
});

test("a slow sway moves far more over a second than a step; a buzz moves no further", () => {
  // Positions a step apart (three frames, a tenth of a second at 30 a second).
  const film = (at: (s: number) => number) => Array.from({ length: 181 }, (_, t) => frame(() => [at(Math.floor(t / 3)), 0]));
  // Eight pixels each way over four seconds; and a pixel back and forth every step.
  const sway = measure(film((s) => Math.round(8 * Math.sin((2 * Math.PI * s) / 40))));
  const buzz = measure(film((s) => s % 2));
  assert.ok(sway.slow > 5, `sway: second against step ${sway.slow}`);
  assert.ok(buzz.slow < 1, `buzz: second against step ${buzz.slow}`);
  assert.ok(!judge(sway).some((f) => f.includes("buzz")));
  assert.ok(judge(buzz).some((f) => f.includes("buzz")));
});
