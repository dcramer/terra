// The motion measure's numbers on clips whose answers are known by construction.
import assert from "node:assert/strict";
import { test } from "node:test";
import { analyse, type Clip, flicker, GATE, type Motion, roughness } from "./motion.ts";

const FPS = 30;
const N = 180;
/// A fixed per-pixel texture in [0.85, 1], so blocks have something to match.
const grain = (x: number, y: number) => {
  let v = Math.imul(x * 73856093 ^ y * 19349663, 0x27d4eb2d) >>> 0;
  v = Math.imul(v ^ (v >>> 15), 0x2c1b3c6d) >>> 0;
  return 0.85 + 0.15 * ((v >>> 8) / (1 << 24));
};

/// The synthetic fire: a pale body (rows 170-229) whose width beats at 2.5 Hz (80 +- 20 px),
/// and three red tongues (10 x 24 px, their grain moving with them) each born at y 110 and rising
/// 2 px a frame (60 px/s) for 30 frames (1 s), staggered by 10 frames, over dark sky and
/// flame-coloured but dim lit ground (rows 230 on) with grain. `shake` moves the picture by
/// round(3 sin(2 pi 0.7 t)) px down, as a hand-held camera does.
function fire(shake = false): Clip {
  const [w, h] = [200, 260];
  const frames: Uint8Array[] = [];
  for (let t = 0; t < N; t++) {
    const s = t / FPS;
    const cam = shake ? Math.round(3 * Math.sin(2 * Math.PI * 0.7 * s)) : 0;
    const bw = Math.round(80 + 20 * Math.sin(2 * Math.PI * 2.5 * s));
    const f = new Uint8Array(w * h * 3);
    for (let y = 0; y < h; y++)
      for (let x = 0; x < w; x++) {
        const wy = y - cam; // the world row seen at y
        let c = [12, 10, 8];
        if (wy >= 230) c = [120, 60, 20].map((v) => v * grain(x, wy));
        else if (wy >= 170 && x >= 100 - bw / 2 && x < 100 + bw / 2) c = [255, 240, 170].map((v) => v * grain(x, wy));
        else
          [68, 95, 122].forEach((x0, k) => {
            const age = (((t - 10 * k) % 30) + 30) % 30;
            const top = 110 - 2 * age - 24;
            if (x >= x0 && x < x0 + 10 && wy >= top && wy < top + 24) c = [255, 130, 60].map((v) => v * grain(x - x0, wy - top));
          });
        f.set(c.map(Math.round), 3 * (y * w + x));
      }
    frames.push(f);
  }
  return { w, h, fps: FPS, frames };
}

const near = (got: number, want: number, within: number, what: string) => assert.ok(Math.abs(got - want) <= within, `${what}: ${got} is not within ${within} of ${want}`);

test("the synthetic fire's flicker, tongues, rise, roughness and colour are found", () => {
  const m = analyse(fire());
  near(m.flicker.dominant, 2.5, 0.1, "dominant Hz");
  // The body's area beats by 60 x 20 px about 60 x 80 px, the tongues add 3 x 240 px.
  near(m.flicker.areaCv, (60 * 20) / Math.SQRT2 / (60 * 80 + 720), 0.01, "area sd/mean");
  near(m.tongues.count, 3, 0.01, "tongues");
  near(m.tongues.lifetime, 1, 0.04, "tongue lifetime");
  near(m.tongues.births, 3, 0.2, "tongues born a second");
  near(m.tongues.rise * m.height, 60, 3, "the tongues' rise, px/s");
  near(m.colour.pale[0], 100, 0.5, "the low band's pale");
  near(m.colour.red[2], 100, 0.5, "the top band's red");
  // The dim lit ground is not flame: the flame's base is the body's lowest row.
  assert.ok(m.height > 150 && m.height < 205, `height ${m.height}`);
});

test("a hand-held camera's shake leaves the tongues' rise", () => {
  const m = analyse(fire(true));
  near(m.tongues.rise * m.height, 60, 4, "the tongues' rise, px/s");
});

test("the run's matte stands in for the luma cut", () => {
  const clip = fire();
  clip.mattes = clip.frames.map((f) => {
    const m = new Uint8Array(clip.w * clip.h);
    for (let p = 0; p < m.length; p++) m[p] = f[3 * p] >= 200 ? 255 : 0;
    return m;
  });
  const m = analyse(clip);
  near(m.tongues.count, 3, 0.01, "tongues");
  near(m.flicker.dominant, 2.5, 0.1, "dominant Hz");
});

/// The synthetic fire with its run's matte, its body's brightness scaled by `core` and its third
/// tongue (x 122-131) dimmed to luma 109-129, under half white.
function matted(core: number): Clip {
  const clip = fire();
  clip.mattes = clip.frames.map((f) => {
    const m = new Uint8Array(clip.w * clip.h);
    for (let p = 0; p < m.length; p++) m[p] = f[3 * p] >= 200 ? 255 : 0;
    return m;
  });
  for (const f of clip.frames)
    for (let p = 0; p < clip.w * clip.h; p++) {
      const [x, y] = [p % clip.w, (p / clip.w) | 0];
      const k = y >= 170 && y < 230 ? core : y < 170 && x >= 122 && x < 132 ? 0.72 : 1;
      for (let c = 0; c < 3; c++) f[3 * p + c] = Math.round(f[3 * p + c] * k);
    }
  return clip;
}

test("a render's cut does not follow its core: a dimmer core keeps no dimmer tip", () => {
  const [bright, dim] = [analyse(matted(1)), analyse(matted(0.78))];
  near(bright.tongues.count, 2, 0.01, "tongues, bright core");
  near(dim.tongues.count, 2, 0.01, "tongues, dim core");
});

test("roughness is outline over root area, and no edge where a band cuts the flame", () => {
  const [w, h] = [40, 30];
  const rect = new Uint8Array(w * h);
  for (let y = 10; y < 20; y++) for (let x = 10; x < 30; x++) rect[y * w + x] = 1;
  near(roughness(rect, w, h), 60 / Math.sqrt(200), 1e-9, "a 20 x 10 rectangle");
  near(roughness(rect, w, h, 0, 15), 30 / Math.sqrt(100), 1e-9, "its upper half");
  const comb = rect.slice();
  for (let y = 10; y < 15; y++) for (let x = 10; x < 30; x += 2) comb[y * w + x] = 0;
  assert.ok(roughness(comb, w, h) > 2 * roughness(rect, w, h), "a comb is rougher");
});

test("flicker finds a beat between the spectrum's bins", () => {
  const series = Array.from({ length: 300 }, (_, i) => 1000 + 100 * Math.sin(2 * Math.PI * 3.25 * (i / FPS)));
  near(flicker(series, FPS).dominant, 3.25, 0.05, "dominant Hz");
});

test("the gate's flicker is the median: a narrow swell beside broad puffing passes, a swell alone does not", () => {
  // A 0.5 Hz swell of amplitude 100 and six puffs of amplitude `puff` on the spectrum's bins
  // (2.0-2.83 Hz, a sixth of a hertz apart). Power goes as amplitude squared and the Hann window
  // gives a bin's sine 2/3 of its power and each neighbour 1/6, so with puffs of 60 the swell
  // peaks (6667 against 3600) while the half of the power (15800 of 31600) is reached in the
  // second puff's bin, 13/6 Hz; with puffs of 20 the swell holds the half alone.
  const series = (puff: number) =>
    Array.from({ length: 600 }, (_, i) => {
      const s = i / FPS;
      let v = 1000 + 100 * Math.sin(2 * Math.PI * 0.5 * s);
      for (let k = 12; k < 18; k++) v += puff * Math.sin(2 * Math.PI * (k / 6) * s + k);
      return v;
    });
  const flick = GATE.find((g) => g.name === "flick")!;
  const passes = (f: ReturnType<typeof flicker>) => {
    const v = flick.of({ flicker: { dominant: f.dominant, median: f.median, areaCv: f.cv, heightCv: 0 } } as Motion);
    return v >= flick.lo && v <= flick.hi;
  };
  const broad = flicker(series(60), FPS);
  near(broad.dominant, 0.5, 0.05, "dominant Hz");
  near(broad.median, 13 / 6, 0.01, "median Hz");
  assert.ok(passes(broad), "puffing under a swell meets the flicker bar");
  const swell = flicker(series(20), FPS);
  near(swell.median, 0.5, 0.01, "median Hz");
  assert.ok(!passes(swell), "a swell alone does not");
});
