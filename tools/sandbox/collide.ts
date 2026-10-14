#!/usr/bin/env node
// Walks the player into what stands in the lush forest and checks that what stops them is
// what they see (D69, D243, D267): trunks, boulders, rocks, stumps, fallen logs, and anything
// else the forest gives a footing. A sample of every kind, low and tall, is taken from the
// game (lush.solids), and for each the player is set down a few metres off, facing it, at
// a different turn round it each time, with nothing else in the way to it nor, for a low
// thing, past it (ways.ts), and walks at it; one with no such way is passed over for the
// next nearest of its kind. Measured against the meshes
// themselves (lush.footprint), not the shapes fitted to them, it fails when:
//
//   (a) the player's body ever stands more than 0.1 m into a solid mesh, where the mesh is
//       taller than a step: walked into a face or through a corner;
//   (b) walking at something taller than a step, the player never came within 0.35 m of it
//       as drawn: an invisible wall, a footprint fatter than the model;
//   (c) something lower than a step (under 0.3 m) was not stepped onto and over, or the
//       player was lifted more than a step onto something taller.
//
// Things not yet fitted after the forest has loaded fail too, and so does a kind of thing
// that should stand in the forest and is missing. Pictures of the first few, and of every
// failure, in shots/collide/.
//
//   node tools/sandbox/collide.ts [--count 20] [--here]
//
// About a minute on a built release. Node 22.18 or later; no dependencies.

import * as game from "./client.ts";
import { APPROACH, BEYOND, BODY, clear, type Spot } from "./ways.ts";

const arg = (name: string, fallback: string) => {
  const at = process.argv.indexOf(name);
  return at > 0 ? process.argv[at + 1] : fallback;
};
const count = Number(arg("--count", "20"));
const windows = game.onWsl && !process.argv.includes("--here");

/// The walker's step (terra_build::Walker).
const STEP = 0.4;
/// How far into a solid mesh a body may stand, how far short of a tall thing it may be stopped,
/// and how high a thing may be and count as low.
const INTO = 0.1;
const SHORT = 0.35;
const LOW = 0.35;
const TALL = 0.6;
/// The kinds the forest must have.
const KINDS = ["Rock", "Boulder", "Stump", "Log", "Snag", "Oak", "Pine", "Spruce"];

type Low = { at: [number, number]; across: [number, number]; top: number };
type Solid = { entity: number; name: string; at: [number, number]; top: number | null; low: Low | null; fitted: boolean };
type Print = { player: { at: [number, number]; lift: number } | null; track: [number, number, number][]; points: Spot[] };
/// One walk: at a thing, aimed at a point of it as high as `top`, along `across` if given; one
/// of `group`, of which the first `wanted` with a clear way are walked.
type Walk = { o: Solid; aim: [number, number]; top: number; across?: [number, number]; what: string; group: string; wanted: number };
/// How many more of a group are kept in hand, nearest first, for those with no clear way.
const SPARES = 3;

console.log(`Starting the game${windows ? " on Windows" : ""}…`);
await game.start(["--experiment", "play", "--experiment", "lush", "--level", "yard", "--time", "13"], { windows, copy: "collide", release: true });
await game.waitFor("the floor has", { since: 0, timeout: 120 });

const failures: string[] = [];
const fail = (line: string) => {
  failures.push(line);
  console.log(`collide: FAIL: ${line}`);
};
const where = (at: [number, number]) => at.map((v) => v.toFixed(1)).join(",");

// Everything solid near the start, once each has been fitted to its mesh.
const solids = await game
  .waitUntil(
    "every footing to be fitted",
    async () => (await game.call("lush.solids", { at: [0, 0], within: 120 })) as Solid[],
    (all) => all.length > 0 && all.every((s) => s.fitted),
    60,
  )
  .catch(async () => (await game.call("lush.solids", { at: [0, 0], within: 120 })) as Solid[]);
const unfitted = solids.filter((s) => !s.fitted);
if (unfitted.length) fail(`${unfitted.length} things never fitted, such as a ${unfitted[0].name} at ${where(unfitted[0].at)}`);

// Of each kind, the nearest low one and the nearest tall ones, walked at their middles; trees
// are all tall. And of each kind with low ends, as a log's thin end, a walk across one. A few
// more of each are kept in hand, for one with no clear way to it.
const kinds = [...new Set([...KINDS, ...solids.map((s) => s.name).filter((n) => n)])];
const per = Math.max(2, Math.ceil(count / kinds.length));
const walks: Walk[] = [];
for (const kind of kinds) {
  const of = solids.filter((s) => s.fitted && s.name === kind).sort((a, b) => Math.hypot(...a.at) - Math.hypot(...b.at));
  if (!of.length) {
    fail(`nothing called ${kind} stands within 120 m of the start`);
    continue;
  }
  const top = (s: Solid) => s.top ?? Infinity;
  const group = (what: string, of: Solid[], wanted: number, walk: (s: Solid) => Omit<Walk, "group" | "wanted" | "what">) =>
    walks.push(...of.slice(0, wanted + SPARES).map((s) => ({ ...walk(s), what, group: `${kind} ${what}`, wanted: Math.min(wanted, of.length) })));
  const middle = (s: Solid) => ({ o: s, aim: s.at, top: top(s) });
  const low = of.filter((s) => top(s) < LOW);
  group("low", low, 1, middle);
  group("tall", of.filter((s) => top(s) >= TALL), per - Math.min(1, low.length), middle);
  const ends = of.filter((s) => top(s) >= LOW && s.low && s.low.top < LOW);
  group("low end", ends, kind === "Log" ? 2 : 1, (s) => ({ o: s, aim: s.low!.at, top: s.low!.top, across: s.low!.across }));
  const tops = of.map(top).filter((t) => t < 90).sort((a, b) => a - b);
  if (tops.length) console.log(`collide: ${tops.length} ${kind}, ${tops.filter((t) => t < LOW).length} low: ${tops[0].toFixed(2)} to ${tops[tops.length - 1].toFixed(2)} m high, half under ${tops[tops.length >> 1].toFixed(2)}; ${of.filter((s) => s.low && s.low.top < LOW).length} with a low end`);
}
const wanted = new Map(walks.map((w) => [w.group, w.wanted]));
const planned = [...wanted.values()].reduce((a, b) => a + b, 0);
console.log(`collide: ${planned} walks at things of ${kinds.length} kinds: ${kinds.join(", ")}`);

const footprint = async (points: [number, number][], thing?: number) => (await game.call("lush.footprint", { points, thing })) as Print;

// A spot to start from: four metres off, at a turn round it (across it, for a low end), with
// nothing else solid on the way (`ways.ts`), nor, for a low thing, past it, where the walker
// must step down off it. The player is set down there, facing it, and must stay put: one
// pushed aside at once stood in something that stops a walker but is not wood or stone, as
// a spruce's skirt (D248), and would walk past the thing rather than at it.
const standFor = async (w: Walk, n: number): Promise<[number, number] | undefined> => {
  const turns = w.across ? [Math.atan2(w.across[1], w.across[0]), Math.atan2(-w.across[1], -w.across[0])] : [...Array(12).keys()].map((t) => n * 2.4 + (t / 12) * Math.PI * 2);
  const over = w.top < LOW ? BEYOND : [];
  for (const angle of turns) {
    const way: [number, number][] = [...APPROACH, ...over.map((d) => -d)].map((d) => [w.aim[0] + Math.cos(angle) * d, w.aim[1] + Math.sin(angle) * d]);
    const spots = (await footprint(way, w.o.entity)).points;
    if (!clear(spots.slice(0, APPROACH.length), spots.slice(APPROACH.length))) continue;
    await game.warp(way[0], w.aim);
    const settled = (await game.status()).frame + 3;
    await game.waitUntil("the player to settle", game.status, (now) => now.frame > settled, 10);
    const placed = (await footprint([])).player?.at;
    if (placed && Math.hypot(placed[0] - way[0][0], placed[1] - way[0][1]) < 0.1) return way[0];
  }
  return undefined;
};

let tried = 0;
const walked = new Map<string, number>();
for (const w of walks) {
  const { o, aim, top } = w;
  if ((walked.get(w.group) ?? 0) >= w.wanted) continue;
  const n = tried;
  const stand = await standFor(w, n);
  if (!stand) {
    console.log(`collide: ${o.name} at ${where(aim)}${w.what === "low end" ? ", its low end" : ""}: no clear way to it or past it, skipped`);
    continue;
  }
  walked.set(w.group, (walked.get(w.group) ?? 0) + 1);
  tried++;
  const toward = [aim[0] - stand[0], aim[1] - stand[1]];
  const length = Math.hypot(toward[0], toward[1]);
  const ahead = [toward[0] / length, toward[1] / length];
  // Far enough to have passed it, were it not solid.
  const frames = 110;
  // Forget where the player was before; then where they are every frame of the walk.
  await footprint([]);
  const from = (await game.status()).frame;
  await game.press(`w:${frames}`);
  await game.waitUntil("the walk", game.status, (now) => now.frame > from + frames + 4, 60);
  const track = (await footprint([])).track.map(([east, south, lift]) => ({ at: [east, south] as [number, number], lift }));
  // Every place walked through against the meshes.
  const spots = (await footprint(
    track.map((t) => t.at),
    o.entity,
  )).points;
  const deepest = spots.reduce((worst, s, i) => (s.solid !== null && (worst < 0 || s.solid < (spots[worst].solid as number)) ? i : worst), -1);
  const into = deepest >= 0 ? BODY - (spots[deepest].solid as number) : -Infinity;
  const short = Math.min(...spots.map((s) => (s.seen === null ? Infinity : s.seen))) - BODY;
  const lifted = Math.max(0, ...track.map((t) => t.lift));
  const end = track[track.length - 1].at;
  // How far past where it was aimed at the player got, along the way they walked.
  const past = (end[0] - aim[0]) * ahead[0] + (end[1] - aim[1]) * ahead[1];
  const kind = top < LOW ? "low" : top >= TALL ? "tall" : "middling";
  let line = `collide: ${String(n).padStart(2)}: ${o.name.padEnd(7)} ${w.what.padEnd(8)} ${top === Infinity ? "tree  " : top.toFixed(2) + " m"} at ${where(aim)}: came within ${short.toFixed(2)} m of it, ${into > -Infinity ? `${into.toFixed(2)} m into anything solid` : "nothing solid near"}, lifted ${lifted.toFixed(2)} m, ended ${past.toFixed(1)} m past it`;
  const was = failures.length;
  const what = `${o.name} at ${where(aim)}${w.what === "low end" ? ", its low end" : ""}`;
  if (into > INTO) fail(`${what}: the body stood ${into.toFixed(2)} m into the ${spots[deepest].solid_of} at ${where(track[deepest].at)}`);
  if (kind === "tall" && short > SHORT) fail(`${what}: stopped ${short.toFixed(2)} m short of it: an invisible wall`);
  if (kind === "low" && !(past > 0.3 && lifted > top * 0.4)) fail(`${what}, ${top.toFixed(2)} m high: not stepped over (ended ${past.toFixed(1)} m past it, lifted ${lifted.toFixed(2)} m)`);
  if (kind === "tall" && lifted > STEP + 0.05) fail(`${what}: climbed ${lifted.toFixed(2)} m onto something taller than a step`);
  if (failures.length > was) line += ": FAILED";
  console.log(line);
  if (n < 4 || failures.length > was) await game.shot(`shots/collide/${n}.png`).catch(() => {});
}
await game.stop();
if (tried < planned / 2) fail(`only ${tried} of ${planned} walks could be tried`);
console.log(failures.length ? `collide: FAIL: ${failures.length} failures over ${tried} walks` : `collide: pass: ${tried} walks; each thing stopped the player where it is drawn, or was stepped over`);
process.exit(failures.length ? 1 : 0);
