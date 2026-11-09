#!/usr/bin/env node
// A crossing between two realms of one host, over the network (D640, docs/instances.md): the
// instance host runs with no window (terra_walker's nethost example) holding two realms of
// different seeds; a copy of the game joins it from its title screen and plays the first;
// the host hands it on to the second, and the game makes that land under its loading screen
// and plays it, standing where the second realm has it, with no body left in the first.
// Checked with no picture, by what the game (`title.state`, `net.state`) and the host say.
//
//   node tools/sandbox/crossing.ts
//
// Node 22.18 or later; no dependencies.

import { mkdtempSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";


const ROOT = resolve(import.meta.dirname, "../..");
const PORT = 47000 + (process.pid % 2000);
const SEED = 7;
/// How near the game must stand to where a realm has it, m. Not the centimetre of two games
/// (join.ts): a realm here has none of the solids the game stands among (trees, props, its
/// own start's things, D640), and the game is pushed off by them each tick; the offset is
/// said as a reading. A game not held to the realm's place at all stands 2 m off, at its
/// own start.
const NEAR_M = 0.5;

type Place = [number, number, number];
type Title = { screen: string; seed: string | null; picked: string | null; notice: string | null };
type Net = { mine: Place | null; joined: boolean };
type Where = { where: Place[][] };

function remote(...words: string[]): string {
  const ran = spawnSync("node", [resolve(ROOT, "tools/sandbox/remote.ts"), ...words], { cwd: ROOT, encoding: "utf8" });
  if (ran.status !== 0) throw new Error(`remote ${words.join(" ")} failed:\n${ran.stdout}${ran.stderr}`);
  return ran.stdout;
}
const title = () => JSON.parse(remote("call", "title.state")) as Title;
const net = () => JSON.parse(remote("call", "net.state")) as Net;
const sleep = (seconds: number) => new Promise((done) => setTimeout(done, seconds * 1000));
async function until(what: () => string, test: () => boolean, seconds: number) {
  for (let waited = 0; waited < seconds; waited += 1) {
    if (test()) return;
    await sleep(1);
  }
  throw new Error(`${what()}, after ${seconds} s. Read shots/remote.log for what the game said.`);
}
async function pick(choice: string, enter = true) {
  for (let tries = 0; tries < 12 && title().picked !== choice; tries++) {
    remote("press", "down");
    await sleep(0.3);
  }
  if (title().picked !== choice) throw new Error(`the title screen never offered ${choice}: it shows ${JSON.stringify(title())}`);
  if (enter) remote("press", "enter");
  await sleep(0.3);
}

let failed = "";
const built = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_walker", "--example", "nethost"], { cwd: ROOT, encoding: "utf8" });
if (built.status !== 0) {
  console.error(`✗ crossing: the host did not build:\n${built.stderr}`);
  process.exit(1);
}
const host = spawn(resolve(ROOT, "target/release/examples/nethost"), [String(PORT), String(SEED), "2000", "3600", "2"], { cwd: ROOT });
let [out, err] = ["", ""];
host.stdout.on("data", (chunk) => (out += chunk));
host.stderr.on("data", (chunk) => (err += chunk));
let started = false;
try {
  await until(() => `the host never listened: ${err || "it said nothing"}`, () => out.includes("listening"), 120);
  // Its own folder for its worlds and its player, new each run: who the player is, and the
  // address they last joined, are kept there, and one left by another run was in the address
  // field when this typed its own ("127.0.0.1:48283127.0.0.1:47305", refused as no address).
  const worlds = mkdtempSync(join(tmpdir(), "terra-crossing-"));
  remote("start", "--real-time", "--", "--experiment", "play", "--offscreen", "--title", `--worlds=${worlds}`);
  started = true;
  await until(() => "the game's title screen never came up", () => title().screen === "Title", 240);
  await pick("Join");
  await pick("Address", false);
  remote("call", "title.type", JSON.stringify({ text: `127.0.0.1:${PORT}` }));
  await pick("Connect");
  /// Waits until the game plays a seed's land, joined and standing, then says how far it
  /// stands from the one body the realm of that number has, the other realm having none.
  const stands = async (seed: number, realm: number): Promise<number> => {
    await until(() => `the game never played the realm of seed ${seed}: ${JSON.stringify(title())}`, () => {
      const now = title();
      return now.screen === "Playing" && now.seed === String(seed) && net().joined && net().mine !== null;
    }, 600);
    await sleep(5);
    const mine = net().mine!;
    const lines = out.split("\n").length;
    host.stdin.write("where\n");
    await until(() => "the host never said where its guests are", () => out.split("\n").length > lines, 20);
    const said = (JSON.parse(out.split("\n")[lines - 1]) as Where).where;
    if (said[realm].length !== 1 || said[1 - realm].length !== 0) throw new Error(`playing seed ${seed}, the host's realms have ${said.map((guests) => guests.length)} bodies, not one in realm ${realm} alone`);
    const [there] = said[realm];
    const off = Math.hypot(mine[0] - there[0], mine[1] - there[1], mine[2] - there[2]);
    if (off > NEAR_M) throw new Error(`playing seed ${seed}, the game stands ${off.toFixed(2)} m from where its realm has it (the game at ${mine.map((x) => x.toFixed(2))}, the realm at ${there.map((x) => x.toFixed(2))})`);
    if (off > 0.05) {
      // More than two games differ by: say what the game has standing where the realm put
      // its body (2 m east of the start), which the realm, with no solids, does not (D640).
      type Near = { at: [number, number]; gap: number | null; apart: number; top: number | null }[];
      const near = JSON.parse(remote("call", "world.obstacles", JSON.stringify({ at: [2, 0], within: 1.5 }))) as Near;
      const said = near.slice(0, 3).map((thing) => `one ${(thing.gap ?? thing.apart).toFixed(2)} m off, ${thing.top === null ? "low" : `${thing.top.toFixed(1)} m tall`}`);
      console.log(`  seed ${seed}: ${(off * 100).toFixed(0)} cm from the realm's place; trees or rocks the game has within 1.5 m of it: ${near.length}${said.length ? ` (${said.join("; ")})` : ""}`);
    }
    return off;
  };
  const first = await stands(SEED, 0);
  host.stdin.write("hand\n");
  const second = await stands(SEED + 1, 1);
  console.log(`✓ crossing: joined to a host of two realms, the game played seed ${SEED} (${(first * 100).toFixed(0)} cm from where that realm has it), was handed on, made the land of seed ${SEED + 1} and plays it (${(second * 100).toFixed(0)} cm), one body in its realm and none in the other each time`);
} catch (problem) {
  failed = String(problem instanceof Error ? problem.message : problem);
} finally {
  host.kill();
  if (started) {
    try {
      remote("stop");
    } catch {}
  }
}
if (failed) {
  console.error(`✗ crossing: ${failed}`);
  process.exit(1);
}
