#!/usr/bin/env node
// The vantage sheet (docs/biomes.md, "How we'd know"; D69): the lush forest from fixed views,
// the forest floor at eye height, its edge, the clearing and an overlook, at each hour asked
// for, side by side in one picture, so a change to how the world looks is judged on the same
// views every time. One scripted run per hour moves its camera from view to view
// (--vantage …@frame) and saves each with --shots.
//
//   node tools/sandbox/vantage.ts [--time 13,17] [--out shots/vantage]
//
// Writes <out>/<hour>-<view>.png and <out>/sheet.jpg. Needs ImageMagick (cargo xtask doctor).
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, renameSync } from "node:fs";
import { resolve } from "node:path";
import { ROOT } from "./client.ts";

/// Each view: a name, where the camera stands and what it looks at, in metres east, up and
/// south of the start. The forest is 220 m round the start, with a clearing of 14 m.
const VIEWS: [string, string][] = [
  ["floor", "-56,1.7,-43,-72,1.0,-55"],
  ["edge", "0,1.8,250,0,5,200"],
  ["clearing", "7,1.8,9,-8,1.2,-7"],
  ["overlook", "0,45,150,0,0,0"],
];
/// Frames each view is held for before it is pictured: grass is built round the camera as
/// it arrives, and levels of detail fade in.
const HOLD = 200;

const arg = (name: string, fallback: string) => {
  const at = process.argv.indexOf(name);
  return at > 0 ? process.argv[at + 1] : fallback;
};
const hours = arg("--time", "13,17").split(",");
const out = resolve(ROOT, arg("--out", "shots/vantage"));
mkdirSync(out, { recursive: true });

const build = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: ROOT, stdio: "inherit" });
if (build.status !== 0) process.exit(1);

const tiles: string[] = [];
for (const hour of hours) {
  const vantages = VIEWS.flatMap(([, view], index) => ["--vantage", `${view}@${index * HOLD}`]);
  const frames = VIEWS.map((_, index) => index * HOLD + HOLD - 10).join(",");
  const prefix = resolve(out, `run-${hour}`);
  const started = Date.now();
  const run = spawnSync(
    resolve(ROOT, "target/release/terra_sandbox"),
    ["--offscreen", "--clean", "--experiment", "walk", "--experiment", "lush", "--level", "yard", "--rolling", "--time", hour, ...vantages, "--shots", `${prefix}@${frames}`],
    // Run outside Cargo, so told where assets/ is, as Cargo would.
    { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT } },
  );
  const said = `${run.stdout}${run.stderr}`;
  if (run.status !== 0 || !said.includes("SANDBOX saved")) {
    console.error(said.split("\n").slice(-20).join("\n"));
    console.error(`vantage: the run at ${hour}:00 failed`);
    process.exit(1);
  }
  VIEWS.forEach(([name], index) => {
    const frame = index * HOLD + HOLD - 10;
    const file = resolve(out, `${hour}-${name}.png`);
    renameSync(`${prefix}-${String(frame).padStart(5, "0")}.png`, file);
    // The share of the ground within 30 m that nothing grows on (docs/biomes.md), as the
    // run measured it with the picture.
    const bare = new RegExp(`SANDBOX frame ${frame}: bare ground within 30 m ([\\d.]+)%, paths ([\\d.]+)%`).exec(said);
    const measured = bare ? `, bare ${bare[1]}% (paths ${bare[2]}%)` : "";
    if (bare) console.log(`vantage: ${name} at ${hour}:00: bare ground within 30 m ${bare[1]}%, paths ${bare[2]}%`);
    tiles.push("-label", `${name}, ${hour}:00${measured}`, file);
  });
  console.log(`vantage: ${hour}:00 in ${((Date.now() - started) / 1000).toFixed(0)} s`);
}

const sheet = resolve(out, "sheet.jpg");
const montage = spawnSync(
  "magick",
  ["montage", ...tiles, "-tile", `${VIEWS.length}x`, "-geometry", "640x360+4+4", "-pointsize", "18", "-background", "#222", "-fill", "#eee", sheet],
  { encoding: "utf8" },
);
if (montage.status !== 0 || !existsSync(sheet)) {
  console.error(montage.stderr);
  process.exit(1);
}
console.log(`vantage: ${sheet}`);
