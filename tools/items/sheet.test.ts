import assert from "node:assert/strict";
import { test } from "node:test";
import { type Assemble, cells, label, plain, trimSets } from "./sheet.ts";

// A made-up axe kind and parts: two heads, two hafts, two collars; no cap exists.
const kind = {
  kind: "axe",
  slots: [
    { slot: "working", name: "head", fits: ["axe_head"] },
    { slot: "handle", name: "haft", fits: ["haft"] },
    { slot: "trim", name: "binding", fits: ["collar"], optional: true },
    { slot: "trim", name: "butt", fits: ["cap"], optional: true },
  ],
};
const parts: Record<string, any> = Object.fromEntries(
  [
    ["big_axe_head", "axe_head"],
    ["small_axe_head", "axe_head"],
    ["ash_haft", "haft"],
    ["oak_haft", "haft"],
    ["iron_collar", "collar"],
    ["leather_binding", "collar"],
    ["spear_haft", "haft", ["spear"]],
  ].map(([part, type, kinds]) => [part, { part, type, ...(kinds ? { kinds } : {}) }]),
);

test("trim sets: one or none a slot, the fullest first and the bare recipe last", () => {
  assert.deepEqual(trimSets(kind, parts), [["iron_collar"], ["leather_binding"], []]);
});

test("each head by each haft of the kind, trims turned by place, and the least recipe's refusal when none is whole", () => {
  const asked: string[] = [];
  // The big head on oak is too long in any trim; the big head on ash only with leather.
  const assemble: Assemble = (_k, ids) => {
    asked.push(ids.join(" "));
    if (ids[0] === "big_axe_head" && ids[1] === "oak_haft") return { why: "too long" };
    if (ids[0] === "big_axe_head" && ids[2] !== "leather_binding") return { why: "too hard to swing" };
    return { item: { mass_kg: 1, length_m: 0.7 } };
  };
  const got = cells(kind, parts, "iron", assemble).map((c) => [c.parts.join(" "), c.why ?? "whole"]);
  assert.deepEqual(got, [
    ["big_axe_head ash_haft leather_binding", "whole"],
    ["big_axe_head oak_haft", "too long"],
    ["small_axe_head ash_haft leather_binding", "whole"],
    ["small_axe_head oak_haft iron_collar", "whole"],
  ]);
  assert.ok(!asked.some((a) => a.includes("spear_haft")), "a haft made for another kind is not tried");
});

test("labels and refusals in plain words", () => {
  const cell = { row: "big_axe_head", col: "ash_haft", parts: ["big_axe_head", "ash_haft", "iron_collar"], item: { mass_kg: 1.4291, length_m: 0.7051 } };
  assert.equal(label(cell, kind, parts), "big head, ash haft, iron collar: 1.43 kg, 0.71 m");
  const said = "the assembler refused `assemble item axe a b`: ✗ assemble: the axe's length_m 0.8074 is outside its envelope [0.612, 0.748] (assets/data/kinds/axe.json)";
  assert.equal(plain(said, "axe"), "too long: 0.81 m\n(an axe is at most 0.75 m)");
  assert.match(plain("✗ assemble: the axe's inertia_kgm2 0.2290 is outside its envelope [0.2424, 0.4436]", "axe"), /^too light to swing as an axe:.*at least 0.242\)$/s);
  assert.equal(plain("the assembler refused `x`: ✗ assemble: the axe's head slot is empty", "axe"), "the axe's head slot is empty");
});
