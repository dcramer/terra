#!/usr/bin/env node
// A saved game comes back as it was saved, on a whole world behind the loading screen (a
// continuous check, tools/continuous/checks.json; D579, D623): a new 30 km world is made in
// a worlds folder of its own, and in it a chest set down and a wall laid in the base, the
// clock run on, the woods' nearest grown tree felled with the axe, which leaves a stump, and
// the trees round it felled outright, and the player moved; the game is closed, which saves
// it, and opened again on the same world. The base, what stands in it, the hour and the day,
// the felled trees, the stump and where the player stands must all be as they were. A world is not loaded behind the loading screen in a layer's test, so this
// runs the real game.
//
//   node tools/sandbox/saved.ts [--seed S] [--out target/check/saved]
//
// Release build, offscreen; the run's log is shots/remote.log, the save is kept under --out.
// A line a check; exits 1 when one fails, saying what came back instead. Node 22.18 or
// later; no dependencies.

import { rmSync } from "node:fs";
import { resolve } from "node:path";
import * as game from "./client.ts";

const argv = process.argv.slice(2);
const option = (name: string, otherwise: string) => {
  const at = argv.indexOf(name);
  return at >= 0 ? argv[at + 1] : otherwise;
};
/// j2's first seed, whose start is on a meadow by the woods.
const SEED = option("--seed", "9697367627075512559");
const OUT = resolve(option("--out", "target/check/saved"));
const WORLDS = resolve(OUT, "worlds");

type Clock = { hour: number; day: number };
type Played = game.State & { clock: Clock | null };
type Base = { cubes: number; planned: number; parts: unknown[]; props: unknown[]; since: number };
type Felled = { felled: number; trees: { at: [number, number]; height: number }[] };

const played = async () => (await game.state()) as Played;
const base = async () => (await game.call("build.state")) as Base;
/// The storage set down in the claim and what each holds, as a string to compare.
const stored = async () => JSON.stringify(await game.call("storage.state"));
const fell = async (at: [number, number], within: number) => (await game.call("world.fell", { at, within, taller_than: 3 })) as Felled;
/// The stumps standing within a metre of a spot.
const stumps = async (at: [number, number]) => ((await game.call("lush.solids", { at, within: 1 })) as { name: string; at: [number, number] }[]).filter((solid) => solid.name === "Stump");
/// What of the base is kept, less what moves by itself.
const kept = (base: Base) => JSON.stringify({ cubes: base.cubes, planned: base.planned, parts: base.parts, props: base.props });

let failed = 0;
function check(that: boolean, name: string, said: string) {
  console.log(`${that ? "ok  " : "FAIL"}  ${name}: ${said}`);
  if (!that) failed++;
}

const run = (fresh: boolean) =>
  game.start(["--experiment", "play", `--worlds=${WORLDS}`, ...(fresh ? ["--fresh"] : []), "--seed", SEED, "--free-build"], { release: true });

async function play() {
  rmSync(WORLDS, { recursive: true, force: true });
  await run(true);
  console.log("saved: a new world made");

  // The trees round the woods' nearest grown tree to the start, felled.
  const nearest = (await game.said(0, "nearest grown tree to the start", 1)).lines[0];
  const where = nearest && /stands (-?[\d.]+) m east and (-?[\d.]+) m south/.exec(nearest.text);
  if (!where) throw new Error(`the woods never said their nearest grown tree; is seed ${SEED} wooded? See shots/remote.log`);
  const tree: [number, number] = [Number(where[1]), Number(where[2])];
  // It felled with the axe the player starts with, as a player fells one, leaving a stump.
  await game.warp([tree[0] + 1.5, tree[1]]);
  await game.aim(tree, 1.4);
  const axe = ((await played()) as Played & { bar?: (string | null)[] }).bar?.find((tool) => !!tool && /axe/i.test(tool));
  if (!axe) throw new Error("the player starts with no axe on the bar");
  await game.hold(axe);
  await game.press("e:1800");
  await game.waitFor("Cut something for", { timeout: 90 });
  await game.unpress();
  await game.waitSeconds(4);
  const stumped = await stumps(tree);
  if (stumped.length !== 1) throw new Error(`the tree felled with the axe at ${tree} left ${stumped.length} stumps, not one: ${JSON.stringify(stumped)}`);
  // The trees round it felled outright, as a level designer clears them.
  const felled = await fell(tree, 8);
  felled.trees.push({ at: tree, height: 0 });
  if (felled.felled === 0) throw new Error(`world.fell felled nothing within 8 m of ${tree}`);

  // A claim stone, and in its claim a chest set down and a wall laid.
  for (const [item, at] of [
    ["Claim stone", [0, 0, 0]],
    ["Chest", [3, 0, 6]],
  ] as const) {
    const placed = (await game.call("storage.place", { item, at })) as { done: boolean; refused?: string };
    if (!placed.done) throw new Error(`the ${item} was refused: ${placed.refused}`);
    await game.waitSeconds(0.5);
  }
  const wall = (await game.call("build.lay", { what: "wall", kind: "logs", min: [0, 0, 10], max: [5, 0, 10] })) as { done: boolean; refused?: string };
  if (!wall.done) throw new Error(`the wall was refused: ${wall.refused}`);

  // The clock run on some hours, then held, so the hour saved is the hour read.
  const was = (await played()).clock;
  await game.tune("daylight.pace", 60);
  await game.waitSeconds(4);
  await game.tune("daylight.pace", 0);

  // The player moved off the start.
  await game.warp([tree[0] + 12, tree[1] - 9]);
  await game.waitSeconds(1);
  const before = { played: await played(), base: await base(), stored: await stored() };
  console.log(`saved: felled ${felled.felled} trees by ${tree}, a chest and a wall, the clock from ${JSON.stringify(was)} to ${JSON.stringify(before.played.clock)}, the player at ${before.played.at}`);
  await game.stop();

  await run(false);
  console.log("saved: the world opened again");
  const after = { played: await played(), base: await base(), stored: await stored() };
  check(kept(after.base) === kept(before.base), "base", kept(after.base) === kept(before.base) ? `${before.base.parts.length} parts and ${before.base.cubes} cubes as saved` : `saved ${kept(before.base)}\nloaded ${kept(after.base)}`);
  const chests = (state: string) => {
    const read = JSON.parse(state) as unknown;
    return Array.isArray(read) ? read.length : Object.keys(read as object).length;
  };
  check(after.stored === before.stored && chests(before.stored) > 0, "pieces", after.stored === before.stored ? `${chests(before.stored)} storage pieces set down as saved` : `saved ${before.stored}\nloaded ${after.stored}`);
  const [hour, day] = [before.played.clock, after.played.clock];
  const sameClock = !!hour && !!day && hour.day === day.day && Math.abs(hour.hour - day.hour) < 0.25;
  check(sameClock, "clock", `saved ${JSON.stringify(hour)}, loaded ${JSON.stringify(day)}`);
  const moved = Math.hypot(after.played.at[0] - before.played.at[0], after.played.at[1] - before.played.at[1]);
  check(moved < 1, "player", `saved at ${before.played.at}, loaded at ${after.played.at} (${moved.toFixed(2)} m apart)`);
  const again = await fell(tree, 8);
  const back = again.trees.filter((tree) => felled.trees.some((was) => Math.hypot(was.at[0] - tree.at[0], was.at[1] - tree.at[1]) < 0.5));
  check(back.length === 0, "felled", back.length === 0 ? `none of the ${felled.trees.length} felled stands again` : `${back.length} of the ${felled.trees.length} felled stand again: ${JSON.stringify(back.map((tree) => tree.at))}`);
  const stumpsBack = await stumps(tree);
  const apart = stumpsBack.length === 1 ? Math.hypot(stumpsBack[0].at[0] - stumped[0].at[0], stumpsBack[0].at[1] - stumped[0].at[1]) : Infinity;
  check(apart < 0.1, "stump", apart < 0.1 ? `the stump of the tree felled with the axe stands as saved (${apart.toFixed(3)} m)` : `saved one stump at ${stumped[0].at}, loaded ${stumpsBack.length}: ${JSON.stringify(stumpsBack)}`);
}

try {
  await play();
} catch (error) {
  console.log(`FAIL  ${(error as Error).message}`);
  failed++;
} finally {
  await game.stop().catch(() => {});
}
console.log(failed ? `saved: ✗ ${failed} failed (log shots/remote.log)` : "saved: the game came back as saved");
process.exit(failed ? 1 : 0);
