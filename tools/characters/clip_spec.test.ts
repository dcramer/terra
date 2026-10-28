import assert from "node:assert/strict";
import { test } from "node:test";
import { againstSpec, choose, playedClip, readSpec, speedFor, writePhases, writeShared } from "./clip_spec.ts";

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

test("a take's playback speed meets its timing where one can, else its hit's", () => {
  const light = { hit: [0.22, 0.32] as [number, number], back: [0.8, 1.1] as [number, number] };
  // Hit 0.4 s into 1.6: speeds 1.25-1.82 put the hit in, 1.09-1.5 the back; between, 1.375.
  const fits = speedFor(light, 0.4, 1.6);
  assert.ok(fits.ok && Math.abs(fits.speed - 1.375) < 1e-9, `speed ${fits.speed}`);
  assert.ok(fits.hit >= 0.22 && fits.hit <= 0.32 && fits.back >= 0.8 && fits.back <= 1.1);
  // Hit 0.75 s in: a speed fast enough for the hit leaves too little back; the hit's is kept.
  const late = speedFor(light, 0.75, 1.6);
  assert.equal(late.ok, false);
  assert.ok(late.hit >= 0.22 && late.hit <= 0.32 && late.back < 0.8);
});

test("a spec's chain, hold, raise and timing are read and checked", () => {
  const spec = readSpec("x", { ...entry, blow: { move: "block", raised: [0.4, 0.8], next: "slash" }, timing: { hit: [0.2, 0.3], back: [0.3, 0.6] } });
  assert.deepEqual(spec.blow.raised, [0.4, 0.8]);
  assert.equal(spec.blow.next, "slash");
  assert.deepEqual(spec.timing?.hit, [0.2, 0.3]);
  assert.equal(readSpec("x", { ...entry, blow: { move: "guard", held: true } }).blow.held, true);
  assert.throws(() => readSpec("x", { ...entry, blow: { move: "b", raised: [0.8, 0.4] } }), /raised/);
  assert.throws(() => readSpec("x", { ...entry, blow: { move: "b", raised: [0.4, 2] } }), /raised/);
  assert.throws(() => readSpec("x", { ...entry, timing: { hit: [0.3] } }), /timing/);
});

test("a move's speed, hold and chain are written with its phases", () => {
  const out = writePhases(SET, "block", { clip: "Keyed_Block#1", from: 0, done: 1.4, speed: 1.8, hold: 0.6 });
  assert.match(out, /"block": \(clip: "Keyed_Block#1", from: 0\.00, speed: 1\.80, done: 1\.40, hold: 0\.60\)/);
  const added = writePhases(SET, "backhand", { clip: "K#0", from: 0, done: 1.5, speed: 2.5, next: "slash" });
  assert.match(added, /"backhand": \(clip: "K#0", from: 0\.00, done: 1\.50, next: "slash", speed: 2\.50\),/);
});

test("a blow is written as the clip the game plays of its take, and every move playing that clip gets its frames", () => {
  const list = { Keyed_Slash: { prompt: "x" }, Open_Slash: { of: "Keyed_Slash", hold: "rest" }, Keyed_Heavy: {} };
  assert.equal(playedClip(list, "Keyed_Slash"), "Open_Slash");
  assert.equal(playedClip(list, "Keyed_Heavy"), "Keyed_Heavy");
  assert.throws(() => playedClip({ ...list, Low_Slash: { of: "Keyed_Slash" } }, "Keyed_Slash"), /Open_Slash, Low_Slash are each made of Keyed_Slash/);
  const set = `(
    moves: {
        "slash": (clip: "Open_Slash", from: 0.00, speed: 1.72, active: (0.48, 0.60), done: 1.96, next: "finisher"),
        // The slash again, slower.
        "finisher": (clip: "Open_Slash", from: 0.0, speed: 1.5, active: (0.48, 0.60), done: 1.96, damage: 18),
        "partner_slash": (
            clip: "Open_Slash", from: 0.0, speed: 0.58, done: 1.96,
        ),
        "heavy": (clip: "Open_Heavy", from: 0.00, speed: 1.5, active: (0.82, 1.17), done: 2.38),
    },
)`;
  let out = writePhases(set, "slash", { clip: playedClip(list, "Keyed_Slash"), from: 0, done: 1.9, active: [0.32, 0.48], speed: 1.59 });
  out = writeShared(out, "Open_Slash", "slash", { done: 1.9, active: [0.32, 0.48] });
  assert.match(out, /"slash": \(clip: "Open_Slash", from: 0\.00, speed: 1\.59, active: \(0\.32, 0\.48\), done: 1\.90, next: "finisher"\)/);
  assert.match(out, /"finisher": \(clip: "Open_Slash", from: 0\.0, speed: 1\.5, active: \(0\.32, 0\.48\), done: 1\.90, damage: 18\)/);
  assert.match(out, /clip: "Open_Slash", active: \(0\.32, 0\.48\), from: 0\.0, speed: 0\.58, done: 1\.90,/);
  assert.match(out, /"heavy": \(clip: "Open_Heavy", from: 0\.00, speed: 1\.5, active: \(0\.82, 1\.17\), done: 2\.38\)/);
  assert.match(out, /\/\/ The slash again, slower\./);
});

test("a move the set lacks is added to its moves", () => {
  const out = writePhases(SET, "backhand", { clip: "Keyed_Backhand", from: 0, done: 1.5, active: [0.7, 0.9] });
  assert.match(out, /"backhand": \(clip: "Keyed_Backhand", from: 0\.00, active: \(0\.70, 0\.90\), done: 1\.50, speed: 1\.0\),\n    },\n\)/);
  assert.equal(writePhases(out, "backhand", { clip: "Keyed_Backhand", from: 0, done: 1.5, active: [0.7, 0.9] }), out);
});
