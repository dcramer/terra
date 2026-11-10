import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "node:test";
import { BAR_PER_MIN, KNOWN_BAD_PER_MIN, faults, readKinds } from "./judge.ts";

const RON = `[
    (
        name: "Crow",
        calls_s: Some((90.0, 300.0)),
        voice: Croak,
        // cry: Some("voice.not"),
        cry: Some("voice.crow"),
        zones: [
            (joint: "Head", part: Head),
        ],
    ),
    (
        name: "Hind",
        voice: Snort,
    ),
    (
        name: "Finch",
        voice: Chirp,
        call: Some("voice.finch.song"),
    ),
    (
        name: "Hare",
        voice: Silent,
    ),
]`;

test("a kind is heard as its recorded cry, else its voice made in code, and calls with its call", () => {
  assert.deepEqual(readKinds(RON), [
    { name: "Crow", cry: "voice.crow", call: "voice.crow", calls: [90, 300] },
    { name: "Hind", cry: "snort", call: "snort", calls: undefined },
    { name: "Finch", cry: "chirp", call: "voice.finch.song", calls: undefined },
    { name: "Hare", cry: undefined, call: undefined, calls: undefined },
  ]);
  assert.throws(() => readKinds(RON.replace("Snort", "Yodel")), /Yodel is not one heard.ts knows/);
});

test("the walk the developers refused is over the bar", () => {
  assert.ok(BAR_PER_MIN < KNOWN_BAD_PER_MIN);
});

test("a voice over the bar is a fault that names its kind, and one under is none", () => {
  const kinds = readKinds(RON);
  // The refused walk: 326 caws in three minutes.
  const loud = faults(kinds, { "voice.crow": 4 }, { "voice.crow": 330, "step.grass": 900 }, 3, []);
  assert.equal(loud.length, 1);
  assert.match(loud[0], /voice\.crow was heard 108\.7 times a minute.*Crow \(calls_s 90 to 300\)/);
  assert.deepEqual(faults(kinds, {}, { "voice.crow": 3 * BAR_PER_MIN, "step.grass": 900 }, 3, []), []);
});

test("a kind met must be heard as its data says, unless its data says it is silent", () => {
  const kinds = readKinds(RON);
  assert.deepEqual(faults(kinds, {}, {}, 3, [{ kind: "Crow", after: ["wings", "voice.crow"] }, { kind: "Hare", after: [] }, { kind: "Hind", after: ["snort"] }]), []);
  const mute = faults(kinds, {}, {}, 3, [{ kind: "Hind", after: ["step.hoof"] }, { kind: "Wolf", after: [] }]);
  assert.match(mute[0], /Hind was stepped up to and snort was not heard \(heard: step\.hoof\)/);
  assert.match(mute[1], /Wolf was met and is not in/);
});

test("the game's own bestiary reads, every kind with a name", () => {
  const kinds = readKinds(readFileSync(resolve(import.meta.dirname, "../../../assets/data/creatures.ron"), "utf8"));
  assert.ok(kinds.length > 20);
  assert.equal(kinds.find((kind) => kind.name === "Carrion crow")?.cry, "voice.crow");
  assert.equal(kinds.find((kind) => kind.name === "Red deer hind")?.cry, "snort");
});
