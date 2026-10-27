// The motion films' held hearth light (film.ts, D472).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { filmArgs, hold, PLACEMENTS, REFERENCE, TAKES, takes } from "./film.ts";
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
