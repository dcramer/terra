// The foot-down bake on made-up feet whose plants are known. node --test tools/characters/markers.test.ts

import { test } from "node:test";
import assert from "node:assert/strict";
import { groundOf, HIGHEST_M, spans } from "./markers.ts";
import type { V3 } from "./glb.ts";

const FPS = 60;

/** A stride played in place: a second's loop, the foot down (back along -z at `pace`, its ball
 * `rest` over the ground) from `down` to `up` seconds, swung forward and lifted otherwise. */
function stride(pace: number, rest: number, down: number, up: number): V3[] {
  const n = FPS;
  const planted = (t: number) => (down < up ? t >= down && t < up : t >= down || t < up);
  const stance = (up - down + 1) % 1 || 1;
  const path: V3[] = [];
  for (let i = 0; i <= n; i++) {
    const t = (i % n) / n;
    const into = (t - down + 1) % 1;
    if (planted(t)) path.push([0.1, rest, pace * stance * (0.5 - into / stance)]);
    else {
      const s = (into - stance) / (1 - stance);
      path.push([0.1, rest + 0.15 * Math.sin(Math.PI * s), pace * stance * (-0.5 + s)]);
    }
  }
  return path;
}

test("a stride's foot is down while it stands on the ground the clip passes, across the loop", () => {
  // The jog's rolling foot stands its ball 6 cm up: a height for every clip would miss it.
  for (const [down, up] of [
    [0.2, 0.5],
    [0.8, 0.15],
  ]) {
    const path = stride(3.7, 0.06, down, up);
    const ground = groundOf(path, FPS);
    assert.ok(Math.abs(ground[1] + 3.7) < 0.05, `the ground passes at the pace: ${ground}`);
    const found = spans(path, FPS, ground, true);
    assert.equal(found.length, 1, `one plant a stride: ${JSON.stringify(found)}`);
    const [a, b] = found[0];
    // Within a few frames of the stance's ends: the swing's first and last frames go as
    // slowly as a plant's.
    const near = (x: number, y: number) => Math.min(Math.abs(x - y), 1 - Math.abs(x - y)) <= 4 / FPS;
    assert.ok(near(a, down) && near(b, up), `${down}-${up} found ${a}-${b}`);
  }
});

test("a stance's feet are down the whole clip; a foot held high, never; a swing low, never", () => {
  const still: V3[] = Array.from({ length: 61 }, () => [0.1, 0.035, 0.0]);
  assert.deepEqual(groundOf(still, FPS), [0, 0]);
  assert.deepEqual(spans(still, FPS, [0, 0], true), [[0, 1]]);
  const held: V3[] = Array.from({ length: 61 }, (_, i) => [0.1, HIGHEST_M + 0.05 + 0.01 * Math.sin(i), 0]);
  assert.deepEqual(spans(held, FPS, [0, 0], false), []);
  // A foot skimming the ground fast in a clip that stands (a kick along the ground).
  const skim: V3[] = Array.from({ length: 31 }, (_, i) => [0.1, 0.03, -0.6 + (i / 30) * 1.2]);
  assert.deepEqual(spans(skim, FPS, [0, 0], false), []);
});
