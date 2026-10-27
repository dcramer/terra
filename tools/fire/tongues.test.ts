// The tongue tracker's numbers on masks whose answers are known by construction.
import assert from "node:assert/strict";
import { test } from "node:test";
import { framePieces, leanOf, trackTongues } from "./tongues.ts";

const FPS = 30;
const [W, H] = [160, 240];
const g = { base: 229, height: 160, x0: 0, x1: W - 1 }; // tongues from row 149 (half of H) up
const N = 130;

/// A fire's masks, a 30-frame cycle from frame 5 (frames 0-4 are a cycle's end), four whole:
/// a body (rows 170-229) under two tongues. Tongue A (x 40-50, rows 90-170) stands for 10 frames,
/// then its tip (rows 90-110) comes off as a blob rising 3 px a frame while its foot (rows
/// 117-170) stays, both gone after 10 more frames. Tongue B (x 118-128, rows 100-170) stands
/// all through; for 10 frames from the cycle's 15th it forks into prongs (x 108-114 and 124-132,
/// rows 100-155), the left one then gone till the next cycle.
function fire(): Uint8Array[] {
  const masks: Uint8Array[] = [];
  const box = (m: Uint8Array, x0: number, x1: number, y0: number, y1: number) => {
    for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) m[y * W + x] = 1;
  };
  for (let t = 0; t < N; t++) {
    const k = (((t - 5) % 30) + 30) % 30;
    const m = new Uint8Array(W * H);
    box(m, 30, 140, 170, 229);
    if (k < 10) box(m, 40, 50, 90, 170);
    else if (k < 20) {
      box(m, 40, 50, 117, 170);
      box(m, 40, 50, 90 - 3 * (k - 10), 110 - 3 * (k - 10));
    }
    if (k >= 15 && k < 25) {
      box(m, 108, 114, 100, 155);
      box(m, 124, 132, 100, 170);
      box(m, 108, 132, 156, 170);
    } else if (k >= 25) box(m, 124, 132, 100, 170);
    else box(m, 118, 128, 100, 170);
    masks.push(m);
  }
  return masks;
}

/// Attached tongue-seconds: B always, its second prong for 10 frames a cycle, A's tongue and
/// then its foot for 20.
const tongueSeconds = (frames: number[]) =>
  frames.reduce((n, t) => {
    const k = (((t - 5) % 30) + 30) % 30;
    return n + 1 + (k >= 15 && k < 25 ? 1 : 0) + (k < 20 && t >= 5 ? 1 : 0);
  }, 0) / FPS;

const near = (got: number, want: number, within: number, what: string) => assert.ok(Math.abs(got - want) <= within, `${what}: ${got} is not within ${within} of ${want}`);

test("tongues' lifetimes, sheds and splits are found, and a third of the speed reads three times the life", () => {
  const masks = fire();
  const m = trackTongues(masks, W, H, g, FPS, 0.5, 0.0025);
  // Whole tracks: A 20 frames, its blob 10, B's left prong 10, four of each.
  near(m.lifetime, (20 + 10 + 10) / 3 / FPS, 0.01, "mean lifetime, s");
  assert.equal(m.tracks, 12, "whole tracks");
  const seconds = tongueSeconds(Array.from({ length: N }, (_, t) => t));
  near(m.sheds, 4 / seconds, 1e-9, "sheds per tongue a second");
  near(m.splits, 4 / seconds, 1e-9, "splits per tongue a second");
  near(m.shedLife, 10 / FPS, 0.01, "a shed blob's life, s");
  near(m.shedRise, (3 * FPS) / g.height, 0.05, "a shed blob's rise, H/s");
  // The same frames each shown three times: a third of the speed.
  const slow = Array.from({ length: 3 * N }, (_, i) => masks[Math.floor(i / 3)]);
  const s = trackTongues(slow, W, H, g, FPS, 0.5, 0.0025);
  near(s.lifetime / m.lifetime, 3, 0.1, "lifetime slowed / at speed");
  near(s.sheds / m.sheds, 1 / 3, 0.02, "sheds slowed / at speed");
  near(s.splits / m.splits, 1 / 3, 0.02, "splits slowed / at speed");
});

test("a tongue's lean is its foot-to-tip line's angle from vertical", () => {
  const px: number[] = [];
  const lean = 20;
  for (let y = 40; y <= 100; y++) {
    const x0 = Math.round(80 + (100 - y) * Math.tan((lean * Math.PI) / 180));
    for (let x = x0; x < x0 + 8; x++) px.push(y * W + x);
  }
  near(leanOf(px, W, g.height), lean, 1.5, "lean, degrees");
  const upright = px.map((p) => Math.floor(p / W) * W + 80 + (p % W) - Math.round(80 + (100 - Math.floor(p / W)) * Math.tan((lean * Math.PI) / 180)));
  near(leanOf(upright, W, g.height), 0, 0.5, "upright, degrees");
});

test("a frame's sheared tongue reads its shear next to an upright one off the same body", () => {
  // Fire round g63: torn upright lanes cut a leaning plume into upright tongues, and the motion
  // gate's lean fell; so a frame of both must read each one's own angle, not their mix.
  const mask = new Uint8Array(W * H);
  const level = g.base - g.height / 2;
  const lean = 20;
  for (let y = level; y <= g.base; y++) for (let x = 30; x <= 130; x++) mask[y * W + x] = 1;
  for (let y = 60; y < level; y++) for (let x = 40; x < 48; x++) mask[y * W + x] = 1;
  for (let y = 70; y < level; y++) {
    const x0 = Math.round(100 + (level - 1 - y) * Math.tan((lean * Math.PI) / 180));
    for (let x = x0; x < x0 + 8; x++) mask[y * W + x] = 1;
  }
  const tongues = framePieces(mask, W, H, level, 50, g.height).filter((t) => t.attached && Number.isFinite(t.lean));
  assert.equal(tongues.length, 2, "two tongues measured");
  const [left, right] = [...tongues].sort((a, b) => a.cx - b.cx);
  near(left.lean, 0, 1, "upright, degrees");
  near(right.lean, lean, 1.5, "sheared, degrees");
});
