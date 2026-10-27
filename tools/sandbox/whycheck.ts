#!/usr/bin/env node
// The `why` check (D548): in one running play sandbox, the player is warped with the clock
// stopped, and `sandbox.why Player Transform` must say the body is drawn between ticks and
// was moved, outside the ticks, by terra_walker's `warp`, on the frame the game said it
// warped. Then, the clock let run ten frames, what moved the player since is named
// instead, in a tick on one of those frames, or still the warp. Fails, saying what it was
// told, if not. A dev build, which keeps the lines (`r start`).
//
//   node tools/sandbox/whycheck.ts --windows
//
// Run it with --windows, the Windows dev build on the GPU: WSL's CPU-rendered dev build
// had not left the loading screen after 300 s (round 1, 2026-10-26).
// Node 22.18 or later; no dependencies.

import * as game from "./client.ts";
import { describeWhy, enclosingFn, type Part, type Why, where } from "./why.ts";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const windows = process.argv.includes("--windows");
const ask = async () => (await game.call("sandbox.why", { entity: "Player", component: "Transform" })) as Why;

function fail(why: string, answer?: Why): never {
  throw new Error([`✗ ${why}`, ...(answer ? describeWhy(answer) : [])].join("\n"));
}

const fnAt = (place: NonNullable<Part["by"]>) => enclosingFn(readFileSync(resolve(game.ROOT, place.file), "utf8"), place.line);

console.log(`why: starting the play sandbox${windows ? " on Windows" : ""}…`);
try {
  await game.start(["--experiment", "play", "--fresh", "--save", "whycheck.ron"], { windows, copy: "why" });
  await game.call("sandbox.pause", { paused: true });
  await game.markNow();
  await game.call("play.warp", { at: [6, 4] });
  const warped = await game.waitFor("Warped to", { timeout: 60 });
  // The drawing notices a move outside the ticks on the frame after it.
  await game.waitUntil("a frame past the warp", game.status, (now) => now.frame > warped.frame + 1);
  const answer = await ask();
  const part = answer.components[0];
  if (!answer.tracked) fail("this build keeps no lines of code (built without track_location)", answer);
  if (part?.name !== "Transform") fail("no Transform answered", answer);
  if (!part.drawn || part.moved === undefined) fail("the player is drawn between ticks, but why does not say what moved them", answer);
  const moved = part.moved;
  if (!moved?.by || !moved.by.file.endsWith("terra_walker/src/walker.rs") || fnAt(moved.by) !== "warp")
    fail(`the warp moved the player, but why says moved by ${moved ? where(moved.by) : "nothing"}`, answer);
  if (moved.in_tick) fail("the warp moves the player outside the ticks, but why says in a tick", answer);
  if (moved.changed.frame !== warped.frame) fail(`the warp was said on frame ${warped.frame}, but why says moved on frame ${moved.changed.frame}`, answer);
  console.log(`✓ warped on frame ${warped.frame}: ${describeWhy(answer)[1].trim()}`);

  // Frames go on while the clock is held, and a call takes some to arrive, so the ten run
  // are some after this one.
  const from = (await game.status()).frame;
  await game.step(10);
  const after = await ask();
  const next = after.components[0]?.moved;
  if (!next) fail("10 frames run, but why no longer says what moved the player", after);
  if (next.changed.frame === warped.frame && !next.in_tick) console.log("✓ 10 frames run: nothing moved the player since the warp");
  else if (!next.in_tick || next.changed.frame == null || next.changed.frame <= from || next.changed.frame > (after.frame ?? 0))
    fail(`10 frames run after frame ${from}, but the move is told on frame ${next.changed.frame}${next.in_tick ? "" : " outside the ticks"}`, after);
  else if (!next.by || next.by.file.endsWith("terra_world/src/tick.rs"))
    fail(`a tick moved the player, but why names ${where(next.by)}, the ticks' own bookkeeping`, after);
  else console.log(`✓ 10 frames run: ${describeWhy(after)[1].trim()}`);
} catch (error) {
  console.error((error as Error).message);
  process.exitCode = 1;
} finally {
  await game.stop();
}
