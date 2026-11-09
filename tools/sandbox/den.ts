#!/usr/bin/env node
// A world's den, walked in the real game (D591, D643, docs/caves.md): a new 30 km world of a
// seed is played; `den.state` says where the den's mouth opens and its chamber's floor is;
// the player, put at the mouth facing in, walks forward and comes to the chamber's floor,
// under the land, with no way cut for them. The troll keeps the den (D590): met on the way,
// it is struck down by the script (`creatures.strike`, the rule a swing's blow goes by), and
// the player, unmarked before, bears its mark. Checked with no picture, by what the game
// says. How long the walk took is said: a reading.
//
//   node tools/sandbox/den.ts [--seed 3] [--fight]
//
// --fight: the troll is not struck down by the script. The player walks in and stands; the
// troll, roused, must warn (its roar) and then deal blows that land, by what `play.state`
// says it is doing and the health the player loses; what it did, in order, is said.
//
// Node 22.18 or later; no dependencies. About three minutes.

import { call, holdKey, press, start, stop, waitSeconds, waitUntil, warp } from "./client.ts";

const args = process.argv.slice(2);
const SEED = args.includes("--seed") ? args[args.indexOf("--seed") + 1] : "3";
const FIGHT = args.includes("--fight");
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
  const marks = async () => ((await call("marks.state")) as { marks: string[] }).marks;
  const troll = async () => ((await call("sandbox.find", { name: "Troll" })) as Found)[0]?.from_start;
  const home = await troll();
  if (!home) throw new Error(`no Troll is in seed ${SEED}'s world: data/creatures.ron's Troll lives in the Den, and terra_creatures puts one on its floor`);
  if (Math.hypot(home[0] - den.floor[0], home[1] - den.floor[1], home[2] - den.floor[2]) > den.room_across_m) throw new Error(`the troll stands at ${home.map((x) => x.toFixed(0)).join(", ")}, not in its den, whose floor is at ${den.floor.map((x) => x.toFixed(0)).join(", ")}`);
  if ((await marks()).includes("troll")) throw new Error("a new game's player bears the troll's mark before meeting it");
  let fell = "";
  await warp([den.mouth[0], den.mouth[2]], [den.ahead[0], den.ahead[2]]);
  const began = Date.now();
  let at = await player();
  // The camera on the way in: how near the rock it came, and whether it was ever in it.
  let eyeNearest = Infinity;
  const eye = async () => {
    const clear = ((await call("den.state")) as { eye_clear_m: number | null }).eye_clear_m;
    if (clear !== null) eyeNearest = Math.min(eyeNearest, clear);
  };
  // Forward, a second at a time, until the chamber or the clock.
  while ((off(at)[0] > NEAR_M || off(at)[1] > LEVEL_M) && Date.now() - began < MOST_S * 1000) {
    await holdKey("w", 1);
    for (let look = 0; look < 4; look++) {
      await waitSeconds(0.25);
      await eye();
    }
    at = await player();
    // Met on the way in: struck down before it lands a blow.
    const it = await troll();
    if (FIGHT && it && Math.hypot(it[0] - at[0], it[1] - at[1], it[2] - at[2]) < 14) break;
    if (!FIGHT && !fell && it && Math.hypot(it[0] - at[0], it[1] - at[1], it[2] - at[2]) < 25) {
      const blow = (await call("creatures.strike", { name: "Troll", damage: 5000 })) as { health: number; away_m: number };
      if (blow.health > 0) throw new Error(`struck for 5000 the troll has ${blow.health} health left`);
      fell = `${blow.away_m.toFixed(0)} m off`;
    }
  }
  if (FIGHT) {
    // Standing where the troll has noticed them: what it does, a quarter second at a time.
    type State = { health: number; near: { what?: string; doing?: string; distance?: number }[] };
    const doing: string[] = [];
    let health = 100;
    const hale = ((await call("play.state")) as State).health;
    for (let looks = 0; looks < 80 && health > 25; looks++) {
      const now = (await call("play.state")) as State;
      health = now.health;
      const it = now.near.find((each) => each.what === "Troll");
      const what = it?.doing?.replace(/\(.*/, "") ?? "gone";
      if (doing.at(-1) !== what) doing.push(what);
      await waitSeconds(0.25);
    }
    const bouts = doing.filter((what) => what === "Attack").length;
    if (bouts < 2 || health >= hale) throw new Error(`roused by a player standing 14 m off for 20 s the troll did ${doing.join(", ")} and took ${(hale - health).toFixed(0)} health: it should warn, come, and deal blows that land (data/creatures.ron's Troll: warns, blows; terra_creatures mind::hostile)`);
    console.log(`✓ den --fight: seed ${SEED}'s troll, roused, did ${doing.join(", ")}; the player, standing, lost ${(hale - health).toFixed(0)} of ${hale.toFixed(0)} health`);
    await stop().catch(() => undefined);
    process.exit(0);
  }
  const [away, level] = off(at);
  if (away > NEAR_M || level > LEVEL_M) throw new Error(`walking in from the den's mouth for ${MOST_S} s the player stands ${away.toFixed(1)} m from the chamber floor's middle and ${level.toFixed(1)} m off its level (at ${at.map((x) => x.toFixed(1)).join(", ")} east, up, south of the start; the floor is at ${den.floor.map((x) => x.toFixed(1)).join(", ")}): the way in is shut, too steep, or bends where walking straight on does not follow it (terra_land::cave, terra_voxel::caves)`);
  if (!fell) throw new Error("the player came to the chamber and never came within 25 m of the troll");
  await waitSeconds(1);
  const borne = await marks();
  if (!borne.includes("troll")) throw new Error(`the troll fell ${fell} and the player bears ${JSON.stringify(borne)}, not its mark: a kind's \`marks\` in data/creatures.ron is imprinted on the players by it when it falls (play::marks::earned, D590)`);
  if (eyeNearest < 0) throw new Error(`on the way into the den the camera was ${(-eyeNearest).toFixed(2)} m inside the rock: in a passage the camera must keep to the open (terra_walker camera: its boom is shortened by the ground behind the player)`);
  const down = den.mouth[1] - den.floor[1];
  console.log(`✓ den: seed ${SEED}'s den (way ${den.layout.way}, room ${den.layout.room}, ${den.layout.alcoves} alcoves) opens ${Math.hypot(den.mouth[0], den.mouth[2]).toFixed(0)} m from the start; walking in from its mouth the player came to its chamber's floor, ${down.toFixed(1)} m below the mouth, in ${((Date.now() - began) / 1000).toFixed(0)} s (a reading); the troll kept the den, fell ${fell} to the script's blow, and the player bears its mark; the camera kept ${eyeNearest.toFixed(2)} m and more from the rock`);
} catch (problem) {
  failed = String(problem instanceof Error ? problem.message : problem);
} finally {
  await stop().catch(() => undefined);
}
if (failed) {
  console.error(`✗ den: ${failed}`);
  process.exit(1);
}
