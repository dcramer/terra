// The looks check (looks.ts): judging numbers against a place's style, on made-up numbers whose
// answers are known, and the style file kept in step with the places, the references and the
// art guide. `node --test tools/sandbox/looks.test.ts`.

import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "node:test";
import { reference } from "./look.ts";
import { MEASURES, judge, numbers, shown, style } from "./looks.ts";
import { PLACES } from "./places.ts";

const views = (values: number[]) => values.map((v, k) => ({ view: `v${k}`, numbers: { x: v } }));

test("held within, a mean inside the tolerance holds and one outside fails by how far out", () => {
  const [inside] = judge({ x: { rule: "light", within: 0.1 } }, { x: 0.5 }, views([0.45, 0.6]));
  assert.ok(inside.holds);
  assert.equal(inside.out, 0);
  const [outside] = judge({ x: { rule: "light", within: 0.1 } }, { x: 0.5 }, views([0.8, 0.7]));
  assert.ok(!outside.holds);
  // The mean 0.75 is 0.15 past the bound 0.6: one and a half tolerances.
  assert.ok(Math.abs(outside.out - 1.5) < 1e-9, `${outside.out}`);
  assert.equal(outside.worst.view, "v0");
  // Below it too.
  assert.ok(!judge({ x: { rule: "light", within: 0.1 } }, { x: 0.5 }, views([0.2]))[0].holds);
});

test("held under, any mean up to the tolerance over the reference holds, and the highest view is the worst", () => {
  const [low] = judge({ x: { rule: "tint", under: 0.01 } }, { x: 0.02 }, views([0, 0]));
  assert.ok(low.holds);
  assert.equal(low.low, -Infinity);
  const [high] = judge({ x: { rule: "tint", under: 0.01 } }, { x: 0.02 }, views([0.01, 0.07]));
  assert.ok(!high.holds);
  assert.equal(high.worst.view, "v1");
});

test("held over, any mean down to the tolerance under the reference holds, and the lowest view is the worst", () => {
  assert.ok(judge({ x: { rule: "horizon", over: 0.1 } }, { x: 0.2 }, views([0.9]))[0].holds);
  const [low] = judge({ x: { rule: "horizon", over: 0.1 } }, { x: 0.2 }, views([0.15, 0.01]));
  assert.ok(!low.holds);
  assert.equal(low.worst.view, "v1");
});

test("a number the reference lacks is an error, not a pass", () => {
  assert.throws(() => judge({ y: { rule: "light", within: 1 } }, { x: 0 }, views([0])));
});

test("shares print as percents, the rest to three places", () => {
  assert.equal(shown("dark", 0.4612), "46.1%");
  assert.equal(shown("mean", 0.14812), "0.148");
});

test("every place's style names a place, a reference with every number held, a known measure and a rule the art guide states", () => {
  const kept = style();
  const guide = readFileSync(resolve(import.meta.dirname, "../../docs/art-style.md"), "utf8");
  for (const title of Object.values(kept.rules)) assert.ok(guide.includes(title), `docs/art-style.md does not state "${title}"`);
  assert.ok(Object.keys(kept.places).length > 0);
  for (const [name, held] of Object.entries(kept.places)) {
    const where = PLACES[name];
    assert.ok(where, `no place called ${name}`);
    assert.ok(/^\d{1,2}$/.test(held.time), name);
    const ref = reference(where.look);
    const target = numbers(ref.look, ref.bands);
    for (const [measure, h] of Object.entries(held.hold)) {
      assert.ok(MEASURES[measure], `${name}: no measure called ${measure}`);
      assert.ok(Number.isFinite(target[measure]), `${name}: ${ref.name} has no ${measure}; measure it again (look.ts reference)`);
      assert.ok(kept.rules[h.rule], `${name}: ${measure}'s rule ${h.rule} is not one of the style's`);
      const given = [h.within, h.under, h.over].filter((v) => v !== undefined);
      assert.equal(given.length, 1, `${name}: ${measure} is held one way`);
      assert.ok((given[0] as number) > 0, `${name}: ${measure}'s tolerance`);
    }
  }
});

test("a look's and its bands' numbers are named as the style names them", async () => {
  const { look, bands } = await import("./look.ts");
  const p = { width: 32, height: 32, rgba: new Uint8Array(32 * 32 * 4).fill(128) };
  const n = numbers(look(p), bands(p));
  for (const k of ["mean", "hardEdges", "facets", "far.mean", "near.contrast", "farUnderSky"]) assert.ok(k in n, k);
  for (const k of Object.keys(n)) assert.ok(MEASURES[k], `${k} has no name in the table`);
});
