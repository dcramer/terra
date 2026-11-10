#!/usr/bin/env node
// The land is as bright whichever way the camera faces (a continuous check,
// tools/continuous/checks.json): the play start at dusk, the view turned to the four quarters,
// a still each; the ground and the player in the lower middle of the picture must be within
// HELD of the brightest quarter's, in linear light. The developers, 2026-11-09: "joined a game,
// turned 90 degrees, it has weird shadows, turn 90 degrees, its in color". Bevy 0.20 took a
// fragment no irradiance volume holds as lit by the volumes with no light whenever the view
// had any volume in its frustum, so facing the den's volume (`den_light`), a hundred metres
// off, put out the sky's diffuse light on the whole land and left the player grey, lit by
// the sky's reflection alone; facing away culled the volume and lit it again (the patch:
// `irradiance_volume_light_held`, patches/README.md). This holds it, whatever in the picture
// next changes the light of what is outside it.
//
//   node tools/sandbox/turnlight.ts [--out target/check/turnlight] [--from <dir>]
//
// Builds the release sandbox, then runs it through the GPU scheduler (`cargo xtask gpu still`).
// --from measures a folder a run wrote instead. A line a stage; exits 1 below the bar.
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { light } from "./groundlight.ts";
import { readPng } from "./png.ts";

const ROOT = resolve(import.meta.dirname, "../..");
/// Where the player starts, east and south, and how far off each quarter's aim is.
const START = [7.5, 14];
const OFF = 30;
const QUARTERS: [string, number, number][] = [
  ["north", 0, -OFF],
  ["east", OFF, 0],
  ["south", 0, OFF],
  ["west", -OFF, 0],
];
/// The first aim, the frames a quarter is held before its still, and between quarters.
const FIRST = 120;
const HELD_FRAMES = 150;
const EVERY = 160;
/// The ground and the player, as fractions of the picture: under the horizon, over the bar.
const LAND = [[0.2, 0.6, 0.8, 0.85]];
/// The least a quarter's land may be of the brightest's: 0.53 with the patch and 0.15
/// without it (the west, the den's volume in the picture), whole pictures by hand; the
/// quarters differ by what stands in them and the dusk falls over the run, so not near 1.
const HELD = 0.3;

const shotAt = (i: number) => FIRST + i * EVERY + HELD_FRAMES;

function run(out: string): boolean {
  const build = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: ROOT, encoding: "utf8" });
  if (build.status !== 0) {
    console.log(`turnlight: ✗ build\n${build.stderr.split("\n").slice(-20).join("\n")}`);
    return false;
  }
  console.log("turnlight: built");
  mkdirSync(out, { recursive: true });
  const calls = QUARTERS.map(([, east, south], i) => ({
    frame: FIRST + i * EVERY,
    method: "play.aim",
    params: { at: [START[0] + east, START[1] + south] },
  }));
  const file = join(out, "calls.jsonl");
  writeFileSync(file, calls.map((c) => JSON.stringify(c)).join("\n") + "\n");
  const sandbox = [resolve(ROOT, "target/release/terra_sandbox"), "--offscreen", "--clean", "--picture", "1920x1080", "--experiment", "play", "--mute",
    "--time", "19.25", "--calls", file, "--shots", `${join(out, "f")}@${QUARTERS.map((_, i) => shotAt(i)).join(",")}`];
  const done = spawnSync("cargo", ["xtask", "gpu", "still", "--", "timeout", "900", ...sandbox], { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT }, maxBuffer: 1 << 28 });
  writeFileSync(join(out, "run.log"), `${done.stdout}${done.stderr}`);
  if (done.status !== 0) {
    console.log(`turnlight: ✗ run exited ${done.status}, see ${join(out, "run.log")}`);
    return false;
  }
  console.log("turnlight: ran");
  return true;
}

/// Each quarter's land against the brightest's, and whether every one holds.
export function judged(lights: number[]): { shares: number[]; holds: boolean } {
  const most = Math.max(...lights);
  const shares = lights.map((l) => (most > 1e-5 ? l / most : 0));
  return { shares, holds: shares.every((s) => s >= HELD) };
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const flag = (name: string) => {
    const i = argv.indexOf(name);
    return i < 0 ? undefined : argv[i + 1];
  };
  const from = flag("--from");
  const out = resolve(from ?? flag("--out") ?? join(ROOT, "target/check/turnlight"));
  if (!from && !run(out)) process.exit(1);
  const stills = QUARTERS.map((_, i) => join(out, `f-${String(shotAt(i)).padStart(5, "0")}.png`));
  const missing = stills.find((s) => !existsSync(s));
  if (missing) {
    console.log(`turnlight: ✗ no still ${missing}; the run's log is ${join(out, "run.log")}`);
    process.exit(1);
  }
  const { shares, holds } = judged(stills.map((s) => light(readPng(s), LAND)));
  const told = QUARTERS.map(([name], i) => `${name} ${shares[i].toFixed(2)}`).join(", ");
  const fix = holds ? "" : "; a quarter this dark has lost the sky's light: see what entered its picture (a light probe or volume, den_light, sky_occlusion) and patches/README.md's bevy_pbr row, and do not lower the bar";
  console.log(`turnlight: ${holds ? "✓" : "✗"} the land's light facing each quarter, of the brightest's: ${told} (at least ${HELD})${fix}; stills in ${out}`);
  process.exit(holds ? 0 : 1);
}
