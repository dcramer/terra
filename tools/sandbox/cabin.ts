#!/usr/bin/env node
// Builds a cabin from plan to door in the play experiment and walks in (D69, D139; step 4 of
// docs/building-plan.md), checking every step by the base's state, not only by picture:
// a claim stone; a room's plan of log walls on a stone floor, a doorway and a window cut
// into it and a gable roof laid over it, all for nothing; then built, what goes in next
// each time as holding the hammer does, the floor first, the walls course by course, the
// roof by stages, taking exactly what the kits say from the pack; a cube taken out and the
// taking undone; a refusal where the roof rests on a wall; a door and a window fitted; the
// shut door keeping the player out, and opened with the interact key, letting them in; and
// the game saved and loaded again, the base the same.
//
// One running game, driven live over the remote protocol (tools/sandbox, the `build.*`
// methods in crates/terra_game/src/build/remote.rs). Screenshots of the plan, the cabin
// going up and the cabin finished go to shots/cabin-*.png; a failing step says what the
// game said and leaves shots/cabin-<step>.png.
//
// Runs on the Windows GPU from WSL, and on this machine's own elsewhere; --here runs on
// this machine under WSL too. --until STEP stops after that step. Node 22.18 or later; no
// dependencies. `cargo xtask cabin` runs it.

import * as game from "./client.ts";

const SAVE = "cabin.ron";
const here = process.argv.includes("--here");
const until = process.argv[process.argv.indexOf("--until") + 1];
const windows = game.onWsl && !here;
class Done extends Error {}
let failed = false;

type Part = { id: number; kind: string; cubes: number; built: number; started: number; openings: number; done: boolean; min: number[]; max: number[] };
type Roof = { id: number; lowest: number; highest: number; covered: boolean };
type Fitted = { kind: string; min: number[]; max: number[]; open: boolean; at: number[] };
type Base = { origin: number[]; cubes: number; planned: number; openings: number; floating: number; room_tiles: number; parts: Part[]; roofs: Roof[]; props: Fitted[]; offcuts: string; since: number };
type Answer = { done: boolean; what: string; took?: string; gave?: string; refused?: string };

const base = async () => (await game.call("build.state")) as Base;
const act = async (method: string, params?: unknown) => (await game.call(method, params)) as Answer;
const acts = async (method: string, params?: unknown) => (await game.call(method, params)) as Answer[];
/// A place in the base, in metres east, up and south of its claim stone: in the world, and
/// east and south of the start.
const point = async (at: [number, number, number]) => (await game.call("build.point", { at })) as { world: number[]; from_start: number[] };
const eastSouth = (from: number[]): [number, number] => [from[0], from[2]];
const carried = async (item: string) => (await game.state()).carrying[item] ?? 0;

function check(that: boolean, what: string) {
  if (!that) throw new Error(what);
}

async function step(name: string, act: () => Promise<string>) {
  try {
    console.log(`ok    ${name}: ${await act()}`);
  } catch (error) {
    if (error instanceof Done) throw error;
    console.log(`FAIL  ${name}: ${(error as Error).message}`);
    await game.shot(`shots/cabin-${name}.png`).catch(() => {});
    failed = true;
    throw error;
  }
  if (name === until) throw new Done();
}

/// The cabin, in the base's tiles (half a metre a side, on the ground storey): five metres
/// east to west and four north to south outside its walls, two metres from the stone.
const MIN: [number, number, number] = [4, 0, 4];
const MAX: [number, number, number] = [13, 0, 11];
const DOOR = [
  [8, 0, 11],
  [9, 0, 11],
];
const WINDOW = [
  [13, 0, 7],
  [13, 0, 8],
];
/// Where the builder stands, south of the cabin, in metres in the base's own space.
const BUILDER: [number, number, number] = [4.5, 0, 8.6];

/// A picture of the cabin from the south-east, a little above it, in shots/cabin-NAME.png;
/// the camera is let go again after.
async function picture(name: string, distance = 11) {
  const target = await point([4.5, 1.6, 4]);
  const eye = await point([4.5 + distance * 0.75, 4.5, 4 + distance * 0.9]);
  await game.call("sandbox.look", { at: target.world, from: eye.world });
  await game.waitSeconds(2);
  await game.shot(`shots/cabin-${name}.png`);
  await game.call("sandbox.release");
}

async function build() {
  console.log(`Starting the game${windows ? " on Windows" : ""}…`);
  await game.start(["--experiment", "play", "--fresh", "--save", SAVE, "--time", "10"], { windows, copy: "cabin" });

  await step("claim", async () => {
    // On the meadow north of the lake, facing north, the stone set down ahead.
    await game.warp([0, -8], [0, -14]);
    await game.give("stone:4");
    await game.hold("Hammer");
    await game.press("b@20", "1@30", "enter@40", "e@70:80");
    const line = await game.waitFor("Placed a Claim stone", { timeout: 60 });
    return line.text;
  });

  await step("plan", async () => {
    const stand = await point(BUILDER);
    await game.warp(eastSouth(stand.from_start), eastSouth((await point([4.5, 0, 4])).from_start));
    const room = await act("build.lay", { what: "room", min: MIN, max: MAX, kind: "logs", floor: "stone" });
    check(room.done, `the room's plan was refused: ${room.refused}`);
    const door = await act("build.opening", { tiles: DOOR, door: true });
    check(door.done, `the doorway was refused: ${door.refused}`);
    const window = await act("build.opening", { tiles: WINDOW, door: false });
    check(window.done, `the window was refused: ${window.refused}`);
    const roof = await act("build.lay", { what: "roof", min: MIN, max: MAX, kind: "logs", style: "Gable", pitch: 0.6 });
    check(roof.done, `the roof's plan was refused: ${roof.refused}`);
    const now = await base();
    // A floor, four walls and a roof; the flat roof the room shortcut laid gave way.
    check(now.parts.length === 5, `${now.parts.length} parts, not 5`);
    check(now.parts.every((part) => part.built === 0 && part.started === 0), "something of the plan is in");
    check(now.openings === 12, `${now.openings} open cubes, not a doorway of 8 and a window of 4`);
    check(now.room_tiles === 0, "a plan closes a room in");
    check(now.roofs.length === 1 && now.roofs[0].highest === 0, "the roof is not planned");
    check(!room.took && !roof.took, "laying a plan took something");
    await picture("plan");
    return `${now.planned} cubes planned in ${now.parts.length} parts, a doorway, a window and a roof, for nothing`;
  });

  // What the cabin takes, by the kits: a stone floor of 80 cubes at a quarter of a Stone;
  // log walls of 192 cubes less 12 open, each in two courses of a sixteenth of a Wood; a
  // roof of 80 cells in three stages at a thirty-second of a Wood: 20 Stone and 30 Wood.
  // A pack holds about twenty raw things, so they are brought as they are needed.
  const RAW = ["Wood", "Stone", "Fibre"];
  const emptyPack = async () => {
    const carrying = (await game.state()).carrying;
    const have = RAW.filter((name) => carrying[name]);
    if (have.length) await game.take(...have.map((name) => name.toLowerCase()));
  };

  await step("floor", async () => {
    await emptyPack();
    await game.give("stone:20");
    // What goes in first is the floor, whatever is aimed at: it holds the rest up.
    const [first] = await acts("build.fill", { steps: 1 });
    check(first?.done === true, `the first step was refused: ${first?.refused}`);
    check(first.what.includes("stone"), `the first step was ${first.what}, not the stone floor`);
    // Taken out again, it gives back what it took; undone, it is taken again.
    const floor = (await base()).parts.find((part) => part.kind === "stone")!;
    const [out] = await acts("build.take_apart", { at: floor.min, steps: 1 });
    check(out?.done === true && out.gave === first.took, `taking it out gave ${out?.gave}, not ${first.took}: ${out?.refused}`);
    const undone = await act("build.undo");
    check(undone.done, `the undo was refused: ${undone.refused}`);
    const rest = await acts("build.fill", { steps: 79 });
    const refused = rest.find((each) => !each.done);
    check(!refused, `refused: ${refused?.refused} (${refused?.what})`);
    const now = await base();
    const after = now.parts.find((part) => part.kind === "stone")!;
    check(after.done, `the floor is not done: ${after.built} of ${after.cubes}`);
    const stone = 20 - (await carried("Stone"));
    check(stone === 20 && now.offcuts === "", `the floor took ${stone} Stone, offcuts ${now.offcuts}, not 20 and none`);
    return `${first.what} took ${first.took}; taken out it gave ${out.gave}; ${undone.what}; the floor took exactly ${stone} Stone`;
  });

  await step("walls", async () => {
    await emptyPack();
    await game.give("wood:20");
    // The walls to a metre and a half, course by course, all four together.
    const done = await acts("build.fill", { steps: 24 });
    const refused = done.find((each) => !each.done);
    check(!refused, `refused: ${refused?.refused} (${refused?.what})`);
    const now = await base();
    const walls = now.parts.filter((part) => part.kind === "logs");
    check(walls.every((wall) => wall.built > 0), "a wall has nothing in it");
    check(now.floating === 0, `${now.floating} cubes float`);
    // The roof can't go on half-built walls: every cube under its eaves holds it.
    const early = await act("build.op", { ops: [{ RoofStage: { roof: now.roofs[0].id, cells: [[MIN[0] + 1, MIN[2] + 1]], up: true } }] });
    check(early.done === false, "a roof went on before the walls under it were up");
    await picture("going-up");
    return `${done.length} courses in; walls ${walls.map((wall) => `${wall.built}/${wall.cubes - wall.openings}`).join(", ")}; the roof too early: "${early.refused}"`;
  });

  await step("roof", async () => {
    // Until the wood runs out, which is refused and said; then the rest.
    const first = await acts("build.fill", { steps: 400 });
    const short = first.find((each) => !each.done);
    check(short?.refused?.startsWith("You need") === true, `stopped for ${short?.refused ?? "nothing"}, not for want of wood`);
    await game.give("wood:20");
    const done = await acts("build.fill", { steps: 400 });
    const refused = done.find((each) => !each.done);
    check(!refused, `refused: ${refused?.refused} (${refused?.what})`);
    const now = await base();
    check(now.parts.every((part) => part.done), `not all built: ${JSON.stringify(now.parts)}`);
    check(now.roofs.every((roof) => roof.covered), "the roof is not covered");
    check(now.room_tiles > 0, "the finished cabin closes no room in");
    const wood = 40 - (await carried("Wood"));
    check(wood === 30 && now.offcuts === "", `took ${wood} Wood, offcuts ${now.offcuts}, not 30 and none`);
    return `${first.length + done.length} more steps, one refused: "${short!.refused}"; ${now.room_tiles} tiles indoors; the walls and roof took exactly ${wood} Wood`;
  });

  await step("refused", async () => {
    // A wall's top holds the roof up, so it cannot come out, and says so.
    const wall = (await base()).parts.find((part) => part.kind === "logs" && part.min[2] === MAX[2])!;
    const out = await act("build.op", { ops: [{ Empty: { cubes: [[wall.min[0], 5, wall.min[2]]] } }] });
    check(out.done === false, "a cube the roof rests on came out");
    return `taking out a cube under the roof's eaves: "${out.refused}"`;
  });

  await step("fit", async () => {
    await game.give("fibre:1");
    const door = await act("build.fit", { at: DOOR[0], door: true });
    check(door.done, `the door was refused: ${door.refused}`);
    const window = await act("build.fit", { at: [WINDOW[0][0], 2, WINDOW[0][2]], door: false });
    check(window.done, `the window was refused: ${window.refused}`);
    const now = await base();
    check(now.props.length === 2 && now.props.every((prop) => !prop.open), "not a shut door and a window");
    return `a door (${door.took}) and a window (${window.took})`;
  });

  await step("shut", async () => {
    // Outside the door, facing it, walking north: the shut door stops the player.
    const outside = await point([4.5, 0, 7.6]);
    const inside = await point([4.5, 0, 3.5]);
    await game.warp(eastSouth(outside.from_start), eastSouth(inside.from_start));
    await game.press("w:90");
    await game.waitSeconds(2);
    const at = (await game.state()).at;
    check(at[1] > eastSouth(outside.from_start)[1] - 1.4, `the player went through the shut door to ${at}`);
    return `stopped at ${at.map((n) => n.toFixed(2))}`;
  });

  await step("open", async () => {
    // A step back from the door, out from under the eaves, to look at it and use it.
    const door = (await base()).props.find((prop) => prop.kind === "door")!;
    const back = await point([4.5, 0, 8.3]);
    await game.warp(eastSouth(back.from_start), eastSouth(door.at));
    await game.aim(eastSouth(door.at), 1.1);
    await game.markNow();
    await game.press("f");
    const line = await game.waitFor("Build: open the door: done", { timeout: 20 });
    check((await base()).props.find((prop) => prop.kind === "door")!.open, "the door is not open");
    return line.text;
  });

  await step("in", async () => {
    // Straight at the doorway, as before, and in.
    const outside = await point([4.5, 0, 7.6]);
    const inside = await point([4.5, 0, 3.5]);
    await game.warp(eastSouth(outside.from_start), eastSouth(inside.from_start));
    await game.press("w:120");
    await game.waitSeconds(3);
    const now = await game.state();
    // Inside the walls: between them east to west, north of the south wall.
    const [west, east] = [(await point([2.5, 0, 0])).from_start[0], (await point([6.5, 0, 0])).from_start[0]];
    const south = (await point([0, 0, 5.5])).from_start[2];
    check(now.at[0] > west && now.at[0] < east && now.at[1] < south, `the player is at ${now.at}, not inside`);
    check(now.indoors, "the game does not count the player indoors");
    // The cabin from the south-east, its door open.
    const look = await point([4.5, 1.2, 4.5]);
    const eye = await point([9.5, 3.0, 11.5]);
    await game.call("sandbox.look", { at: look.world, from: eye.world });
    await game.waitSeconds(2);
    await game.shot("shots/cabin-finished.png");
    await game.call("sandbox.release");
    return `inside at ${now.at.map((n) => n.toFixed(2))}, indoors`;
  });

  await step("reload", async () => {
    const before = await base();
    await game.stop();
    await game.start(["--experiment", "play", "--save", SAVE, "--time", "10"], { windows, copy: "cabin" });
    const after = await base();
    const same = (base: Base) => JSON.stringify({ parts: base.parts, roofs: base.roofs, props: base.props.map(({ at: _, ...prop }) => prop), cubes: base.cubes });
    check(same(after) === same(before), `the base loaded is not the base saved:\n${same(before)}\n${same(after)}`);
    return `saved as a snapshot and ${before.since} operations, loaded the same`;
  });
}

try {
  await build();
} catch (error) {
  if (!(error instanceof Done) && !failed) {
    console.log(`FAIL  ${(error as Error).message} (see shots/remote.log)`);
    failed = true;
  }
} finally {
  await game.stop().catch(() => {});
}
process.exit(failed ? 1 : 0);
