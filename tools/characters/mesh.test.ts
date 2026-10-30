import assert from "node:assert/strict";
import { test } from "node:test";
import type { V3 } from "./glb.ts";
import { type Frame, RATE, feet, fit, heldFrames, mesh, parse, strike } from "./mesh.ts";

/** A body standing, its left foot 40 cm ahead, as `change` moves it at frame `i`. */
const stand = (i: number, change: (f: Frame) => void = () => {}): Frame => {
  const f: Frame = {
    t: i / RATE,
    ball: [[0.2, 0.04, 0.5], [-0.2, 0.04, 0.1]],
    heel: [[0.2, 0.1, 0.35], [-0.2, 0.1, -0.05]],
    pelvis: [0, 0.9, 0.1],
    hip: [[0.1, 0.9, 0.1], [-0.1, 0.9, 0.1]],
    shoulder: [[0.2, 1.4, 0.1], [-0.2, 1.4, 0.1]],
    hand: [[0.3, 1, 0.3], [-0.3, 1, 0.3]],
  };
  change(f);
  return f;
};
const guard = stand(0);
const take = (n: number, change: (f: Frame, i: number) => void) => Array.from({ length: n }, (_, i) => stand(i, (f) => change(f, i)));
const move = (p: V3, x: number, y: number, z: number) => {
  p[0] += x;
  p[1] += y;
  p[2] += z;
};
const turn = (p: V3, a: number): V3 => [p[0] * Math.cos(a) + p[2] * Math.sin(a), p[1], -p[0] * Math.sin(a) + p[2] * Math.cos(a)];

test("the guard's stance turned and shifted along the ground fits, and says the turn back", () => {
  const a = (25 * Math.PI) / 180;
  const f = stand(0, (x) => {
    for (const k of ["ball", "heel", "hip", "shoulder", "hand"] as const) x[k] = x[k].map((p) => turn(p, a).map((c, i) => c + [1, 0, 2][i])) as [V3, V3];
    x.pelvis = turn(x.pelvis, a).map((c, i) => c + [1, 0, 2][i]) as V3;
  });
  const got = fit(f, guard);
  assert.ok(Math.max(...got.feet) < 0.01, `feet off by ${got.feet}`);
  assert.ok(Math.abs(got.turn + a) < 0.03, `turn ${got.turn}`);
});

test("another stance does not fit: feet level read off the guard's by half the stagger", () => {
  const f = stand(0, (x) => {
    move(x.ball[0], 0, 0, -0.2);
    move(x.heel[0], 0, 0, -0.2);
    move(x.ball[1], 0, 0, 0.2);
    move(x.heel[1], 0, 0, 0.2);
  });
  assert.ok(Math.max(...fit(f, guard).feet) > 0.12);
});

test("hips lower than the guard's are reported, feet or not", () => {
  const f = stand(0, (x) => move(x.pelvis, 0, -0.2, 0));
  assert.ok(Math.abs(fit(f, guard).hips + 0.2) < 1e-9);
});

test("a standing foot's creep is its slip, a pivot on the ball is none", () => {
  const n = RATE;
  const creep = feet(take(n, (f, i) => {
    move(f.ball[0], (0.06 * i) / n, 0, 0);
    move(f.heel[0], (0.06 * i) / n, 0, 0);
  }));
  // The stand's first and last 0.05 s are trimmed: 0.9 of 6 cm.
  assert.ok(Math.abs(creep.slip[0] - 0.054) < 0.002, `slip ${creep.slip[0]}`);
  assert.equal(creep.slip[1], 0);
  const pivot = feet(take(n, (f, i) => move(f.heel[0], (0.2 * i) / n, 0, 0)));
  assert.ok(pivot.slip[0] < 0.002, `slip ${pivot.slip[0]}`);
});

/** The left foot steps `m` ahead over frames 30-60, its ball rising `up` at the middle. */
const step = (m: number, up: number) =>
  take(120, (f, i) => {
    const u = Math.min(1, Math.max(0, (i - 30) / 30));
    for (const p of [f.ball[0], f.heel[0]]) move(p, 0, up * Math.sin(u * Math.PI), m * u);
  });

test("a step that lifts is a step; one that stays on the floor is dragged", () => {
  const lifted = feet(step(0.5, 0.08));
  assert.equal(lifted.steps.length, 1);
  assert.ok(Math.abs(lifted.steps[0].m - 0.5) < 0.05);
  assert.ok(Math.abs(lifted.steps[0].lift - 0.08) < 0.001);
  assert.equal(lifted.dragged, 0);
  assert.equal(lifted.swept, 0);
  assert.equal(feet(step(0.5, 0.01)).dragged, 1);
});

test("a foot crossing the floor at sprint speed is swept, however high it lifts later", () => {
  const fr = take(120, (f, i) => {
    const flat = Math.min(1, Math.max(0, (i - 30) / 12));
    const up = i >= 42 && i < 60 ? 0.1 : 0;
    for (const p of [f.ball[0], f.heel[0]]) move(p, 0, up, 1 * flat);
  });
  const got = feet(fr);
  assert.equal(got.swept, 1);
  assert.equal(got.dragged, 0);
});

/** The right hand held, then 10 m/s across for 0.2 s from frame `from`, then held. */
const blow = (f: Frame, i: number, from: number) => move(f.hand[1], 2 * Math.min(1, Math.max(0, (i - from) / (0.2 * RATE))), 0, 0);

test("the strike is the hand's fastest 100 ms in the body's frame, and the body's own travel is off it", () => {
  const got = strike(take(120, (f, i) => {
    blow(f, i, 60);
    const walked = (3 * i) / RATE;
    for (const p of [f.pelvis, ...f.hand]) move(p, 0, 0, walked);
  }));
  assert.ok(Math.abs(got.hand - 10) < 0.5, `hand ${got.hand}`);
  assert.ok(got.peak >= 60 && got.peak <= 84, `peak ${got.peak}`);
  assert.ok(got.left < 0.1);
});

test("a cut never starts on a foot passing the guard's place in mid step", () => {
  // Standing 30 cm wide of the guard; the left foot sweeps through its place at frame 30.
  const fr = take(180, (f, i) => {
    const out = 0.3 - 0.6 * Math.min(1, Math.max(0, (i - 15) / 30));
    move(f.ball[0], out, 0, 0);
    move(f.heel[0], out, 0, 0);
    blow(f, i, 120);
  });
  assert.equal(heldFrames(fr)[30], false);
  assert.equal(heldFrames(fr)[5], true);
  const m = mesh(fr, guard);
  assert.ok(Math.max(...m.start.feet) > 0.1, `start ${m.start.feet}`);
  assert.ok(m.cut <= 15 / RATE || m.cut >= 45 / RATE, `cut ${m.cut}`);
});

test("hands kept together on one grip read as near", () => {
  const m = mesh(take(180, (f, i) => {
    f.hand[0] = [...f.hand[1]] as V3;
    move(f.hand[0], 0.1, 0, 0);
    blow(f, i, 60);
    move(f.hand[0], 2 * Math.min(1, Math.max(0, (i - 60) / (0.2 * RATE))), 0, 0);
  }), guard);
  assert.ok(Math.abs(m.apart - 0.1) < 1e-6);
});

test("a clip is named by file, clip and rate", () => {
  assert.deepEqual(parse("a/b.glb"), { file: "a/b.glb", clip: undefined, rate: 1 });
  assert.deepEqual(parse("a/b.glb:Open_Slash@1.21"), { file: "a/b.glb", clip: "Open_Slash", rate: 1.21 });
  assert.throws(() => parse("nothing"), /file\.glb/);
});
