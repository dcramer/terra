#!/usr/bin/env node
// The boss fought by a player's own hand (D590, v0's "they beat the boss, are marked"): in
// a new world the player walks into the den with a sword and shield, draws, and fights the
// troll by the keys a person presses, nothing dealt by the script: they face it, close,
// swing while it recovers, and raise their guard while a blow of its is coming. The troll
// must fall to their blows, the player must live, and they must bear its mark. What the
// fight took is said: its length, the blows each landed, the health left.
//
//   node tools/sandbox/fight.ts [--seed 3] [--tries 2] [--trace]
//
// Node 22.18 or later; no dependencies. About five minutes.

import { call, give, press, start, stop, unpress, waitSeconds, waitUntil, warp } from "./client.ts";

const args = process.argv.slice(2);
const SEED = args.includes("--seed") ? args[args.indexOf("--seed") + 1] : "3";
/// The fight is over by this long, in seconds: some eight times what it takes.
const MOST_S = 240;
/// Swings reach about this far, metres; the player closes to it and no nearer.
const CLOSE_M = 2.4;
/// When a blow of the troll's may land, in seconds from its beginning as the game says it:
/// its sweep lands at 1.4 s and its smash at 2.2 s (data/creatures.ron), and the script sees
/// each a fifth of a second late; which of the two is coming it cannot tell, as a player
/// new to the troll cannot. The guard is up through all of it.
const LANDS_S: [number, number][] = [[0.9, 2.5]];
/// A swing commits the arm for this long, seconds (the set's slash, to where a guard may cut
/// it short): none is begun with a blow due to land sooner, nor sooner after the last.
const SWING_S = 0.7;

type Place = [number, number, number];
type Den = { den: { mouth: Place; floor: Place; ahead: Place } | null };
type Near = { what?: string; at?: [number, number]; distance?: number; health?: number; doing?: string };
type State = { health: number; stamina: number; near: Near[] };

/// The fight is hard, and need only be possible (the developers, 2026-11-08): it is tried
/// again, in a new game, this many times in all before the troll is said to be unbeaten.
const TRIES = args.includes("--tries") ? Number(args[args.indexOf("--tries") + 1]) : 2;
let traceOut: string[] = [];

/// One fight in a new game: what is said of it won; thrown, how it was lost.
async function fight(): Promise<string> {
  await start(["--experiment", "play", "--fresh", "--save", "shots/fight.ron", "--radius", "30000", "--seed", SEED, "--offscreen", "--mute"], { realTime: true, release: true });
  await waitUntil("the world to be played", async () => (await call("title.state")) as { screen: string }, (now) => now.screen === "Playing", 600);
  const { den } = (await call("den.state")) as Den;
  if (!den) throw new Error(`seed ${SEED}'s world has no den (cargo xtask worlds says why)`);
  await give("arming_sword", "heater_shield");
  const state = async () => (await call("play.state")) as State;
  const troll = async () => (await state()).near.find((each) => each.what === "Troll");

  // In by the mouth, forward until the troll is near.
  await warp([den.mouth[0], den.mouth[2]], [den.ahead[0], den.ahead[2]]);
  for (let walked = 0; walked < 60 && !(((await troll())?.distance ?? Infinity) < 14); walked++) {
    await press("w:60");
    await waitSeconds(1);
  }
  const met = await troll();
  if (!met || (met.distance ?? Infinity) >= 14) throw new Error(`a minute's walk in by the den's mouth met no troll within 14 m (the nearest: ${met ? `${met.distance?.toFixed(0)} m` : "none near"}): node tools/sandbox/den.ts holds the walk in`);
  // A key is held for a count of frames, and how fast frames come is the machine's: measured
  // here, so each hold below is so many seconds on any of them.
  const frames = async () => ((await call("sandbox.status")) as { frame: number }).frame;
  const before = await frames();
  await waitSeconds(1);
  const fps = Math.max(20, (await frames()) - before);
  const hold = (key: string, seconds: number) => press(`${key}:${Math.ceil(seconds * fps)}`);
  const hale = (await state()).health;
  const whole = met.health ?? 0;
  await press("t");
  await waitSeconds(1.5);

  // The fight, ten looks a second: face it, close, guard its blow, swing between.
  const began = Date.now();
  let [guards, swings, lowest, left] = [0, 0, hale, whole];
  let guarding = false;
  let swung = 0;
  let held = "";
  // What each second of the fight was, said if it is lost (--trace says it of a won one).
  const trace = traceOut;
  let traced = -1;
  let was: [number, number] = [0, 0];
  while (Date.now() - began < MOST_S * 1000) {
    const now = await state();
    const it = now.near.find((each) => each.what === "Troll");
    lowest = Math.min(lowest, now.health);
    const second = Math.floor((Date.now() - began) / 1000);
    if (second !== traced) {
      traced = second;
      const here = (now as unknown as { at: [number, number] }).at;
      const stepped = Math.hypot(here[0] - was[0], here[1] - was[1]);
      was = here;
      trace.push(`${second}s walked ${stepped.toFixed(1)}m ${it ? `${it.distance?.toFixed(1)}m ${it.doing?.replace(/\(([0-9.]+).*/, (_, t) => ` ${Number(t).toFixed(1)}`)} ${it.health?.toFixed(0)}` : "no troll"} me ${now.health.toFixed(0)}/${now.stamina.toFixed(0)}${guarding ? " guard" : ""}`);
    }
    if (now.health <= 0) throw new Error(`the player fell ${((Date.now() - began) / 1000).toFixed(0)} s into the fight, the troll at ${left.toFixed(0)} of ${whole.toFixed(0)} health after ${swings} swings and ${guards} guards: the troll's blows and the sword's are data (data/creatures.ron's Troll, the sword's moves), and a fight a guard and a sword cannot win is the fault`);
    if (!it || (it.health ?? 0) <= 0) {
      left = 0;
      break;
    }
    left = it.health ?? left;
    // At its chest on the den's floor, from the player's own level: the ground over the spot
    // is the hill above the den.
    await call("play.aim", { at: it.at, up: 2.5, level: true }).catch(() => undefined);
    // "Attack(1.2)": a blow of its, so far through. Guard from its beginning until it has landed.
    const through = /^Attack\(([0-9.]+)/.exec(it.doing ?? "")?.[1];
    const coming = through !== undefined && (it.distance ?? 99) < 7 && LANDS_S.some(([from, to]) => Number(through) >= from && Number(through) < to);
    // One key is held at a time, long, and let go by the clock when what the player is at
    // changes: on their guard through a blow, stepping in while the troll is out of the
    // sword's reach, else free to swing. (A step breaks the guard, a guard stops the step,
    // and a key pressed again while held is pressed anew, which lowers a raised guard.)
    const want = coming ? "b" : (it.distance ?? 0) > CLOSE_M ? "w" : "";
    if (want !== held) {
      if (held) await unpress();
      if (want) await hold(want, 10);
      if (want === "b") guards++;
      held = want;
    }
    guarding = coming;
    const due = through === undefined || LANDS_S.some(([from, to]) => Number(through) + SWING_S > from && Number(through) < to);
    if (!want && !due && now.stamina > 25 && Date.now() - swung > SWING_S * 1000) {
      swung = Date.now();
      swings++;
      await press("e");
    }
    await waitSeconds(0.1);
  }
  const took = (Date.now() - began) / 1000;
  if (left > 0) throw new Error(`after ${MOST_S} s the troll has ${left.toFixed(0)} of ${whole.toFixed(0)} health, the player ${lowest.toFixed(0)} at their lowest, after ${swings} swings and ${guards} guards: the sword's blows are not landing or do too little (shots/remote.log's "Melee:" lines say what each swing did)`);
  await waitUntil("the player to bear the troll's mark after felling it themselves", async () => (await call("marks.state")) as { marks: string[] }, (now) => now.marks.includes("troll"), 10);
  const won = `✓ fight: seed ${SEED}'s troll fell to the player's own sword in ${took.toFixed(0)} s, ${swings} swings and ${guards} guards; the player, of ${hale.toFixed(0)} health, was at ${lowest.toFixed(0)} at their lowest and bears its mark`;
  return args.includes("--trace") ? `${won}\n${trace.join("\n")}` : won;
}

const lost: string[] = [];
let said = "";
for (let attempt = 1; attempt <= TRIES && !said; attempt++) {
  traceOut = [];
  try {
    said = await fight();
  } catch (problem) {
    let how = String(problem instanceof Error ? problem.message : problem);
    if (traceOut.length > 0) how += `\n  second by second (how far the player walked; the troll: how far off, what it is at and how far through, its health; the player's health/stamina):\n  ${traceOut.join("\n  ")}`;
    lost.push(how);
  } finally {
    await stop().catch(() => undefined);
  }
}
if (said) console.log(lost.length > 0 ? `${said} (won on try ${lost.length + 1} of ${TRIES}: the fight is hard)` : said);
else {
  console.error(`✗ fight: lost ${lost.length} of ${lost.length} tries; the last: ${lost.at(-1)}`);
  process.exit(1);
}
