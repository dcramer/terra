#!/usr/bin/env node
// The character creator's choices on the player's body (D111), with no picture: a running
// game's player is given each hairstyle, a beard, and each skin tone, hair colour and eye
// colour on offer, and the game says what its body then shows (`creator.state`): which
// pieces, and what the skin's, hair's and eyes' parts are drawn with. Each must be what
// assets/models/character/creator.json says that choice is, on every part, and a name not
// on offer must be refused.
//
//   node tools/sandbox/creator.ts
//
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const ROOT = resolve(import.meta.dirname, "../..");

type Drawn = { colour: [number, number, number]; map: string | null; parts: number };
type State = {
  body: string;
  features: Record<string, string | null> | null;
  offered: { hair: string[]; beard: string[]; hair_colour: string[]; skin: string[]; eyes: string[] };
  pieces: string[];
  drawn: { skin?: Drawn[]; hair?: Drawn[]; eyes?: Drawn[] };
};
type Data = { skin: { name: string; colour: number[] }[]; hair: { name: string; colour: number[] }[]; eyes: { name: string; map: string }[] };

const data = JSON.parse(readFileSync(resolve(ROOT, "assets/models/character/creator.json"), "utf8")) as Data;

function remote(...words: string[]): { ok: boolean; out: string } {
  const ran = spawnSync("node", [resolve(ROOT, "tools/sandbox/remote.ts"), ...words], { cwd: ROOT, encoding: "utf8" });
  return { ok: ran.status === 0, out: `${ran.stdout}${ran.stderr}` };
}
function call(method: string, params?: unknown): string {
  const ran = remote("call", method, ...(params === undefined ? [] : [JSON.stringify(params)]));
  if (!ran.ok) throw new Error(`${method} failed:\n${ran.out}`);
  return ran.out;
}
const state = () => JSON.parse(call("creator.state")) as State;
const sleep = (seconds: number) => new Promise((done) => setTimeout(done, seconds * 1000));

/// Sets some features and waits until the body shows them: `wrong` says what is still not
/// as asked, or nothing once it is.
async function show(what: string, features: Record<string, string | null>, wrong: (now: State) => string) {
  call("creator.set", features);
  let last = "";
  for (let waited = 0; waited < 30; waited += 0.5) {
    await sleep(0.5);
    last = wrong(state());
    if (!last) return;
  }
  throw new Error(`${what}: ${last}, 30 s after it was chosen`);
}

/// What is wrong with how a role's parts are drawn, against the colour or the map they
/// should all have.
function drawn(now: State, role: "skin" | "hair" | "eyes", colour?: number[], map?: string): string {
  const with_ = now.drawn[role] ?? [];
  if (with_.length === 0) return `no part of the body is ${role}`;
  if (with_.length > 1) return `the ${role} is drawn ${with_.length} ways: ${JSON.stringify(with_)}`;
  const [one] = with_;
  if (colour && one.colour.some((channel, at) => Math.abs(channel - colour[at]) > 0.002)) return `the ${role} is multiplied by ${one.colour.map((c) => c.toFixed(3))}, not ${colour}`;
  if (map && one.map !== map) return `the ${role}' map is ${one.map}, not ${map}`;
  return "";
}

const file = (piece: string) => `/${piece}.gltf#Scene0`;
let failed = "";
try {
  const started = remote("start", "--real-time", "--", "--experiment", "play", "--fresh", "--save", "shots/creator.ron", "--offscreen");
  if (!started.ok) throw new Error(`the game did not start:\n${started.out}`);
  let first: State | undefined;
  for (let waited = 0; waited < 120 && !first; waited += 1) {
    const asked = remote("call", "creator.state");
    if (asked.ok) first = JSON.parse(asked.out) as State;
    else await sleep(1);
  }
  if (!first) throw new Error("the player never stood on a wardrobe body: creator.state was refused for two minutes");
  const { offered } = first;
  for (const [what, names, least] of [["hairstyles", offered.hair, 4], ["skin tones", offered.skin, 4], ["hair colours", offered.hair_colour, 4], ["eye colours", offered.eyes, 3]] as const) {
    if (names.length < least) throw new Error(`only ${names.length} ${what} are on offer; D111 wants a choice of at least ${least}`);
  }

  // Each hairstyle is the one head of hair shown; none is a bare head.
  for (const style of offered.hair) {
    await show(`the hairstyle ${style}`, { hair: style }, (now) => {
      const hair = now.pieces.filter((piece) => offered.hair.some((each) => piece.endsWith(file(each))));
      return hair.length === 1 && hair[0].endsWith(file(style)) ? "" : `the body shows the hair ${JSON.stringify(hair)}`;
    });
  }
  await show("no hair", { hair: null }, (now) => (now.pieces.some((piece) => offered.hair.some((each) => piece.endsWith(file(each)))) ? "hair is still shown" : ""));
  for (const beard of offered.beard) {
    await show(`the beard ${beard}`, { beard, hair: offered.hair[0] }, (now) => (now.pieces.some((piece) => piece.endsWith(file(beard))) ? "" : `the body shows no ${beard}`));
  }
  await show("no beard", { beard: null }, (now) => (now.pieces.some((piece) => offered.beard.some((each) => piece.endsWith(file(each)))) ? "a beard is still shown" : ""));
  console.log(`✓ hair: each of ${offered.hair.length} hairstyles and ${offered.beard.length} beards is shown alone when chosen, and none when none is`);

  for (const tone of data.skin) await show(`the skin tone ${tone.name}`, { skin: tone.name }, (now) => drawn(now, "skin", tone.colour));
  for (const tone of data.hair) await show(`the hair colour ${tone.name}`, { hair_colour: tone.name }, (now) => drawn(now, "hair", tone.colour));
  for (const eyes of data.eyes) await show(`the eye colour ${eyes.name}`, { eyes: eyes.name }, (now) => drawn(now, "eyes", undefined, eyes.map));
  const last = state();
  const parts = (role: "skin" | "hair" | "eyes") => last.drawn[role]![0].parts;
  console.log(`✓ colour: each of ${data.skin.length} skin tones, ${data.hair.length} hair colours and ${data.eyes.length} eye colours is on every part of its kind (${parts("skin")} of skin, ${parts("hair")} of hair, ${parts("eyes")} of eyes)`);

  // What is not on offer is refused, saying what is.
  const refused = remote("call", "creator.set", JSON.stringify({ eyes: "Purple" }));
  if (refused.ok || !refused.out.includes(data.eyes[0].name)) throw new Error(`an eye colour not on offer was not refused with those that are: ${refused.out}`);
  if (state().features?.eyes !== data.eyes.at(-1)!.name) throw new Error("a refused choice changed the player's eyes");
  console.log("✓ refused: a colour not on offer changes nothing and is told which are");
} catch (problem) {
  failed = String(problem instanceof Error ? problem.message : problem);
} finally {
  remote("stop");
}
if (failed) {
  console.error(`✗ creator: ${failed}`);
  process.exit(1);
}
console.log("✓ creator: every choice D111 names shows on the player's body as its data says");
