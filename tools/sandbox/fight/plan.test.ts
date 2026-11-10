// node --test tools/sandbox/fight/plan.test.ts
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { test } from "node:test";
import { type Arm, KEPT_AFTER_S, SPARE_S, readBlows, readChain, swing, threatens, untilLanding } from "./plan.ts";

const CREATURES = `[
    (
        name: "Boar",
        temper: Hostile(bite: 5.0, bite_at_s: 0.4, attack_s: 1.0),
    ),
    (
        name: "Ogre",
        temper: Hostile(
            // bite_at_s: 9.0 in a comment is not read
            bite: 30.0,
            bite_at_s: 2.0,
            attack_s: 4.0,
        ),
        blows: [
            (clip: "Sweep", within_m: 4.0, damage: 20.0, lands_at_s: 1.5, reach_m: 5.0, lasts_s: 3.0),
        ],
    ),
    (
        name: "Hare",
    ),
]`;
const SET = `(
    buttons: (main: "cut", off: "block"),
    moves: {
        "cut": (clip: "A", from: 0.0, speed: 2.0, done: 3.0, cancel: 1.0, next: "back"),
        "back": (clip: "B", from: 0.5, speed: 1.0, done: 1.5, cancel: 1.0),
        "block": (clip: "C", from: 0.0, speed: 1.0, done: 1.0, hold: 0.5),
    },
)`;

test("a kind's blows are its temper's own and each of its others, and no other kind's", () => {
  assert.deepEqual(readBlows(CREATURES, "Ogre"), [{ lands: 2.0, lasts: 4.0 }, { lands: 1.5, lasts: 3.0 }]);
  assert.deepEqual(readBlows(CREATURES, "Boar"), [{ lands: 0.4, lasts: 1.0 }]);
  assert.throws(() => readBlows(CREATURES, "Hare"), /names no blow/);
  assert.throws(() => readBlows(CREATURES, "Wyrm"), /no kind named Wyrm/);
});

test("the chain is the main button's move and each next, in seconds of their own time", () => {
  assert.deepEqual(readChain(SET), [
    { name: "cut", lasts: 1.5, cancel: 0.5 },
    { name: "back", lasts: 1.0, cancel: 0.5 },
  ]);
});

test("a blow threatens from its beginning until the latest of them has surely landed", () => {
  const blows = readBlows(CREATURES, "Ogre");
  assert.equal(threatens(blows, [0.0, 0.3]), true);
  assert.equal(threatens(blows, [1.9, 2.4]), true);
  assert.equal(threatens(blows, [2.0 + KEPT_AFTER_S, 2.4]), false);
});

test("the time to a landing is never more than any blow it may be allows, however late it is seen", () => {
  const blows = readBlows(CREATURES, "Ogre");
  assert.equal(untilLanding(blows, undefined), 1.5);
  // Before the soonest, by the most it can be through.
  assert.equal(untilLanding(blows, [0.3, 0.5]), 1.0);
  // The soonest may have landed, the latest may not: none.
  assert.equal(untilLanding(blows, [1.4, 1.8]), 0);
  assert.equal(untilLanding(blows, [1.9, 2.6]), 0);
  // All landed: it may be the sweep, over at 3.0, then the soonest of the next.
  assert.equal(untilLanding(blows, [2.25, 2.5]), 0.5 + 1.5);
  // It may still be the sweep (the least is before its end), which by the most is over.
  assert.equal(untilLanding(blows, [2.9, 3.2]), 1.5);
  // Past the sweep's end it is the smash, over at 4.0.
  assert.equal(untilLanding(blows, [3.25, 3.5]), 0.5 + 1.5);
  assert.equal(untilLanding(blows, [4.5, 4.6]), 1.5);
});

test("a swing is begun only if it is done, with the spare, before a blow may land", () => {
  const chain = readChain(SET);
  assert.equal(swing(chain, undefined, 1.5 + SPARE_S), 0);
  assert.equal(swing(chain, undefined, 1.5 + SPARE_S - 0.01), undefined);
  // A caller that sees late needs that much more.
  assert.equal(swing(chain, undefined, 1.5 + SPARE_S + 0.2, 0.3), undefined);
  assert.equal(swing(chain, undefined, 1.5 + SPARE_S + 0.3, 0.3), 0);
  // In the first blow: not before its cancel; from it, the next, by the next's own length.
  const early: Arm = { swing: 0, for: 0.4 };
  const open: Arm = { swing: 0, for: 0.6 };
  assert.equal(swing(chain, early, 9), undefined);
  assert.equal(swing(chain, open, 1.0 + SPARE_S), 1);
  assert.equal(swing(chain, open, 1.0 + SPARE_S - 0.01), undefined);
  // The last of the chain past its cancel goes round to the first; done, the arm is free.
  assert.equal(swing(chain, { swing: 1, for: 0.6 }, 9), 0);
  assert.equal(swing(chain, { swing: 1, for: 1.2 }, 9), 0);
});

test("the game's own data reads: the troll has two blows and the sword a chain of three, and between two blows a swing fits", () => {
  const root = resolve(import.meta.dirname, "../../..");
  const blows = readBlows(readFileSync(resolve(root, "assets/data/creatures.ron"), "utf8"), "Troll");
  const chain = readChain(readFileSync(resolve(root, "assets/data/moves/sword.shielded.ron"), "utf8"));
  assert.equal(blows.length, 2);
  assert.equal(chain.length, 3);
  for (const each of [...blows, ...chain]) assert.ok(each.lasts > 0 && each.lasts < 10, JSON.stringify(each));
  // Possible at all: just past the guard's window, whichever blow it was, the first swing goes.
  const latest = Math.max(...blows.map((each) => each.lands));
  const seen = latest + KEPT_AFTER_S;
  assert.equal(swing(chain, undefined, untilLanding(blows, [seen, seen + 0.1]), 0.1), 0);
});
