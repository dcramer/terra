import { test } from "node:test";
import assert from "node:assert/strict";
import type { Wardrobe } from "./build_steps.ts";
import { figures, judge, type Measure, type Pairing, pairs, written } from "./pairings.ts";

const clips = (rows: Record<string, [number, number]>): Measure["each"] => new Map(Object.entries(rows).map(([n, [poke, open]]) => [n, { poke, open }]));

test("a mix passes with deep pokes only where their own outfits poke, and is refused for a new poke, a new opening or more poking at rest", () => {
  const theirs: Measure = { each: clips({ Roll: [0.044, 1], Walk: [0, 1] }), rest: 2 };
  const clean: Measure = { each: clips({ Roll: [0.003, 0], Walk: [0, 0] }), rest: 0 };
  assert.equal(judge({ each: clips({ Roll: [0.077, 1], Walk: [0.004, 1] }), rest: 2 }, [theirs, clean]).ok, true);
  const poke = judge({ each: clips({ Roll: [0.01, 0], Walk: [0.006, 0] }), rest: 0 }, [theirs, clean]);
  assert.deepEqual([poke.ok, poke.worse], [false, ["Walk"]]);
  const opened = judge({ each: clips({ Roll: [0, 2], Walk: [0, 1] }), rest: 0 }, [theirs, clean]);
  assert.deepEqual([opened.ok, opened.worse], [false, ["Roll"]]);
  assert.equal(judge({ each: clips({ Roll: [0, 0], Walk: [0, 0] }), rest: 3 }, [theirs, clean]).ok, false);
  // Theirs apart: any contact past 5 mm the mix brings is its own.
  assert.equal(judge({ each: clips({ Walk: [0.006, 0] }), rest: 0 }, [undefined, undefined]).ok, false);
});

const w = {
  slots: ["body", "legs"],
  regions: ["torso", "legs"],
  bodies: { b: { model: "m.glb", pieces: "p", head: "head", bare: { body: "skin", legs: "bare_legs" }, look: "x" } },
  pieces: {
    head: { body: "b", source: "", layer: 0, regions: [] },
    skin: { body: "b", source: "", slot: "body", layer: 0, regions: ["torso"] },
    bare_legs: { body: "b", source: "", slot: "legs", layer: 0, regions: ["legs"] },
    coat: { body: "b", source: "", slot: "body", item: "coat", layer: 1, regions: ["torso"], colourways: ["a", "b", "c"] },
    tunic: { body: "b", source: "", slot: "body", item: "shirt", layer: 1, regions: ["torso"] },
    hose: { body: "b", source: "", slot: "legs", item: "trousers", layer: 1, regions: ["legs"] },
    breeches: { body: "b", source: "", slot: "legs", item: "trousers", layer: 1, regions: ["legs"] },
  },
  looks: {},
} as Wardrobe;

test("the matrix pairs every worn piece with every worn piece of the body's other slots, and counts the figures it lets through", () => {
  assert.deepEqual(pairs(w), [
    ["breeches", "coat"],
    ["breeches", "tunic"],
    ["coat", "hose"],
    ["hose", "tunic"],
  ]);
  const rows: Pairing[] = [{ pieces: ["breeches", "coat"], verdict: "refused" }];
  assert.deepEqual(figures(w, rows, "b"), { count: 3, all: 4, coloured: 5 });
  assert.equal(figures(w, rows, "b", true).count, 8);
});

test("the pairings are written in place, a row a line, the rest of the catalogue as written", () => {
  const text = '{\n  "looks": { "x": [1, 2] },\n  "pairings": [],\n  "populations": {}\n}\n';
  const out = written(text, [{ pieces: ["a", "b"], verdict: "apart" }]);
  assert.equal(out, '{\n  "looks": { "x": [1, 2] },\n  "pairings": [\n    {"pieces":["a","b"],"verdict":"apart"}\n  ],\n  "populations": {}\n}\n');
  assert.deepEqual(JSON.parse(out).pairings, [{ pieces: ["a", "b"], verdict: "apart" }]);
});
