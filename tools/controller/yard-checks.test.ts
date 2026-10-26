// yard-checks.ts on made-up walks: each check holds on a clean walk and finds the fault
// planted in one. node --test tools/controller/yard-checks.test.ts
import assert from "node:assert/strict";
import { test } from "node:test";
import type { Frame } from "../sandbox/duel/checks.ts";
import { type Walk, feet, ground, over, reach, smooth, steps, turn, yard } from "./yard-checks.ts";

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

/// A walk standing on its left foot (locked, its ball 5 cm over its sole): on level ground at
/// the root for 30 frames, then on a tread `under` m lower, the ball `sole` m over that tread,
/// the leg `short` m short of its foot's place there.
function onTread(under: number, sole: number, short: number): Walk {
  const frames = walk(60).map((f, i) => {
    const low = i >= 30;
    const g = low ? -under : 0;
    const body = f.fighters[0];
    const up = g + (low ? sole : 0.05);
    return {
      ...f,
      fighters: [
        {
          ...body,
          balls: [{ at: [1, up, 0.1], up }, { at: [0, 0.3, 0], up: 0.3 }],
          ground: [g, 0],
          kept: [low && i === 30 ? "stepping" : "planted", "free"],
          short: [low ? short : 0, 0],
        } as unknown as Frame["fighters"][number],
      ],
    };
  });
  return of(frames);
}

test("a planted foot off the ground under it is feet.ground's; a leg short of it pelvis.drop's", () => {
  const put = ground([onTread(0.3, 0.05, 0)]);
  assert.ok(put.ok && put.value < 1e-9 && /1 on steps/.test(put.line), put.line);
  // Its sole floating where the clip had it, over the root's level, 0.3 m over the tread.
  const floats = ground([onTread(0.3, 0.35, 0)]);
  assert.ok(!floats.ok && Math.abs(floats.value - 0.3) < 1e-6, floats.line);
  assert.ok(reach([onTread(0.3, 0.05, 0)]).ok);
  const short = reach([onTread(0.3, 0.05, 0.12)]);
  assert.ok(!short.ok && Math.abs(short.value - 0.12) < 1e-9, short.line);
});

test("feet sliding reads a ball's height over the ground under it, not over the root", () => {
  // On a tread 0.3 m over the root the ball slides 10 cm: over the root it reads lifted, a
  // step, and nothing slid; over its own ground it is the same plant, and slid.
  const tread = onTread(-0.3, 0.05, 0);
  const slides = { ...tread, frames: tread.frames.map((f, i) => ({ ...f, fighters: [{ ...f.fighters[0], balls: [{ ...f.fighters[0].balls[0], at: [i >= 30 ? 1.1 : 1, 0, 0.1] }, null] }] })) } as unknown as Walk;
  assert.match(feet([slides]).line, /1 of 1 plants slid/);
});
