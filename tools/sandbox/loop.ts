#!/usr/bin/env node
// Plays the play experiment's loop end to end and checks each step happened (D69): talk to
// the elder, eat, hunt a deer with a bow and butcher it, dig stone out of the ground and
// find the hole still there after loading, kill the spiders, free the smith and bring them
// home, claim land, build a smelter and a forge, have the smith take up the forge, smelt
// iron, and have the smith make an iron axe.
//
// One running game, driven live over the remote protocol (tools/sandbox), restarted once,
// to load what it saved. Each step sets itself up (warp, give), acts, and waits for the
// game to say what happened; where it needs to know where something is, it asks the game.
// A step that fails says what the game said instead, and leaves a screenshot in
// shots/loop-<step>.png.
//
// Runs on the Windows GPU from WSL, and on this machine's own elsewhere; --here runs on
// this machine under WSL too. --until STEP stops after that step. Node 22.18 or later; no dependencies.

import * as game from "./client.ts";

const SAVE = "loop.ron";
const here = process.argv.includes("--here");
// --until STEP stops after that step, to try one again without the rest.
const until = process.argv[process.argv.indexOf("--until") + 1];
class Done extends Error {}
const windows = game.onWsl && !here;
let failed = false;

/// Runs a step: what it does, then what the game must have said since it began, in any
/// order. Stops the loop at the first step that fails, since later steps build on it.
async function step(name: string, act: () => Promise<void>, ...said: string[]) {
  const since = await game.markNow();
  try {
    await act();
    for (const words of said) {
      const line = await game.waitFor(words, { since, timeout: 90 });
      console.log(`ok    ${name}: ${line.text}`);
    }
  } catch (error) {
    if (error instanceof Done) throw error;
    console.log(`FAIL  ${name}: ${(error as Error).message}`);
    await game.shot(`shots/loop-${name}.png`).catch(() => {});
    failed = true;
    throw error;
  }
  if (name === until) throw new Done();
}

const nearest = async (test: (near: game.Near) => boolean) => {
  const found = (await game.state()).near.find(test);
  if (!found) throw new Error("nothing like that is near");
  return found;
};
const alive = (what: string) => (near: game.Near) => near.what === what && !near.doing?.startsWith("Dead");

/// Where a step said something stood, east and south, from a line like
/// "It fell -33.6 m east and 28.1 m south" or "Placed a Forge at [12.3, 0.5, 18.1], 0 up."
function where(line: game.Line): [number, number] {
  const fell = /(-?[\d.]+) m east and (-?[\d.]+) m south/.exec(line.text);
  if (fell) return [Number(fell[1]), Number(fell[2])];
  const placed = /at \[(-?[\d.]+), -?[\d.]+, (-?[\d.]+)\]/.exec(line.text);
  if (placed) return [Number(placed[1]), Number(placed[2])];
  throw new Error(`no place in "${line.text}"`);
}

/// Every cell holds one raw item, so after a few steps the pack is full. A step that needs
/// room takes away what earlier steps left, then gives what it needs.
const RAW = ["Wood", "Stone", "Fibre", "Hide", "Ore", "Iron", "Meat", "Berries", "Cooked meat"];
async function emptyPack() {
  const carried = Object.keys((await game.state()).carrying).filter((name) => RAW.includes(name));
  if (carried.length) await game.take(...carried.map((name) => name.replace(" ", "_")));
}

/// Up to a station, a little short of it, looking at it.
async function toward([east, south]: [number, number]) {
  await game.warp([east, south - 2.8], [east, south]);
  await game.aim([east, south], 0.6);
}

async function play() {
  console.log(`Starting the game${windows ? " on Windows" : ""}…`);
  await game.start(["--experiment", "play", "--fresh", "--save", SAVE], { windows, copy: "loop" });

  await step(
    "talk",
    async () => {
      await game.give("berries:2", "fed:50");
      // The village stands where the world has dry ground for it, said as the game starts.
      const [east, south] = where(await game.waitFor("The village stands", { since: 0 }));
      await game.warp([east + 2, south - 5], [east, south]);
      const maren = await nearest((near) => near.who === "Maren");
      await game.warp([maren.at[0], maren.at[1] - 2.3], maren.at);
      await game.aim(maren.at, 1.2);
      await game.press("f", "h@20");
    },
    'Notice: Maren: "Spiders took Bren',
    "Notice: Ate berries",
  );

  // A deer from beyond its hearing with a bow, then butchered with the knife where it fell.
  let fell: [number, number] = [0, 0];
  await step(
    "hunt",
    async () => {
      await game.give("bow", "quiver", "arrow:6");
      // The herd grazes where the world has dry ground for it, said as the game starts.
      const [east, south] = where(await game.waitFor("Deer graze", { since: 0 }));
      await game.warp([east + 15, south], [east, south]);
      await game.hold("Bow");
      const since = await game.markNow();
      for (let arrow = 0; arrow < 6; arrow++) {
        // Within a sure shot of the nearest deer, but beyond where it would bolt.
        let target = await nearest(alive("Deer"));
        if (target.distance > 14) {
          await game.warp([target.at[0] + 12, target.at[1]], target.at);
          target = await nearest(alive("Deer"));
        }
        await game.aim(target.at, 0.8);
        await game.press("e:60");
        await game.waitFor("Loosed an arrow", { timeout: 20 });
        await game.waitSeconds(1);
        const { lines } = await game.said(since, "It fell");
        if (lines.length) {
          fell = where(lines[0]);
          break;
        }
      }
      await game.warp([fell[0] + 2, fell[1]], fell);
      await game.hold("Knife");
      await game.aim(fell, 0.2);
      await game.press("f");
    },
    "An arrow hit a Deer",
    "Killed a Deer.",
    "Notice: Butchered it",
  );

  // Digging the meadow with the pickaxe: soil, then stone.
  await step(
    "dig",
    async () => {
      await game.warp([7.5, 14], [7.5, 10]);
      await game.hold("Stone pickaxe");
      await game.aim([7.5, 12.5], 0);
      await game.press("e:65", "e@75:65", "e@150:65");
    },
    "Gained 2 Stone.",
  );

  // Saved as it closes, and the hole is there again when the game is loaded. The new run
  // numbers what it says from the start again.
  await step("reload", async () => {
    await game.stop();
    await game.start(["--experiment", "play", "--save", SAVE], { windows, copy: "loop" });
    const line = await game.waitFor("changes to the ground.", { since: 0, timeout: 60 });
    console.log(`ok    reload: ${line.text}`);
  });

  // In leather with a spear, facing the spiders as they come, then over to Bren.
  await step(
    "rescue",
    async () => {
      await game.give("spear", "leather_coat", "leather_cap", "health:100", "stamina:100", "fed:80");
      // Spear in hand before walking in among them.
      await game.hold("Spear");
      await game.warp([58, -58], [70, -60]);
      // Swing every half second until no spider is left alive near.
      await game.press(...Array.from({ length: 240 }, (_, i) => `e@${i * 30}`));
      await game.waitFor("Killed a Spider", { timeout: 120 });
      await game.waitUntil("every spider near to be dead", game.state, (now) => !now.near.some((near) => near.what === "Spider" && !near.doing?.startsWith("Dead") && near.distance < 30), 180);
      await game.unpress();
      const bren = await nearest((near) => near.who === "Bren");
      await game.warp([bren.at[0] - 2.2, bren.at[1] + 2.5], bren.at);
      await game.aim(bren.at, 0.8);
      await game.press("f");
      await game.waitFor("Freed Bren.", { timeout: 30 });
      // Home, and Bren follows.
      await game.warp([8, 6], [0, 0]);
      await game.waitUntil("Bren to follow home", game.state, (now) => now.near.some((near) => near.who === "Bren" && near.distance < 8), 180);
    },
    "Killed a Spider",
    "Freed Bren.",
  );

  // A claim stone, and a smelter in the claim, from the builder's menu.
  let smelter: [number, number] = [0, 0];
  await step(
    "build",
    async () => {
      await emptyPack();
      await game.give("stone:12", "wood:2");
      // On the meadow north of the lake, facing north, as the cabin builds.
      await game.warp([0, -8], [0, -14]);
      await game.hold("Hammer");
      await game.press("b@20", "2@30", "enter@40", "e@70:80", "b@180", "4@190", "right@200", "enter@210", "right@230:40", "e@290:200");
      smelter = where(await game.waitFor("Placed a Smelter", { timeout: 60 }));
    },
    "Placed a Claim stone",
    "Placed a Smelter",
  );

  // A forge beside it, which Bren takes up.
  let forge: [number, number] = [0, 0];
  await step(
    "forge",
    async () => {
      await emptyPack();
      await game.give("stone:8", "wood:4");
      // Beside the smelter, inside the claim.
      await game.warp([-4, -8], [-4, -14]);
      await game.hold("Hammer");
      await game.press("b@80", "4@90", "right@100", "right@105", "enter@115", "e@150:200");
      forge = where(await game.waitFor("Placed a Forge", { timeout: 60 }));
    },
    "Placed a Forge",
    "Bren took up the forge.",
  );

  // Three iron queued at the smelter, which works on its own; it hands them over when
  // opened again after they are done.
  await step(
    "smelt",
    async () => {
      await emptyPack();
      await game.give("ore:6", "wood:3");
      await toward(smelter);
      await game.press("f", "enter@20", "enter@25", "enter@30", "f@50");
      await game.waitFor("Queued Iron at the Smelter.", { timeout: 30 });
      await game.waitSeconds(35);
      await game.press("f");
    },
    "Queued Iron at the Smelter.",
    "Made Iron.",
  );

  // An iron axe at the forge, made while you stay: the better tool the loop is for.
  await step(
    "axe",
    async () => {
      await game.press("esc");
      await game.give("wood:2");
      await toward(forge);
      await game.press("f", "enter@20");
    },
    "Queued Iron axe at the Forge.",
    "Made Iron axe.",
  );
}

try {
  await play();
} catch (error) {
  // A failed step has said so; anything else, such as the game not starting, has not.
  if (!(error instanceof Done) && !failed) {
    console.log(`FAIL  ${(error as Error).message} (see shots/remote.log)`);
    failed = true;
  }
} finally {
  await game.stop().catch(() => {});
}
process.exit(failed ? 1 : 0);
