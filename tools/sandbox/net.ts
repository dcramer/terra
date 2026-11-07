#!/usr/bin/env node
// Two copies of the game, one hosting and one joined (docs/netcode.md step 4, D638): the
// host's world walks the joined player from its intents, the joined game predicts its own
// player and draws the host. Checked with no picture, by what each game says (`net.state`):
// each holds the other, the joined player walks, and where it stands is the same in both
// games to a centimetre. Says how often the joined game's prediction was corrected: a
// reading, since on a loaded machine frames carry several ticks and intents come late.
//
//   node tools/sandbox/net.ts [--shots DIR] [--walk FRAMES]
//
// --shots keeps a still from each game of the other player. Node 22.18 or later; no
// dependencies. Both copies run in real time: two games' ticks must pass at one rate.

import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync } from "node:fs";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "../..");
const args = process.argv.slice(2);
const option = (flag: string) => (args.includes(flag) ? args[args.indexOf(flag) + 1] : undefined);
const SHOTS = option("--shots");
const WALK = Number(option("--walk") ?? 120);
// A port of this run's own, so two checkouts' runs never meet.
const GAME_PORT = 47000 + (process.pid % 2000);
const SAVES = ["shots/net-host.ron", "shots/net-join.ron"];

type Place = [number, number, number];
type State = { mine: Place | null; guests: { at: Place }[]; puppets: { at: Place }[]; corrections: number | null; clients: number | null; tick: number | null };

/// One `remote.ts` command to a copy (by its remote port; none: the one last started).
function remote(port: number | undefined, ...words: string[]): string {
  const env = { ...process.env };
  if (port) env.TERRA_REMOTE_PORT = String(port);
  else delete env.TERRA_REMOTE_PORT;
  const ran = spawnSync("node", [resolve(ROOT, "tools/sandbox/remote.ts"), ...words], { cwd: ROOT, env, encoding: "utf8" });
  if (ran.status !== 0) throw new Error(`remote ${words.join(" ")} failed:\n${ran.stdout}${ran.stderr}`);
  return ran.stdout;
}

/// Starts a copy and gives the port its remote answers on.
function start(save: string, more: string[]): number {
  remote(undefined, "start", "--real-time", "--", "--experiment", "play", "--fresh", "--save", save, "--offscreen", ...more);
  return Number(readFileSync(resolve(ROOT, "shots/remote.port"), "utf8").split(" ")[0]);
}

const state = (port: number) => JSON.parse(remote(port, "call", "net.state")) as State;
const apart = (a: Place, b: Place) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
const sleep = (seconds: number) => new Promise((done) => setTimeout(done, seconds * 1000));

async function until(what: string, test: () => boolean, seconds = 40) {
  for (let waited = 0; waited < seconds; waited += 0.5) {
    if (test()) return;
    await sleep(0.5);
  }
  throw new Error(`${what}, after ${seconds} s. Read shots/remote.log (the copy last started) for what the game said.`);
}

/// A still of a place from four metres, at chest height above it.
function still(port: number, feet: Place, file: string) {
  const up = Math.hypot(...feet);
  const chest = feet.map((along) => (along + (0.9 * along) / up).toFixed(3)).join(",");
  remote(port, "look", chest, "--distance", "4");
  spawnSync("sleep", ["1"]);
  remote(port, "shot", file);
  remote(port, "release");
}

const ports: number[] = [];
let failed = "";
try {
  ports.push(start(SAVES[0], []));
  remote(ports[0], "call", "net.host", JSON.stringify({ port: GAME_PORT }));
  ports.push(start(SAVES[1], ["--multirun"]));
  const [host, joiner] = ports;
  remote(joiner, "call", "net.join", JSON.stringify({ address: `127.0.0.1:${GAME_PORT}`, name: "friend" }));
  await until("the joined game never held the host, or the host never had its guest", () => state(joiner).puppets.length === 1 && state(host).guests.length === 1);
  console.log("✓ joined: the host has a guest and the joined game holds the host");

  const began = state(joiner).mine!;
  remote(joiner, "press", `w:${WALK}`);
  let last = began;
  await sleep(1);
  await until("the joined player never came to rest", () => {
    const now = state(joiner).mine!;
    const still = apart(now, last) < 1e-4;
    last = now;
    return still && apart(now, began) > 0.5;
  });
  await sleep(1);
  const [theirs, ours] = [state(host), state(joiner)];
  const walked = apart(ours.mine!, began);
  const between = apart(ours.mine!, theirs.guests[0].at);
  const puppet = apart(ours.puppets[0].at, theirs.mine!);
  if (walked < 3) failed = `the joined player walked ${walked.toFixed(2)} m on ${WALK} frames of forward: its intents are not reaching the host's walk (terra_walker::net's hear)`;
  else if (between > 0.01) failed = `the joined game has its player ${between.toFixed(3)} m from where the host has it: the prediction is not being held to the host's answers (net::link's listen)`;
  else if (puppet > 0.01) failed = `the joined game draws the host ${puppet.toFixed(3)} m from where the host stands (net::link's show)`;
  else console.log(`✓ walked ${walked.toFixed(1)} m: both games have the joined player in one place (${(between * 1000).toFixed(1)} mm apart) and the host drawn where it stands (${(puppet * 1000).toFixed(1)} mm); ${ours.corrections} corrections (a reading)`);

  if (SHOTS && !failed) {
    mkdirSync(SHOTS, { recursive: true });
    still(host, theirs.guests[0].at, resolve(SHOTS, "host-sees-guest.png"));
    still(joiner, theirs.mine!, resolve(SHOTS, "joiner-sees-host.png"));
    console.log(`✓ stills of each game's other player in ${SHOTS}`);
  }
} catch (error) {
  failed = (error as Error).message;
} finally {
  for (const port of ports.reverse()) {
    try {
      remote(port, "stop");
    } catch {
      // Already gone.
    }
  }
  for (const save of SAVES) rmSync(resolve(ROOT, save), { force: true });
}
if (failed) {
  console.error(`✗ net: ${failed}`);
  process.exit(1);
}
