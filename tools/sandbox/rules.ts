#!/usr/bin/env node
// The building rules, checked live in the play experiment by the base's state, not by
// picture (D69; research/building-audit.md). One running game, driven over the remote
// protocol as cabin.ts is, through the same checked operations as the hammer (`build.*`):
//
// - Structure: nothing is built in the air, off an unbuilt post, or off an overhang into
//   nothing, while a cantilever joined to the ground goes in; nothing is taken from under a
//   cube or a roof's eave; carving a brush's box, changing a cube's material in place and
//   taking out a brush's box are refused where they would leave something hanging; a plan
//   outside the claim is refused.
// - Physics: a cube is not built where the player stands; a post stops the player at its
//   own width; a slope cut upside down at head height stops them under it rather than
//   letting their head through.
// - Roofs (D241): a roof's cubes over a stone room on four posts and its walls, "add a roof"
//   as a hip, built a layer at a time a stage at a time, nothing floating; pictures of it
//   half built and finished close up.
// - Loss and repair (D193, D305): a log wall's first course knocked out, all of the wall
//   over it comes down, breaking into fragments that fall and fade, and nothing floats,
//   filmed as it falls; the stone room's wall damaged short of breaking, drawn
//   worn, then mended with the hammer's repair, pictures of each.
// - Timing: a single cube's change from the operation to its sections on screen (`Build:
//   change on screen`, a median of several), the worst frame while building continuously,
//   and while walking down the village's street (`sandbox.frames`).
//
// Each step prints ok or FAIL with what it saw; screenshots go to shots/rules-*.png. Runs
// an optimised build on the Windows GPU from WSL (--debug for a debug one, --here on this
// machine). `cargo xtask rules` runs it after terra_build's rules tests and the drawing
// audit. Node 22.18 or later; no dependencies.

import * as game from "./client.ts";

const here = process.argv.includes("--here");
const release = !process.argv.includes("--debug");
const windows = game.onWsl && !here;
let failures = 0;

type Answer = { done: boolean; what: string; took?: string; gave?: string; refused?: string; cubes?: number[][] };
type Part = { id: number; kind: string; cubes: number; built: number; done: boolean };
type Base = { cubes: number; planned: number; floating: number; damaged: number; fragments: number; parts: Part[]; roofs: { cubes: number; built: number; stages: number[] }; player: number[] | null };
type Spread = { median: number; p99: number; worst: number } | null;
type Frames = { frames: number; frame: Spread; work: Spread; game: Spread; gpu: Spread };

const base = async () => (await game.call("build.state")) as Base;
const act = async (method: string, params?: unknown) => (await game.call(method, params)) as Answer;
const acts = async (method: string, params?: unknown) => (await game.call(method, params)) as Answer[];
const op = (ops: unknown[]) => act("build.op", { ops });
const point = async (at: number[]) => (await game.call("build.point", { at })) as { world: number[]; from_start: number[] };
const eastSouth = (from: number[]): [number, number] => [from[0], from[2]];
const ground = async (x: number, z: number) => ((await game.call("build.ground", { columns: [[x, z]] })) as number[])[0];
/// The layer of the cube the ground passes through under a column.
const foot = async (x: number, z: number) => Math.floor((await ground(x, z)) / 0.5);
const block = (at: number[], shape: unknown = "Whole", kind = "stone") => act("build.lay", { what: "block", min: at, max: at, kind, shape });
const cube = async (at: number[]) => (await game.call("build.cube", { at })) as { kind: string | null; built?: boolean; pending?: string; level?: number; damage?: number; health?: number };

function check(that: boolean, what: string) {
  if (!that) throw new Error(what);
}

async function step(name: string, run: () => Promise<string>) {
  try {
    console.log(`ok    ${name}: ${await run()}`);
  } catch (error) {
    failures++;
    console.log(`FAIL  ${name}: ${(error as Error).message}`);
    await game.shot(`shots/rules-${name}.png`).catch(() => {});
  }
}

/// Lays whole stone cubes and builds them, bottom first, as built cubes.
async function built(cubes: number[][], shape: unknown = "Whole", kind = "stone") {
  for (const at of cubes) {
    const laid = await block(at, shape, kind);
    check(laid.done, `laying ${at} was refused: ${laid.refused}`);
  }
  const sorted = [...cubes].sort((a, b) => a[1] - b[1]);
  for (const at of sorted) {
    for (let i = 0; i < 4 && !(await cube(at)).built; i++) {
      const done = await op([{ Fill: { cubes: [at] } }]);
      check(done.done, `building ${at} was refused: ${done.refused}`);
    }
  }
}

/// A column of cubes from a layer to another.
const column = (x: number, z: number, from: number, to: number) => Array.from({ length: to - from + 1 }, (_, i) => [x, from + i, z]);

/// A picture from an eye to a point, both in the base's own space, in metres.
async function picture(name: string, at: number[], from: number[]) {
  const [target, eye] = [await point(at), await point(from)];
  await game.call("sandbox.look", { at: target.world, from: eye.world });
  await game.waitSeconds(2);
  await game.shot(`shots/rules-${name}.png`);
  await game.call("sandbox.release");
}

/// Plenty to build with, in what a pack holds.
async function stock() {
  const carrying = (await game.state()).carrying;
  const have = ["Wood", "Stone"].filter((name) => carrying[name]);
  if (have.length) await game.take(...have.map((name) => name.toLowerCase()));
  await game.give("stone:15", "wood:10");
}

async function main() {
  console.log(`Starting the game${windows ? " on Windows" : ""}${release ? ", optimised" : ""}…`);
  await game.start(["--experiment", "play", "--fresh", "--save", "rules.ron", "--time", "11"], { windows, copy: "rules", release });

  await step("claim", async () => {
    await game.warp([0, -8], [0, -14]);
    await game.give("stone:4");
    await game.hold("Hammer");
    await game.press("b@20", "2@30", "enter@40", "e@70:80");
    const placed = (await game.waitFor("Placed a Claim stone", { timeout: 60 })).text;
    // The pickaxe in hand, so no part's outline is drawn over the pictures; the build.*
    // methods build as the hammer does whatever is held.
    await game.hold("Stone pickaxe");
    return placed;
  });
  // The builder stands south of the work, which is north east of the stone.
  const stand = await point([6, 0, 4]);
  await game.warp(eastSouth(stand.from_start), eastSouth((await point([6, 0, -2])).from_start));
  await stock();

  await step("in-the-air", async () => {
    const f = await foot(4, -4);
    const laid = await block([4, f + 4, -4]);
    check(laid.done, `the plan was refused: ${laid.refused}`);
    const up = await op([{ Fill: { cubes: [[4, f + 4, -4]] } }]);
    check(!up.done && /float/.test(up.refused ?? ""), `a cube two metres up went in: ${up.refused ?? "done"}`);
    await op([{ Unplan: { cubes: [[4, f + 4, -4]] } }]);
    return `"${up.refused}"`;
  });

  await step("unbuilt-post", async () => {
    const f = await foot(2, -4);
    const post = column(2, -4, f, f + 5);
    for (const at of post) check((await block(at, "Post", "logs")).done, `the post's cube ${at} was refused`);
    // Only its foot in.
    for (let i = 0; i < 2; i++) await op([{ Fill: { cubes: [post[0]] } }]);
    const top = await op([{ Fill: { cubes: [post[5]] } }]);
    check(!top.done && /float/.test(top.refused ?? ""), `the post's top went in over nothing: ${top.refused ?? "done"}`);
    check((await block([3, f + 5, -4], "Whole", "logs")).done, "the deck's plan was refused");
    const deck = await op([{ Fill: { cubes: [[3, f + 5, -4]] } }]);
    check(!deck.done && /float/.test(deck.refused ?? ""), `a deck went in beside an unbuilt post: ${deck.refused ?? "done"}`);
    return `the post's top: "${top.refused}"; a deck beside it: "${deck.refused}"`;
  });

  // A stone column three high with a beam two out from its top, used by the next steps.
  const cx = 8;
  const cz = -4;
  const cf = await foot(cx, cz);
  await step("overhang", async () => {
    await stock();
    await built(column(cx, cz, cf, cf + 2));
    await built([[cx + 1, cf + 2, cz], [cx + 2, cf + 2, cz]]);
    // Hung under the beam's end: joined, so allowed.
    await built([[cx + 2, cf + 1, cz]]);
    // Two out past it into nothing, with air between.
    check((await block([cx + 4, cf + 2, cz])).done, "the plan was refused");
    const off = await op([{ Fill: { cubes: [[cx + 4, cf + 2, cz]] } }]);
    check(!off.done, "a cube went in with air between it and the beam");
    await op([{ Unplan: { cubes: [[cx + 4, cf + 2, cz]] } }]);
    const now = await base();
    check(now.floating === 0, `${now.floating} cubes float`);
    return `a cantilever and a cube hung under it went in; past it into nothing: "${off.refused}"`;
  });

  await step("rests-on", async () => {
    const under = await op([{ Empty: { cubes: [[cx, cf + 1, cz]] } }]);
    check(!under.done && /holds up/.test(under.refused ?? ""), `a cube under a cube came out: ${under.refused ?? "done"}`);
    // Out by the hammer's take-out on a box of two round the column's foot.
    const box = await act("build.take_out", { at: [cx, cf, cz], brush: 1 });
    check(!box.done, "the take-out of the column's foot was done");
    return `under a cube: "${under.refused}"; a take-out: "${box.refused}"`;
  });

  await step("carve-and-replace", async () => {
    await stock();
    // The column's middle carved: marked, then refused when it comes out from under the
    // beam that hangs on it.
    // Each step out as building takes it (`Base::next_carve`, `next_change`): an Empty of the
    // cube marked.
    const carve = await act("build.carve", { at: [cx, cf + 1, cz] });
    check(carve.done, `marking the carving was refused: ${carve.refused}`);
    check((await cube([cx, cf + 1, cz])).pending === "Carve", "the cube is not marked to be carved");
    const cut = await op([{ Empty: { cubes: [[cx, cf + 1, cz]] } }]);
    check(!cut.done, "the carving came out from under the beam");
    await op([{ Cancel: { cubes: [[cx, cf + 1, cz]] } }]);
    // Its foot changed to logs in place: the stone out from under it refused the same.
    const change = await act("build.replace", { at: [cx, cf, cz], kind: "logs" });
    check(change.done, `marking the change was refused: ${change.refused}`);
    const out = await op([{ Empty: { cubes: [[cx, cf, cz]] } }]);
    check(!out.done, "the stone came out from under the column");
    await op([{ Cancel: { cubes: [[cx, cf, cz]] } }]);
    const now = await base();
    check(now.floating === 0, `${now.floating} cubes float`);
    return `carving its middle: "${cut.refused}"; changing its foot: "${out.refused}"`;
  });

  await step("outside-claim", async () => {
    const far = await block([70, 0, -4]);
    check(!far.done && /claim/.test(far.refused ?? ""), `a plan 35 m out went in: ${far.refused ?? "done"}`);
    return `"${far.refused}"`;
  });

  await step("where-you-stand", async () => {
    const f = await foot(12, 2);
    // Its foot first, then the player stands in the cube over it.
    await built([[12, f, 2]]);
    check((await block([12, f + 1, 2])).done, "the plan was refused");
    const at = await point([6.25, 0, 1.25]);
    await game.warp(eastSouth(at.from_start), eastSouth((await point([6.25, 0, -2])).from_start));
    await game.waitSeconds(1);
    const into = await op([{ Fill: { cubes: [[12, f + 1, 2]] } }]);
    check(!into.done && /in the way/.test(into.refused ?? ""), `a cube went in where the player stands: ${into.refused ?? "done"}`);
    await game.warp(eastSouth(stand.from_start), eastSouth((await point([6, 0, -2])).from_start));
    await game.waitSeconds(1);
    const after = await op([{ Fill: { cubes: [[12, f + 1, 2]] } }]);
    check(after.done, `with the player gone it was refused: ${after.refused}`);
    return `"${into.refused}", and with them gone it went in`;
  });

  await step("item-in-the-way", async () => {
    // The claim stone, something set down, stands at the base's origin: a column planned
    // through it is built up to it and no further.
    const f = await foot(0, 0);
    const through = column(0, 0, f, 1);
    for (const at of through) check((await block(at)).done, `the plan at ${at} was refused`);
    let refusal = "";
    for (const at of through) {
      const done = await op([{ Fill: { cubes: [at] } }]);
      if (!done.done) {
        refusal = `${at}: "${done.refused}"`;
        check(/in the way/.test(done.refused ?? ""), `refused for another reason: ${refusal}`);
        break;
      }
    }
    check(refusal !== "", "a column was built through the claim stone");
    return refusal;
  });

  await step("post", async () => {
    await stock();
    // A post a storey high, and the player walked west at it along its middle.
    const [px, pz] = [16, 6];
    const f = await foot(px, pz);
    await built(column(px, pz, f, f + 5), "Post", "logs");
    const start = await point([px * 0.5 - 3, 0, pz * 0.5 + 0.25]);
    await game.warp(eastSouth(start.from_start), eastSouth((await point([px * 0.5 + 3, 0, pz * 0.5 + 0.25])).from_start));
    await game.press("w:150");
    await game.waitSeconds(3);
    const feet = (await base()).player!;
    // Stopped a body's radius short of the post's own 36 cm, not its cube's half metre.
    const gap = px * 0.5 + 0.25 - feet[0];
    check(gap > 0.4 && gap < 0.56, `the player stopped ${gap.toFixed(2)} m from the post's middle`);
    return `stopped ${gap.toFixed(2)} m from its middle (its half width 0.18 and a body's 0.3)`;
  });

  await step("head-height", async () => {
    await stock();
    // A slope upside down hung off a column, its underside raking down toward the east,
    // its high edge over a walker's head and its low edge at their chest; walked at from
    // the west under its high edge.
    const [hx, hz] = [20, 2];
    const g = await ground(hx, hz);
    const layer = Math.ceil((g + 1.3) / 0.5);
    const f = await foot(hx, hz - 1);
    await built(column(hx, hz - 1, f, layer));
    await built([[hx, layer, hz]], { InvertedSlope: "East" });
    // Clear of the column north of it by a few centimetres.
    const start = await point([hx * 0.5 - 3, 0, hz * 0.5 + 0.4]);
    await game.warp(eastSouth(start.from_start), eastSouth((await point([hx * 0.5 + 4, 0, hz * 0.5 + 0.4])).from_start));
    await game.press("w:200");
    await game.waitSeconds(4);
    const feet = (await base()).player!;
    check(feet[0] < hx * 0.5 + 0.5, `the player walked through under it to ${feet[0].toFixed(2)} m east`);
    return `its underside ${(layer * 0.5 - g).toFixed(2)} to ${(layer * 0.5 + 0.5 - g).toFixed(2)} m over the ground; stopped at ${(feet[0] - hx * 0.5).toFixed(2)} m into its cube`;
  });

  await step("roof", async () => {
    // A stone room, and "add a roof" laying a hip of slate over it; built a layer at a time
    // a stage at a time, then pictures of it half up and done.
    // South of it, clear of its footing: what goes in where the player stands is refused.
    const near = await point([10, 0, 10.5]);
    await game.warp(eastSouth(near.from_start), eastSouth((await point([10, 0, 4])).from_start));
    const room = await act("build.lay", { what: "room", min: [14, 0, 10], max: [23, 0, 17], kind: "stone" });
    check(room.done, `the room was refused: ${room.refused}`);
    const hip = await act("build.lay", { what: "roof", over: [18, 0, 13], kind: "stone", style: "Hip" });
    check(hip.done, `the roof was refused: ${hip.refused}`);
    let steps = 0;
    let half = false;
    for (let round = 0; round < 120; round++) {
      await stock();
      const done = await acts("build.fill", { steps: 12 });
      steps += done.filter((each) => each.done).length;
      const now = await base();
      if (!half && now.roofs.stages[0] < now.roofs.cubes * 0.6 && now.roofs.built < now.roofs.cubes) {
        half = true;
        await picture("roof-half", [9.5, 3.0, 6.75], [16.5, 6.5, 13.5]);
        await picture("roof-half-close", [11.3, 3.2, 8.3], [13.6, 4.4, 10.6]);
      }
      const refused = done.find((each) => !each.done);
      if (!refused && done.length < 12) break;
      if (refused && !refused.refused?.startsWith("You need")) throw new Error(`refused: ${refused.refused} (${refused.what})`);
    }
    const now = await base();
    check(now.roofs.built === now.roofs.cubes && now.roofs.cubes > 0, `the roof is ${JSON.stringify(now.roofs)}`);
    check(now.floating === 0, `${now.floating} cubes float`);
    await picture("roof", [9.5, 3.0, 6.75], [16.5, 6.5, 13.5]);
    await picture("roof-close", [11.3, 3.2, 8.3], [13.6, 4.4, 10.6]);
    // Its eave cannot come out from under it.
    const eave = await op([{ Empty: { cubes: [[14, 5, 12]] } }]);
    check(!eave.done, "a wall's top came out from under the roof");
    return `${now.roofs.cubes} roof cubes built in ${steps} steps, ${JSON.stringify(now.roofs.stages)}; under its eave: "${eave.refused}"`;
  });

  await step("collapse", async () => {
    // A log wall a storey high on its stone footings, built through; then its first course
    // of logs knocked out, as an attack would, filmed from the side as it falls.
    await game.give("stone:20", "wood:20");
    const near = await point([19, 0, 7]);
    await game.warp(eastSouth(near.from_start), eastSouth((await point([15, 0, 6])).from_start));
    const laid = await act("build.lay", { what: "wall", min: [30, 0, 10], max: [30, 0, 15], kind: "logs" });
    check(laid.done, `the wall was refused: ${laid.refused}`);
    for (let round = 0; round < 60; round++) {
      await stock();
      await game.give("wood:10");
      const done = await acts("build.fill", { steps: 12 });
      if (done.length < 12 && done.every((each) => each.done)) break;
      const refused = done.find((each) => !each.done);
      if (refused && !refused.refused?.startsWith("You need")) throw new Error(`refused: ${refused.refused} (${refused.what})`);
    }
    // The first course of logs in each column, over its footing.
    const course: number[][] = [];
    for (let z = 10; z <= 15; z++) {
      for (let y = -6; y < 8; y++) {
        if ((await cube([30, y, z])).kind === "logs") {
          course.push([30, y, z]);
          break;
        }
      }
    }
    check(course.length === 6, `the wall's first course is ${JSON.stringify(course)}`);
    const before = await base();
    const [eye, at] = [await point([10.5, 2.4, 13.5]), await point([15.25, 1.2, 6.5])];
    await game.call("sandbox.look", { at: at.world, from: eye.world });
    await game.waitSeconds(2);
    await game.shot("shots/rules-collapse-0.png");
    const last = await game.film("shots/rules-collapse-fall.png", 60);
    const hit = (await game.call("build.damage", { at: course[0], to: course[course.length - 1] })) as Answer & { fell: number; took_ms: number };
    check(hit.done, `the blow was refused: ${hit.refused}`);
    await game.waitSeconds(0.5);
    const flying = (await base()).fragments;
    check(flying > 6, `${flying} fragments after the wall came down`);
    await game.filmed(last);
    await game.waitSeconds(2);
    await game.shot("shots/rules-collapse-1.png");
    const after = await base();
    check(after.floating === 0, `${after.floating} cubes float`);
    // Every cube of the wall over the course came down; its footings stand.
    for (const at of course) {
      check((await cube([at[0], at[1] + 1, at[2]])).kind === null, `the log over ${at} still stands`);
      check((await cube([at[0], at[1] - 1, at[2]])).kind !== null, `the footing under ${at} fell`);
    }
    // Settled, they shrink away and are gone.
    for (let i = 0; i < 4; i++) await game.waitSeconds(2);
    const gone = (await base()).fragments;
    check(gone === 0, `${gone} fragments left after eight seconds`);
    await game.shot("shots/rules-collapse-2.png");
    await game.call("sandbox.release");
    check(before.cubes - after.cubes === hit.fell, `${before.cubes - after.cubes} cubes gone, ${hit.fell} said to fall`);
    return `${hit.fell} cubes fell, worked out in ${hit.took_ms.toFixed(1)} ms; ${flying} fragments half a second on, none eight seconds on`;
  });

  await step("repair", async () => {
    // The stone room's west wall hit short of breaking: worn where it was hit, then mended.
    // Three courses of it over the ground, three cubes along.
    const wall: number[][] = [];
    for (let z = 11; z <= 13; z++) {
      const f = await foot(14, z);
      for (let y = f + 1; y <= f + 3; y++) {
        const found = await cube([14, y, z]);
        if (found.kind === "stone" && found.built) wall.push([14, y, z]);
      }
    }
    wall.sort((a, b) => a[1] - b[1] || a[2] - b[2]);
    check(wall.length === 9, `the wall's cubes are ${JSON.stringify(wall)}`);
    const health = (await cube(wall[0])).health ?? 600;
    const hit = await act("build.op", { ops: [{ Damage: { cubes: wall, amount: Math.floor(health * 0.6) } }] });
    check(hit.done, `the blow was refused: ${hit.refused}`);
    const worn = await base();
    check(worn.damaged === 9, `${worn.damaged} cubes damaged`);
    await picture("repair-0", [7, 1.0, 6], [4.5, 2.2, 7.5]);
    // Out from under damage is refused: mended first.
    const out = await op([{ Empty: { cubes: [wall[wall.length - 1]] } }]);
    check(!out.done && /repair/.test(out.refused ?? ""), `a damaged cube came out: ${out.refused ?? "done"}`);
    await stock();
    const mended = await op([{ Repair: { cubes: wall } }]);
    check(mended.done, `the repair was refused: ${mended.refused}`);
    check((await base()).damaged === 0, "cubes still damaged after the repair");
    await picture("repair-1", [7, 1.0, 6], [4.5, 2.2, 7.5]);
    return `9 cubes worn to ${Math.floor(health * 0.6)} of ${health}; mended, taking ${mended.took}`;
  });

  await step("edit-on-screen", async () => {
    // The top of the two cubes where the player stood taken out and put back, five times,
    // each timed from the operation to all it set waiting on screen.
    const times: number[] = [];
    const said: string[] = [];
    const hung = [12, (await foot(12, 2)) + 1, 2];
    for (let i = 0; i < 5; i++) {
      for (const ops of [[{ Empty: { cubes: [hung] } }], [{ Fill: { cubes: [hung] } }]]) {
        await game.markNow();
        const done = await op(ops);
        check(done.done, `refused: ${done.refused}`);
        const line = await game.waitFor("Build: change on screen", { timeout: 30 });
        times.push(Number(/in ([\d.]+) ms/.exec(line.text)?.[1]));
        said.push(line.text.replace(/^.*?: /, ""));
      }
    }
    times.sort((a, b) => a - b);
    return `median ${times[times.length >> 1].toFixed(1)} ms, worst ${times[times.length - 1].toFixed(1)} ms; the last: ${said[said.length - 1]}`;
  });

  await step("building-frames", async () => {
    // Built continuously for ten seconds or so: a wall laid and filled a step at a time.
    await stock();
    const wall = await act("build.lay", { what: "wall", min: [26, 0, 10], max: [26, 0, 17], kind: "stone" });
    check(wall.done, `the wall was refused: ${wall.refused}`);
    const began = Date.now();
    let steps = 0;
    while (Date.now() - began < 10_000) {
      const [done] = await acts("build.fill", { steps: 1 });
      if (!done) break;
      if (!done.done) await stock();
      else steps++;
    }
    const frames = (await game.call("sandbox.frames", { seconds: (Date.now() - began) / 1000 })) as Frames;
    return `${steps} steps; ${describe(frames)}`;
  });

  await step("village-frames", async () => {
    await game.waitFor("Settlement vantage: street", { since: 0, timeout: 60 });
    const line = (await game.said(0, "Settlement vantage: street", 1)).lines[0].text;
    const street = line.split("--vantage ")[1].split(",").map(Number);
    await game.warp([street[0], street[2]], [street[3], street[5]]);
    await game.waitSeconds(3);
    await game.press("w:600");
    await game.waitSeconds(11);
    const frames = (await game.call("sandbox.frames", { seconds: 10 })) as Frames;
    return describe(frames);
  });
}

const describe = (f: Frames) =>
  `${f.frames} frames; between frames median ${f.frame?.median.toFixed(1)}, 99th ${f.frame?.p99.toFixed(1)}, worst ${f.frame?.worst.toFixed(1)} ms; the game's update worst ${f.game?.worst.toFixed(1)} ms; GPU median ${f.gpu?.median.toFixed(1) ?? "-"}, worst ${f.gpu?.worst.toFixed(1) ?? "-"} ms`;

try {
  await main();
} catch (error) {
  failures++;
  console.log(`FAIL  ${(error as Error).message} (see shots/remote.log)`);
} finally {
  await game.stop().catch(() => {});
}
console.log(failures ? `rules: ${failures} failed` : "rules: all held");
process.exit(failures ? 1 : 0);
