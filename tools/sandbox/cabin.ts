#!/usr/bin/env node
// Builds a cabin from plan to doorway in the play experiment and walks in (D69, D139; step 4 of
// docs/building-plan.md), checking every step by the base's state, not only by picture:
// a claim stone; a room's plan of log walls on a stone floor, on a stone footing down to the
// ground (D194), a doorway and a window cut into it and "add a roof" laying a gable of
// shakes over it, roof cubes on gable ends of logs (D241), all for nothing; then built,
// what goes in next each time as holding the hammer does, the footing first and whole
// before anything else, then the floor, the walls course by course, the roof a layer at a
// time a stage at a time, taking exactly what the kits say from the pack; a cube taken out
// and the taking undone; a refusal where the roof rests on a wall; a second window cut into the
// built wall, a course at a time from under the logs over it, giving the logs back; the
// player walking in through the open doorway, as no door or window is fitted for now (the
// developers, 2026-10-13); the ground outside the cabin as it was before it was planned; a
// deck a storey up on four posts beside it, reached by a flight of stairs (D193), built,
// and walked up onto; stone picked in the hammer, a wall laid and built in it, a cube in
// its middle changed to logs in place and an outer corner slope laid on its end (D231,
// D232); a shed whose roof is built by hand, a cube at a time cut to slopes, over half of
// it, a storey added over its other half and "add a roof" laying a hip over that (D226,
// D241); and the game saved and loaded again, the base the same.
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

type Part = { id: number; kind: string; cubes: number; shaped: number; built: number; started: number; openings: number; done: boolean; min: number[]; max: number[] };
type Roof = { cubes: number; built: number; stages: number[]; shapes: Record<string, number> };
type Fitted = { kind: string; min: number[]; max: number[]; open: boolean; at: number[] };
type Base = { origin: number[]; cubes: number; planned: number; openings: number; floating: number; room_cubes: number; rooms: number; parts: Part[]; roofs: Roof; props: Fitted[]; offcuts: string; since: number; player: number[] | null };
type Answer = { done: boolean; what: string; took?: string; gave?: string; refused?: string; cubes?: number[][] };

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
/// A window cut into the north wall once it is built.
const CUT = [
  [8, 0, 4],
  [9, 0, 4],
];
/// The columns round the cabin, a tile out from its walls: the ground there is not changed
/// by building (D131, D193).
const AROUND: [number, number][] = [];
for (let x = MIN[0] - 1; x <= MAX[0] + 1; x++) AROUND.push([x, MIN[2] - 1], [x, MAX[2] + 1]);
for (let z = MIN[2]; z <= MAX[2]; z++) AROUND.push([MIN[0] - 1, z], [MAX[0] + 1, z]);
/// A deck a storey up east of the cabin, on a post at each corner, in tiles; and the foot of
/// the flight of stairs up to it from the south, a metre wide, rising north.
const POSTS = [
  [17, 0, 5],
  [22, 0, 5],
  [17, 0, 10],
  [22, 0, 10],
];
const DECK_MIN = [17, 1, 5];
const DECK_MAX = [22, 1, 10];
const FLIGHT_FOOT = [18, 0, 17];
/// A shed west of the stone wall, in tiles, and where the storey over its east half starts.
const SHED_MIN = [-24, 0, 12];
const SHED_MAX = [-15, 0, 17];
const STOREY_X = -19;
const groundAround = async () => (await game.call("build.ground", { columns: AROUND })) as number[];
let groundBefore: number[] = [];

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
    groundBefore = await groundAround();
    const room = await act("build.lay", { what: "room", min: MIN, max: MAX, kind: "logs", floor: "stone" });
    check(room.done, `the room's plan was refused: ${room.refused}`);
    const door = await act("build.opening", { tiles: DOOR, door: true });
    check(door.done, `the doorway was refused: ${door.refused}`);
    const window = await act("build.opening", { tiles: WINDOW, door: false });
    check(window.done, `the window was refused: ${window.refused}`);
    // "Add a roof" over the room aimed into, in the roof its log walls take: shakes.
    const inside = [MIN[0] + 4, 0, MIN[2] + 3];
    const roof = await act("build.lay", { what: "roof", over: inside, kind: "logs", style: "Gable" });
    check(roof.done, `the roof's plan was refused: ${roof.refused}`);
    const now = await base();
    // A footing, a floor, four walls, the gable ends and the roof's cubes; the flat roof the
    // room shortcut laid gave way. The footing comes first, in stone, under every column of
    // the walls; the floor inside them.
    check(now.parts.length === 8, `${now.parts.length} parts, not 8`);
    const [footing, floor] = now.parts;
    check(footing.kind === "stone" && footing.cubes >= 32, `the first part is ${footing.cubes} cubes of ${footing.kind}, not a stone footing under 32 columns`);
    check(floor.kind === "stone" && floor.cubes === 48, `the floor is ${floor.cubes} cubes of ${floor.kind}, not 48 of stone`);
    check(now.parts.every((part) => part.built === 0 && part.started === 0), "something of the plan is in");
    check(now.openings === 12, `${now.openings} open cubes, not a doorway of 8 and a window of 4`);
    check(now.room_cubes === 0, "a plan closes a room in");
    const shakes = now.parts.find((part) => part.kind === "shakes");
    const gables = now.parts.filter((part) => part.kind === "logs" && part.min[1] >= 6);
    check(shakes?.cubes === 80 && now.roofs.cubes === 80 && now.roofs.built === 0, `the roof is ${JSON.stringify(now.roofs)}, not 80 cubes of shakes planned`);
    check(shakes!.shaped === 80 && now.roofs.shapes["Slope"] === 80, `the roof's cubes are ${JSON.stringify(now.roofs.shapes)}, not 80 slopes`);
    check(gables.length === 1 && gables[0].cubes === 24, `the gable ends are ${JSON.stringify(gables)}, not 24 cubes of logs`);
    check(!room.took && !roof.took, "laying a plan took something");
    await picture("plan");
    return `${now.planned} cubes planned in ${now.parts.length} parts, a footing of ${footing.cubes} cubes reaching ${footing.min[1]} down, a doorway, a window and a roof, for nothing`;
  });

  // What the cabin takes, by the kits: a stone floor of 80 cubes at a quarter of a Stone;
  // log walls of 192 cubes less 12 open, and gable ends of 24, each in two courses of a
  // sixteenth of a Wood; a roof of 80 cubes of shakes in four stages at a thirty-second of
  // a Wood: 20 Stone and 35 and a half Wood.
  // A pack holds about twenty raw things, so they are brought as they are needed.
  const RAW = ["Wood", "Stone", "Fibre"];
  const emptyPack = async () => {
    const carrying = (await game.state()).carrying;
    const have = RAW.filter((name) => carrying[name]);
    if (have.length) await game.take(...have.map((name) => name.toLowerCase()));
  };

  await step("footing", async () => {
    await emptyPack();
    const footing = (await base()).parts[0];
    const stone = Math.ceil(footing.cubes / 4);
    await game.give(`stone:${stone}`);
    // What goes in first is the footing, whatever is aimed at: it holds the rest up.
    const [first] = await acts("build.fill", { steps: 1 });
    check(first?.done === true, `the first step was refused: ${first?.refused}`);
    check(first.what.includes("part 0"), `the first step was ${first.what}, not the footing`);
    // Taken out again, it gives back what it took; undone, it is taken again.
    const [out] = await acts("build.take_apart", { at: first.cubes![0], steps: 1 });
    check(out?.done === true && out.gave === first.took, `taking it out gave ${out?.gave}, not ${first.took}: ${out?.refused}`);
    const undone = await act("build.undo");
    check(undone.done, `the undo was refused: ${undone.refused}`);
    // The rest of it, a cube at a time, and nothing else until it is all in.
    for (let i = 1; i < footing.cubes; i++) {
      const [done] = await acts("build.fill", { steps: 1 });
      check(done?.done === true, `refused: ${done?.refused} (${done?.what})`);
      const now = await base();
      const early = now.parts.slice(1).find((part) => part.built + part.started > 0);
      check(!early, `part ${early?.id} went in before the footing was done`);
    }
    const now = await base();
    check(now.parts[0].done, `the footing is not done: ${now.parts[0].built} of ${now.parts[0].cubes}`);
    check(now.floating === 0, `${now.floating} cubes float`);
    const used = stone - (await carried("Stone"));
    check(used === stone, `the footing took ${used} Stone, not ${stone}`);
    return `${first.what} took ${first.took}; taken out it gave ${out.gave}; ${undone.what}; the footing's ${footing.cubes} cubes in first, from ${footing.min[1]} up, for ${used} Stone`;
  });

  await step("floor", async () => {
    await game.give("stone:12");
    const done = await acts("build.fill", { steps: 48 });
    const refused = done.find((each) => !each.done);
    check(!refused, `refused: ${refused?.refused} (${refused?.what})`);
    check(done.every((each) => each.what.includes("part 1")), "something but the floor went in");
    const now = await base();
    check(now.parts[1].done, `the floor is not done: ${now.parts[1].built} of ${now.parts[1].cubes}`);
    await emptyPack();
    return `the floor's 48 cubes, for the rest of ${12 + Math.ceil(now.parts[0].cubes / 4)} Stone in all, offcuts ${now.offcuts || "none"}`;
  });

  await step("walls", async () => {
    await emptyPack();
    await game.give("wood:20");
    // The walls to a metre and a half, course by course, all four together.
    const done = await acts("build.fill", { steps: 24 });
    const refused = done.find((each) => !each.done);
    check(!refused, `refused: ${refused?.refused} (${refused?.what})`);
    const now = await base();
    // The four walls, not the gable ends over them, which wait for their tops.
    const walls = now.parts.filter((part) => part.kind === "logs" && part.min[1] < 6);
    check(walls.length === 4 && walls.every((wall) => wall.built > 0), `a wall has nothing in it: ${JSON.stringify(walls)}`);
    check(now.floating === 0, `${now.floating} cubes float`);
    // The roof can't go on half-built walls: its cubes rest on them, and climb from those.
    const early = await act("build.op", { ops: [{ Fill: { cubes: [[MIN[0] + 2, 7, MIN[2] + 1]] } }] });
    check(early.done === false, "a roof's cube went in before the walls under it were up");
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
    check(now.roofs.built === 80 && now.roofs.stages[4] === 80, `the roof is ${JSON.stringify(now.roofs)}, not 80 cubes with their trims on`);
    check(now.room_cubes > 0, "the finished cabin closes no room in");
    check(now.floating === 0, `${now.floating} cubes float`);
    const wood = 40 - (await carried("Wood"));
    check(wood === 36 && now.offcuts.includes("1/2 Wood"), `took ${wood} Wood, offcuts ${now.offcuts}, not 36 and half a Wood over`);
    return `${first.length + done.length} more steps, one refused: "${short!.refused}"; ${now.room_cubes} cubes indoors; the walls and roof took exactly ${wood} Wood`;
  });

  await step("refused", async () => {
    // A wall's top holds the roof up, so it cannot come out, and says so.
    const wall = (await base()).parts.find((part) => part.kind === "logs" && part.min[2] === MAX[2])!;
    const out = await act("build.op", { ops: [{ Empty: { cubes: [[wall.min[0], 5, wall.min[2]]] } }] });
    check(out.done === false, "a cube the roof rests on came out");
    return `taking out a cube under the roof's eaves: "${out.refused}"`;
  });

  await step("cut", async () => {
    // A window cut into the north wall, built: marked, then carved out a course at a time
    // from under the logs over it, which stay, held by the wall either side; the logs come
    // back to the pack.
    const before = await base();
    const wood = await carried("Wood");
    const cut = await act("build.opening", { tiles: CUT, door: false });
    check(cut.done, `the cut was refused: ${cut.refused}`);
    const done = await acts("build.fill", { steps: 20 });
    const refused = done.find((each) => !each.done);
    check(!refused, `refused: ${refused?.refused} (${refused?.what})`);
    const now = await base();
    check(now.openings === before.openings + 4, `${now.openings - before.openings} cubes opened, not 4`);
    check(now.floating === 0, `${now.floating} cubes float`);
    check(now.parts.every((part) => part.done), "the cut left part of the cabin unbuilt");
    check(now.room_cubes === before.room_cubes, "the cut opened the room to the weather");
    const back = (await carried("Wood")) - wood;
    check(back >= 0, `the cut took ${-back} Wood`);
    return `${done.length} steps: ${done.map((each) => each.what).join("; ")}; ${now.offcuts || "no"} offcuts`;
  });

  await step("in", async () => {
    // Outside the doorway, facing it, walking north, and in: no door is fitted (the
    // developers, 2026-10-13), so the doorway stands open.
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
    // The cabin from the south-east, through its doorway.
    const look = await point([4.5, 1.2, 4.5]);
    const eye = await point([9.5, 3.0, 11.5]);
    await game.call("sandbox.look", { at: look.world, from: eye.world });
    await game.waitSeconds(2);
    await game.shot("shots/cabin-finished.png");
    await game.call("sandbox.release");
    return `inside at ${now.at.map((n) => n.toFixed(2))}, indoors`;
  });

  await step("ground", async () => {
    // Round the cabin, a tile out from its walls, the ground is as it was before the plan
    // was laid: building cut only within its walls, and its footing and steps stand on it.
    const after = await groundAround();
    const moved = AROUND.map((column, i) => ({ column, by: after[i] - groundBefore[i] })).filter(({ by }) => Math.abs(by) > 0.001);
    check(groundBefore.length === AROUND.length && moved.length === 0, `the ground moved round the cabin: ${JSON.stringify(moved.slice(0, 6))}`);
    const [low, high] = [Math.min(...after), Math.max(...after)];
    return `${AROUND.length} columns round it unchanged, from ${low.toFixed(2)} to ${high.toFixed(2)} m`;
  });

  await step("upstairs", async () => {
    // Laid from beside it: four posts, the deck over them, and the stairs.
    const near = await point([9.5, 0, 10]);
    await game.warp(eastSouth(near.from_start), eastSouth((await point([9.5, 0, 4])).from_start));
    const before = (await base()).parts.length;
    for (const at of POSTS) {
      const post = await act("build.lay", { what: "post", min: at, max: at, kind: "logs" });
      check(post.done, `the post at ${at} was refused: ${post.refused}`);
    }
    const deck = await act("build.lay", { what: "floor", min: DECK_MIN, max: DECK_MAX, kind: "logs" });
    check(deck.done, `the deck was refused: ${deck.refused}`);
    // Its foot the cube the ground passes through, as the hammer aims it: the ground here
    // lies below the base's floor.
    const [footGround] = (await game.call("build.ground", { columns: [[FLIGHT_FOOT[0], FLIGHT_FOOT[2]]] })) as number[];
    const foot = [FLIGHT_FOOT[0], Math.floor(footGround / 0.5), FLIGHT_FOOT[2]];
    const stairs = await act("build.lay", { what: "flight", min: foot, max: foot, kind: "logs", facing: "North", wide: 1.0 });
    check(stairs.done, `the stairs were refused: ${stairs.refused}`);
    const laid = (await base()).parts.slice(before);
    const rise = 5 - foot[1] + 1;
    const flight = laid.find((part) => part.kind === "logs" && part.shaped === 2 * rise && part.cubes > 2 * rise);
    check(flight !== undefined, `no flight of ${2 * rise} stairs over whole cubes among ${JSON.stringify(laid)}`);
    const posts = laid.filter((part) => part.kind === "logs" && part.shaped === part.cubes);
    check(posts.length === 4 && posts.every((post) => post.cubes === 6), `the posts are ${JSON.stringify(posts)}, not four of six post cubes`);
    // The deck waits for the posts it rests on: built, everything goes in in its order.
    await emptyPack();
    await game.give("wood:14", "stone:6");
    let steps = 0;
    for (let round = 0; round < 6; round++) {
      const done = await acts("build.fill", { steps: 400 });
      steps += done.filter((each) => each.done).length;
      const refused = done.find((each) => !each.done);
      if (!refused) break;
      check(refused.refused?.startsWith("You need") === true, `refused: ${refused.refused} (${refused.what})`);
      await emptyPack();
      await game.give("wood:10", "stone:6");
    }
    const now = await base();
    check(now.parts.every((part) => part.done), `not all built: ${JSON.stringify(now.parts.filter((part) => !part.done))}`);
    check(now.floating === 0, `${now.floating} cubes float`);
    await emptyPack();
    // A picture of the deck and its stairs from the south-east.
    const target = await point([9.8, 1.5, 4.5]);
    const eye = await point([16.5, 4.5, 12.5]);
    await game.call("sandbox.look", { at: target.world, from: eye.world });
    await game.waitSeconds(2);
    await game.shot("shots/cabin-deck.png");
    await game.call("sandbox.release");
    // Walked up from the south, onto the deck.
    const below = await point([9.5, 0, 11]);
    const onto = await point([9.5, 0, 3.5]);
    await game.warp(eastSouth(below.from_start), eastSouth(onto.from_start));
    await game.press("w:105");
    await game.waitSeconds(3);
    const feet = (await base()).player;
    check(feet !== null, "the player is nowhere");
    const [, up, south] = feet!;
    check(up > 2.9 && south > 2.5 && south < 5.5, `the player stands ${up.toFixed(2)} m up, ${south.toFixed(2)} m south, not on the deck`);
    await game.call("sandbox.look", { at: target.world, from: eye.world });
    await game.waitSeconds(2);
    await game.shot("shots/cabin-upstairs.png");
    await game.call("sandbox.release");
    return `${laid.length} parts laid and built in ${steps} steps: four posts, a deck and a flight of ${flight!.cubes} cubes; walked up to ${up.toFixed(2)} m`;
  });

  await step("materials", async () => {
    // Stone picked in the hammer (D232), a wall laid with no kind named, so in stone, and
    // built; a cube in the middle of it changed to logs in place, the stone out from under
    // the stone over it, given back, and the logs built in; and an outer corner slope laid
    // on the wall's end (D231) and built.
    const near = await point([-2.75, 0, 6.2]);
    await game.warp(eastSouth(near.from_start), eastSouth((await point([-2.75, 0, 4])).from_start));
    const picked = (await game.call("build.material", { kind: "stone" })) as { material: string };
    check(picked.material === "stone", `the hammer holds ${picked.material}, not stone`);
    const before = (await base()).parts.length;
    const wall = await act("build.lay", { what: "wall", min: [-8, 0, 8], max: [-3, 0, 8] });
    check(wall.done, `the wall was refused: ${wall.refused}`);
    const laid = (await base()).parts.slice(before);
    check(laid.length > 0 && laid.every((part) => part.kind === "stone"), `laid ${JSON.stringify(laid.map((part) => part.kind))}, not stone`);
    await emptyPack();
    await game.give("stone:16", "wood:2");
    const fill = async () => {
      const done = await acts("build.fill", { steps: 200 });
      const refused = done.find((each) => !each.done);
      check(!refused, `refused: ${refused?.refused} (${refused?.what})`);
      return done;
    };
    await fill();
    check((await base()).parts.every((part) => part.done), "the stone wall is not built");
    const MIDDLE = [-5, 2, 8];
    const stone = await carried("Stone");
    const change = await act("build.replace", { at: MIDDLE, kind: "logs" });
    check(change.done, `changing it was refused: ${change.refused}`);
    const marked = (await game.call("build.cube", { at: MIDDLE })) as { kind: string; pending: string };
    check(marked.pending === "Replace" && marked.kind === "stone", `marked as ${JSON.stringify(marked)}`);
    const steps = await fill();
    const changed = (await game.call("build.cube", { at: MIDDLE })) as { kind: string; built: boolean; pending: string };
    check(changed.kind === "logs" && changed.built && changed.pending === "None", `the cube is ${JSON.stringify(changed)}, not logs built`);
    const over = (await game.call("build.cube", { at: [-5, 3, 8] })) as { kind: string; built: boolean };
    check(over.kind === "stone" && over.built, "the stone over it did not stay");
    const now = await base();
    check(now.floating === 0, `${now.floating} cubes float`);
    const back = (await carried("Stone")) - stone;
    // A quarter of a Stone back, kept as offcuts until it makes a whole one.
    check(back >= 0 && now.offcuts.includes("Stone"), `${back} Stone back, offcuts ${now.offcuts}`);
    // An outer corner slope on the wall's west end, rising north and east.
    const corner = await act("build.lay", { what: "block", min: [-8, 6, 8], max: [-8, 6, 8], shape: { OuterCorner: "North" } });
    check(corner.done, `the corner was refused: ${corner.refused}`);
    await fill();
    const slope = (await game.call("build.cube", { at: [-8, 6, 8] })) as { kind: string; shape: string; built: boolean };
    check(slope.kind === "stone" && slope.shape === "OuterCorner(North)" && slope.built, `the corner is ${JSON.stringify(slope)}`);
    const target = await point([-2.75, 1.5, 4]);
    const eye = await point([0.5, 3.0, 9.5]);
    await game.call("sandbox.look", { at: target.world, from: eye.world });
    await game.waitSeconds(2);
    await game.shot("shots/cabin-materials.png");
    await game.call("sandbox.release");
    await emptyPack();
    return `a wall laid in stone, its middle changed to logs in ${steps.length} steps (${steps.map((each) => each.what).join("; ")}), offcuts ${now.offcuts}; a stone outer corner on its end`;
  });

  await step("by-hand", async () => {
    // A shed of logs on a stone floor, five metres by three, west of the stone wall; over
    // its west half a gable of shakes built by hand, a cube at a time cut to slopes, its
    // west end a gable of logs, its east end against the storey added over the shed's east
    // half; and "add a roof" laying a hip of shakes over that storey.
    const near = await point([-7.5, 0, 9.8]);
    await game.warp(eastSouth(near.from_start), eastSouth((await point([-7.5, 0, 7.5])).from_start));
    const before = (await base()).parts.length;
    const shed = await act("build.lay", { what: "room", min: SHED_MIN, max: SHED_MAX, kind: "logs", floor: "stone" });
    check(shed.done, `the shed's plan was refused: ${shed.refused}`);
    const storey = await act("build.lay", { what: "room", min: [STOREY_X, 1, SHED_MIN[2]], max: [SHED_MAX[0], 1, SHED_MAX[2]], kind: "logs" });
    check(storey.done, `the storey's plan was refused: ${storey.refused}`);
    // By hand: rows of slopes climbing from the eaves either side to the ridge, a block at
    // a time in shakes, and the west gable's cubes in logs under them.
    const rows = SHED_MAX[2] - SHED_MIN[2] + 1;
    let blocks = 0;
    for (let x = SHED_MIN[0]; x < STOREY_X; x++) {
      for (let row = 0; row < rows; row++) {
        const up = Math.min(row, rows - 1 - row);
        const facing = row < rows / 2 ? "South" : "North";
        const at = [x, 6 + up, SHED_MIN[2] + row];
        const block = await act("build.lay", { what: "block", min: at, max: at, kind: "shakes", shape: { Slope: facing } });
        check(block.done, `the roof's cube at ${at} was refused: ${block.refused}`);
        blocks++;
        if (x === SHED_MIN[0]) {
          for (let y = 6; y < 6 + up; y++) {
            const gable = await act("build.lay", { what: "block", min: [x, y, at[2]], max: [x, y, at[2]], kind: "logs", shape: "Whole" });
            check(gable.done, `the gable's cube was refused: ${gable.refused}`);
          }
        }
      }
    }
    const byHand = await base();
    check(byHand.roofs.cubes === 80 + blocks && byHand.roofs.built === 80, `the roof's cubes are ${JSON.stringify(byHand.roofs)}`);
    // "Add a roof" over the storey, aimed into it.
    const hip = await act("build.lay", { what: "roof", over: [STOREY_X + 2, 1, SHED_MIN[2] + 2], kind: "logs", style: "Hip" });
    check(hip.done, `the storey's roof was refused: ${hip.refused}`);
    const laid = (await base()).parts.slice(before);
    const upper = (STOREY_X <= SHED_MAX[0] ? SHED_MAX[0] - STOREY_X + 1 : 0) * rows;
    check((await base()).roofs.cubes === 80 + blocks + upper, `${(await base()).roofs.cubes} roof cubes, not ${80 + blocks + upper}`);
    // Built: walls, the storey on them, the roof by hand and the hip, in their order.
    await emptyPack();
    await game.give("wood:16", "stone:8");
    let steps = 0;
    for (let round = 0; round < 12; round++) {
      const done = await acts("build.fill", { steps: 400 });
      steps += done.filter((each) => each.done).length;
      const refused = done.find((each) => !each.done);
      if (!refused) break;
      check(refused.refused?.startsWith("You need") === true, `refused: ${refused.refused} (${refused.what})`);
      await emptyPack();
      await game.give("wood:16", "stone:8");
    }
    const now = await base();
    check(now.parts.every((part) => part.done), `not all built: ${JSON.stringify(now.parts.filter((part) => !part.done))}`);
    check(now.floating === 0, `${now.floating} cubes float`);
    check(now.roofs.built === now.roofs.cubes, `the roofs are ${JSON.stringify(now.roofs)}`);
    await emptyPack();
    const target = await point([-7.5, 2.5, 7.5]);
    const eye = await point([-1.5, 6.0, 14.5]);
    await game.call("sandbox.look", { at: target.world, from: eye.world });
    await game.waitSeconds(2);
    await game.shot("shots/cabin-roofs.png");
    await game.call("sandbox.release");
    return `${laid.length} parts: a shed, a storey over its east half, ${blocks} roof cubes laid by hand over its west half and a hip of ${upper} added over the storey, built in ${steps} steps; ${now.roofs.cubes} roof cubes in all, ${JSON.stringify(now.roofs.shapes)}`;
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
