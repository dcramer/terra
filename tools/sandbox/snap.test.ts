// The differ behind `r diff` says what changed between two made-up digests, and only that.
// `node --test tools/sandbox/snap.test.ts`.

import assert from "node:assert/strict";
import { test } from "node:test";
import { type Snap, changes, diff } from "./snap.ts";

const before = {
  at: [10, -4],
  health: 100,
  holding: "stone_axe",
  carrying: { stone: 5, wood: 2 },
  near: [
    { what: "deer", at: [12, -4], doing: "Grazing" },
    { what: "deer", at: [20, 1], doing: "Grazing" },
    { what: "boar", at: [30, 3], doing: "Wandering" },
  ],
  tags: ["indoors", "lit"],
};

const snap = (name: string, state: unknown, frame: number | null = 10): Snap => ({ name, taken: "", frame, seconds: null, state });

test("the same state, or numbers within the epsilon, is nothing changed", () => {
  assert.deepEqual(changes(before, structuredClone(before)), []);
  const jittered = structuredClone(before);
  jittered.at = [10.004, -4.006];
  jittered.near[2].at = [30.009, 3];
  assert.deepEqual(changes(before, jittered), []);
  assert.deepEqual(diff(snap("a", before), snap("b", jittered)), ["a (frame 10) -> b (frame 10): nothing changed"]);
});

test("a move, a changed value, and what appears and goes are a line each", () => {
  const after = structuredClone(before) as any;
  after.at = [13, 0]; // 3 east, 4 south: 5 m
  after.health = 80;
  after.holding = null;
  after.carrying = { stone: 5, meat: 1 };
  assert.deepEqual(changes(before, after), [
    "~ at: [10,-4] -> [13,0] (moved 5)",
    "~ health: 100 -> 80",
    '~ holding: "stone_axe" -> null',
    "+ carrying.meat: 1",
    "- carrying.wood: 2",
  ]);
});

test("things in a list are matched by what they are, not where they are in it", () => {
  const after = structuredClone(before) as any;
  // The first deer goes; the second steps half a metre, and is now first in the list.
  after.near = [{ ...after.near[1], at: [20.5, 1] }, { ...after.near[2], doing: "Fleeing" }, { what: "wolf", at: [5, 5], doing: "Hunting" }];
  assert.deepEqual(changes(before, after), [
    // Paired with the deer nearest it, not the one first in the list before.
    "~ near[deer#1].at: [20,1] -> [20.5,1] (moved 0.5)",
    '- near[deer#2]: {"what":"deer","at":[12,-4],"doing":"Grazing"}',
    '~ near[boar].doing: "Wandering" -> "Fleeing"',
    '+ near[wolf]: {"what":"wolf","at":[5,5],"doing":"Hunting"}',
  ]);
});

test("a list of objects that changes in number says so; plain values come and go in any order", () => {
  const after = structuredClone(before) as any;
  after.near.pop();
  after.tags = ["lit", "wet"];
  assert.deepEqual(changes(before, after), ["# near: 3 -> 2", '- near[boar]: {"what":"boar","at":[30,3],"doing":"Wandering"}', '+ tags: "wet"', '- tags: "indoors"']);
});

test("build.state's parts are matched by id", () => {
  const a = { parts: [{ id: 1, kind: "wall", built: 4 }, { id: 2, kind: "wall", built: 0 }] };
  const b = { parts: [{ id: 2, kind: "wall", built: 3 }] };
  assert.deepEqual(changes(a, b), ["# parts: 2 -> 1", "~ parts[2].built: 0 -> 3", '- parts[1]: {"id":1,"kind":"wall","built":4}']);
});

test("a long diff is capped, with a count of the rest", () => {
  const many = (value: number) => Object.fromEntries(Array.from({ length: 50 }, (_, i) => [`k${i}`, value]));
  const lines = diff(snap("a", many(0), 5), snap("now", many(1), null), { most: 10 });
  assert.equal(lines[0], "a (frame 5) -> now: 50 changes");
  assert.equal(lines.length, 1 + 10 + 1);
  assert.equal(lines[1], "~ state.k0: 0 -> 1");
  assert.equal(lines.at(-1), "… and 40 more (--most to see them)");
});

test("a bigger epsilon lets a small move go", () => {
  const after = structuredClone(before);
  after.at = [10.3, -4];
  assert.equal(changes(before, after).length, 1);
  assert.deepEqual(changes(before, after, 0.5), []);
});
