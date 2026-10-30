// The foot-lock bake on made-up feet whose plants are known, and on the game's body files.
// node --test tools/characters/markers.test.ts

import { test } from "node:test";
import assert from "node:assert/strict";
import { bake, distanceOf, foot, groundOf, HIGHEST_M, RATE, SKATE_M, WALK_MPS } from "./markers.ts";
import { Glb, type V3 } from "./glb.ts";

const FPS = 60;

/** A stride played in place: a second's loop, the foot down (back along -z at `pace`, its toe
 * tip `rest` over the ground) from `down` to `up` seconds, swung forward and lifted otherwise;
 * its heel `heel` over the toe. With `rolls`, the toe tip rises 4 cm over the plant's last
 * fifth as it goes back (a toe-off, nearly still). */
function stride(pace: number, rest: number, down: number, up: number, rolls = false): { heel: V3[]; toe: V3[] } {
  const n = FPS;
  const planted = (t: number) => (down < up ? t >= down && t < up : t >= down || t < up);
  const stance = (up - down + 1) % 1 || 1;
  const toe: V3[] = [];
  for (let i = 0; i <= n; i++) {
    const t = (i % n) / n;
    const into = (t - down + 1) % 1;
    if (planted(t)) {
      const roll = rolls ? 0.04 * Math.max(0, (into / stance - 0.8) / 0.2) : 0;
      toe.push([0.1, rest + roll, pace * stance * (0.5 - into / stance)]);
    } else {
      const s = (into - stance) / (1 - stance);
      toe.push([0.1, rest + 0.15 * Math.sin(Math.PI * s), pace * stance * (-0.5 + s)]);
    }
  }
  // The heel lifts with the swing and off the ground a toe-off rolls up from.
  const heel = toe.map((p): V3 => [p[0], p[1] + 0.12, p[2] - 0.15]);
  return { heel, toe };
}

const near = (x: number, y: number, frames = 4) => Math.min(Math.abs(x - y), 1 - Math.abs(x - y)) <= frames / FPS;

test("a stride's foot locks while it stands on the ground the clip passes, across the loop, and lets go by a ramp", () => {
  for (const [down, up] of [
    [0.2, 0.5],
    [0.8, 0.15],
  ]) {
    const { heel, toe } = stride(3.7, 0.06, down, up, true);
    const ground = groundOf([toe], FPS, 3.7);
    assert.ok(Math.abs(ground[1] + 3.7) < 1e-6, `the ground passes at the pace: ${ground}`);
    const found = foot(heel, toe, FPS, ground, true);
    assert.equal(found.runs.length, 1, `one plant a stride: ${JSON.stringify(found.runs)}`);
    const [a, b] = found.runs[0].map((f) => (f % FPS) / FPS);
    // The toe-off's rise is within the loose height gate: the plant runs to the lift.
    assert.ok(near(a, down) && near(b, up), `${down}-${up} found ${a}-${b}`);
    // 1 through the plant, falling as the foot leaves, 0 in the swing.
    const at = (t: number) => found.lock[Math.round(((t + 1) % 1) * FPS)];
    assert.equal(at(down + 0.05), 1);
    assert.equal(at(up - 0.03), 1);
    assert.ok(at(up + 0.05) > 0 && at(up + 0.05) < 1, `ramp ${at(up + 0.05)}`);
    assert.equal(at(up + 0.15), 0);
    assert.ok(found.skate_m < 0.01, `a clean stride does not skate: ${found.skate_m}`);
  }
});

test("a stance's feet are locked the whole clip; a foot held high, never; a swing low, never", () => {
  const still: V3[] = Array.from({ length: 61 }, () => [0.1, 0.035, 0.0]);
  const heel = still.map((p): V3 => [p[0], p[1] + 0.1, p[2]]);
  assert.deepEqual(groundOf([still], FPS), [0, 0]);
  assert.ok(foot(heel, still, FPS, [0, 0], true).lock.every((x) => x === 1));
  const held: V3[] = Array.from({ length: 61 }, (_, i) => [0.1, HIGHEST_M + 0.05 + 0.01 * Math.sin(i), 0]);
  assert.deepEqual(foot(held, held, FPS, [0, 0], false).runs, []);
  // A foot skimming the ground fast in a clip that stands (a kick along the ground).
  const skim: V3[] = Array.from({ length: 31 }, (_, i) => [0.1, 0.03, -0.6 + (i / 30) * 1.2]);
  assert.deepEqual(foot(skim, skim, FPS, [0, 0], false).runs, []);
});

test("a plant that skates is split where it has gone too far, and says how far", () => {
  // Standing, drifting 0.2 m/s over a second: 20 cm.
  const drift: V3[] = Array.from({ length: 61 }, (_, i) => [0.1, 0.03, (0.2 * i) / FPS]);
  const found = foot(drift, drift, FPS, [0, 0], false);
  assert.ok(Math.abs(found.skate_m - 0.2) < 0.01, `skate ${found.skate_m}`);
  assert.ok(found.splits >= Math.floor(0.2 / SKATE_M) - 1, `splits ${found.splits}`);
});

test("the game's walks have a foot down at every moment, and its strides do not skate", () => {
  for (const file of ["assets/models/character/base.glb", "assets/models/character/fight.glb"]) {
    const baked = bake(new Glb(file));
    const walks = Object.entries(baked).filter(([, b]) => b.pace !== undefined && b.pace < WALK_MPS && b.pace > 0.5);
    assert.ok(walks.length >= (file.includes("fight") ? 4 : 1), `${file}: its walks ${walks.map(([c]) => c)}`);
    for (const [clip, b] of walks) {
      assert.equal(b.neither_s, 0, `${file}: ${clip} has neither foot down ${b.neither_s} s`);
      assert.equal(b.splits, 0, `${file}: ${clip} skates ${b.skate_m}`);
    }
    for (const clip of ["Jog_Fwd_Loop", "Sprint_Loop"]) assert.equal(baked[clip].splits, 0, `${file}: ${clip} skates ${baked[clip].skate_m}`);
  }
});

test("a clip that keeps its root motion has its pelvis's travel along the way it goes, in its parent's space; one that stays has none", () => {
  // Braking along -x at 30 frames a second: 2 m/s for half a second, then stopping, swaying 3 cm across.
  const fps = 30;
  const path: V3[] = Array.from({ length: 31 }, (_, i) => {
    const t = i / fps;
    const x = t < 0.5 ? 2 * t : 1 + 2 * (t - 0.5) - 2 * (t - 0.5) ** 2;
    return [-Math.min(x, 1.25), 1, 0.03 * Math.sin(t * 10)];
  });
  // The parent turned a quarter about up (+y): its +z is the world's +x.
  const half = Math.SQRT1_2;
  const curve = distanceOf(path, fps, 1, { t: [0, 0, 0], r: [0, half, 0, half], s: [1, 1, 1] })!;
  assert.equal(curve.distance.length, RATE + 1);
  assert.ok(Math.abs(curve.distance[Math.round(0.2 * RATE)] - 0.4) < 0.01, `${curve.distance[12]}`);
  assert.ok(Math.abs(curve.distance.at(-1)! - 1.25) < 0.01);
  // World -x is the parent's -z.
  assert.deepEqual(curve.way.map((x) => Math.round(x) + 0), [0, 0, -1]);
  // A parent drawn twice its size: a model metre is half of one of its units.
  const scaled = distanceOf(path, fps, 1, { t: [0, 0, 0], r: [0, half, 0, half], s: [2, 2, 2] })!;
  assert.deepEqual(scaled.way.map((x) => Math.round(x * 10) / 10 + 0), [0, 0, -0.5]);
  const sways: V3[] = Array.from({ length: 31 }, (_, i) => [0.05 * Math.sin(i), 1, 0]);
  assert.equal(distanceOf(sways, fps, 1), undefined);
});
