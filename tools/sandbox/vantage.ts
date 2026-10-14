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
/// How many places the canopy is looked up through, and the frames each is held.
const SKY_PLACES = 12;
const SKY_HOLD = 60;

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
    ["--offscreen", "--clean", "--experiment", "walk", "--experiment", "lush", "--level", "yard", "--time", hour, ...vantages, "--shots", `${prefix}@${frames}`],
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
  if (hour === hours[0]) tiles.push(...canopy(hour));
}

/// How closed the canopy is: from eye height, 1.7 m over the ground, looking straight up at
/// places spread through the forest, the share of the picture that is sky, told by its
/// colour. Under a closed canopy little sky shows; the forest's aim is under 15% (D244).
/// Places on a spiral from 40 to 170 m out, the same every run; some fall on paths and in
/// glades, as a walker's would.
function canopy(hour: string): string[] {
  const places = Array.from({ length: SKY_PLACES }, (_, k) => {
    const distance = 40 + (130 * k) / (SKY_PLACES - 1);
    const angle = k * 2.39996;
    return [Math.round(Math.cos(angle) * distance), Math.round(Math.sin(angle) * distance)];
  });
  const vantages = places.flatMap(([x, z], index) => ["--vantage", `${x},1.7,${z},${x + 0.02},40,${z}@${index * SKY_HOLD}`]);
  const frames = places.map((_, index) => index * SKY_HOLD + SKY_HOLD - 10);
  const prefix = resolve(out, `sky-${hour}`);
  const run = spawnSync(
    resolve(ROOT, "target/release/terra_sandbox"),
    ["--offscreen", "--clean", "--experiment", "walk", "--experiment", "lush", "--level", "yard", "--time", hour, "--vantage-ground", ...vantages, "--shots", `${prefix}@${frames.join(",")}`],
    { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT } },
  );
  if (run.status !== 0 || !`${run.stdout}`.includes("SANDBOX saved")) {
    console.error(`${run.stdout}${run.stderr}`.split("\n").slice(-20).join("\n"));
    console.error("vantage: the canopy run failed");
    process.exit(1);
  }
  const shares = frames.map((frame, index) => {
    const file = resolve(out, `sky-${index}.png`);
    renameSync(`${prefix}-${String(frame).padStart(5, "0")}.png`, file);
    // Sky is blue over red and green, not dark, and smooth (`v`, how much the brightness
    // varies over a few pixels), or a white cloud. Leaves lit through are green over blue;
    // bark and needles in the sky's light can be as blue but are darker, greyer or textured.
    // The sheet keeps each view's mask beside it, to check what counted.
    const sky = "(u.b>u.g && u.b>u.r*1.08 && u.b-u.r>0.09 && (u.r+u.g+u.b)>0.9 && v.r<0.012) || (u.r>0.75 && u.g>0.75 && u.b>0.75)";
    const measured = spawnSync(
      "magick",
      [file, "-resize", "480x270!", "(", "+clone", "-colorspace", "gray", "-statistic", "StandardDeviation", "3x3", ")", "-fx", `${sky} ? 1 : 0`, "-write", file.replace(/\.png$/, "-mask.png"), "-format", "%[fx:mean]", "info:"],
      { encoding: "utf8" },
    );
    return Number(measured.stdout) * 100;
  });
  const sorted = [...shares].sort((a, b) => a - b);
  const mean = shares.reduce((sum, s) => sum + s, 0) / shares.length;
  const median = sorted[Math.floor(sorted.length / 2)];
  console.log(`vantage: sky looking up at ${hour}:00, at ${places.map(([x, z], k) => `${x},${z} ${shares[k].toFixed(0)}%`).join(", ")}`);
  console.log(`vantage: sky looking up from eye height: mean ${mean.toFixed(1)}%, median ${median.toFixed(1)}%`);
  // The view up from the middle place, on the sheet.
  return ["-label", `up, ${hour}:00, sky ${mean.toFixed(0)}% (median ${median.toFixed(0)}%)`, resolve(out, `sky-${Math.floor(SKY_PLACES / 2)}.png`)];
}

const sheet = resolve(out, "sheet.jpg");
const montage = spawnSync(
  "magick",
  ["montage", ...tiles, "-tile", `${VIEWS.length + 1}x`, "-geometry", "640x360+4+4", "-pointsize", "18", "-background", "#222", "-fill", "#eee", sheet],
  { encoding: "utf8" },
);
if (montage.status !== 0 || !existsSync(sheet)) {
  console.error(montage.stderr);
  process.exit(1);
}
console.log(`vantage: ${sheet}`);
