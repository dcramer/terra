// The motion films' held hearth light (film.ts, D472).
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";
import { hold, REFERENCE } from "./film.ts";

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

test("the reference names every hearth constant light.rs has", () => {
  const live = readFileSync(new URL("../../crates/terra_world/src/fire/light.rs", import.meta.url), "utf8");
  assert.doesNotThrow(() => hold(live, REFERENCE));
});
