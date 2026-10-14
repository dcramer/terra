#!/usr/bin/env node
// The construction demo's pictures, the same every time (D69, D229): the village and the
// player's own building from fixed views, so anyone can make them again and a change to how
// building looks is judged on the same views.
//
// First `cargo xtask cabin` builds the player's cabin, deck and stairs from plan to doorway
// and saves the game (tools/sandbox/cabin.ts); then that game is loaded again, the player
// set down at the end of the village's street, and the camera taken round fixed vantages:
// the street at eye height, the green, the street from its east end, the village from a
// rise and from above, each building of the village close up (a log cabin, a stone house,
// stone under timber with its outside stair, logs on a stone plinth, a hewn L, log walls
// with stone ends, a log T hall, a stone U farmstead, a hewn house under thatch, the
// woodshed and the lookout deck), a stone house going up beside the cabin with the rest of
// it a plan of ghosts, and the cabin built. The village says where to stand for its views
// (`Settlement vantage: …` and `Settlement: …: its door …` in the log), so the views follow
// the village wherever its seed puts it.
//
//   node tools/sandbox/demo.ts [--skip-cabin] [--hour 15] [--out shots/demo]
//
// Writes <out>/NN-<view>.png and, where ImageMagick is installed, <out>/sheet.jpg. Runs on
// the Windows GPU from WSL, as the cabin script does. The clock keeps running; it is set to
// the hour before each picture, so the light is the same each time. Node 22.18 or later; no
// dependencies. `cargo xtask demo` runs it.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync } from "node:fs";
import { resolve } from "node:path";
import * as game from "./client.ts";

const arg = (name: string, fallback: string) => {
  const at = process.argv.indexOf(name);
  return at > 0 ? process.argv[at + 1] : fallback;
};
const hour = Number(arg("--hour", "15"));
const out = resolve(game.ROOT, arg("--out", "shots/demo"));
const windows = game.onWsl;
mkdirSync(out, { recursive: true });

if (!process.argv.includes("--skip-cabin")) {
  console.log("demo: building the cabin first (cargo xtask cabin)…");
  const cabin = spawnSync("node", ["tools/sandbox/cabin.ts"], { cwd: game.ROOT, stdio: "inherit" });
  if (cabin.status !== 0) {
    console.error("demo: the cabin script failed, so there is no cabin to show");
    process.exit(1);
  }
}

type Look = { at?: string | number[]; from?: number[]; vantage?: number[]; distance?: number; around?: number; height?: number; aim?: number };
const shots: string[] = [];

/// Holds the camera, sets the hour, lets detail and TAA settle, and saves a picture.
async function picture(name: string, look: Look) {
  await game.call("world.mutate_resources", { resource: "terra_world::daylight::Clock", path: ".hour", value: hour });
  await game.call("sandbox.look", look);
  await game.waitSeconds(3);
  const file = resolve(out, `${String(shots.length + 1).padStart(2, "0")}-${name}.png`);
  await game.shot(file);
  shots.push(file);
  console.log(`demo: ${file}`);
}

/// Where the game says something is: every line of the run's record containing `text`.
const lines = async (text: string) => (await game.said(0, text, 200)).lines.map((line) => line.text);
const numbers = (text: string) => (text.match(/-?\d+(\.\d+)?/g) ?? []).map(Number);

async function main() {
  console.log(`demo: loading the cabin's game${windows ? " on Windows" : ""}…`);
  // The cabin script's game, from its own folder: the cabin, the deck and the claim saved.
  await game.start(["--experiment", "play", "--save", "cabin.ron", "--time", String(hour)], { windows, copy: "cabin" });
  await game.waitFor("Settlement vantage: above", { since: 0, timeout: 120 });

  // The village's views, as it gives them.
  const vantages = new Map<string, number[]>();
  for (const line of await lines("Settlement vantage: ")) {
    const [, name, values] = /Settlement vantage: (.*): --vantage (.*)$/.exec(line) ?? [];
    if (name) vantages.set(name, values.split(",").map(Number));
  }
  const doors = new Map<string, number[]>();
  for (const line of await lines("its door")) {
    const [, name] = /Settlement: (.*): its door/.exec(line) ?? [];
    if (name && !name.includes("garden wall")) doors.set(name, numbers(line.slice(line.indexOf("its door"))));
  }
  if (!vantages.size || !doors.size) throw new Error("the village did not say where it is");

  // The player a third of the way along the street from its west end, looking along it,
  // so the street's picture has someone in it and the camera is clear of them.
  const street = vantages.get("street")!;
  const along = (a: number, b: number) => a + (b - a) * 0.35;
  await game.warp([along(street[0], street[3]), along(street[2], street[5])], [street[3], street[5]]);

  for (const name of ["street", "green", "street east", "rise", "above"]) {
    const vantage = vantages.get(name);
    if (vantage) await picture(name.replace(" ", "-"), { vantage });
  }
  // Each building close up, from a front corner, as far off as it is big.
  for (const name of doors.keys()) {
    const big = /hall|two storeys|L-shaped/.test(name);
    // A farmstead's door is in its yard, between its wings: seen from over the yard's mouth.
    const yard = /farmstead/.test(name);
    const small = /woodshed|deck/.test(name);
    await picture(name.split(" ").slice(0, 6).join("-"), {
      at: `Settlement: ${name}`,
      distance: yard ? 15 : big ? 16 : small ? 10 : 14,
      around: yard ? 0 : big ? 20 : 35,
      height: yard ? 8 : big ? 7 : 3,
      aim: 1.8,
    });
  }

  // A stone house going up north of the cabin: its plan laid, its footing, floor and the
  // first courses of its walls in, the rest ghosts with the next part lit.
  const point = async (at: number[]) => (await game.call("build.point", { at })) as { world: number[]; from_start: number[] };
  const stand = await point([4.5, 0, -3]);
  await game.warp([stand.from_start[0], stand.from_start[2]]);
  await game.give("stone:120", "wood:40");
  await game.hold("Hammer");
  const room = (await game.call("build.lay", { what: "room", min: [4, 0, -16], max: [13, 0, -9], kind: "stone" })) as { done: boolean; refused?: string };
  if (room.done) {
    await game.call("build.lay", { what: "roof", min: [4, 0, -16], max: [13, 0, -9], kind: "stone", style: "Hip", pitch: 0.7 });
    // A pack holds about twenty raw things, so stone is brought as it is needed: about
    // two hundred steps in, a quarter of a Stone each.
    for (let round = 0; round < 4; round++) {
      await game.give("stone:13");
      await game.call("build.fill", { steps: 50 });
    }
    const target = await point([4.5, 1.2, -6]);
    const eye = await point([13.5, 4.5, -1.5]);
    await picture("going-up", { at: target.world, from: eye.world });
  } else {
    console.log(`demo: the stone house's plan was refused: ${room.refused}`);
  }

  // The cabin as the cabin script built it, its deck and stairs beside it.
  const cabin = await point([9.5, 1.4, 6]);
  const eye = await point([19.5, 5.0, 17.5]);
  await picture("cabin", { at: cabin.world, from: eye.world });
  await game.call("sandbox.release");
}

try {
  await main();
} finally {
  await game.stop().catch(() => {});
}
const tile = (tool: string[]) => spawnSync(tool[0], [...tool.slice(1), ...shots, "-tile", "4x", "-geometry", "720x405+3+3", "-background", "#222", resolve(out, "sheet.jpg")], { encoding: "utf8" });
const made = [["magick", "montage"], ["montage"]].some((tool) => tile(tool).status === 0);
if (made && existsSync(resolve(out, "sheet.jpg"))) console.log(`demo: ${resolve(out, "sheet.jpg")}`);
console.log(`demo: ${shots.length} pictures in ${out}`);
