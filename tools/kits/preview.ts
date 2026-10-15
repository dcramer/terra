#!/usr/bin/env node
// A material's preview sheet (D349, docs/proposals/construction-002.md): one picture of it
// on every role and shape its pattern lays, from 10 m and from 2 m, at noon and in the
// evening, for a developer to look over once when the material is added. The kit sheet lays
// its preview alone (`--sheet-rows "<name>: preview"`, crates/terra_game/src/build/sheet.rs):
// a house with a doorway, windows, a foot on the ground, a gable and a floor, beside a
// platform with a flight of steps, a top, a slab and a wall part-built, a hole carved in the
// house's front and its corner damaged; for a roof covering, an L of stone walls under a hip
// with a gabled wing, a valley where they meet, part of it damaged and part not yet laid.
// It is laid west of the start, the same for every material, so its views are fixed here.
//
//   node tools/kits/preview.ts <material> [--hours 12,18] [--out shots/preview]
//
// One scripted run an hour on the release build, offscreen, its views one after another
// (`--vantage …@frame`, `--shots`), then a sheet of them, `shots/preview/<material>.jpg`.
// Under WSL the Linux build draws on the CPU: it works, slowly (a few minutes an hour).
// Needs ImageMagick. Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, renameSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { parseArgs } from "node:util";

const ROOT = resolve(dirname(new URL(import.meta.url).pathname), "../..");

/** Where the kit sheet lays a preview's origin, in metres east and south of the start
 * (`PREVIEW` in sheet.rs). */
export const ORIGIN = [-40, 0];

/** A view: its name, and its eye and what it looks at in the preview's own metres (east,
 * up from the ground, south), as `--vantage` takes them with `--vantage-ground`. */
type View = [string, number[]];

/** A wall material's views: the whole from the street, 10 m off; its front close, the
 * doorway's jamb, the carved hole, the damage and its foot; the steps, the platform's top
 * and the wall part-built close. */
const WALLS: View[] = [
  ["10 m", [6.25, 5, 15, 6.25, 1.5, 2]],
  ["2 m: openings, a carved hole, damage, the foot", [3.9, 1.3, 6.1, 3.9, 1.0, 4.0]],
  ["2 m: steps, a top, part-built", [8.4, 2.3, 6.3, 8.4, 0.9, 3.0]],
];

/** A roof covering's views: the whole L from the south-east, 10 m off; the valley and the
 * eaves either side of it close; the wing's gable, its verge and ridge's end close. */
const ROOFS: View[] = [
  ["10 m", [12, 7, 14, 3.5, 2.5, 4]],
  ["2 m: valley and eaves", [6.5, 3.6, 6.5, 4.5, 3.0, 4.5]],
  ["2 m: verge, ridge's end, damage", [2, 4.4, 10.5, 2, 3.6, 7.5]],
];

/** The views for a material by its pattern, in the start's metres. */
export function views(pattern: string): View[] {
  return (pattern === "roof" ? ROOFS : WALLS).map(([name, v]) => [
    name,
    [v[0] + ORIGIN[0], v[1], v[2] + ORIGIN[1], v[3] + ORIGIN[0], v[4], v[5] + ORIGIN[1]],
  ]);
}

/** How many frames each view is held: the preview laid and the camera settled. */
export const HOLD = 240;

/** The sandbox's arguments for one hour's run, and the frames its pictures are taken on. */
export function run(material: string, pattern: string, hour: string, prefix: string): { args: string[]; frames: number[] } {
  const all = views(pattern);
  const frames = all.map((_, i) => i * HOLD + HOLD - 10);
  const args = [
    "--experiment", "play", "--level", "yard", "--kit-sheet", "--sheet-rows", `${material}: preview`,
    "--offscreen", "--clean", "--time", hour, "--vantage-ground",
    ...all.flatMap(([, v], i) => ["--vantage", `${v.join(",")}@${i * HOLD}`]),
    "--shots", `${prefix}@${frames.join(",")}`,
  ];
  return { args, frames };
}

/** The command that makes a material's preview sheet, as `cargo xtask pack` prints it. */
export const command = (material: string) => `node tools/kits/preview.ts ${material}`;

function patternOf(material: string): string {
  const file = join(ROOT, "assets/data/kits", `${material}.ron`);
  if (!existsSync(file)) throw new Error(`no material ${material} (assets/data/kits/${material}.ron)`);
  const found = readFileSync(file, "utf8").match(/pattern:\s*"(\w+)"/);
  if (!found) throw new Error(`${material}.ron names no pattern`);
  return found[1];
}

async function main() {
  const { positionals, values } = parseArgs({
    allowPositionals: true,
    options: { hours: { type: "string", default: "12,18" }, out: { type: "string", default: "shots/preview" }, help: { type: "boolean", short: "h" } },
  });
  const [material] = positionals;
  if (!material || values.help) {
    console.log("usage: node tools/kits/preview.ts <material> [--hours 12,18] [--out shots/preview]");
    process.exit(material ? 0 : 2);
  }
  const pattern = patternOf(material);
  const out = resolve(ROOT, values.out!);
  mkdirSync(out, { recursive: true });
  const built = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: ROOT, stdio: "inherit" });
  if (built.status !== 0) throw new Error("the sandbox does not build");
  const tiles: string[] = [];
  for (const hour of values.hours!.split(",")) {
    const prefix = join(out, `${material}-${hour}`);
    const { args, frames } = run(material, pattern, hour, prefix);
    const started = Date.now();
    const done = spawnSync(join(ROOT, "target/release/terra_sandbox"), args, { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT } });
    const said = `${done.stdout}${done.stderr}`;
    if (done.status !== 0 || !said.includes("SANDBOX saved")) {
      console.error(said.split("\n").slice(-20).join("\n"));
      throw new Error(`the run at ${hour}:00 failed`);
    }
    views(pattern).forEach(([name], i) => {
      const file = join(out, `${material}-${hour}-${i}.png`);
      renameSync(`${prefix}-${String(frames[i]).padStart(5, "0")}.png`, file);
      tiles.push("-label", `${name}, ${hour}:00`, file);
    });
    console.log(`preview: ${material} at ${hour}:00 in ${((Date.now() - started) / 1000).toFixed(0)} s`);
  }
  const sheet = join(out, `${material}.jpg`);
  const made = spawnSync("montage", [...tiles, "-tile", `${views(pattern).length}x`, "-geometry", "640x360+4+4", "-title", `${material} (${pattern} pattern)`, sheet], { encoding: "utf8" });
  if (made.status !== 0) throw new Error(`montage: ${made.stderr}`);
  console.log(`preview: ${sheet}`);
}

if (import.meta.main) {
  try {
    await main();
  } catch (error: any) {
    console.error(`preview: ${error?.message ?? error}`);
    process.exit(1);
  }
}
