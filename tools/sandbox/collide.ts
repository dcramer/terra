#!/usr/bin/env node
// Walks the player into what stands in the lush forest and checks it stops them (D69, D243):
// trunks, boulders, rocks, stumps and fallen logs. A sample of obstacles of every size is
// taken from the game (world.obstacles), and for each the player is set down a few metres
// off, facing it, and walks at it for two seconds, far enough to pass through had it not
// stopped them. It passes when the player never overlaps the obstacle: they stop at it, or
// slide round it as a body glances off a rock; sampled through the walk, it fails if they were
// ever inside it. It also takes a picture of the first few, to look at the obstacle against
// where the player stopped, so an invisible wall or a trunk walked into shows.
//
//   node tools/sandbox/collide.ts [--count 30]
//
// Writes shots/collide/<n>.png. About a minute. Node 22.18 or later; no dependencies.

import * as game from "./client.ts";

type Obstacle = { at: [number, number]; radius: number; apart: number };


const arg = (name: string, fallback: string) => {
  const at = process.argv.indexOf(name);
  return at > 0 ? process.argv[at + 1] : fallback;
};
const count = Number(arg("--count", "30"));
const windows = game.onWsl && !process.argv.includes("--here");

console.log(`Starting the game${windows ? " on Windows" : ""}…`);
await game.start(["--experiment", "play", "--experiment", "lush", "--level", "yard", "--time", "13"], { windows, copy: "collide" });
await game.waitFor("the floor has", { since: 0, timeout: 120 });
// Footprints are worked out as meshes load.
await game.waitSeconds(3);

// What stands in the forest, as it is drawn: every kind that should stop a walker, found by
// name, so a thing with no footprint at all is tried too.
type Found = { name: string; from_start: [number, number, number] };
const KINDS = ["Rock", "Boulder", "Stump", "Log", "Oak", "Pine", "Spruce"];
const per = Math.max(2, Math.ceil(count / KINDS.length));
const sample: { name: string; at: [number, number] }[] = [];
for (const kind of KINDS) {
  const found = ((await game.call("sandbox.find", { name: kind })) as Found[])
    .filter((f) => f.name === kind)
    .map((f) => ({ name: kind, at: [f.from_start[0], f.from_start[2]] as [number, number] }))
    .filter((f) => Math.hypot(...f.at) < 150)
    .slice(0, per);
  if (!found.length) console.log(`collide: nothing called ${kind} found`);
  sample.push(...found);
}
console.log(`collide: trying ${sample.length} things: ${KINDS.join(", ")}`);

const clear = async (spot: [number, number]) => {
  const near = (await game.call("world.obstacles", { at: spot, within: 3 })) as Obstacle[];
  return near.every((o) => o.apart > o.radius + 0.6);
};

let failed = 0;
let tried = 0;
for (const [n, o] of sample.entries()) {
  // A clear spot a few metres off, at some turn round it.
  let stand: [number, number] | undefined;
  for (let turn = 0; turn < 12 && !stand; turn++) {
    const angle = (turn / 12) * Math.PI * 2 + n;
    const spot: [number, number] = [o.at[0] + Math.cos(angle) * 4.5, o.at[1] + Math.sin(angle) * 4.5];
    if (await clear(spot)) stand = spot;
  }
  if (!stand) continue;
  tried++;
  await game.warp(stand, o.at);
  await game.press("w:120");
  // Where the player is through the walk, nearest the obstacle's middle.
  let nearest = Infinity;
  let now: [number, number] = stand;
  const until = Date.now() + 2300;
  while (Date.now() < until) {
    now = (await game.state()).at;
    nearest = Math.min(nearest, Math.hypot(now[0] - o.at[0], now[1] - o.at[1]));
    await game.sleep(40);
  }
  // Its own footprint, as the game has it, and how close the body came to its middle: within
  // 0.3 m of the middle of anything solid is through it.
  const footprint = ((await game.call("world.obstacles", { at: o.at, within: 2.5 })) as Obstacle[])[0];
  const through = nearest < 0.3;
  const moved = Math.hypot(now[0] - stand[0], now[1] - stand[1]);
  const line = `collide: ${String(n).padStart(2)}: ${o.name.padEnd(7)} at ${o.at.map((v) => v.toFixed(1)).join(",")}: footprint ${footprint ? footprint.radius.toFixed(2) + " m" : "none"}; walked ${moved.toFixed(1)} m, came within ${nearest.toFixed(2)} m of its middle`;
  if (through) {
    failed++;
    console.log(`${line}: THROUGH IT`);
  } else {
    console.log(line);
  }
  if (n < 6 || through) await game.shot(`shots/collide/${n}.png`).catch(() => {});
}
await game.stop();
console.log(failed ? `collide: FAIL: walked through ${failed} of ${tried}` : `collide: pass: stopped by all ${tried}`);
process.exit(failed ? 1 : 0);
