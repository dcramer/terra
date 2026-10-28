// The dungeon film's shot follows the run's logged tour, and its stills fall at even times
// along it on frames the film saved. `node --test tools/sandbox/dungeon.test.ts`.

import assert from "node:assert/strict";
import { test } from "node:test";
import { shotOf, stillFrames } from "./dungeon.ts";

const LOG = [
  "INFO Dungeon view 1: 1.00,2.00,3.00",
  "INFO Dungeon view 0: 0.00,1.60,0.00",
  "INFO Dungeon view 2: 2.00,2.00,3.50",
  "INFO Dungeon look 0: 0.00,1.50,-4.00",
  "INFO Dungeon look 1: 1.00,1.50,-1.00",
  "INFO Dungeon look 2: 2.00,1.50,0.00",
  "INFO Dungeon pace 0: 0.400",
  "INFO Dungeon pace 1: 1.250",
  "INFO Dungeon pace 2: 0.000",
].join("\n");

test("a key at each eye, in order, at the time the pace brings it there", () => {
  const shot = shotOf(LOG, "a tour");
  assert.deepEqual(
    shot.keys.map((k) => k.t),
    [0, 0.4, 1.65],
  );
  assert.deepEqual(shot.keys[1].at, [1, 2, 3]);
  assert.deepEqual(shot.keys[2].look, [2, 1.5, 0]);
  assert.equal(shot.seconds, 1.65);
  assert.equal(shot.player.hidden, true);
});

test("a log without the whole tour is refused", () => {
  assert.throws(() => shotOf(LOG.replace("Dungeon look 2", "Dungeon other 2"), "a tour"), /3 eyes, 2 looks/);
});

test("the stills fall in the middle of even stretches, on the film's even frames", () => {
  // 10 s at 60 frames a second from frame 300: the middles of four 2.5 s stretches.
  assert.deepEqual(stillFrames(10, 4), [374, 524, 674, 824]);
  for (const f of stillFrames(7.3, 5)) assert.equal(f % 2, 0);
});
