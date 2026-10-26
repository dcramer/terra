// swim.ts's pose check on made-up lake swims: each fault planted, and its clean twin passing.
// node --test tools/controller/swim.test.ts

import assert from "node:assert/strict";
import { test } from "node:test";
import type { Frame } from "../sandbox/duel/checks.ts";
import { EASE_S, POSE_DEG, pose } from "./swim.ts";
import type { Walk } from "./yard-checks.ts";

type Row = { swimming: boolean; shows: string; tilt: number; going: number[] };
const EASE = EASE_S * 60;

/// A walker's frames from rows, each repeated `n` times: whether it swims, its clip, its torso
/// tilted `tilt` deg from upright toward +Z, and the way it goes.
function lake(rows: [Row, number][]): Walk {
  const frames = rows
    .flatMap(([row, n]) => Array.from({ length: n }, () => row))
    .map((row, i) => {
      const tilt = (row.tilt * Math.PI) / 180;
      return {
        frame: i,
        fighters: [{ who: "you", swimming: row.swimming, shows: row.shows, torso: [0, Math.cos(tilt), Math.sin(tilt)], going: row.going }],
      } as unknown as Frame;
    });
  return { name: "lake", frames, route: [], ground: [] };
}

const wade: Row = { swimming: false, shows: "Jog_Fwd_Loop", tilt: 5, going: [0, 0, 3] };
const tread: Row = { swimming: true, shows: "Swim_Tread_Loop", tilt: 5, going: [0, 0, 0.1] };
const stroke: Row = { swimming: true, shows: "Swim_Fwd_Loop", tilt: 80, going: [0, 0, 1.2] };
// Diving 45 deg down along +Z: lying 135 deg from upright.
const dive: Row = { swimming: true, shows: "Swim_Dive_Loop", tilt: 135, going: [0, -1, 1] };

test("a swim that treads upright, strokes level and dives along its way passes", () => {
  const got = pose(lake([[wade, 60], [tread, 90], [stroke, 120], [dive, 120], [stroke, 60]]));
  assert.ok(got.ok, got.line);
  assert.equal(got.value, 0);
});

test("a stroke shown standing up fails", () => {
  const got = pose(lake([[tread, 90], [{ ...stroke, tilt: 0 }, 120]]));
  assert.ok(!got.ok);
  assert.ok(Math.abs(got.value - (120 - EASE) / (90 - EASE + 120 - EASE)) < 1e-9, got.line);
});

test("a swimmer showing a walk's clip fails", () => {
  const got = pose(lake([[{ ...stroke, shows: "Jog_Fwd_Loop" }, 120]]));
  assert.equal(got.value, 1);
});

test("a dive lying level rather than along its way fails, one within the bar passes", () => {
  assert.ok(!pose(lake([[{ ...dive, tilt: 90 }, 120]])).ok);
  assert.ok(pose(lake([[{ ...dive, tilt: 135 - POSE_DEG + 1 }, 120]])).ok);
});

test("the ease after a change is not judged; no swimming at all is a failure, not a pass", () => {
  // Upright for the first half second of a stroke (the figure lying down), then level.
  assert.ok(pose(lake([[tread, 90], [{ ...stroke, tilt: 0 }, EASE - 1], [stroke, 120]])).ok);
  assert.ok(!pose(lake([[wade, 120]])).ok);
});

test("a sharp turn of the way is eased into; a figure that never follows it fails after", () => {
  // Diving 45 deg down, then pulled straight up (rising): the figure lies along the dive for
  // the ease, then stands.
  const up: Row = { ...dive, going: [0, 1.5, 0] };
  assert.ok(pose(lake([[dive, 120], [up, EASE - 1], [{ ...up, tilt: 0 }, 60]])).ok);
  assert.ok(!pose(lake([[dive, 120], [up, 120]])).ok);
});
