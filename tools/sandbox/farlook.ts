#!/usr/bin/env node
// The far realm, looked at in one step (D642, D71): a new world's troll is felled, its gate
// opened and gone through, and stills are taken of the realm at each hour asked, looking at
// the nearest things of a name, laid on one sheet; how many frames a second the first realm
// and this one run at where the player stands is said. Each hour is a run of its own, about
// four minutes, most of it reaching the gate. The stills are deleted once on the sheet.
//
//   node tools/sandbox/farlook.ts [--hours 13,22] [--look "Cap tree"] [--views 2]
//                                 [--seed 3] [--out shots/far.png] [--quality high]
//                                 [--stage models/creatures/A.glb,models/creatures/B.glb]
//
// --stage stands those models in a row before the player, where they arrive, and the stills
// are of them (a view each two): a creature or a prop seen in the realm's light without
// waiting to meet one.
//
// Node 22.18 or later; no dependencies.

import { rmSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { call, shot, start, stop, waitSeconds, waitUntil, warp } from "./client.ts";

const args = process.argv.slice(2);
const option = (name: string, otherwise: string) => (args.includes(name) ? args[args.indexOf(name) + 1] : otherwise);
const HOURS = option("--hours", "13").split(",");
const LOOK = option("--look", "Cap tree");
const VIEWS = Number(option("--views", "2"));
const SEED = option("--seed", "3");
const OUT = option("--out", "shots/far.png");
const QUALITY = option("--quality", "high");
const STAGE = args.includes("--stage") ? option("--stage", "").split(",") : [];

type Found = { name: string; from_start: number[] };
const playing = (seed: (s: string) => boolean) =>
  waitUntil("a world to be played", async () => (await call("title.state")) as { screen: string; seed: string | null }, (now) => now.screen === "Playing" && now.seed !== null && seed(now.seed), 600);
const rate = async () => {
  const before = (await call("sandbox.status")) as { frame: number };
  await waitSeconds(5);
  return (((await call("sandbox.status")) as { frame: number }).frame - before.frame) / 5;
};

/// The realm at an hour: its stills' files.
async function look(hour: string): Promise<string[]> {
  const stills: string[] = [];
  try {
    await start(["--experiment", "play", "--fresh", "--save", "shots/farlook.ron", "--radius", "30000", "--seed", SEED, "--date", "7-10", "--time", hour, "--offscreen", "--mute", "--clean", "--quality", QUALITY], { realTime: true, release: true });
    await playing((seed) => seed === SEED);
    await waitSeconds(12);
    const first = await rate();
    // The troll's mark opens the gate.
    const { den } = (await call("den.state")) as { den: { mouth: number[] } };
    await warp([den.mouth[0], den.mouth[2]]);
    await call("creatures.strike", { name: "Troll", damage: 5000 });
    await waitUntil("the troll's mark", async () => (await call("marks.state")) as { marks: string[] }, (now) => now.marks.includes("troll"), 20);
    const gate = ((await call("sandbox.find", { name: "Portal" })) as Found[])[0].from_start;
    const away = Math.hypot(gate[0], gate[2]);
    await warp([gate[0] * (1 - 6 / away), gate[2] * (1 - 6 / away)]);
    await call("portal.step");
    await waitUntil("the gate to open", async () => (await call("portal.state")) as { open: boolean }, (now) => now.open, 30);
    await waitSeconds(20);
    const through = (await call("portal.step")) as { to: { seed: string } };
    await playing((seed) => seed === through.to.seed);
    await waitSeconds(15);
    console.log(`at ${hour}: the first realm ${first.toFixed(0)} frames a second, the far realm ${(await rate()).toFixed(0)}`);
    const me = ((await call("sandbox.find", { name: "Player" })) as Found[])[0].from_start;
    const from = (at: number[]) => Math.hypot(at[0] - me[0], at[2] - me[2]);
    if (STAGE.length > 0) {
      // Twenty metres on from the gate the player arrived by, so its ring is out of the
      // picture: in a row across the way, two metres apart, five metres before the player.
      const gate = ((await call("sandbox.find", { name: "Portal" })) as Found[])[0]?.from_start ?? [me[0] - 1, me[1], me[2]];
      const out = Math.hypot(me[0] - gate[0], me[2] - gate[2]) || 1;
      const [ax, az] = [(me[0] - gate[0]) / out, (me[2] - gate[2]) / out];
      const along = (m: number, across: number) => [me[0] + ax * m - az * across, me[2] + az * m + ax * across];
      for (const [index, model] of STAGE.entries()) {
        const [x, z] = along(25, (index - (STAGE.length - 1) / 2) * 2);
        const [fx, fz] = along(25, 50);
        // In the base's metres, which are the start's, as `from_start` is.
        await call("staged.spawn", { name: `staged ${index}`, model, at: [x, me[1], z], facing: [fx, me[1], fz], base: true });
      }
      for (let view = 0; view * 2 < STAGE.length; view++) {
        const across = (view * 2 + 0.5 - (STAGE.length - 1) / 2) * 2;
        await warp(along(20, across), along(25, across));
        await waitSeconds(4);
        stills.push(`shots/farlook-${hour}-${view}.png`);
        await shot(stills[stills.length - 1]);
      }
      return stills;
    }
    const found = ((await call("sandbox.find", { name: LOOK })) as Found[]).map((one) => one.from_start).sort((a, b) => from(a) - from(b));
    if (found.length === 0) console.log(`  nothing named ${LOOK} stands within reach: the stills are of where the player arrived`);
    for (let view = 0; view < VIEWS; view++) {
      const at = found[view];
      if (at) {
        // From 26 m off, on the line from where the player arrived, looking up at it.
        const off = from(at);
        await warp([at[0] - ((at[0] - me[0]) / off) * 26, at[2] - ((at[2] - me[2]) / off) * 26], [at[0], at[2]]);
        await waitSeconds(10);
        await call("play.aim", { at: [at[0], at[2]], up: 8, level: true }).catch(() => undefined);
        await waitSeconds(2);
      } else if (view > 0) break;
      stills.push(`shots/farlook-${hour}-${view}.png`);
      await shot(stills[stills.length - 1]);
    }
  } finally {
    await stop().catch(() => undefined);
  }
  return stills;
}

const stills: string[] = [];
let failed = "";
try {
  for (const hour of HOURS) stills.push(...(await look(hour)));
  const sheet = spawnSync(process.execPath, ["tools/sandbox/sheet.ts", ...stills, "--out", OUT, "--wide", "640", "--across", String(VIEWS)], { encoding: "utf8" });
  if (sheet.status !== 0) throw new Error(`the sheet was not made: ${sheet.stderr.trim().split("\n").pop()}`);
  console.log(`✓ farlook: ${stills.length} stills of the far realm at ${HOURS.join(" and ")} o'clock, a row an hour, in ${OUT} (delete it once looked at)`);
} catch (problem) {
  failed = String(problem instanceof Error ? problem.message : problem);
} finally {
  for (const still of stills) rmSync(still, { force: true });
}
if (failed) {
  console.error(`✗ farlook: ${failed}`);
  process.exit(1);
}
