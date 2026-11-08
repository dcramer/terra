#!/usr/bin/env node
// Two players walk a world's biomes in changing weather, each game saying the same (v0's
// step 3; Land and Living world): two copies of the game on one seed's 30 km world, one
// hosting and one joined. `land.state` says the biome where a game's eye is, by the land's
// own survey, the weather there and the clock. The host is warped to the nearest place of
// each biome the land round the start has, and puts its guest beside it; there each game
// must say that biome, hold the other within a few metres, and say the same weather and
// hour. Then the host's clock is run fast: the weather must change, and the joined game's
// must change with it. Checked with no picture, by what each game says.
//
//   node tools/sandbox/land.ts [--seed 3]
//
// Not yet a gate (D632): of ten runs three failed, each with the friend's game not standing
// where its host had put it, or not following, after a hop: moving a guest by script is
// rougher than a walk, and what goes wrong in the joined game then is not yet understood.
// A pass says the two games agreed; a failure of that kind is the script's, until it is.
//
// Node 22.18 or later; no dependencies. About five minutes.

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "../..");
const args = process.argv.slice(2);
const SEED = args.includes("--seed") ? args[args.indexOf("--seed") + 1] : "3";
const GAME_PORT = 47000 + (process.pid % 2000);
/// The biomes a player walks (water is swum, not walked), and how many of them the land
/// within reach of the start must have for the walk to count.
const WALKED = ["forest", "woods", "tundra", "bare"];
const AT_LEAST = 3;
/// How far from the start a place of each biome is looked for, in metres: a walk of some
/// minutes.
const REACH_M = 3000;
/// How far a hop toward a place goes, in metres.
const HOP_M = 120;
/// Two games say the same weather when each number is within this of the other's; the
/// weather is eased toward what the clock and the seed say, so they are never bit for bit.
const SAME = { cloud: 0.08, falling_mm_h: 0.5, wind_m_s: 1.0, temperature_c: 1.0 };

type Place = [number, number, number];
type Weather = { cloud: number; falling_mm_h: number; snow_share: number; wind_m_s: number; temperature_c: number };
type Land = { biome: string; weather: Weather | null; clock: { hour: number; day: number } | null; nearest: Record<string, { at: [number, number]; away_m: number }> | null };
type Net = { mine: Place | null; hosting: boolean; joined: boolean; guests: { at: Place }[]; puppets: { at: Place }[] };

function remote(port: number | undefined, ...words: string[]): string {
  const env = { ...process.env };
  if (port) env.TERRA_REMOTE_PORT = String(port);
  else delete env.TERRA_REMOTE_PORT;
  const ran = spawnSync("node", [resolve(ROOT, "tools/sandbox/remote.ts"), ...words], { cwd: ROOT, env, encoding: "utf8" });
  if (ran.status !== 0) throw new Error(`remote ${words.join(" ")} failed:\n${ran.stdout}${ran.stderr}`);
  return ran.stdout;
}
function start(save: string, more: string[]): number {
  remote(undefined, "start", "--real-time", "--", "--experiment", "play", "--fresh", "--save", save, "--radius", "30000", "--seed", SEED, "--offscreen", "--mute", ...more);
  return Number(readFileSync(resolve(ROOT, "shots/remote.port"), "utf8").split(" ")[0]);
}
const land = (port: number) => JSON.parse(remote(port, "call", "land.state")) as Land;
const net = (port: number) => JSON.parse(remote(port, "call", "net.state")) as Net;
const sleep = (seconds: number) => new Promise((done) => setTimeout(done, seconds * 1000));
const apart = (a: Place, b: Place) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);
async function until(what: () => string, test: () => boolean, seconds = 120) {
  for (let waited = 0; waited < seconds; waited += 0.5) {
    if (test()) return;
    await sleep(0.5);
  }
  throw new Error(`${what()}, after ${seconds} s`);
}
/// What differs between two games' weather, if anything does by more than `SAME` allows.
function differs(a: Weather, b: Weather): string[] {
  return (Object.keys(SAME) as (keyof typeof SAME)[]).filter((key) => Math.abs(a[key] - b[key]) > SAME[key]).map((key) => `${key} ${a[key].toFixed(2)} against ${b[key].toFixed(2)}`);
}

const ports: number[] = [];
let failed = "";
try {
  const host = start("shots/land-host.ron", []);
  ports.push(host);
  remote(host, "call", "net.host", JSON.stringify({ port: GAME_PORT }));
  const friend = start("shots/land-join.ron", ["--multirun"]);
  ports.push(friend);
  remote(friend, "call", "net.join", JSON.stringify({ address: `127.0.0.1:${GAME_PORT}`, name: "friend" }));
  await until(() => "the two games never held each other", () => net(friend).puppets.length === 1 && net(host).guests.length === 1 && net(friend).mine !== null);

  const round = JSON.parse(remote(host, "call", "land.state", JSON.stringify({ reach_m: REACH_M }))) as Land;
  const places = Object.entries(round.nearest ?? {}).filter(([biome]) => WALKED.includes(biome));
  if (places.length < AT_LEAST) throw new Error(`the land within ${REACH_M / 1000} km of seed ${SEED}'s start has ${places.map(([biome]) => biome).join(", ") || "no biome"} to walk, under ${AT_LEAST}: v0 asks for three biomes reached by walking (try another --seed; the sweep, cargo xtask worlds, says what each seed's walk reaches)`);
  const walked: string[] = [];
  let from: [number, number] = [0, 0];
  for (const [biome, { at }] of places) {
    // There in hops of a hundred metres and more, a second and a half apart, as a fast walk
    // goes: put there at once, a game makes all the land round the new place before it
    // next speaks, and its host, hearing nothing for ten seconds, takes it for gone.
    const hops = Math.max(1, Math.ceil(Math.hypot(at[0] - from[0], at[1] - from[1]) / HOP_M));
    for (let hop = 1; hop <= hops; hop++) {
      const here = [from[0] + ((at[0] - from[0]) * hop) / hops, from[1] + ((at[1] - from[1]) * hop) / hops];
      remote(host, "warp", `${here[0]},${here[1]}`);
      remote(host, "call", "net.place_guests", JSON.stringify({ at: [here[0] + 2, here[1]] }));
      // On to the next hop once the friend's game stands where its host has it: its land
      // there is made, however long this machine took over it.
      await until(
        () => `on the way to the nearest ${biome}, at hop ${hop} of ${hops}, the friend's game never stood where the host put it (the host has ${net(host).guests.length} guests)`,
        () => {
          const [mine, theirs] = [net(friend).mine, net(host).guests[0]?.at];
          return !!mine && !!theirs && apart(mine, theirs) < 0.5;
        },
        40,
      );
    }
    from = at;
    if (net(host).guests.length !== 1 || !net(friend).joined) throw new Error(`on the way to the nearest ${biome} the friend's game was dropped by its host (the host has ${net(host).guests.length} guests; the friend's game is ${net(friend).joined ? "" : "not "}joined): what the host said of it follows`);
    let said = "";
    await until(
      () => `at the nearest ${biome} (${at.map((x) => x.toFixed(0)).join(", ")} m east and south of the start) ${said}`,
      () => {
        const [here, there, mine, theirs] = [land(host), land(friend), net(friend).mine, net(host).guests[0]?.at];
        if (!mine || !theirs || apart(mine, theirs) > 0.5) return !(said = "the friend's game never stood where the host put it");
        if (here.biome !== biome || there.biome !== biome) return !(said = `the host's game says ${here.biome} and the friend's ${there.biome}`);
        if (!here.weather || !there.weather || !here.clock || !there.clock) return !(said = "a game has no weather or clock");
        const off = differs(here.weather, there.weather);
        if (off.length > 0) return !(said = `the two games' weather differs: ${off.join("; ")} (host against friend)`);
        const hours = Math.abs(here.clock.hour - there.clock.hour);
        if (here.clock.day !== there.clock.day || hours > 0.1) return !(said = `the host's clock says day ${here.clock.day} ${here.clock.hour.toFixed(2)} h and the friend's day ${there.clock.day} ${there.clock.hour.toFixed(2)} h`);
        return true;
      },
      60,
    );
    const now = land(host).weather!;
    walked.push(`${biome} (${now.temperature_c.toFixed(0)} C, cloud ${now.cloud.toFixed(1)})`);
  }
  console.log(`✓ both walked ${walked.join(", ")}: each game said the biome, held the other, and the same weather and hour`);

  // Changing weather: the host's clock run twelve times as fast for a while (a day in two and a half
  // minutes: at sixty a game spends its frames repainting the sky and may be dropped); the weather where they stand must change, and the joined game's with it.
  const before = land(host).weather!;
  remote(host, "tune", "set", "daylight.pace", "12");
  let changed: string[] = [];
  await until(() => `with the clock run fast the host's weather never changed from cloud ${before.cloud.toFixed(2)}, ${before.temperature_c.toFixed(1)} C`, () => (changed = differs(land(host).weather!, before)).length > 0, 120);
  remote(host, "tune", "set", "daylight.pace", "1");
  let off: string[] = [];
  let clocks = "";
  await until(
    () => `after the host's weather changed (${changed.join("; ")}) the joined game's did not follow: ${off.join("; ")}; ${clocks} (host against friend): nothing travels but the clock and the seed (D540), so the joined game's clock must be the host's`,
    () => {
      const [here, there] = [land(host), land(friend)];
      off = differs(here.weather!, there.weather!);
      clocks = `the host's clock says day ${here.clock!.day} ${here.clock!.hour.toFixed(2)} h and the friend's day ${there.clock!.day} ${there.clock!.hour.toFixed(2)} h`;
      return off.length === 0 && here.clock!.day === there.clock!.day && Math.abs(here.clock!.hour - there.clock!.hour) < 0.25;
    },
    40,
  );
  if (net(host).guests.length !== 1) throw new Error(`by the end the host has ${net(host).guests.length} guests: the friend's game was dropped on the way (what the host said of it follows)`);
  console.log(`✓ the weather changed for both: ${changed.join("; ")} (after against before), the joined game's the same as the host's after`);
} catch (problem) {
  failed = String(problem instanceof Error ? problem.message : problem);
} finally {
  // What the host said of its guests coming and going, when the walk failed.
  if (failed) {
    try {
      console.error(remote(ports[0], "said", "--all", "--containing", "the world.").trim().split("\n").slice(-4).join("\n"));
    } catch {}
  }
  for (const port of ports) {
    try {
      remote(port, "stop");
    } catch {}
  }
}
if (failed) {
  console.error(`✗ land: ${failed}`);
  process.exit(1);
}
console.log("✓ land: two players walked the land's biomes in changing weather, each game saying the same");
