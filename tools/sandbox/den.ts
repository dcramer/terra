#!/usr/bin/env node
// A world's den, walked in the real game (D591, D643, docs/caves.md): a new 30 km world of a
// seed is played; `den.state` says where the den's mouth opens and its chamber's floor is;
// the player, put at the mouth facing in, walks forward and comes to the chamber's floor,
// under the land, with no way cut for them. Checked with no picture, by where the game says
// the player stands. How long the walk took is said: a reading.
//
//   node tools/sandbox/den.ts [--seed 3]
//
// Node 22.18 or later; no dependencies. About three minutes.

import { call, press, start, stop, waitSeconds, waitUntil, warp } from "./client.ts";

const args = process.argv.slice(2);
const SEED = args.includes("--seed") ? args[args.indexOf("--seed") + 1] : "3";
/// The player has come to the chamber when they stand in it: within its half width of its
/// floor's middle, less a pace, and this near its floor's level, in metres; and this long to
/// walk there, in seconds (some 45 m at a walk). Walking straight on does not follow a bend
/// to the room's middle, only into the room.
const LEVEL_M = 2.5;
const MOST_S = 60;

type Place = [number, number, number];
type Den = { den: { mouth: Place; floor: Place; ahead: Place; layout: { way: number; room: number; alcoves: number }; room_across_m: number } | null };
type Found = { from_start: Place }[];
type Title = { screen: string; seed: string | null };

let failed = "";
try {
  await start(["--experiment", "play", "--fresh", "--save", "shots/den.ron", "--radius", "30000", "--seed", SEED, "--offscreen", "--mute"], { realTime: true, release: true });
  await waitUntil("the world to be played", async () => (await call("title.state")) as Title, (now) => now.screen === "Playing", 600);
  const { den } = (await call("den.state")) as Den;
  if (!den) throw new Error(`seed ${SEED}'s world has no den: terra_land::cave::find found no place for one (the sweep, cargo xtask worlds, says why)`);
  const player = async () => ((await call("sandbox.find", { name: "Player" })) as Found)[0].from_start;
  // How far the player is from the floor's middle: over the ground, and in height.
  const off = (at: Place) => [Math.hypot(at[0] - den.floor[0], at[2] - den.floor[2]), Math.abs(at[1] - den.floor[1])];
  const NEAR_M = den.room_across_m / 2 - 1;
  await warp([den.mouth[0], den.mouth[2]], [den.ahead[0], den.ahead[2]]);
  const began = Date.now();
  let at = await player();
  // Forward, a second at a time, until the chamber or the clock.
  while ((off(at)[0] > NEAR_M || off(at)[1] > LEVEL_M) && Date.now() - began < MOST_S * 1000) {
    await press("w:60");
    await waitSeconds(1);
    at = await player();
  }
  const [away, level] = off(at);
  if (away > NEAR_M || level > LEVEL_M) throw new Error(`walking in from the den's mouth for ${MOST_S} s the player stands ${away.toFixed(1)} m from the chamber floor's middle and ${level.toFixed(1)} m off its level (at ${at.map((x) => x.toFixed(1)).join(", ")} east, up, south of the start; the floor is at ${den.floor.map((x) => x.toFixed(1)).join(", ")}): the way in is shut, too steep, or bends where walking straight on does not follow it (terra_land::cave, terra_voxel::caves)`);
  const down = den.mouth[1] - den.floor[1];
  console.log(`✓ den: seed ${SEED}'s den (way ${den.layout.way}, room ${den.layout.room}, ${den.layout.alcoves} alcoves) opens ${Math.hypot(den.mouth[0], den.mouth[2]).toFixed(0)} m from the start; walking in from its mouth the player came to its chamber's floor, ${down.toFixed(1)} m below the mouth, in ${((Date.now() - began) / 1000).toFixed(0)} s (a reading), ${away.toFixed(1)} m from the floor's middle`);
} catch (problem) {
  failed = String(problem instanceof Error ? problem.message : problem);
} finally {
  await stop().catch(() => undefined);
}
if (failed) {
  console.error(`✗ den: ${failed}`);
  process.exit(1);
}
