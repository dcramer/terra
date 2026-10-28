// yard-checks.ts on made-up walks: each check holds on a clean walk and finds the fault
// planted in one. node --test tools/controller/yard-checks.test.ts
import assert from "node:assert/strict";
import { test } from "node:test";
import type { Frame } from "../sandbox/duel/checks.ts";
import { type Walk, aloft, drops, feet, glide, ground, over, sinking, smooth, steps, turn, yard } from "./yard-checks.ts";

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

test("the wait after the route's last mark is not stuck; a wait short of it is", () => {
  // The last mark (12 m east) is reached at frame 151; standing from then on is after it.
  const route = ["reached 3,0", "reached 12,0"];
  const waits = (from: number, to: number) => walk(300, course).map((f, i) => (i > from && i < to ? { ...f, fighters: [{ ...f.fighters[0], speed: 0 }] } : f));
  assert.ok(!/stuck/.test(yard([of(waits(151, 300), route)]).line));
  assert.match(yard([of(waits(10, 150), route)]).line, /stuck at frame/);
});

/// A walk on the spot whose left ball is, frame by frame, `x` m east, `up` m over the shown
/// root and `sole` m over the world's ground under it (no \`sole\` in the record with `record`
/// false: the duel's and older records).
function footing(rows: { x: number; up: number; sole: number }[], record = true): Walk {
  const frames = walk(rows.length).map((f, i) => {
    const body = f.fighters[0];
    const { x, up, sole } = rows[i];
    const extra = record ? { sole: [[sole + 0.04, sole], null] } : {};
    return { ...f, fighters: [{ ...body, feet: [0, 0, 0], speed: 0, balls: [{ at: [x, up, 0.1], up }, null], ...extra } as unknown as Frame["fighters"][number]] };
  });
  return of(frames);
}
const span = (n: number, row: (i: number) => { x: number; up: number; sole: number }) => Array.from({ length: n }, (_, i) => row(i));

test("contact is the ball over the world's ground: a swing lowered under the root is no plant", () => {
  // Planted 30 frames, then swinging 0.6 m on over a lower tread, lowered with the pelvis to
  // the plant's height over the root but 0.35 m over the tread.
  const rows = [...span(30, () => ({ x: 0, up: 0.04, sole: 0.04 })), ...span(20, (i) => ({ x: 0.03 * (i + 1), up: 0.04, sole: 0.35 }))];
  assert.match(feet([footing(rows)]).line, /1 plants, none slid/);
  // Read over the root, as the duel and older records are, it is one plant sliding 60 cm.
  assert.match(feet([footing(rows, false)]).line, /1 of 1 plants slid past 3 cm, median 60.0 cm/);
});

test("a lock dragged 5 cm on a lower tread is caught", () => {
  const rows = [
    ...span(30, () => ({ x: 0, up: 0.04, sole: 0.04 })),
    ...span(10, (i) => ({ x: 0.06 * (i + 1), up: 0.2, sole: 0.2 })),
    ...span(20, (i) => ({ x: 0.6 + (0.05 * i) / 19, up: -0.36, sole: 0.04 })),
  ];
  const check = feet([footing(rows)]);
  assert.ok(!check.ok && Math.abs(check.value - 0.05) < 1e-9, check.line);
  assert.match(check.line, /1 of 2 plants slid/);
});

test("a ball in the ground is no contact: the plants either side of it are two, the frames counted", () => {
  // A free foot through a tread 10 cm for 5 frames between two plants 30 cm apart.
  const rows = [
    ...span(20, () => ({ x: 0, up: 0.04, sole: 0.04 })),
    ...span(5, (i) => ({ x: 0.05 * (i + 1), up: 0.04, sole: -0.1 })),
    ...span(20, () => ({ x: 0.3, up: 0.04, sole: 0.04 })),
  ];
  const check = feet([footing(rows)]);
  assert.ok(check.ok && check.value === 0, check.line);
  assert.match(check.line, /2 plants, none slid past 3 cm, 5 frames a ball past 2 cm in the ground \(no contact; worst 10.0 cm, left foot, frame 20\)/);
  assert.match(feet([footing(rows, false)]).line, /1 of 1 plants slid/);
});

/// A walk standing on its left foot, locked 30 frames: its heel and ball `sole(i)` m over the
/// world's ground, its clip posing them 0.08 and 0.04 m over its flat ground, its leg short of
/// its goal by `short` and of the lock's goal alone by `lock`.
function standing(sole: (i: number) => number[], short = 0, lock = 0, record = true): Walk {
  const frames = walk(30).map((f, i) => {
    const body = f.fighters[0];
    const extra = record ? { sole: [sole(i), [0.3, 0.3]], clip: [[0.08, 0.04], [0.3, 0.3]], short: [short, 0], short_lock: [lock, 0] } : {};
    return { ...f, fighters: [{ ...body, balls: [{ at: [0, 0.04, 0.1], up: 0.04 }, null], kept: ["planted", "free"], ...extra } as unknown as Frame["fighters"][number]] };
  });
  return of(frames);
}

test("a locked sole where its clip has it passes; floating, pitched or sunk fails", () => {
  const level = ground([standing(() => [0.08, 0.04])]);
  assert.ok(level.ok && level.value < 1e-9, level.line);
  // Left at the root's level over a tread 0.3 m lower: it floats.
  const floats = ground([standing(() => [0.38, 0.34])]);
  assert.ok(!floats.ok && Math.abs(floats.value - 0.3) < 1e-9 && /1 on steps/.test(floats.line), floats.line);
  // Flat on a slope: the heel 4 cm into it, the ball as the clip has it.
  const pitched = [standing(() => [0.04, 0.04])];
  assert.ok(!ground(pitched).ok && Math.abs(ground(pitched).value - 0.04) < 1e-9, ground(pitched).line);
  assert.ok(!sinking(pitched).ok && Math.abs(sinking(pitched).value - 0.04) < 1e-9, sinking(pitched).line);
  // Sunk a frame alone: the plant's median holds, sinking's worst frame does not.
  const once = [standing((i) => (i === 10 ? [0.08, 0.0] : [0.08, 0.04]))];
  assert.ok(ground(once).ok && !sinking(once).ok, sinking(once).line);
  assert.ok(sinking([standing(() => [0.38, 0.34])]).ok);
  assert.match(ground([standing(() => [0.08, 0.04], 0, 0, false)]).line, /no locked plant/);
});

/// `standing`, the record also giving the ground under heel and ball over the root (`under`)
/// and the heel 0.15 m behind the ball (`span`).
function straddling(sole: number[], under: number[]): Walk {
  const walk = standing(() => sole);
  const frames = walk.frames.map((f) => ({ ...f, fighters: f.fighters.map((w) => ({ ...w, under: [under, null], span: [0.15, null] })) }));
  return of(frames as Frame[]);
}

test("a sole straddling a riser rests on the higher tread; floating, sunk or hanging there fails", () => {
  const near = (check: { value: number }, want: number) => Math.abs(check.value - want) < 1e-9;
  // Going up: the ball on the upper tread as the clip has it, the heel over a tread 0.5 m lower
  // at the clip's height over the upper one (a rigid sole resting on the higher ground).
  const up = ground([straddling([0.58, 0.04], [-0.5, 0])]);
  assert.ok(up.ok && up.value < 1e-9, up.line);
  // Going down: the heel on the upper tread, the ball over the lower.
  assert.ok(ground([straddling([0.08, 0.54], [0, -0.5])]).ok);
  // The whole sole 10 cm up over the riser: the higher point floats.
  const floats = ground([straddling([0.68, 0.14], [-0.5, 0])]);
  assert.ok(!floats.ok && near(floats, 0.1), floats.line);
  // The ball 3 cm into the upper tread: off, and sunk by its own reading.
  const sunk = [straddling([0.55, 0.01], [-0.5, 0])];
  assert.ok(!ground(sunk).ok && near(ground(sunk), 0.03), ground(sunk).line);
  assert.ok(!sinking(sunk).ok && near(sinking(sunk), 0.03), sinking(sunk).line);
  // The heel hanging 30 cm over the lower tread, on neither tread's level: off by 22 cm.
  const hangs = ground([straddling([0.3, 0.04], [-0.5, 0])]);
  assert.ok(!hangs.ok && near(hangs, 0.22), hangs.line);
  // Level ground, and a slope the sole pitches to (3 cm over 15 cm, 11 deg): own readings, so
  // a floating sole and a heel 4 cm into the slope still fail.
  const level = ground([straddling([0.38, 0.34], [0, 0])]);
  assert.ok(!level.ok && near(level, 0.3), level.line);
  const slope = [straddling([0.04, 0.04], [-0.03, 0])];
  assert.ok(!ground(slope).ok && near(ground(slope), 0.04) && !sinking(slope).ok, ground(slope).line);
});

test("feet.aloft fails a foot locked in the air, not a heel strike or a stance's end", () => {
  assert.ok(aloft([standing(() => [0.08, 0.04])]).ok);
  // Locked 20 cm up, coming down over 5 frames (a stop's lock): the lock's start is aloft.
  const air = aloft([standing((i) => (i < 5 ? [0.28 - i * 0.04, 0.24 - i * 0.04] : [0.08, 0.04]))]);
  assert.ok(!air.ok && Math.abs(air.value - 0.2) < 1e-9, air.line);
  // A heel strike (the ball up, the heel down) and a toe-off at the end (the heel up, then both).
  const strike = (i: number) => (i < 3 ? [0.08, 0.15] : i > 26 ? [0.2, 0.15] : [0.08, 0.04]);
  assert.ok(aloft([standing(strike)]).ok, aloft([standing(strike)]).line);
});

test("pelvis.drop counts the shortfall placement adds, not the lock's own", () => {
  const [p95, worst] = drops([standing(() => [0.08, 0.04], 0.05, 0.05)]);
  assert.ok(p95.ok && worst.ok && p95.value === 0 && /lock alone leaves p95 5.0 cm/.test(p95.line), p95.line);
  const [placed, placedWorst] = drops([standing(() => [0.08, 0.04], 0.07, 0.02)]);
  assert.ok(!placed.ok && Math.abs(placed.value - 0.05) < 1e-9 && !placedWorst.ok, placed.line);
  const frames = standing(() => [0.08, 0.04]).frames.map((f) => ({ ...f, fighters: f.fighters.map((w) => ({ ...w, short_lock: undefined })) }));
  assert.match(drops([of(frames as Frame[])])[0].line, /without the leg's shortfalls/);
});

test("feet.glide fails a foot its clip has down left free as a swing on the ground, not a lock, a step or a foot in the air", () => {
  // The left foot: its clip locks it from frame 10, the lock leaves it free as a swing for 6
  // frames (a fade carrying it 5 cm a frame along the ground), then plants it.
  const glided = (why: string, speed: number, kept = "free", up = 0.04) =>
    of(
      walk(30).map((frame, i) => {
        const at = [Math.min(Math.max(i - 10, 0), 5) * 0.05, 0, 0.1];
        const ball = i >= 10 && i < 16 ? up : 0.04;
        const w = { ...frame.fighters[0], speed, balls: [{ at }, { at: [0, 0, -0.1] }], lock: [i >= 10 ? [1, 1] : [0, 0], [1, 1]], why: [i >= 10 && i < 16 ? why : "", ""], kept: [i >= 16 ? "planted" : kept, "planted"], sole: [[0.08, ball], [0.08, 0.04]] };
        return { ...frame, fighters: [w as unknown as Frame["fighters"][number]] };
      }),
    );
  const slid = glide([glided("swing", 4.2)]);
  assert.ok(!slid.ok && Math.abs(slid.value - 0.25) < 1e-9, slid.line);
  assert.ok(glide([glided("", 4.2)]).ok);
  assert.ok(glide([glided("high", 4.2)]).ok);
  // The body stopped at a wall, the fade dragging the foot: a glide all the same.
  assert.ok(!glide([glided("swing", 0)]).ok);
  assert.ok(!glide([glided("swing", 4.2, "blending")]).ok);
  // The rule's step lifts the foot over the way: no glide.
  assert.ok(glide([glided("swing", 4.2, "stepping")]).ok);
  // A foot coming down through the air 20 cm up: no glide.
  assert.ok(glide([glided("swing", 4.2, "free", 0.24)]).ok);
});
