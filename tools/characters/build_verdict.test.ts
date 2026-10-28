// build_bodies.sh's verdict on made-up build logs: every failure its steps can print fails it.
// node --test tools/characters/build_verdict.test.ts

import { test } from "node:test";
import assert from "node:assert/strict";
import { verdict } from "./build_verdict.ts";

const RIG_PASS = JSON.stringify({ file: "tools/characters/out/base-male.glb", pass: true, checks: { skeleton: true, motion: true } });

/** A build that passed: the clips, the base's looks, two morphs' looks, the game's files. */
const CLEAN = [
  "== 0. the clips",
  'CLIPCHECK {"clip": "Idle_Loop", "pass": true, "worst_step_deg": 3.1}',
  'CLIPCHECK pass {"step_deg": 135.0}',
  "== 1. the base",
  'RIGCHECK {"file": "tools/characters/out/base-male.glb", "pass": true}',
  "== 2. its gear",
  "FIT small_pack touching 31%, nearest 0.2 cm ok",
  'GEARCHECK {"item": "tools/characters/out/base-male-trousers.glb", "pass": true}',
  "GEARCHECKS 1 of 1 pass",
  "== 3. the morphs, side by side",
  "MORPH heavy fitted: mean 1.6 mm, 95% 11.3 mm, unmatched 4.0%",
  "GEARCHECKS 1 of 1 pass",
  "MORPH female fitted: mean 1.2 mm, 95% 5.0 mm, unmatched 1.8%",
  'GEARCHECK {"item": "tools/characters/out/female-shift.glb", "body": "female", "pass": true}',
  "GEARCHECKS 1 of 1 pass",
  "== 5. the game's files",
  "MORPH female target added; 79 joints and sockets move",
  "markers: assets/models/character/base.glb, 34 clips baked",
  "BUILT base.glb with morphs female,heavy, 1 looks",
];

const failing = (extra: string[], at = 8) => verdict([...CLEAN.slice(0, at), ...extra, ...CLEAN.slice(at)].join("\n"), RIG_PASS);

test("a clean build passes", () => {
  assert.deepEqual(verdict(CLEAN.join("\n"), RIG_PASS), []);
});

test("every failure a step prints fails the build", () => {
  for (const lines of [
    ['CLIPCHECK {"clip": "Swim_Fwd_Loop", "pass": false, "worst_step_deg": 176.5}'],
    ['CLIPCHECK fail {"step_deg": 135.0}'],
    // A morph's job that exited non-zero (build_bodies.sh step 3).
    ["MORPH thin failed: see tools/characters/out/build-thin.log"],
    ["FIT quiver touching 2%, nearest 3.1 cm FLOATING: under 10% touching, or nothing within 2 cm"],
    // made_clips.py's SystemExit, which prints no "Error": its exit, as tools/blender/run.sh
    // says it (Blender 5.2 exits 1, measured), is what fails it.
    ["no clean stride in Walk.glb: left heel strikes at []", "run.sh: made_clips.py exited 1"],
    // A sys.exit(1) that prints nothing.
    ["run.sh: lift.py exited 1"],
    // An uncaught exception in any step: Python's line, then Blender's.
    ["Exception: plain exception", "Error: script failed, expr: 'import sys, runpy, bpy"],
    ["✗ assets/models/character/base.glb: Crouch_Fwd_Loop skates 8.2/6.4 cm a plant (over 6)"],
    // A look on the female, from her morph's log.
    ['GEARCHECK {"item": "tools/characters/out/female-skirt.glb", "body": "female", "pass": false}'],
  ]) {
    const found = failing(lines);
    assert.ok(found.length > 0, `passed with: ${lines.join(" / ")}`);
  }
});

test("a step that stopped early, or a check that never ran, fails the build", () => {
  // The clip check died before its verdict.
  assert.match(verdict(CLEAN.filter((l) => !l.startsWith("CLIPCHECK pass")).join("\n"), RIG_PASS).join(), /CLIPCHECK never finished/);
  // One morph's look never checked.
  const once = CLEAN.filter((l, i) => !(l.startsWith("GEARCHECKS") && i > 12));
  assert.match(verdict(once.join("\n"), RIG_PASS).join(), /GEARCHECK finished 2 times of 3 \(1 looks on 3 bodies\)/);
  // A look on each body, but two looks were made.
  assert.match(verdict(CLEAN.join("\n").replace("1 looks", "2 looks"), RIG_PASS).join(), /finished 3 times of 6/);
  // The build stopped before the game's files.
  assert.match(verdict(CLEAN.slice(0, -1).join("\n"), RIG_PASS).join(), /no BUILT line/);
  assert.match(verdict(CLEAN.join("\n"), RIG_PASS, 1).join(), /stopped before its end \(exit 1\)/);
  // The base's rig check wrote no report (build_bodies.sh removes the last one first), or failed.
  assert.match(verdict(CLEAN.join("\n"), undefined).join(), /RIGCHECK missing/);
  const rigFail = JSON.stringify({ file: "base-male.glb", pass: false, checks: { skeleton: true, motion: false } });
  assert.deepEqual(verdict(CLEAN.join("\n"), rigFail), ["RIGCHECK base-male.glb fails: motion"]);
});
