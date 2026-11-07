#!/usr/bin/env node
// The title screen's worlds in one process (docs/v0-tracks.md, Front's bar; D579, D596): a
// run on the title screen makes a world, walks in it and leaves for the title screen, makes
// a second from another seed and sees other land, leaves again and loads the first, which
// is as it was left. The game is never started again. Checked with no picture, by what the
// game says (`title.state`, `play.state`).
//
//   node tools/sandbox/title.ts
//
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "../..");

type Title = { screen: string; seed: string | null; playing: string | null; page: string | null; picked: string | null; worlds: { name: string; seed: string }[] | null; notice: string | null };
type Place = [number, number, number];

function remote(...words: string[]): string {
  const ran = spawnSync("node", [resolve(ROOT, "tools/sandbox/remote.ts"), ...words], { cwd: ROOT, encoding: "utf8" });
  if (ran.status !== 0) throw new Error(`remote ${words.join(" ")} failed:\n${ran.stdout}${ran.stderr}`);
  return ran.stdout;
}
const title = () => JSON.parse(remote("call", "title.state")) as Title;
/// The running game's token, made as it starts: another game has another.
const pid = () => readFileSync(resolve(ROOT, "shots/remote.port"), "utf8").split(" ")[1];
const feet = () => (JSON.parse(remote("call", "net.state")) as { mine: Place | null }).mine;
const sleep = (seconds: number) => new Promise((done) => setTimeout(done, seconds * 1000));
const apart = (a: Place, b: Place) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

async function until(what: string, test: () => boolean, seconds = 240) {
  for (let waited = 0; waited < seconds; waited += 0.5) {
    if (test()) return;
    await sleep(0.5);
  }
  throw new Error(`${what}, after ${seconds} s. Read shots/remote.log for what the game said.`);
}

/// Presses keys one at a time until the title screen's pick is the choice named.
async function pick(choice: string) {
  for (let tries = 0; tries < 12 && title().picked !== choice; tries++) {
    remote("press", "down");
    await sleep(0.3);
  }
  if (title().picked !== choice) throw new Error(`the title screen never offered ${choice}: it shows ${JSON.stringify(title())}`);
}

async function choose(choice: string) {
  await pick(choice);
  remote("press", "enter");
  await sleep(0.3);
}

/// On the title screen: makes a new world, with the name and seed offered, and plays it.
async function make(): Promise<Title> {
  await until("the title screen never came up", () => title().screen === "Title");
  await choose("NewWorld");
  await choose("Create");
  await until("the new world never began", () => title().screen === "Playing" && feet() !== null);
  return title();
}

async function leave() {
  remote("call", "title.leave");
  await until("the game never came back to the title screen", () => title().screen === "Title");
}

const worlds = mkdtempSync(join(tmpdir(), "terra-title-"));
let failed = "";
try {
  remote("start", "--real-time", "--", "--experiment", "play", "--offscreen", "--title", `--worlds=${worlds}`);
  const began = pid();

  const first = await make();
  const stood = feet()!;
  remote("press", "w:120");
  await until(`the player of ${first.playing} did not walk a metre in 120 frames`, () => apart(stood, feet()!) > 1, 60);
  // Standing again: where it is left is where it must be found.
  let walked = feet()!;
  await until("the player never came to rest", () => {
    const now = feet()!;
    const still = apart(now, walked) < 0.001;
    walked = now;
    return still;
  }, 60);
  await leave();
  console.log(`✓ made ${first.playing} from seed ${first.seed}, walked ${apart(stood, walked).toFixed(1)} m in it and left for the title screen`);

  const second = await make();
  if (second.seed === first.seed) throw new Error(`the second world has the first's seed, ${first.seed}: New world must roll another (D596)`);
  const there = feet()!;
  if (apart(there, walked) < 0.5) throw new Error(`the second world's player stands where the first's was left: the player of the world left came along`);
  if (apart(there, stood) < 0.01) throw new Error(`the second world's start is the first's to a centimetre: its land is the same land`);
  await leave();
  const listed = title().worlds ?? [];
  if (listed.length !== 2) throw new Error(`the title screen lists ${listed.length} worlds of the 2 made`);
  console.log(`✓ made ${second.playing} from seed ${second.seed}: other land (its start ${apart(there, stood).toFixed(0)} m from the first's), and left`);

  await choose("Worlds");
  // The last played is first; the first made is the second.
  if (listed[1].name !== first.playing) throw new Error(`the worlds are listed ${listed.map((world) => world.name).join(", ")}: the last played must come first`);
  await choose("World(1)");
  await until("the first world never loaded again", () => title().screen === "Playing" && title().playing === first.playing && feet() !== null);
  const back = title();
  if (back.seed !== first.seed) throw new Error(`${first.playing} was made from seed ${first.seed} and loads on ${back.seed}`);
  const again = feet()!;
  if (apart(again, walked) > 0.5) throw new Error(`${first.playing}'s player was left at ${walked} and is back at ${again}: ${apart(again, walked).toFixed(2)} m off, so its save was not kept or not read`);
  if (pid() !== began) throw new Error("the game was started again: all of this must be one process");
  console.log(`✓ loaded ${first.playing} again on seed ${back.seed}, its player ${apart(again, walked).toFixed(2)} m from where it was left; one process throughout`);
} catch (problem) {
  failed = String(problem instanceof Error ? problem.message : problem);
} finally {
  try {
    remote("stop");
  } catch {}
  rmSync(worlds, { recursive: true, force: true });
}
if (failed) {
  console.error(`✗ title: ${failed}`);
  process.exit(1);
}
console.log("✓ title: two worlds from two seeds and back to the first, in one process");
