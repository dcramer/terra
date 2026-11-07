#!/usr/bin/env node
// Two copies of the game, one hosting and one joined (docs/netcode.md step 4, D638): the
// host's world walks the joined player from its intents, the joined game predicts its own
// player and draws the host. Checked with no picture, by what each game says (`net.state`):
// each holds the other, the joined player walks, and where it stands is the same in both
// games to a centimetre; the joined player digs and the ground is the same in both; then
// each lists the other, and the host leaving is told to the
// joined game in words. Says how often the joined game's prediction was corrected: a
// reading, since on a loaded machine frames carry several ticks and intents come late.
//
//   node tools/sandbox/net.ts [--shots DIR] [--walk FRAMES]
//   node tools/sandbox/net.ts --soak MINUTES [--bots 4]
//
// --soak is the Server track's bar against a running game (docs/v0-tracks.md): one copy
// hosting and so many players with no game behind them (terra_walker's netbot example, D73:
// a machine runs two copies of the game at most), each over UDP made 100 ms there and back
// with 1% lost, walking by a script for so many minutes. Each must end where the host has
// it, to a centimetre, never having gone a second without word; the bytes a second a player
// are said.
//
// --shots keeps a still from each game of the other player. Node 22.18 or later; no
// dependencies. Both copies run in real time: two games' ticks must pass at one rate.

import { spawn, spawnSync } from "node:child_process";
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
type State = { mine: Place | null; guests: { at: Place }[]; puppets: { at: Place }[]; corrections: number | null; clients: number | null; tick: number | null; players: string[] | null; ended: string | null; ground: number | null; ground_pending: number | null };

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
/// A copy's frames and seconds so far, for its frame rate over a stretch.
const clock = (port: number) => JSON.parse(remote(port, "call", "sandbox.status")) as { frame: number; seconds: number };
/// Under this many frames a second a game cannot run its 64 ticks a second (six a frame at
/// most, D138), so two copies' ticks no longer pass at one rate and corrections say nothing.
const REAL_TIME_FPS = 64 / 6;
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

type Report = { name: string; at?: Place; closed?: string; answers?: number; longest_silence_ticks?: number; up_bytes_s?: number; down_bytes_s?: number };

/// A bot, started: its report when it gives it, and its leaving.
function bot(name: string, minutes: number): { report: Promise<Report>; left: Promise<number | null> } {
  const run = spawn(resolve(ROOT, "target/debug/examples/netbot"), [`127.0.0.1:${GAME_PORT}`, name, String(minutes), "50", "0.01"], { cwd: ROOT });
  let out = "";
  const report = new Promise<Report>((done, fail) => {
    run.stdout.on("data", (chunk) => {
      out += chunk;
      const line = out.split("\n").find((line) => line.startsWith("{"));
      if (line) done(JSON.parse(line) as Report);
    });
    run.on("exit", () => fail(new Error(`${name} left without saying where it was`)));
  });
  const left = new Promise<number | null>((done) => run.on("exit", done));
  return { report, left };
}

/// The soak: see the head of this file.
async function soak(minutes: number, bots: number): Promise<string> {
  const built = spawnSync("cargo", ["build", "-q", "-p", "terra_walker", "--example", "netbot"], { cwd: ROOT, encoding: "utf8" });
  if (built.status !== 0) return `the bot did not build:\n${built.stderr}`;
  const host = start(SAVES[0], []);
  ports.push(host);
  remote(host, "call", "net.host", JSON.stringify({ port: GAME_PORT }));
  const before = clock(host);
  const players = Array.from({ length: bots }, (_, index) => bot(`bot${index + 1}`, minutes));
  console.log(`… ${bots} bots walking for ${minutes} minutes at 100 ms and 1% loss`);
  const reports = await Promise.all(players.map((player) => player.report));
  // They stand now, three seconds from leaving: where the host has each.
  const theirs = state(host);
  const now = clock(host);
  const fps = (now.frame - before.frame) / (now.seconds - before.seconds);
  const closed = reports.find((report) => report.closed);
  if (closed) return `the host ended ${closed.name}'s connection: "${closed.closed}"`;
  if (theirs.guests.length !== bots) return `the host has ${theirs.guests.length} guests of ${bots} at the end`;
  const off = reports.map((report) => Math.min(...theirs.guests.map((guest) => apart(guest.at, report.at!))));
  const silence = Math.max(...reports.map((report) => report.longest_silence_ticks!));
  const mean = (pick: (report: Report) => number) => reports.reduce((sum, report) => sum + pick(report), 0) / bots;
  const worst = Math.max(...off);
  if (worst > 0.01) return `a bot ends ${worst.toFixed(3)} m from where the host has it: a lasting mismatch`;
  if (silence > 64) return `a bot went ${silence} ticks without word of the server: over a second`;
  await Promise.all(players.map((player) => player.left));
  await until("the host still has guests after every bot left", () => state(host).guests.length === 0);
  console.log(
    `✓ soak: ${bots} players for ${minutes} minutes at 100 ms and 1% loss each end where the host has them (${(worst * 1000).toFixed(1)} mm at worst), the longest silence ${silence} ticks; ${mean((r) => r.down_bytes_s!).toFixed(0)} B/s down and ${mean((r) => r.up_bytes_s!).toFixed(0)} B/s up a player (the carrier's count); the host at ${fps.toFixed(0)} fps`,
  );
  return "";
}

const ports: number[] = [];
let failed = "";
const SOAK = option("--soak");
try {
  if (SOAK) {
    failed = await soak(Number(SOAK), Number(option("--bots") ?? 4));
    throw new Error(failed || "done");
  }
  ports.push(start(SAVES[0], []));
  remote(ports[0], "call", "net.host", JSON.stringify({ port: GAME_PORT }));
  ports.push(start(SAVES[1], ["--multirun"]));
  const [host, joiner] = ports;
  remote(joiner, "call", "net.join", JSON.stringify({ address: `127.0.0.1:${GAME_PORT}`, name: "friend" }));
  await until("the joined game never held the host, or the host never had its guest", () => state(joiner).puppets.length === 1 && state(host).guests.length === 1);
  console.log("✓ joined: the host has a guest and the joined game holds the host");

  const began = state(joiner).mine!;
  const before = ports.map(clock);
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
  const fps = ports.map(clock).map((now, index) => (now.frame - before[index].frame) / (now.seconds - before[index].seconds));
  const slow = Math.min(...fps) < REAL_TIME_FPS * 1.5;
  const pace = `host ${fps[0].toFixed(0)} fps, joined ${fps[1].toFixed(0)} fps${slow ? ": too few for real time on this machine now, so the corrections say nothing" : ""}`;
  const walked = apart(ours.mine!, began);
  const between = apart(ours.mine!, theirs.guests[0].at);
  const puppet = apart(ours.puppets[0].at, theirs.mine!);
  if (walked < 3) failed = `the joined player walked ${walked.toFixed(2)} m on ${WALK} frames of forward: its intents are not reaching the host's walk (terra_walker::net's hear)`;
  else if (between > 0.01) failed = `the joined game has its player ${between.toFixed(3)} m from where the host has it: the prediction is not being held to the host's answers (net::link's listen)`;
  else if (puppet > 0.01) failed = `the joined game draws the host ${puppet.toFixed(3)} m from where the host stands (net::link's show)`;
  else console.log(`✓ walked ${walked.toFixed(1)} m: both games have the joined player in one place (${(between * 1000).toFixed(1)} mm apart) and the host drawn where it stands (${(puppet * 1000).toFixed(1)} mm); ${ours.corrections} corrections (a reading; ${pace})`);

  if (SHOTS && !failed) {
    mkdirSync(SHOTS, { recursive: true });
    still(host, theirs.guests[0].at, resolve(SHOTS, "host-sees-guest.png"));
    still(joiner, theirs.mine!, resolve(SHOTS, "joiner-sees-host.png"));
    console.log(`✓ stills of each game's other player in ${SHOTS}`);
  }
  if (!failed) {
    // The joined player digs with its pickaxe, a step east of where it stands: the ground's
    // change is asked of the host, made there, and ordered back (D21).
    const stood = (JSON.parse(remote(joiner, "state")) as { at: [number, number] }).at;
    remote(joiner, "press", "3");
    remote(joiner, "aim", `${stood[0] + 1.5},${stood[1]}`, "--up", "0");
    remote(joiner, "press", "e:240");
    await until("the joined player's dig never reached the host's ground, or never came back", () => {
      const [theirs, ours] = [state(host), state(joiner)];
      return (theirs.ground ?? 0) > 0 && theirs.ground === ours.ground && ours.ground_pending === 0;
    });
    console.log(`✓ dug: ${state(host).ground} operations shape the ground in both games, none of the joined game's waiting on the host`);
  }

  if (!failed) {
    // Who is here, a word each way, and the host leaving.
    const here = [state(host).players, state(joiner).players].map((players) => (players ?? []).join(","));
    remote(joiner, "call", "net.say", JSON.stringify({ text: "is anyone there?" }));
    remote(host, "call", "net.say", JSON.stringify({ text: "i am" }));
    await sleep(1);
    remote(host, "call", "net.leave");
    await until("the joined game was never told the host left", () => state(joiner).ended !== null);
    const ended = state(joiner).ended!;
    if (here[0] !== "host,friend" || here[1] !== "host") failed = `the host lists "${here[0]}" and the joined game "${here[1]}", not host,friend and host (terra_net's roster)`;
    else if (!ended.includes("host left")) failed = `the joined game was told "${ended}" when the host left, not that the host left`;
    else console.log(`✓ each lists the other, words went both ways, and when the host left the joined game was told: "${ended}"`);
  }
} catch (error) {
  if (!SOAK) failed = (error as Error).message;
  else if (!failed && (error as Error).message !== "done") failed = (error as Error).message;
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
