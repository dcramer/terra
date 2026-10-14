#!/usr/bin/env node
// The vantage sheet (docs/biomes.md, "How we'd know"; D69): the lush forest from fixed views,
// the forest floor at eye height, its edge, the clearing, an overlook, close along a path and
// into a spruce grove, at each hour asked
// for, side by side in one picture, so a change to how the world looks is judged on the same
// views every time. One scripted run per hour moves its camera from view to view
// (--vantage …@frame) and saves each with --shots; at the first hour, how closed the canopy
// is, and how much of a view at eye height the undergrowth hides, from a run driven live.
//
//   node tools/sandbox/vantage.ts [--time 13,17] [--out shots/vantage]
//
// Writes <out>/<hour>-<view>.png and <out>/sheet.jpg. Needs ImageMagick (cargo xtask doctor).
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, renameSync } from "node:fs";
import { resolve } from "node:path";
import * as game from "./client.ts";
import { ROOT, imageMagick } from "./client.ts";

/// Each view: a name, where the camera stands and what it looks at, in metres east, up and
/// south of the start. The forest is 220 m round the start, with a clearing of 14 m.
const VIEWS: [string, string][] = [
  ["floor", "-56,1.7,-43,-72,1.0,-55"],
  ["edge", "0,1.8,250,0,5,200"],
  ["clearing", "7,1.8,9,-8,1.2,-7"],
  ["overlook", "0,45,150,0,0,0"],
  // Close along a path at eye height, where the floor, undergrowth and trunks are judged.
  ["path", "-54,1.6,-41,-60,0.5,-48"],
  // Into a spruce grove.
  ["grove", "-21.9,1.7,-100,-21.9,4,-114"],
];
/// Frames each view is held for before it is pictured: grass is built round the camera as
/// it arrives, and levels of detail fade in.
const HOLD = 200;
/// How many places the canopy is looked up through, and the frames each is held.
const SKY_PLACES = 12;
const SKY_HOLD = 60;
/// How many places the middle layer's blocking is measured at, and how near the undergrowth
/// it counts is.
const BLOCK_PLACES = 16;
const BLOCK_M = 20;
/// What is within this of the eye is left out of every picture, as a walker steps aside
/// from a bush rather than standing in it.
const ASIDE_M = 1.2;

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
  if (hour === hours[0]) tiles.push(...canopy(hour), ...(await blocking(hour)));
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
      ...imageMagick([file, "-resize", "480x270!", "(", "+clone", "-colorspace", "gray", "-statistic", "StandardDeviation", "3x3", ")", "-fx", `${sky} ? 1 : 0`, "-write", file.replace(/\.png$/, "-mask.png"), "-format", "%[fx:mean]", "info:"]),
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

/// How much of a view at eye height the undergrowth within 20 m hides: shrubs, saplings,
/// snags, ferns and the rest, the middle layer that ends sightlines in a real forest (Sons of
/// the Forest rarely lets one see past 20 to 30 m). At places on a spiral 30 to 160 m from
/// the start, eye 1.7 m over the ground, looking level, a run driven over the remote protocol
/// stops the clock and the wind and pictures each view, leaving out what is within 1.2 m as
/// a walker steps aside from a bush, then again with what the camera sees of the undergrowth
/// within 20 m hidden (`lush.undergrowth`, shadows left as they were), and
/// again with it back, to know the noise; what differs between the first two is what it
/// hides. Reported for the band round the horizon, the middle fifth of the picture, which is
/// what a sightline crosses, and for the whole picture. The forest's aim is 30 to 60% of the
/// band.
async function blocking(hour: string): Promise<string[]> {
  const places = Array.from({ length: BLOCK_PLACES }, (_, k) => {
    const distance = 30 + (130 * k) / (BLOCK_PLACES - 1);
    const angle = k * 2.39996 + 0.7;
    const [x, z] = [Math.cos(angle) * distance, Math.sin(angle) * distance];
    const heading = angle + 1.9 + k;
    return [x, z, x + Math.cos(heading) * 30, z + Math.sin(heading) * 30].map(Math.round);
  });
  const view = (p: number[]) => [p[0], 1.7, p[1], p[2], 1.7, p[3]];
  await game.start(
    ["--offscreen", "--clean", "--experiment", "walk", "--experiment", "lush", "--level", "yard", "--time", hour, "--vantage-ground", "--vantage", view(places[0]).join(",")],
    { release: true },
  );
  await game.waitFor("undergrowth:", { since: 0, timeout: 120 });
  const share = (mask: string, band: boolean) =>
    Number(spawnSync(...imageMagick([mask, ...(band ? ["-gravity", "center", "-crop", "100%x20%+0+0", "+repage"] : []), "-format", "%[fx:mean]", "info:"]), { encoding: "utf8" }).stdout) * 100;
  // A pixel counts where any channel changed by more than 4% of white, and only in patches:
  // the opening takes out the scattered pixels TAA and the haze's jitter change, and the
  // closing fills the speckle where leaves gave way to leaves of a like colour behind them.
  const differ = (a: string, b: string, mask: string) => {
    const done = spawnSync(...imageMagick([a, b, "-resize", "480x270!", "-compose", "difference", "-composite", "-separate", "-evaluate-sequence", "max", "-threshold", "4%", "-morphology", "Open", "Disk:1", "-morphology", "Close", "Disk:2", mask]), { encoding: "utf8" });
    if (done.status !== 0) throw new Error(done.stderr);
    return [share(mask, true), share(mask, false)];
  };
  const results: { band: number; whole: number; noise: number }[] = [];
  for (const [k, p] of places.entries()) {
    await game.call("sandbox.look", { vantage: view(p) });
    // Grass is built round the camera as it arrives, and levels of detail fade in.
    await game.waitSeconds(4);
    await game.call("sandbox.pause", { paused: true });
    await game.call("lush.undergrowth", { hidden: true, within: ASIDE_M });
    await game.sleep(1500);
    const seen = await game.shot(resolve(out, `block-${k}.png`));
    await game.call("lush.undergrowth", { hidden: true, within: BLOCK_M });
    await game.sleep(1500);
    const bare = await game.shot(resolve(out, `block-${k}-bare.png`));
    await game.call("lush.undergrowth", { hidden: true, within: ASIDE_M });
    await game.sleep(1500);
    const again = await game.shot(resolve(out, `block-${k}-again.png`));
    await game.call("lush.undergrowth", { hidden: false });
    await game.call("sandbox.pause", { paused: false });
    const [band, whole] = differ(seen, bare, resolve(out, `block-${k}-mask.png`));
    const [noise] = differ(seen, again, resolve(out, `block-${k}-noise.png`));
    results.push({ band, whole, noise });
  }
  await game.stop();
  const mean = (pick: (r: (typeof results)[0]) => number) => results.reduce((sum, r) => sum + pick(r), 0) / results.length;
  console.log(`vantage: undergrowth within ${BLOCK_M} m hides, at eye height at ${hour}:00: ${places.map((p, k) => `${p[0]},${p[1]} ${results[k].band.toFixed(0)}%`).join(", ")}`);
  console.log(`vantage: undergrowth within ${BLOCK_M} m hides at eye height: mean ${mean((r) => r.band).toFixed(1)}% of the band round the horizon (median ${[...results].sort((a, b) => a.band - b.band)[Math.floor(results.length / 2)].band.toFixed(1)}%), ${mean((r) => r.whole).toFixed(1)}% of the picture (noise ${mean((r) => r.noise).toFixed(1)}%)`);
  // The place nearest the band's mean, on the sheet, and what counted.
  const typical = results.reduce((best, r, k) => (Math.abs(r.band - mean((q) => q.band)) < Math.abs(results[best].band - mean((q) => q.band)) ? k : best), 0);
  return [
    "-label", `eye height, ${hour}:00, undergrowth hides ${mean((r) => r.band).toFixed(0)}% of the horizon band`, resolve(out, `block-${typical}.png`),
    "-label", `what it hides there (${results[typical].band.toFixed(0)}%)`, resolve(out, `block-${typical}-mask.png`),
  ];
}

const sheet = resolve(out, "sheet.jpg");
const montage = spawnSync(
  ...imageMagick(["montage", ...tiles, "-tile", `${VIEWS.length + 1}x`, "-geometry", "640x360+4+4", "-pointsize", "18", "-background", "#222", "-fill", "#eee", sheet]),
  { encoding: "utf8" },
);
if (montage.status !== 0 || !existsSync(sheet)) {
  console.error(montage.stderr);
  process.exit(1);
}
console.log(`vantage: ${sheet}`);
