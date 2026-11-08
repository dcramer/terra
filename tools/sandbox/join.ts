#!/usr/bin/env node
// Host and Join from the title screen (docs/v0-tracks.md, Front; docs/netcode.md): two
// copies of the game, each on its title screen, driven by its keys. One makes a world and
// hosts it; the other, behind whose title screen is another land, joins by typing the
// host's address, is welcomed, makes the host's land and plays in it, each holding the
// other, and each looking in the other's game as its player made it; the world's gate is opened
// by the friend's mark, for both. The host then quits to its title screen: the joined game comes back to its own,
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
// The friend's own folder: who they are is kept there (player.ron), so a game started anew
// is the same player to the host.
const friendWorlds = mkdtempSync(join(tmpdir(), "terra-join-friend-"));
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
  let friend = start(["--multirun", `--worlds=${friendWorlds}`]);
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

  // Each makes its character its own, and is that person in the other's game (D111).
  type Looks = { hair: string | null; beard: string | null; skin: string; eyes: string } | null;
  const looks = (port: number, who: "guests" | "puppets") => (net(port)[who][0] as { looks?: Looks } | undefined)?.looks ?? null;
  remote(host, "call", "creator.set", JSON.stringify({ hair: "male_hair_dreads", skin: "Deep", beard: null }));
  remote(friend, "call", "creator.set", JSON.stringify({ hair: null, beard: "male_beard_full", eyes: "Blue" }));
  await until(`the friend's game never showed the host as it made itself: ${JSON.stringify(looks(friend, "puppets"))}`, () => {
    const seen = looks(friend, "puppets");
    return seen?.hair === "male_hair_dreads" && seen.skin === "Deep" && seen.beard === null;
  }, 60);
  await until(`the host's game never showed the friend as it made itself: ${JSON.stringify(looks(host, "guests"))}`, () => {
    const seen = looks(host, "guests");
    return seen?.hair === null && seen.beard === "male_beard_full" && seen.eyes === "Blue";
  }, 60);
  console.log("✓ each is the person its player made in the other's game: the host's dreads and deep skin on the friend's puppet, the friend's bare head, beard and blue eyes on the host's guest");

  // The world's gate (D590): shut in both games; the friend, given the mark, holds it to the
  // gate, the host's gate opens, and it is open in both; nobody crosses a shared world.
  const gate = (port: number) => JSON.parse(remote(port, "call", "portal.state")) as { open: boolean };
  if (gate(host).open || gate(friend).open) throw new Error("the world's gate stands open before anyone bears a mark");
  // The mark is earned (D590): the host puts the friend at the den's mouth and fells the
  // troll in it; the fall is ordered where it happened, and the friend by it bears the mark
  // in its own game, the host's own player, far off at the start, not.
  const den = (JSON.parse(remote(host, "call", "den.state")) as { den: { mouth: Place } | null }).den;
  if (!den) throw new Error("the host's world has no den to earn the mark at (cargo xtask worlds says why)");
  const bears = (port: number) => (JSON.parse(remote(port, "call", "marks.state")) as { marks: string[] }).marks;
  remote(host, "call", "net.place_guests", JSON.stringify({ at: [den.mouth[0], den.mouth[2]] }));
  await until("the friend's game never stood at the den's mouth, where the host put it", () => {
    const [mine, there] = [net(friend).mine, net(host).guests[0]?.at];
    return !!mine && !!there && apart(mine, there) < 0.5;
  }, 30);
  remote(host, "call", "creatures.strike", JSON.stringify({ name: "Troll", damage: 5000 }));
  await until(`the troll fell with the friend at its den's mouth and the friend bears ${JSON.stringify(bears(friend))}, not its mark: the host orders a mark earned where the creature fell, and a joined game near imprints it (play::marks, MARKED)`, () => bears(friend).includes("troll"), 20);
  if (bears(host).includes("troll")) throw new Error("the host's player, hundreds of metres from the den, bears the troll's mark too");
  remote(friend, "call", "portal.step");
  await until("the host's gate never opened to the mark the friend bears", () => gate(host).open, 60);
  await until("the friend's game never showed the gate the host opened", () => gate(friend).open, 60);
  // Open, and stepped at by both: nobody crosses a shared world.
  remote(host, "call", "portal.step");
  remote(friend, "call", "portal.step");
  await sleep(3);
  if (title(host).seed !== land || title(friend).seed !== land) throw new Error(`a player crossed out of a shared world: the host on seed ${title(host).seed}, the friend on ${title(friend).seed}`);
  console.log("✓ the gate: shut for both, opened on the host by the mark the friend bears, open in both, and nobody crossed the shared world");

  // Creatures are the host's (netcode.md, step 5): the friend's game puts out none of its
  // own and shows the host's, each where the host has it.
  type Animal = { name: string; at: Place };
  const KINDS = ["Wild boar", "Red squirrel", "Chaffinch", "Brown hare", "Red fox", "Roe deer", "Spider"];
  const animals = (port: number) => KINDS.flatMap((name) => (JSON.parse(remote(port, "call", "sandbox.find", JSON.stringify({ name }))) as Animal[]).map((found) => ({ name, at: found.at })));
  await until("the host's world never had an animal of the kinds looked for near its players", () => animals(host).length > 0, 60);
  let seen = 0;
  let worst = 0;
  await until(`the friend's game never showed the host's animals where the host has them: the host has ${animals(host).length}, the friend shows ${animals(friend).length}`, () => {
    const [theirs, shown] = [animals(host), animals(friend)];
    if (shown.length === 0) return false;
    // Each one shown is one the host has, of its kind, near where the host has it (it is
    // shown a few sends back, and a running animal covers some metres in that).
    const off = shown.map((one) => Math.min(...theirs.filter((other) => other.name === one.name).map((other) => apart(one.at, other.at)), Infinity));
    [seen, worst] = [shown.length, Math.max(...off)];
    return worst < 4;
  }, 60);
  console.log(`✓ creatures are the host's: the friend's game shows ${seen} of the host's animals, each within ${worst.toFixed(1)} m of where the host has it`);

  // A creature's blow on a joined player is the host's to deal and the guest's game's to
  // take: the friend, put by the host among its spiders, loses health in its own game.
  const health = (port: number) => (JSON.parse(remote(port, "call", "play.state")) as { health: number }).health;
  const spiders = JSON.parse(remote(host, "call", "sandbox.find", JSON.stringify({ name: "Spider" }))) as { from_start: Place }[];
  if (spiders.length === 0) throw new Error("the host's world has no spiders to be bitten by: data/creatures.ron's Spider lives at a lair, which every played world has");
  const hale = health(friend);
  remote(host, "call", "net.place_guests", JSON.stringify({ at: [spiders[0].from_start[0] + 1.5, spiders[0].from_start[2]] }));
  await until(`the friend, put among the host's spiders, still has ${health(friend)} of ${hale} health: a creature's blow on a guest is ordered where the guest stands and the guest's own game takes it (terra_creatures::net, HURT)`, () => health(friend) < hale, 40);
  const bitten = hale - health(friend);
  console.log(`✓ a creature's blow reaches a joined player: among the host's spiders the friend lost ${bitten.toFixed(0)} health in its own game`);

  // A joined player's blow is the host's to land: the friend, put by the host beside one of
  // its animals (a guest's body is the host's to walk), strikes it, and it falls on the host.
  type From = { name: string; from_start: Place };
  const kind = KINDS.find((name) => (JSON.parse(remote(host, "call", "sandbox.find", JSON.stringify({ name }))) as From[]).length > 0)!;
  const prey = (JSON.parse(remote(host, "call", "sandbox.find", JSON.stringify({ name: kind }))) as From[])[0].from_start;
  remote(host, "call", "net.place_guests", JSON.stringify({ at: [prey[0] + 2, prey[2]] }));
  await until(`the friend's game never stood where the host put it, beside the ${kind}`, () => {
    const mine = net(friend).mine;
    const there = net(host).guests[0]?.at;
    return !!mine && !!there && apart(mine, there) < 0.5;
  }, 30);
  let fell = false;
  for (let tries = 0; tries < 6 && !fell; tries++) {
    remote(friend, "call", "creatures.strike", JSON.stringify({ name: kind, damage: 300 }));
    await sleep(1);
    fell = /Killed a /.test(remote(host, "said", "--all", "--containing", "Killed a "));
  }
  if (!fell) throw new Error(`the friend, put beside a ${kind} and striking it for 300, never felled it on the host: a joined player's blow is asked of the host, which lands it within reach (terra_creatures::net, STRIKE)`);
  console.log(`✓ a joined player's blow lands on the host: the friend, put beside a ${kind}, struck it and it fell there`);

  // The friend's character is sent to the host to keep (D592), as it is every few seconds.
  remote(friend, "call", "character.keep");
  await sleep(1);

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

  // The host keeps its friends' characters with its world (D592): the world hosted again
  // from its save, and the friend's game closed and started anew, with nothing of its own
  // but who it is, the friend is given the character it left: it bears the troll's mark.
  remote(friend, "stop");
  ports.splice(ports.indexOf(friend), 1);
  await pick(host, "Host");
  await pick(host, "HostWorld(0)");
  await until("the host never hosted its world again", () => title(host).screen === "Playing" && net(host).hosting);
  friend = start(["--multirun", `--worlds=${friendWorlds}`]);
  ports.push(friend);
  await until("the friend's title screen never came up again", () => title(friend).screen === "Title");
  await pick(friend, "Join");
  await pick(friend, "Connect");
  await until(`the friend never played the host's world again: its title screen says ${JSON.stringify(title(friend))}`, () => title(friend).screen === "Playing" && net(friend).joined, 300);
  const marks = () => (JSON.parse(remote(friend, "call", "marks.state")) as { marks: string[] }).marks;
  await until(`come back with a game started anew the friend bears ${JSON.stringify(marks())}, not the troll's mark it left with: the host keeps a guest's character with its world's save and gives it back in its greeting (play::save::guest, D592)`, () => marks().includes("troll"), 30);
  console.log("✓ the host kept the friend's character: its game closed and started anew and the world hosted again from its save, the friend bears the mark it left with");
} catch (problem) {
  failed = String(problem instanceof Error ? problem.message : problem);
} finally {
  for (const port of ports) {
    try {
      remote(port, "stop");
    } catch {}
  }
  rmSync(worlds, { recursive: true, force: true });
  rmSync(friendWorlds, { recursive: true, force: true });
}
if (failed) {
  console.error(`✗ join: ${failed}`);
  process.exit(1);
}
console.log("✓ join: hosted and joined from two title screens, and each end of a session told in words");
