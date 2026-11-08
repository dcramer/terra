#!/usr/bin/env node
// A world's small life, about when and where it would be (D647, D379; v0's world design):
// with no picture, by what the game says drifts round the eye (`drift.state`) and which
// small creatures stand near (`sandbox.find`). A new small world is started three times:
// on a July day butterflies, bees and birds on the wing are about and small animals stand
// near; that night the day's insects are gone and fireflies are out; on a January day no
// insect flies. How much of each kind is seen is said.
//
//   node tools/sandbox/ambience.ts [--seed 3]
//
// Node 22.18 or later; no dependencies. About three minutes.

import { call, start, stop, waitSeconds, waitUntil } from "./client.ts";

const args = process.argv.slice(2);
const SEED = args.includes("--seed") ? args[args.indexOf("--seed") + 1] : "3";
/// A kind is about when this much of it is seen, of 1, and gone below a twentieth.
const ABOUT = 0.3;
const GONE = 0.05;
const INSECTS = ["White butterflies", "Brown butterflies", "Bees", "Midges", "Dragonflies", "Fireflies"];
const BIRDS = ["Birds overhead"];
/// The small animals of the first realm (data/creatures.ron), any of which may stand near.
const SMALL = ["Chaffinch", "Wood pigeon", "House sparrow", "Feral pigeon", "Red squirrel", "Brown hare", "Mallard"];

type Drift = { kinds: { name: string; seen: number }[] };

/// What drifts, and how many small animals stand near, in a new world on a date at an hour.
async function about(date: string, hour: number): Promise<{ seen: Map<string, number>; small: string[]; wild: { birds: number; flying: number } }> {
  try {
    await start(["--experiment", "play", "--fresh", "--save", "shots/ambience.ron", "--radius", "2000", "--seed", SEED, "--date", date, "--time", String(hour), "--offscreen", "--mute"], { realTime: true, release: true });
    await waitUntil("the world to be played", async () => (await call("title.state")) as { screen: string }, (now) => now.screen === "Playing", 600);
    // What drifts is weighed a few times a minute and fades in; the animals are put out as the player stands.
    await waitSeconds(12);
    const drift = (await call("drift.state")) as Drift;
    const small: string[] = [];
    for (const name of SMALL) {
      const found = (await call("sandbox.find", { name })) as unknown[];
      if (found.length > 0) small.push(`${found.length} ${name}`);
    }
    const wild = (await call("wildlife.state")) as { birds: number; flying: number };
    return { seen: new Map(drift.kinds.map((kind) => [kind.name, kind.seen])), small, wild };
  } finally {
    await stop().catch(() => undefined);
  }
}

const said = (seen: Map<string, number>, names: string[]) => names.map((name) => `${name} ${seen.get(name) ?? "none"}`).join(", ");
let failed = "";
try {
  const day = await about("7-10", 13);
  const missing = ["White butterflies", "Brown butterflies", "Bees"].filter((name) => !((day.seen.get(name) ?? 0) >= ABOUT));
  if (missing.length > 0) throw new Error(`on a July day at one o'clock ${missing.join(" and ")} are not about (${said(day.seen, INSECTS)}): each kind of assets/data/drift.ron is seen where its place, season_c, air_c and hours are`);
  if (!BIRDS.some((name) => (day.seen.get(name) ?? 0) >= ABOUT)) throw new Error(`on a July day no bird is on the wing (${said(day.seen, BIRDS)})`);
  if ((day.seen.get("Fireflies") ?? 0) > GONE) throw new Error(`fireflies are out at one in the afternoon (${day.seen.get("Fireflies")})`);
  const life = day.wild;
  if (!(life.birds >= 24) || !(life.flying >= 3)) throw new Error(`on a July day ${life.birds} birds are within 150 m of the start and ${life.flying} of them on the wing, where dozens should be about and some always flying (D647; wildlife.state; data/creatures.ron's small birds, creatures::mind's BIRD_ shares)`);
  if (day.small.length === 0) throw new Error(`on a July day none of ${SMALL.join(", ")} stands near the start: the wildlife is put out in cells as a player nears (data/creatures.ron's lives)`);

  const night = await about("7-10", 22);
  if (!((night.seen.get("Fireflies") ?? 0) >= ABOUT)) throw new Error(`on a July night at ten no fireflies are out (${said(night.seen, INSECTS)})`);
  const late = ["White butterflies", "Brown butterflies", "Bees", ...BIRDS].filter((name) => (night.seen.get(name) ?? 0) > GONE);
  if (late.length > 0) throw new Error(`at ten at night ${late.join(" and ")} are still about (${said(night.seen, [...INSECTS, ...BIRDS])}): each kind's hours say when`);

  const winter = await about("1-15", 13);
  const hardy = INSECTS.filter((name) => (winter.seen.get(name) ?? 0) > GONE);
  if (hardy.length > 0) throw new Error(`on a January day ${hardy.join(" and ")} fly (${said(winter.seen, INSECTS)}): an insect needs its air_c of warmth`);

  console.log(`✓ ambience: on a July day ${said(day.seen, ["White butterflies", "Brown butterflies", "Bees", ...BIRDS])}, with ${day.small.join(", ")} near, ${day.wild.birds} birds in all and ${day.wild.flying} on the wing; that night Fireflies ${night.seen.get("Fireflies")} and the day's gone; on a January day no insect flies`);
} catch (problem) {
  failed = String(problem instanceof Error ? problem.message : problem);
}
if (failed) {
  console.error(`✗ ambience: ${failed}`);
  process.exit(1);
}
