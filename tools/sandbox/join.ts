#!/usr/bin/env node
// Host and Join from the title screen (docs/v0-tracks.md, Front; docs/netcode.md): two
// copies of the game, each on its title screen, driven by its keys. One makes a world and
// hosts it; the other, behind whose title screen is another land, joins by typing the
// host's address, is welcomed, makes the host's land and plays in it, each holding the
// other. The host then quits to its title screen: the joined game comes back to its own,
// told the host left; asking again finds nobody, and it is told that too. Checked with no
// picture, by what each game says (`title.state`, `net.state`).
//
//   node tools/sandbox/join.ts
//
// Node 22.18 or later; no dependencies. The host listens on the game's own port (46464):
// one run of this at a time on a machine.

import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "../..");

type Place = [number, number, number];
type Title = { screen: string; seed: string | null; picked: string | null; notice: string | null; joining: string | null; player: string | null };
type Net = { mine: Place | null; hosting: boolean; joined: boolean; guests: { at: Place }[]; puppets: { at: Place }[]; players: string[] | null };

/// One `remote.ts` command to a copy (by its remote port; none: the one last started).
function remote(port: number | undefined, ...words: string[]): string {
  const env = { ...process.env };
  if (port) env.TERRA_REMOTE_PORT = String(port);
  else delete env.TERRA_REMOTE_PORT;
  const ran = spawnSync("node", [resolve(ROOT, "tools/sandbox/remote.ts"), ...words], { cwd: ROOT, env, encoding: "utf8" });
  if (ran.status !== 0) throw new Error(`remote ${words.join(" ")} failed:\n${ran.stdout}${ran.stderr}`);
  return ran.stdout;
}

/// Starts a copy on its title screen; its remote port.
function start(more: string[]): number {
  remote(undefined, "start", "--real-time", "--", "--experiment", "play", "--offscreen", "--title", ...more);
  return Number(readFileSync(resolve(ROOT, "shots/remote.port"), "utf8").split(" ")[0]);
}

const title = (port: number) => JSON.parse(remote(port, "call", "title.state")) as Title;
const net = (port: number) => JSON.parse(remote(port, "call", "net.state")) as Net;
const sleep = (seconds: number) => new Promise((done) => setTimeout(done, seconds * 1000));
const apart = (a: Place, b: Place) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

async function until(what: string, test: () => boolean, seconds = 240) {
  for (let waited = 0; waited < seconds; waited += 0.5) {
    if (test()) return;
    await sleep(0.5);
  }
  throw new Error(`${what}, after ${seconds} s. Read shots/remote.log (the copy last started) for what the game said.`);
}

/// Moves a title screen's pick to the choice named, and chooses it if asked.
async function pick(port: number, choice: string, enter = true) {
  for (let tries = 0; tries < 12 && title(port).picked !== choice; tries++) {
    remote(port, "press", "down");
    await sleep(0.3);
  }
  if (title(port).picked !== choice) throw new Error(`a title screen never offered ${choice}: it shows ${JSON.stringify(title(port))}`);
  if (!enter) return;
  remote(port, "press", "enter");
  await sleep(0.3);
}

const worlds = mkdtempSync(join(tmpdir(), "terra-join-"));
const ports: number[] = [];
let failed = "";
try {
  // The host: a world made, left, and hosted from the title screen.
  const host = start([`--worlds=${worlds}`]);
  ports.push(host);
  await until("the host's title screen never came up", () => title(host).screen === "Title");
  await pick(host, "NewWorld");
  await pick(host, "Create");
  await until("the host's new world never began", () => title(host).screen === "Playing");
  remote(host, "call", "title.leave");
  await until("the host never came back to its title screen", () => title(host).screen === "Title");
  await pick(host, "Host");
  await pick(host, "HostWorld(0)");
  await until("the host never began hosting the world it chose", () => title(host).screen === "Playing" && net(host).hosting);
  const land = title(host).seed;
  console.log(`✓ hosting from the title screen: a world of seed ${land}`);

  // The friend: another land behind its title screen, the host's address typed.
  const friend = start(["--multirun"]);
  ports.push(friend);
  await until("the friend's title screen never came up", () => title(friend).screen === "Title");
  const behind = title(friend).seed;
  if (behind === land) throw new Error(`the land behind the friend's title screen is already the host's (seed ${land}): this run would not show the host's land being made`);
  await pick(friend, "Join");
  await pick(friend, "Player", false);
  remote(friend, "call", "title.type", JSON.stringify({ text: " the friend" }));
  await pick(friend, "Address", false);
  remote(friend, "call", "title.type", JSON.stringify({ text: "127.0.0.1" }));
  await pick(friend, "Connect");
  await until(`the friend never played the host's world: its title screen says ${JSON.stringify(title(friend))}`, () => title(friend).screen === "Playing" && net(friend).joined, 300);
  if (title(friend).seed !== land) throw new Error(`the friend plays on seed ${title(friend).seed}; the host's land is seed ${land}`);
  await until("the two games never held each other", () => net(friend).puppets.length === 1 && net(host).guests.length === 1 && net(friend).mine !== null);
  // Where the friend stands is where the host has it.
  let off = Infinity;
  await until("the friend's player never stood where the host has it, to 5 cm", () => (off = apart(net(friend).mine!, net(host).guests[0].at)) < 0.05, 60);
  const names = net(host).players ?? [];
  if (!names.some((name) => name.includes("the friend"))) throw new Error(`the host lists ${names.join(", ")}: the friend's typed name is not there`);
  console.log(`✓ joined from the title screen: welcomed to seed ${land} from seed ${behind}, its land made; each holds the other, the friend ${(off * 100).toFixed(1)} cm from where the host has it; players ${names.join(", ")}`);

  // The host quits to its title screen: the friend is brought back to its own, told why.
  remote(host, "call", "title.leave");
  await until(`the friend never came back to its title screen: ${JSON.stringify(title(friend))}`, () => title(friend).screen === "Title", 300);
  const told = title(friend).notice ?? "";
  if (!/host left/i.test(told)) throw new Error(`back on its title screen the friend is told "${told}", not that the host left`);
  console.log(`✓ the host left: the friend is back on its title screen, told "${told}"`);

  // Nobody hosts there now: asking again says so.
  await pick(friend, "Join");
  await pick(friend, "Connect");
  await until(`asking an address nobody hosts at never said so: ${JSON.stringify(title(friend))}`, () => /no answer/i.test(title(friend).notice ?? ""), 90);
  console.log(`✓ nobody hosting: the friend is told "${title(friend).notice}"`);
} catch (problem) {
  failed = String(problem instanceof Error ? problem.message : problem);
} finally {
  for (const port of ports) {
    try {
      remote(port, "stop");
    } catch {}
  }
  rmSync(worlds, { recursive: true, force: true });
}
if (failed) {
  console.error(`✗ join: ${failed}`);
  process.exit(1);
}
console.log("✓ join: hosted and joined from two title screens, and each end of a session told in words");
