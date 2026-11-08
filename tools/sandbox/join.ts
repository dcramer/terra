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

/// The kinds of animal looked for in a world.
const KINDS = ["Wild boar", "Red squirrel", "Chaffinch", "Brown hare", "Red fox", "Roe deer", "Spider"];
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
  // A reading: the animals the host has put out within 80 m of the friend, far from the host's own player.
  const guestAt = net(host).guests[0].at;
  const hostAt = net(host).mine!;
  const byFriend = KINDS.flatMap((name) => JSON.parse(remote(host, "call", "sandbox.find", JSON.stringify({ name }))) as { at: Place }[]).filter((one) => apart(one.at, guestAt) < 80).length;
  console.log(`  (a reading: ${apart(guestAt, hostAt).toFixed(0)} m from the host's player, the host has ${byFriend} animals within 80 m of the friend)`);
  remote(friend, "call", "portal.step");
  await until("the host's gate never opened to the mark the friend bears", () => gate(host).open, 60);
  await until("the friend's game never showed the gate the host opened", () => gate(friend).open, 60);
  // Open, and stepped at by both: nobody crosses a shared world.
  remote(host, "call", "portal.step");
  remote(friend, "call", "portal.step");
  await sleep(3);
  if (title(host).seed !== land || title(friend).seed !== land) throw new Error(`a player crossed out of a shared world: the host on seed ${title(host).seed}, the friend on ${title(friend).seed}`);
  console.log("✓ the gate: shut for both, opened on the host by the mark the friend bears, open in both, and nobody crossed the shared world");

  // What each holds in hand is shown in the other's game (netcode.md, step 5): the friend
  // takes a torch in hand, and the host's guest holds a torch; the friend's puppet of the
  // host holds what the host does.
  type Hands = { mine: { main: string | null } | null; others: { main: string | null }[] };
  const hands = (port: number) => JSON.parse(remote(port, "call", "hands.state")) as Hands;
  // Side by side again first: the mark was earned at the den, far from the host's player.
  const beside = (JSON.parse(remote(host, "call", "play.state")) as { at: [number, number] }).at;
  remote(host, "call", "net.place_guests", JSON.stringify({ at: [beside[0] + 2, beside[1]] }));
  await until("put beside the host again, the friend's game never showed the host", () => net(friend).puppets.length === 1, 60);
  remote(friend, "give", "torch");
  await sleep(1);
  const bar = (JSON.parse(remote(friend, "call", "play.state")) as { bar: (string | null)[] }).bar;
  const slot = bar.findIndex((name) => name === "Torch");
  if (slot < 0) throw new Error(`given a torch, the friend's bar holds ${JSON.stringify(bar)}`);
  remote(friend, "press", String(slot + 1));
  await until(`the friend took a torch in hand and the host's game shows its guest holding ${JSON.stringify(hands(host).others)}: what a joined player holds is asked of the host, a part of its entity (terra_survival::hands_net)`, () => hands(host).others[0]?.main === "Torch", 30);
  const held = hands(host).mine?.main ?? null;
  await until(`the host holds ${held} and the friend's game says ${JSON.stringify(hands(friend))} and has ${net(friend).puppets.length} puppets`, () => (hands(friend).others[0]?.main ?? null) === held, 30);
  console.log(`✓ what each holds is shown in the other's game: the friend's torch on the host's guest, the host's ${held ?? "empty hand"} on the friend's puppet`);

  // A tree felled is felled for both (netcode.md, step 5): the host fells the trees round
  // where they stand, and in the friend's game they are gone: asked to fell them there, it
  // finds none.
  type Felled = { felled: number };
  // First the friend's own felling, of what stands within its reach: asked of the host,
  // those trees are gone there too.
  const near = JSON.stringify({ at: [beside[0] + 2, beside[1]], within: 7 });
  const own = (JSON.parse(remote(friend, "call", "world.fell", near)) as Felled).felled;
  await sleep(4);
  const stood = (JSON.parse(remote(host, "call", "world.fell", near)) as Felled).felled;
  if (own > 0 && stood > 0) throw new Error(`the friend felled ${own} trees within its reach and ${stood} still stood on the host: a joined game asks the host to fell what it fells (terra_walker::net::woods); a refusal for reach is in the host's log`);
  const round = JSON.stringify({ at: beside, within: 60 });
  const cut = (JSON.parse(remote(host, "call", "world.fell", round)) as Felled).felled;
  if (cut === 0) throw new Error(`no tree stands within 60 m of the host at ${beside.map((x) => x.toFixed(0)).join(", ")} to fell: the check needs woods round the players`);
  await sleep(4);
  const left = (JSON.parse(remote(friend, "call", "world.fell", round)) as Felled).felled;
  if (left > 0) throw new Error(`the host felled ${cut} trees round the players and ${left} of them still stood in the friend's game: the host orders each felled tree where it stood, and a joined game fells it again (terra_walker::net::woods)`);
  console.log(`✓ a tree felled is felled for both: the host felled ${cut} round the players, and none of them stood in the friend's game; ${own > 0 ? `the friend felled ${own} within its reach first, and none of those stood on the host` : "no tree stood within the friend's reach to fell first"}`);

  // The base is the same in both games (netcode.md, step 5): the host sets down a claim
  // stone and lays a room's plan; the friend's game has the same parts of as many cubes.
  type Base = { parts: { kind: string; cubes: number }[] };
  const base = (port: number) => (JSON.parse(remote(port, "call", "build.state")) as Base).parts.map((part) => `${part.kind}:${part.cubes}`).join(" ");
  remote(host, "give", "stone:4");
  // By the hammer's menu, as a player does; where the host happens to aim on a world of
  // any seed, the stone is sometimes refused, so it is tried again a pace on.
  let claimed = false;
  for (let tries = 0; tries < 3 && !claimed; tries++) {
    remote(host, "press", "b@20", "2@30", "enter@40", "e@70:80");
    try {
      remote(host, "wait", "Placed a Claim stone", "--timeout", "20");
      claimed = true;
    } catch {
      remote(host, "press", "esc", "w:40");
      await sleep(2);
    }
  }
  if (!claimed) throw new Error("the host could not set down a claim stone in three tries (the hammer's menu: b, 2, enter, e held): what it was told is in its log");
  // What stands in a base stands in both games: the host's claim stone is in the friend's.
  const stones = (port: number) => (JSON.parse(remote(port, "call", "sandbox.find", JSON.stringify({ name: "Claim stone" }))) as { at: Place }[]).map((stone) => stone.at);
  const theirs = stones(host);
  if (theirs.length !== 1) throw new Error(`the host set down a claim stone and has ${theirs.length} of them`);
  await until(`the host's claim stone stands at ${theirs[0].map((x) => x.toFixed(1)).join(", ")} and the friend's game has ${JSON.stringify(stones(friend))}: the host orders each prop where it stands and a joined game sets it down (play::props_net)`, () => stones(friend).length === 1 && apart(stones(friend)[0], theirs[0]) < 0.3, 30);
  const laid = JSON.parse(remote(host, "call", "build.lay", JSON.stringify({ what: "room", min: [0, 0, 0], max: [4, 0, 4], kind: "logs", floor: "stone" }))) as { done: boolean; refused?: string };
  if (!laid.done) throw new Error(`the host's room was refused: ${laid.refused}`);
  const built = base(host);
  if (!built) throw new Error("the host laid a room and its base has no parts");
  await until(`the host's base is ${built} and the friend's game has ${base(friend) || "none of it"}: the host orders every act done to the base where it stands, and a joined game makes them (play::base_net); the host said: ${remote(host, "said", "--all", "--containing", "Base:").trim().split("\n").slice(-3).join(" / ")}`, () => base(friend) === built, 30);
  console.log(`✓ the base is the same in both games: the host's room (${built}) stands in the friend's game too, and its claim stone beside it`);

  // What storage holds is the same in both games (netcode.md, step 5): the host sets down a
  // chest and leaves stone in it, which the friend's game shows in it; the friend leaves wood
  // in it, and the host's chest holds both.
  type Stores = { stores: { item: string; holds: Record<string, number> }[] };
  const chest = (port: number) => {
    const { stores } = JSON.parse(remote(port, "call", "storage.state", JSON.stringify({ all: true }))) as Stores;
    return stores.length === 1 ? Object.entries(stores[0].holds).map(([item, count]) => `${count} ${item}`).sort().join(", ") || "nothing" : `${stores.length} storages`;
  };
  remote(host, "give", "wood:6");
  const set = JSON.parse(remote(host, "call", "storage.place", JSON.stringify({ item: "Chest", at: [6.5, 0, 2.5] }))) as { done: boolean; refused?: string };
  if (!set.done) throw new Error(`the host's chest was refused: ${set.refused}`);
  await until("the host set down a chest and the friend's game has none (play::props_net)", () => chest(friend) !== "0 storages", 60);
  const by = (JSON.parse(remote(host, "call", "storage.state", JSON.stringify({ all: true }))) as { stores: { from_start: Place }[] }).stores[0].from_start;
  remote(host, "call", "play.warp", JSON.stringify({ at: [by[0] + 1.2, by[2]] }));
  remote(host, "call", "net.place_guests", JSON.stringify({ at: [by[0] - 1.2, by[2]] }));
  const from = (port: number) => {
    const at = (JSON.parse(remote(port, "call", "play.state")) as { at: [number, number] }).at;
    return Math.hypot(at[0] - by[0], at[1] - by[2]);
  };
  await until(`both players are put beside the chest, and the host stands ${from(host).toFixed(1)} m from it and the friend ${from(friend).toFixed(1)} m`, () => from(host) < 3 && from(friend) < 3, 30);
  remote(host, "give", "stone:5");
  const stoned = JSON.parse(remote(host, "call", "storage.put", JSON.stringify({ item: "stone" }))) as { put: number };
  if (stoned.put < 5) throw new Error(`the host left ${stoned.put} stone in its chest, not the 5 given`);
  const hosts = chest(host);
  await until(`the host's chest holds ${hosts} and the friend's game shows it holding ${chest(friend)}: each storage is a thing the host tells of, and a joined game fills its own to match (terra_survival::storage_net)`, () => chest(friend) === hosts, 30);
  remote(friend, "give", "wood:3");
  const added = JSON.parse(remote(friend, "call", "storage.put", JSON.stringify({ item: "wood" }))) as { put: number };
  if (added.put !== 3) throw new Error(`the friend left ${added.put} wood in the host's chest, not the 3 given: it stands ${JSON.stringify(by)} from the start and the friend was put beside it`);
  const both = chest(friend);
  await until(`the friend left wood in the chest, which holds ${both} in its game, and the host's holds ${chest(host)}: a joined game asks the host to hold what its player left (terra_survival::storage_net)`, () => chest(host) === both && both.includes("Wood"), 30);
  console.log(`✓ a chest holds the same in both games: the host left stone in it and the friend wood, and it holds ${both} in each`);

  // A station that works on its own is one station in both games (netcode.md, step 5): the
  // friend sets the host's campfire to cook meat, the host's fire cooks it and the host takes
  // it; the host sets it to make a torch, and the friend takes that.
  type Stations = { stations: { station: string; from_start: Place; queue: string[]; made: string[] }[] };
  const fire = (port: number) => {
    const found = (JSON.parse(remote(port, "call", "station.state")) as Stations).stations.filter((each) => each.station === "Campfire");
    return found.length === 1 ? found[0] : undefined;
  };
  const fireSays = (port: number) => {
    const at = fire(port);
    return at ? `queue [${at.queue.join(", ")}], made [${at.made.join(", ")}]` : "no campfire";
  };
  remote(host, "give", "wood:3", "stone:2");
  const lit = JSON.parse(remote(host, "call", "storage.place", JSON.stringify({ item: "Campfire", at: [9.5, 0, 2.5] }))) as { done: boolean; refused?: string };
  if (!lit.done) throw new Error(`the host's campfire was refused: ${lit.refused}`);
  await until("the host set down a campfire and the friend's game has none (play::props_net)", () => fire(friend) !== undefined, 60);
  const hearth = fire(host)!.from_start;
  const fromFire = (port: number) => {
    const at = (JSON.parse(remote(port, "call", "play.state")) as { at: [number, number] }).at;
    return Math.hypot(at[0] - hearth[0], at[1] - hearth[2]);
  };
  remote(host, "call", "play.warp", JSON.stringify({ at: [hearth[0] + 1.6, hearth[2]] }));
  remote(host, "call", "net.place_guests", JSON.stringify({ at: [hearth[0] - 1.6, hearth[2]] }));
  await until(`both players are put beside the campfire, and the host stands ${fromFire(host).toFixed(1)} m from it and the friend ${fromFire(friend).toFixed(1)} m`, () => fromFire(host) < 3 && fromFire(friend) < 3, 30);
  remote(friend, "give", "meat:1");
  remote(friend, "call", "station.queue", JSON.stringify({ recipe: "Cooked meat" }));
  await until(`the friend set the campfire to cook meat and the host's says ${fireSays(host)}: a joined game asks the host for what its player does at a station that works alone, and the host runs it (terra_survival::works_net)`, () => fire(host)!.made.includes("Cooked meat"), 40);
  await until(`the host's campfire has cooked the meat and the friend's says ${fireSays(friend)}: a joined game shows each lone station as the host tells it`, () => fire(friend)!.made.includes("Cooked meat") && fire(friend)!.queue.length === 0, 20);
  const cooked = JSON.parse(remote(host, "call", "station.take")) as { took: string[] };
  if (!cooked.took.includes("Cooked meat")) throw new Error(`the host took ${JSON.stringify(cooked.took)} from its campfire, not the meat the friend set cooking`);
  await until(`the host took the cooked meat and the friend's campfire still says ${fireSays(friend)}`, () => fire(friend)!.made.length === 0, 20);
  remote(host, "give", "wood:1", "fibre:1");
  remote(host, "call", "station.queue", JSON.stringify({ recipe: "Torch" }));
  await until(`the host set the campfire to make a torch and the friend's says ${fireSays(friend)}`, () => fire(friend)!.made.includes("Torch"), 30);
  const torch = JSON.parse(remote(friend, "call", "station.take")) as { took: string[] };
  if (!torch.took.includes("Torch")) throw new Error(`the friend took ${JSON.stringify(torch.took)} from the host's campfire, not the torch it made`);
  await until(`the friend took the torch and the host's campfire still says ${fireSays(host)}: what a joined player takes is asked of the host (terra_survival::works_net)`, () => fire(host)!.made.length === 0, 20);
  await sleep(3);
  if (fire(friend)!.made.length > 0) throw new Error(`the torch the friend took is back in its campfire (${fireSays(friend)}): a word the host said before the ask reached it must take nothing back`);
  console.log("✓ a campfire is one station in both games: the friend set meat cooking, the host's fire cooked it and the host took it; the host's torch was the friend's to take");

  // Creatures are the host's (netcode.md, step 5): the friend's game puts out none of its
  // own and shows the host's, each where the host has it.
  type Animal = { name: string; at: Place };
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
