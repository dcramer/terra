// yard-checks.ts on made-up walks: each check holds on a clean walk and finds the fault
// planted in one. node --test tools/controller/yard-checks.test.ts
import assert from "node:assert/strict";
import { test } from "node:test";
import type { Frame } from "../sandbox/duel/checks.ts";
import { type Walk, feet, over, smooth, steps, turn, yard } from "./yard-checks.ts";

/// A walk east at 4.2 m/s on level ground at 0 for `n` frames; `lift(i)` raises the body,
/// `below(i)` how far under it the figure is shown, `camera(i)` the camera's height.
function walk(n: number, lift = (_: number) => 0, below = (_: number) => 0, camera = (i: number) => 1.6 + lift(i) - below(i)): Frame[] {
  return Array.from({ length: n }, (_, i) => ({
    frame: i,
    step: null,
    asked: [],
    contacts: [],
    camera: { at: [0, camera(i), 0], looking: [1, 0, 0], up: [0, 1, 0], vfov: 50, aspect: 1.78 },
    fighters: [
      {
        who: "you",
        feet: [(i * 4.2) / 60, lift(i), 0],
        facing: [1, 0, 0],
        move: null,
        t: 0,
        phase: null,
        cancels: false,
        reach: false,
        guard: false,
        stop_s: 0,
        stagger_s: 0,
        down: false,
        free: true,
        radius: null,
        balls: [],
        below: below(i),
        speed: 4.2,
        on_ground: true,
      } as Frame["fighters"][number],
    ],
  }));
}
const flat = Array.from({ length: 76 }, () => 0);
const of = (frames: Frame[], route = ["reached 10,0", "reached 50,0"]): Walk => ({ name: "jog", frames, route, ground: flat });

/// A 0.4 m step at frame 100: the body rises at once, the figure shown that much lower and
/// rising to it over 0.12 s (stepping.rs), or not at all (a pop).
const up = (i: number) => (i >= 100 ? 0.4 : 0);
const eased = (i: number) => (i >= 100 ? Math.max(0, 0.4 - ((i - 99) / 60) * (0.4 / 0.12)) : 0);

test("a smoothed step passes; one shown at once is a pop", () => {
  const good = smooth([of(walk(300, up, eased))]);
  assert.ok(good.ok, good.line);
  assert.equal(steps(walk(300, up, eased)).length, 1);
  assert.ok(good.value <= 0.25 && good.value > 0.1, good.line);
  const popped = smooth([of(walk(300, up))]);
  assert.ok(!popped.ok);
  assert.ok(Math.abs(popped.value - 1) < 1e-9, popped.line);
});

test("a camera that pops is caught though the figure is smoothed", () => {
  const camera = smooth([of(walk(300, up, eased, (i) => 1.6 + up(i)))]);
  assert.ok(!camera.ok, camera.line);
});

test("a figure that never catches its body up settles late", () => {
  const stuck = (i: number) => (i >= 100 ? 0.1 : 0);
  const late = smooth([of(walk(300, up, stuck))]);
  assert.ok(!late.ok && late.line.includes("settled after"), late.line);
});

/// The course walked: down to the trench's floor and up on the deck.
const course = (i: number) => (i >= 100 && i < 120 ? -1.5 : i >= 150 && i < 170 ? 1.5 : 0);

test("the yard: clean, a mark given up, stuck, through the ground, the mound climbed", () => {
  const clean = yard([of(walk(300, course))]);
  assert.ok(clean.ok, clean.line);
  assert.equal(yard([of(walk(300, course), ["reached 10,0", "gave up 50,0, 3 m short"])]).value, 1);
  assert.match(yard([of(walk(300))]).line, /trench's floor.*deck/);
  const still = walk(300, course).map((f, i) => (i > 50 && i < 200 ? { ...f, fighters: [{ ...f.fighters[0], speed: 0 }] } : f));
  assert.match(yard([of(still)]).line, /stuck/);
  assert.match(yard([of(walk(300, (i) => (i > 200 ? -3 : course(i))))]).line, /under the ground/);
  // Feet 1 m up over the mound's middle.
  const climb = walk(300, course).map((f, i) => (i === 150 ? { ...f, fighters: [{ ...f.fighters[0], feet: [50, 1.0, 0] }] } : f));
  assert.match(yard([of(climb)]).line, /mound/);
});

test("heights are over the ground as it was, between the metres logged", () => {
  assert.equal(over([0, 1, 2], [1.5, 2, 0]), 0.5);
  assert.equal(over([0, 1, 2], [9, 2, 0]), 0);
});

test("a foot sliding while the body turns in place is turn.foot's, not feet sliding's", () => {
  // Standing still a second, turning 90 deg, its left ball sliding 10 cm on the ground; then
  // walking on with it sliding as much.
  const turning = (moves: boolean): Frame[] =>
    walk(60).map((f, i) => {
      const yaw = moves ? 0 : (Math.PI / 2) * (i / 59);
      const body = f.fighters[0];
      return {
        ...f,
        fighters: [
          {
            ...body,
            feet: moves ? body.feet : [0, 0, 0],
            facing: [Math.sin(yaw), 0, Math.cos(yaw)],
            balls: [{ at: [(moves ? body.feet[0] : 0) + (0.1 * i) / 59, 0.03, 0.1], up: 0.03 }, null],
          } as unknown as Frame["fighters"][number],
        ],
      };
    });
  const still = of(turning(false));
  assert.match(turn([still]).line, /1 of 1 turning plants slid/);
  assert.match(feet([still]).line, /0 plants, none slid/);
  const on = of(turning(true));
  assert.match(feet([on]).line, /1 of 1 plants slid/);
  assert.match(turn([on]).line, /0 turning plants/);
});
