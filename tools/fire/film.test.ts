// The motion films' held hearth light (film.ts, D472).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { filmArgs, hold, HOURS, PLACEMENTS, REFERENCE, TAKES, takes } from "./film.ts";
import { GATE } from "./motion.ts";
import { type Film, paired, spread, told, tolerance, TOLERANCE } from "./paired.ts";
import { HELD_SKY } from "./sheet.ts";

const REF = { HEARTH_A: "1.0", HEARTH_C: "Color::srgb(1.0, 0.5, 0.1)" };

test("every hearth constant is set to its reference, and only those", () => {
  const src = ["/// HEARTH_A in a comment: const HEARTH_A: f32 = 9.0;", "const HEARTH_A: f32 = 3.5;", "pub const HEARTH_C: Color = Color::srgb(1.0, 0.6, 0.1);", "const TORCH_A: f32 = 3.5;"].join("\n");
  const { source, changed } = hold(src, REF);
  assert.equal(source, ["/// HEARTH_A in a comment: const HEARTH_A: f32 = 9.0;", "const HEARTH_A: f32 = 1.0;", "pub const HEARTH_C: Color = Color::srgb(1.0, 0.5, 0.1);", "const TORCH_A: f32 = 3.5;"].join("\n"));
  assert.equal(changed.length, 2);
  assert.deepEqual(hold(source, REF).changed, []);
});

test("a hearth constant added or gone fails the hold", () => {
  assert.throws(() => hold("const HEARTH_A: f32 = 1.0;\nconst HEARTH_C: Color = x;\nconst HEARTH_B: f32 = 2.0;", REF), /HEARTH_B is not in/);
  assert.throws(() => hold("const HEARTH_A: f32 = 1.0;", REF), /no HEARTH_C/);
});

test("the gate films each placement twice or more, each film in a folder of its own", () => {
  const all = takes("o");
  assert.ok(TAKES >= 2, "two films of one build differ, so a placement is filmed more than once");
  assert.equal(all.length, PLACEMENTS.length * TAKES);
  assert.equal(new Set(all.map((t) => t.dir)).size, all.length);
  for (const dx of PLACEMENTS) assert.equal(all.filter((t) => t.dx === dx).length, TAKES);
  assert.deepEqual(all.slice(6, 8).map((t) => t.dir), ["o/p09", "o/p09-2"]);
});

test("the gate films at least seven placements, each a flame seed of its own", () => {
  // hearth.rs's seed of a fire at (x, z), the gate's at (-10 + dx, 10).
  const seed = (dx: number) => ((((-10 + dx) * 0.731 + 10 * 0.377) % 1) + 1) % 1;
  assert.ok(PLACEMENTS.length >= 7, `${PLACEMENTS.length} placements: seven or more`);
  const seeds = PLACEMENTS.map(seed);
  for (let i = 0; i < seeds.length; i++)
    for (let j = i + 1; j < seeds.length; j++) {
      const apart = Math.abs(seeds[i] - seeds[j]);
      // sim.rs hashes the seed's bits into the loop's offset, mirror, turn and scale, so seeds
      // apart at all are independent draws; 0.01 keeps them apart through f32's rounding.
      assert.ok(Math.min(apart, 1 - apart) > 0.01,`placements ${PLACEMENTS[i]} and ${PLACEMENTS[j]} share a seed (${seeds[i].toFixed(3)}, ${seeds[j].toFixed(3)})`);
    }
});

test("every film is staged under the fire sheet's held sky", () => {
  const args = filmArgs("o", "o/f", "18.4", 20);
  const at = args.indexOf("--tune");
  assert.deepEqual(args.slice(at, at + 2), HELD_SKY);
  assert.deepEqual(HELD_SKY, ["--tune", "sky.cloud.volume=0"]);
});

test("the reference names every hearth constant light.rs has", () => {
  const live = readFileSync(new URL("../../crates/terra_world/src/fire/light.rs", import.meta.url), "utf8");
  assert.doesNotThrow(() => hold(live, REFERENCE));
});

// The paired pass rule (paired.ts): a build misses a bar and still passes when it is no worse than
// its base filmed beside it, within the hour's run-to-run tolerance.
const films = (v: Partial<Record<string, number>>, n = 14): Film[] => {
  const at = { flick: 2.8, area: 0.14, life: 0.08, splits: 1.0, lean: 9, topred: 20, ...v };
  return Array.from({ length: n }, (_, i) => ({ dx: PLACEMENTS[i % PLACEMENTS.length], v: GATE.map((g) => at[g.name as keyof typeof at]) }));
};
const TOL = { flick: 0.07, area: 0.01, life: 0.02, splits: 0.11, lean: 0.13, topred: 0.04 };
const splits = (build: Film[], base?: Film[]) => paired(build, base, TOL).find((x) => x.name === "splits")!;

test("a measure meeting its bar passes whatever the base did", () => {
  const j = splits(films({ splits: 1.19 }), films({ splits: 0.5 }));
  assert.ok(j.met && j.pass && !j.kept);
});

test("a miss within the tolerance of a base on the bar passes, as no worse; past it fails", () => {
  // g85: main's night splits read 1.22 and 1.17 in one job, both runs of one build.
  assert.ok(splits(films({ splits: 1.22 }), films({ splits: 1.17 })).kept);
  assert.ok(splits(films({ splits: 1.31 }), films({ splits: 1.2 })).kept, "0.11 over a base at the bar");
  const lost = splits(films({ splits: 1.32 }), films({ splits: 1.2 }));
  assert.ok(!lost.pass && lost.base === 1.2);
  assert.ok(!splits(films({ splits: 1.22 })).pass, "without a base the bar alone");
});

test("no worse is measured from the bar, on either side of it", () => {
  // A base already over: the build may be as far over again within the tolerance, and better passes.
  assert.ok(splits(films({ splits: 1.5 }), films({ splits: 1.45 })).pass);
  assert.ok(splits(films({ splits: 1.3 }), films({ splits: 1.45 })).pass);
  assert.ok(!splits(films({ splits: 1.6 }), films({ splits: 1.45 })).pass);
  // Lean's bar is below (7.6): a lower lean is the worse one, a higher one never is.
  const lean = (b: number, a: number) => paired(films({ lean: b }), films({ lean: a }), TOL).find((x) => x.name === "lean")!;
  assert.ok(lean(7.5, 7.62).pass);
  assert.ok(!lean(7.4, 7.62).pass);
  assert.ok(lean(7.5, 7.3).pass);
});

test("the line names the base only where a measure passed by it or failed", () => {
  const j = paired(films({ splits: 1.25, lean: 7.0 }), films({ splits: 1.18, lean: 8.0 }), TOL);
  assert.equal(told(j), "flick 2.80 area 0.14 life 0.08 splits 1.25~1.18 lean 7.00x(base 8.00) topred 20.00");
});

test("the run-to-run spread is that of the gate's draw of takes", () => {
  // Takes alike at every placement: no spread, so the least tolerance.
  for (const sd of spread(films({}), 2)) assert.ok(sd < 1e-9, `${sd}`);
  assert.deepEqual(Object.values(tolerance(films({}), 2)), GATE.map(() => 0.01));
  // Each placement's takes 1 or 3: scaling the values scales the spread, and more takes lessen it.
  const two = PLACEMENTS.flatMap((dx) => [1, 3].map((s) => ({ dx, v: GATE.map(() => s) })));
  const one = spread(two, 2)[0];
  assert.ok(one > 0.1 && one < 0.8, `${one}`);
  assert.ok(Math.abs(spread(two.map((f) => ({ ...f, v: f.v.map((x) => 2 * x) })), 2)[0] - 2 * one) < 1e-9);
  assert.ok(spread(two, 8)[0] < one);
});

test("the gate's tolerances are those its A/A films give, for every hour and measure", () => {
  const aa = JSON.parse(readFileSync(new URL("gate-aa.json", import.meta.url), "utf8"));
  assert.equal(aa.takes, TAKES);
  for (const [hour] of HOURS) {
    assert.ok(aa.hours[hour]?.length >= 4 * PLACEMENTS.length * TAKES, `${hour}: at least four runs' films`);
    assert.deepEqual(TOLERANCE[hour], tolerance(aa.hours[hour], TAKES), `${hour}: paired.ts TOLERANCE is not gate-aa.json's (node tools/fire/film.ts --aa prints it)`);
  }
});
