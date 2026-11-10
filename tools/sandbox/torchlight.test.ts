// torchlight.ts's measures, on series and views whose answers are known another way.
//   node --test tools/sandbox/torchlight.test.ts

import assert from "node:assert/strict";
import { test } from "node:test";
import { flicker, level, project } from "./torchlight.ts";

test("open ground is level near, nearly level ahead and dry; a bump, a slope or a pond is said", () => {
  // 17 spots of the disc (itself, 4 m, 8 m), then 17 east from 10 m, 2.5 m apart.
  const flat = () => Array.from({ length: 34 }, () => ({ up: 3, water: -5 }));
  const with_ = (i: number, change: { up?: number; water?: number }) => flat().map((g, j) => (j === i ? { ...g, ...change } : g));
  assert.equal(level(flat()), undefined);
  assert.match(level(with_(3, { up: 3.5 }))!, /0\.50 m off level within 8 m/);
  assert.match(level(with_(17 + 4, { up: 1 }))!, /2\.0 m off level within 25 m/);
  // 40 m ahead a rise is allowed, a pond is not.
  assert.equal(level(with_(17 + 12, { up: 5 })), undefined);
  assert.match(level(with_(17 + 12, { water: 0.2 }))!, /water stands 0\.2 m/);
});

test("a sine's depth, period and step are read back", () => {
  // 10% deep at 2.5 Hz over 120 frames at 60 a second: deviation 0.1/sqrt 2, period 0.4 s.
  const series = Array.from({ length: 120 }, (_, i) => 1 + 0.1 * Math.sin((2 * Math.PI * 2.5 * i) / 60));
  const f = flicker(series);
  assert.ok(Math.abs(f.depth - 0.1 / Math.SQRT2) < 0.002, `depth ${f.depth}`);
  assert.ok(Math.abs(f.period - 0.4) < 0.01, `period ${f.period}`);
  // Its steepest frame: 0.1 * 2 pi * 2.5 / 60.
  assert.ok(Math.abs(f.step - 0.026) < 0.002, `step ${f.step}`);
});

test("a steady series does not flicker, and a pop is its largest step", () => {
  const steady = Array.from({ length: 120 }, () => 0.5);
  assert.equal(flicker(steady).depth, 0);
  steady[60] = 0.6;
  assert.ok(Math.abs(flicker(steady).step - 0.2) < 0.01);
});

test("what the eye looks at is the picture's middle, the ground nearer it lower, behind it unseen", () => {
  const picture = { width: 960, height: 540, rgba: new Uint8Array(0) };
  const view = { eye: [-4.5, 2.6, 0] as [number, number, number], at: [4, 0, 0] as [number, number, number], feet: [0, 0, 0] as [number, number, number], ahead: [1, 0] as [number, number] };
  assert.deepEqual(project(view, picture, [4, 0, 0]), [480, 270]);
  const near = project(view, picture, [1, 0, 0]);
  assert.ok(near && near[0] === 480 && near[1] > 270);
  // South is to the right of one facing east.
  const right = project(view, picture, [4, 0, 1]);
  assert.ok(right && right[0] > 480);
  assert.equal(project(view, picture, [-10, 0, 0]), undefined);
});
