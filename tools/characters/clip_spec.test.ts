import assert from "node:assert/strict";
import { test } from "node:test";
import { againstSpec, choose, readSpec, writePhases } from "./clip_spec.ts";

const entry = {
  prompt: "A swordsman cuts.", seconds: 1.6, seed: 2, once: true,
  guard: { clip: "Fight_Guard", frame: 173 },
  hand: [{ at: 0.45, grip: [-0.2, 0.22, 0.45], blade: [-0.8, -0.3, 0.5] }],
  blow: { move: "backhand", sweep: "left-to-right", contact: 0 },
};

test("a spec reads with its keys, and says what is wrong with a bad one", () => {
  const spec = readSpec("Keyed_Backhand", entry);
  assert.equal(spec.samples, 3);
  assert.equal(spec.hand[0].at, 0.45);
  assert.deepEqual(spec.left, []);
  assert.throws(() => readSpec("x", { ...entry, once: false }), /one-shot/);
  assert.throws(() => readSpec("x", { ...entry, hand: [{ at: 1.6, grip: [0, 0, 0] }] }), /inside the clip/);
  assert.throws(() => readSpec("x", { ...entry, hand: [{ at: 0.5, grip: [0, 0] }] }), /grip/);
  assert.throws(() => readSpec("x", { ...entry, blow: { move: "b", sweep: "sideways" } }), /sweep sideways/);
  assert.throws(() => readSpec("x", { ...entry, guard: undefined }), /no guard/);
});

test("a blow sweeping against its spec fails, one sweeping with it passes", () => {
  // The model's space: x to the body's left. A tip going from 0.6 m left to 0.7 m right.
  const leftToRight = againstSpec({ sweep: "left-to-right" }, [0.6, 1.2, 0.8], [-0.7, 1.0, 0.8], 0);
  assert.deepEqual(leftToRight, []);
  const wrongWay = againstSpec({ sweep: "right-to-left" }, [0.6, 1.2, 0.8], [-0.7, 1.0, 0.8], 0);
  assert.equal(wrongWay.length, 1);
  assert.match(wrongWay[0], /130 cm to the right/);
  // Too little travel across counts as not sweeping that way.
  assert.equal(againstSpec({ sweep: "left-to-right" }, [0.05, 1, 1], [-0.05, 1, 1], 0).length, 1);
  // An overhead goes down more than across.
  assert.deepEqual(againstSpec({ sweep: "overhead" }, [0, 1.8, 0.6], [0.1, 0.9, 0.9], 0), []);
  assert.equal(againstSpec({ sweep: "overhead" }, [0.8, 1.2, 0.6], [-0.8, 0.9, 0.9], 0).length, 1);
});

test("a contact far off its spec's bearing fails", () => {
  assert.deepEqual(againstSpec({ contact: 10 }, [0, 0, 0], [0, 0, 0], -10), []);
  const off = againstSpec({ contact: 0 }, [0, 0, 0], [0, 0, 0], 40);
  assert.match(off[0], /40° left of ahead, spec 0° left/);
});

test("the take kept is the first that passes, else the one with fewest failures", () => {
  assert.equal(choose([{ take: 2, failures: [], off: 0.5 }, { take: 0, failures: ["a"], off: 0 }, { take: 1, failures: [], off: 0.9 }])?.take, 1);
  assert.equal(choose([{ take: 0, failures: ["a", "b"], off: 0 }, { take: 1, failures: ["a"], off: 0.3 }, { take: 2, failures: ["c"], off: 0.1 }])?.take, 2);
  assert.equal(choose([]), undefined);
});

const SET = `(
    buffer_s: 0.35,
    moves: {
        // The first blow.
        "slash": (
            clip: "Fight_Slash", from: 0.15, speed: 1.35, active: (0.70, 0.92), done: 1.55,
            next: "backhand",
        ),
        "block": (clip: "Fight_Block", speed: 1.0, done: 1.2, hold: 0.8),
    },
)
`;

test("phases are written into a move, its other fields and the file's comments kept", () => {
  const out = writePhases(SET, "slash", { clip: "Keyed_Slash", from: 0, done: 1.566, active: [0.7, 0.9333] });
  assert.match(out, /clip: "Keyed_Slash", from: 0\.00, speed: 1\.35, active: \(0\.70, 0\.93\), done: 1\.57,/);
  assert.match(out, /\/\/ The first blow\./);
  assert.match(out, /next: "backhand"/);
  // A move without a field gets it after its clip; the others are untouched.
  const block = writePhases(SET, "block", { clip: "Keyed_Block", from: 0, done: 1.5 });
  assert.match(block, /"block": \(clip: "Keyed_Block", from: 0\.00, speed: 1\.0, done: 1\.50, hold: 0\.8\)/);
  assert.match(block, /clip: "Fight_Slash", from: 0\.15/);
});

test("a move the set lacks is added to its moves", () => {
  const out = writePhases(SET, "backhand", { clip: "Keyed_Backhand", from: 0, done: 1.5, active: [0.7, 0.9] });
  assert.match(out, /"backhand": \(clip: "Keyed_Backhand", from: 0\.00, active: \(0\.70, 0\.90\), done: 1\.50, speed: 1\.0\),\n    },\n\)/);
  assert.equal(writePhases(out, "backhand", { clip: "Keyed_Backhand", from: 0, done: 1.5, active: [0.7, 0.9] }), out);
});
