#!/usr/bin/env node
// The boss fought by a player's own hand (D590, v0's "they beat the boss, are marked"): in
// a new world the player walks into the den with a sword and shield, draws, and fights the
// troll by the keys a person presses, nothing dealt by the script: they face it, close,
// swing while it recovers, and raise their guard while a blow of its is coming. The troll
// must fall to their blows, the player must live, and they must bear its mark. What the
// fight took is said: its length, the blows each landed, the health left.
//
//   node tools/sandbox/fight.ts [--seed 3] [--tries 2] [--trace] [--film shots/fight.mp4]
//
// Node 22.18 or later; no dependencies. About five minutes.

import { spawnSync } from "node:child_process";
import { readFileSync, readdirSync, rmSync } from "node:fs";
import { basename, dirname, join, resolve } from "node:path";
import { ROOT, call, film, give, press, sleep, start, stop, unpress, waitSeconds, waitUntil, warp } from "./client.ts";
import { type Arm, type Through, readBlows, readChain, swing, untilLanding } from "./fight/plan.ts";

const args = process.argv.slice(2);
const SEED = args.includes("--seed") ? args[args.indexOf("--seed") + 1] : "3";
/// The fight is over by this long, in seconds: some eight times what it takes.
const MOST_S = 240;
/// Swings reach about this far, metres; the player closes to it and no nearer.
const CLOSE_M = 2.4;
/// When each blow of the troll's lands and how long it lasts, and how long each blow of the
/// sword's chain keeps the arm, read from the game's data (fight/plan.ts): the guard is up
/// from before the soonest may land until the latest has, and a swing is begun only if it
/// is done before the next may land, since a held guard is raised only once the arm is in
/// no move (a swing is not cut short by it).
const BLOWS = readBlows(readFileSync(resolve(ROOT, "assets/data/creatures.ron"), "utf8"), "Troll");
const CHAIN = readChain(readFileSync(resolve(ROOT, "assets/data/moves/sword.shielded.ron"), "utf8"));
/// Presses of the swing's key are at least this far apart, seconds: one look and the next.
const PRESS_APART_S = 0.25;
/// A film of the fight is so many frames from its beginning.
const FILM_FRAMES = 900;

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

  // A key is held for a count of frames, and how fast frames come is the machine's: measured
  // here, so each hold below is so many seconds on any of them.
  const frames = async () => ((await call("sandbox.status")) as { frame: number }).frame;
  const before = await frames();
  await waitSeconds(1);
  // Never taken for slower than 240 a second: a hold that outlasts its need is let go by
  // the clock below, and one that ends early stops the player where they stand.
  const fps = Math.max(240, (await frames()) - before);
  const hold = (key: string, seconds: number) => press(`${key}:${Math.ceil(seconds * fps)}`);
  // In by the mouth, forward until the troll is near.
  await warp([den.mouth[0], den.mouth[2]], [den.ahead[0], den.ahead[2]]);
  let last: [number, number] = [0, 0];
  for (let walked = 0; walked < 60 && !(((await troll())?.distance ?? Infinity) < 14); walked++) {
    await hold("w", 1);
    await waitSeconds(1);
    // Held up on a lip of the passage's floor: hopped, as in the fight.
    const here = (await state() as unknown as { at: [number, number] }).at;
    if (Math.hypot(here[0] - last[0], here[1] - last[1]) < 0.3) await press("space");
    last = here;
  }
  await unpress();
  const met = await troll();
  if (!met || (met.distance ?? Infinity) >= 14) throw new Error(`a minute's walk in by the den's mouth met no troll within 14 m (the nearest: ${met ? `${met.distance?.toFixed(0)} m` : "none near"}): node tools/sandbox/den.ts holds the walk in`);
  const hale = (await state()).health;
  const whole = met.health ?? 0;
  await press("t");
  await waitSeconds(1.5);

  // Filmed, if asked: the next frames from here, the fight's opening, for an eye to judge.
  const FILM = args.includes("--film") ? args[args.indexOf("--film") + 1] : "";
  const filming = FILM ? await film(FILM.replace(/\.mp4$/, ".png"), FILM_FRAMES) : "";
  // The fight, ten looks a second: face it, close, guard its blow, swing between.
  const began = Date.now();
  let [guards, swings, lowest, left] = [0, 0, hale, whole];
  let guarding = false;
  let swung = 0;
  // The blow of the chain the arm was last sent on, and when.
  let arm: { swing: number; began: number } | undefined;
  let held = "";
  let [stood, since, hops] = [[0, 0] as [number, number], Date.now(), 0];
  // What each second of the fight was, said if it is lost (--trace says it of a won one).
  const trace = traceOut;
  let traced = -1;
  let was: [number, number] = [0, 0];
  while (Date.now() - began < MOST_S * 1000) {
    const asked = Date.now();
    const now = await state();
    const answered = Date.now();
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
    // The view is turned from the camera, which the den's walls push to one side of the
    // player: aimed at the troll itself, the way ahead (which the keys walk) would point off
    // it by the camera's offset. Aimed far past it along the line from the player, it points
    // at it within a few degrees. From the player's own level: the ground over the spot is
    // the hill above the den.
    const me = (now as unknown as { at: [number, number] }).at;
    const [east, south] = [it.at![0] - me[0], it.at![1] - me[1]];
    const aimed = 30 / Math.max(0.5, Math.hypot(east, south));
    // "Attack(1.2)": a blow of its, so far through.
    // How far through it is now is between what was said plus the time since the answer came
    // and plus the time since it was asked; a key pressed takes about an answer's time to
    // arrive. Both are the machine's, measured each look.
    const said = /^Attack\(([0-9.]+)/.exec(it.doing ?? "")?.[1];
    const ago = (from: number) => (Date.now() - from) / 1000;
    const through: Through | undefined = said === undefined || (it.distance ?? 99) >= 7 ? undefined : [Number(said) + ago(answered), Number(said) + ago(asked)];
    const late = (answered - asked) / 1000;
    const until = untilLanding(BLOWS, through);
    const far = (it.distance ?? 0) > CLOSE_M;
    // A swing, if the arm will be free of it before a blow may land: the chain's next from
    // a blow past its cancel, its first from a free arm. Never while the troll is in no blow:
    // its next begins the moment it is in reach.
    const busy: Arm = arm && { swing: arm.swing, for: ago(arm.began) };
    const committed = busy !== undefined && busy.for < CHAIN[busy.swing].cancel;
    const next = far || through === undefined || now.stamina <= 25 ? undefined : swing(CHAIN, busy, until, late);
    // One key is held at a time, long, and let go by the clock when what the player is at
    // changes: stepping in while the troll is out of the sword's reach and no blow is about
    // to land, hands free for a swing that fits or is under way, else on their guard through
    // the troll's blow. (A step breaks the guard, a guard stops the step, and a key pressed
    // again while held is pressed anew, which lowers a raised guard.)
    const coming = through !== undefined && (far ? until < 0.7 + late : next === undefined && !committed);
    const want = coming ? "b" : far ? "w" : "";
    if (want !== held) {
      if (held) await unpress();
      if (want) await hold(want, 10);
      if (want === "b") guards++;
      held = want;
    }
    guarding = coming;
    // Held up with the way ahead walked: a lip of the den's floor, which a person hops.
    if (want === "w") {
      if (Math.hypot(me[0] - stood[0], me[1] - stood[1]) > 0.15) [stood, since] = [me, Date.now()];
      else if (Date.now() - since > 800) {
        hops++;
        since = Date.now();
        await press("space");
      }
    } else since = Date.now();
    if (!want && next !== undefined && Date.now() - swung > PRESS_APART_S * 1000) {
      swung = Date.now();
      arm = { swing: next, began: swung };
      swings++;
      await press("e");
    }
    // The view last, each look: what is decided above waits on nothing but the look itself.
    await call("play.aim", { at: [me[0] + east * aimed, me[1] + south * aimed], up: 2.5, level: true }).catch(() => undefined);
    await sleep(50);
  }
  const took = (Date.now() - began) / 1000;
  if (filming) {
    // Saving every frame slows the game to a few a second, so the film is made of what was
    // saved by the fight's end, played at the rate it was saved at: the fight at its own
    // speed, a few pictures a second. For how a move looks, the duel's films; this shows
    // what happened.
    const dir = dirname(resolve(FILM));
    const stem = basename(FILM).replace(/\.mp4$/, "-");
    const saved = readdirSync(dir).filter((name) => name.startsWith(stem) && name.endsWith(".png"));
    const rate = Math.max(1, Math.round(saved.length / Math.max(1, took)));
    const made = spawnSync("ffmpeg", ["-loglevel", "error", "-y", "-framerate", String(rate), "-i", join(dir, `${stem}%05d.png`), "-pix_fmt", "yuv420p", FILM]);
    if (made.status !== 0) throw new Error(`ffmpeg could not make ${FILM} of the fight's ${saved.length} frames: ${made.stderr}`);
    for (const name of saved) rmSync(join(dir, name), { force: true });
  }
  if (left > 0) throw new Error(`after ${MOST_S} s the troll has ${left.toFixed(0)} of ${whole.toFixed(0)} health, the player ${lowest.toFixed(0)} at their lowest, after ${swings} swings and ${guards} guards: the sword's blows are not landing or do too little (shots/remote.log's "Melee:" lines say what each swing did)`);
  await waitUntil("the player to bear the troll's mark after felling it themselves", async () => (await call("marks.state")) as { marks: string[] }, (now) => now.marks.includes("troll"), 10);
  const won = `✓ fight: seed ${SEED}'s troll fell to the player's own sword in ${took.toFixed(0)} s, ${swings} swings, ${guards} guards and ${hops} hops over the floor's lips; the player, of ${hale.toFixed(0)} health, was at ${lowest.toFixed(0)} at their lowest and bears its mark`;
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
