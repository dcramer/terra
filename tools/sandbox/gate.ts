#!/usr/bin/env node
// A world's gate, end to end in the real game (D641, D470; v0's "the gate placed from the
// seed, reachable on every seed"): a new 30 km world of a seed is played; its gate stands
// where the seed places it, 150 to 800 m from the start, shut; the player, warped before it,
// cannot step through until they bear the troll's mark (D590), which opens it for good;
// they step through to the realm beyond, a world of another seed, and stands before that world's gate,
// which leads back; the game is closed there and started again, and is in the realm beyond
// where it was left (the save keeps each world's own record, D471); stepping back, they
// stand where they left the first world. Checked with no picture, by
// what the game says (`title.state`, `find`, `portal.step`). Each crossing's time is said: a
// reading, as these are cold worlds of full size.
//
//   node tools/sandbox/gate.ts [--seed 3]
//
// Node 22.18 or later; no dependencies. About eight minutes.

import { call, said, start, stop, waitSeconds, waitUntil, warp } from "./client.ts";

const args = process.argv.slice(2);
const SEED = args.includes("--seed") ? args[args.indexOf("--seed") + 1] : "3";

type Found = { name: string; from_start: [number, number, number] }[];
type Title = { screen: string; seed: string | null };
const title = async () => (await call("title.state")) as Title;
/// Where the first entity of a name stands, east and south of the start.
const stands = async (name: string): Promise<[number, number]> => {
  const found = (await call("sandbox.find", { name })) as Found;
  if (found.length === 0) throw new Error(`nothing called ${name} stands in this world`);
  return [found[0].from_start[0], found[0].from_start[2]];
};
const apart = (a: [number, number], b: [number, number]) => Math.hypot(a[0] - b[0], a[1] - b[1]);
const playing = (seed: (seed: string) => boolean) => waitUntil("the world to be played", title, (now) => now.screen === "Playing" && now.seed !== null && seed(now.seed), 600);

const game = (fresh: boolean) => start(["--experiment", "play", ...(fresh ? ["--fresh"] : []), "--save", "shots/gate.ron", "--radius", "30000", "--seed", SEED, "--offscreen", "--mute"], { realTime: true, release: true });

let failed = "";
try {
  await game(true);
  await playing((seed) => seed === SEED);
  const gate = await stands("Portal");
  const away = Math.hypot(...gate);
  if (away < 150 || away > 800) throw new Error(`the gate of seed ${SEED} stands ${away.toFixed(0)} m from the start, not 150 to 800 m (terra_land::gate)`);
  // Before the gate, on the start's side of it.
  const before: [number, number] = [gate[0] * (1 - 6 / away), gate[1] * (1 - 6 / away)];
  await warp(before);
  const left = await stands("Player");

  // Shut to one who bears no mark: they are told so and stay.
  type Gate = { open: boolean; away_m: number; to: { seed: string } };
  const gateNow = async () => (await call("portal.state")) as Gate;
  if ((await gateNow()).open) throw new Error("a new world's gate stands open before anyone bears the mark");
  await call("portal.step");
  await waitSeconds(2);
  if ((await title()).seed !== SEED) throw new Error("an unmarked player went through a shut gate");
  const told = (await said(0, "Notice: The gate", 5)).lines.map((line) => line.text).join(" ");
  if (!/gate is shut/i.test(told)) throw new Error(`an unmarked player at the shut gate was told "${told}", not that it is shut`);
  // The mark is earned (D590): at the den's mouth the troll in it is struck down (by the
  // script, through the rule a swing's blow goes by), and the player by it bears its mark.
  // Back before the gate, the first step opens it and the next goes through.
  const { den } = (await call("den.state")) as { den: { mouth: [number, number, number] } | null };
  if (!den) throw new Error(`seed ${SEED}'s world has no den to earn the mark at (cargo xtask worlds says why)`);
  await warp([den.mouth[0], den.mouth[2]]);
  await call("creatures.strike", { name: "Troll", damage: 5000 });
  await waitUntil("the player to bear the troll's mark after its fall", async () => (await call("marks.state")) as { marks: string[] }, (now) => now.marks.includes("troll"), 10);
  await warp(before);
  await call("portal.step");
  await waitUntil("the gate to open to the mark", gateNow, (gate) => gate.open, 20);
  // As one who walked the last hundred metres to it: the world beyond is made meanwhile.
  await waitSeconds(20);
  const out = (await call("portal.step")) as { away_m: number; to: { seed: string } };
  if (out.to.seed === SEED) throw new Error("the first world's gate leads to its own world");
  await playing((seed) => seed === out.to.seed);
  const took = (await said(0, "Portal: crossed", 5)).lines.map((line) => / in ([0-9.]+) s/.exec(line.text)?.[1]).filter(Boolean);
  const [there, me] = [await stands("Portal"), await stands("Player")];
  if (apart(there, me) > 4) throw new Error(`in the realm beyond the player stands ${apart(there, me).toFixed(1)} m from its gate, not before it`);

  // The realm beyond is another realm by its data (D642): its kind, its light, its parts.
  const realm = (await call("realm.state")) as { kind: string; name: string | null; beyond: string; sun: number[] | null };
  const parts = (await said(0, "of its parts stand", 5)).lines.map((line) => / (\d+) of its parts stand/.exec(line.text)?.[1]).filter(Boolean);
  if (realm.kind === "world" || realm.beyond !== "world" || !realm.sun) throw new Error(`the realm beyond is of kind "${realm.kind}" leading on to "${realm.beyond}" with ${realm.sun ? "its own" : "no"} light: assets/data/realms.ron should give the first realm another kind beyond it, with its light, leading back`);
  if (!(Number(parts.at(-1)) > 0)) throw new Error(`${realm.name ?? realm.kind} stood none of its parts round the start (play::realm_parts says how many in the log)`);

  // Its parts follow the player (D127): far from the start they stand round them, and the
  // start's are there again on coming back, as many as before.
  const partsNear = async () => {
    const [found, at] = [(await call("sandbox.find", { name: "Industrial" })) as Found, await stands("Player")];
    return { count: found.length, farthest: Math.max(0, ...found.map((part) => apart([part.from_start[0], part.from_start[2]], at))) };
  };
  const first = await partsNear();
  await warp([me[0] + 1500, me[1]]);
  await waitSeconds(5);
  const far = await partsNear();
  await warp(me);
  await waitSeconds(5);
  const returned = await partsNear();
  if (far.count === 0 || far.farthest > 600) throw new Error(`1500 m from where the player arrived ${far.count} of the realm's parts stand, the farthest ${far.farthest.toFixed(0)} m from them: play::realm_parts stands them in the cells within reach of the player and lets go those past it`);
  if (Math.abs(returned.count - first.count) > 0.1 * first.count) throw new Error(`come back to where they arrived the player has ${returned.count} of the realm's parts round them, where ${first.count} stood before they left: a cell come back to must hold what it held, by the land alone (play::realm_parts)`);

  // Closed in the realm beyond and started again: there, where they stood, not in the first
  // world with the realm's things written over it.
  await stop();
  await game(false);
  await playing((seed) => seed === out.to.seed);
  const again = await stands("Player");
  if (apart(again, me) > 1) throw new Error(`started again, the player stands ${apart(again, me).toFixed(1)} m from where the game was closed in the realm beyond`);

  const back = (await call("portal.step")) as { to: { seed: string } };
  if (back.to.seed !== SEED) throw new Error(`the realm beyond's gate leads to seed ${back.to.seed}, not back to ${SEED}`);
  await playing((seed) => seed === SEED);
  const home = await stands("Player");
  // The first world's gate, opened before the game was closed, is open still, and the mark borne.
  const still = await gateNow();
  const borne = (await call("marks.state")) as { marks: string[] };
  if (!still.open || !borne.marks.includes("troll")) throw new Error(`started again and back, the first gate is ${still.open ? "open" : "shut"} and the player bears ${JSON.stringify(borne.marks)}: the save did not keep them`);
  if (apart(home, left) > 1) throw new Error(`back through the gate the player stands ${apart(home, left).toFixed(1)} m from where they left`);
  console.log(`✓ gate: seed ${SEED}'s gate stands ${away.toFixed(0)} m from the start, shut to an unmarked player and opened by the troll's mark, earned at its den; through it to ${realm.name ?? realm.kind} (seed ${out.to.seed}, ${parts.at(-1)} of its parts standing, ${far.count} round the player 1500 m on and ${returned.count} again on coming back), ${apart(there, me).toFixed(1)} m before its gate, closed and started again there (${apart(again, me).toFixed(1)} m from where it was closed), and back to within ${apart(home, left).toFixed(1)} m of where the player left the first world; the crossing out, its world warmed as the player neared, took ${took.at(-1) ?? "?"} s (a reading)`);
} catch (problem) {
  failed = String(problem instanceof Error ? problem.message : problem);
} finally {
  await stop().catch(() => undefined);
}
if (failed) {
  console.error(`✗ gate: ${failed}`);
  process.exit(1);
}
