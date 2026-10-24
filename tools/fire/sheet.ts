#!/usr/bin/env node
// The fire sheet (docs/fire.md, "The automatic check"; D415): every fire the game draws today,
// at noon, dusk and night, each cropped from its projected bounds and measured against its
// class of real-fire photographs (references.json).
//
//   node tools/fire/sheet.ts [--time noon,dusk,night] [--out shots/fire] [--from <dir>]
//
// One scripted sandbox run an hour on the yard: a campfire (a kindled fire, `staged.spawn`
// with `burns`), a log cabin set alight (the building fire, `fire.light`) and a dragon
// breathing on a stone wall (its stream and the fireball where it strikes). Each fire is shot
// from a fixed camera; at the shot `fire.seen` tells each kind's box on the picture, which is
// the crop measured. Prints one line a fire and hour, ✓ or ✗ on each number, and writes
// `<out>/sheet.jpg` (the crops, a row a fire, a column an hour) and `<out>/table.json`. A run
// is small (the yard), so it skips `~/.cache/terra/gpu.queue` and takes one of the game's own
// shared slots on `gpu.lock` (D319), as every unattended run does. --from measures a folder a run wrote
// (`<hour>/run.log` and its stills) instead of running. Exits 1 when a fire is out of its
// class. Node 22.18 or later, ffmpeg, ImageMagick; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { imageMagick } from "../sandbox/client.ts";
import type { Picture } from "../sandbox/png.ts";
import { type Box, type Held, judge, load, measure, REFERENCES_FILE, references, said } from "./measure.ts";

const ROOT = resolve(import.meta.dirname, "../..");

/// The hours, on the equinox: noon, the sun just down, and full night.
export const HOURS: Record<string, string> = { noon: "12", dusk: "18.4", night: "22" };
const DATE = "09-22";
const PICTURE = "1920x1080";

type Vec3 = [number, number, number];
type Call = { frame: number; method: string; params?: unknown };
/// A fire on the sheet: its name, the class of photographs it is held to, the frame it is
/// shot on, which of `fire.seen`'s boxes is its crop, the camera it is shot from, and whether
/// its flames write the flame share the matte reads (D433): a hearth's and a torch's do, for
/// their film (`film.rs`); a blaze and a breath are tonemapped as the world, leave the matte
/// empty, and are measured by colour.
export type Fire = { name: string; class: string; frame: number; box: "kept" | "blazes" | "stream" | "ball"; eye: Vec3; at: Vec3; matte: boolean };

// Where things stand, in metres east, up and south of the start; the base's cubes, which are
// half a metre, for what is built. The hammer builds only within 14 m of the player, who stays
// at the start (`NEAR_WORK_M`), so the cabin stands 7 m off. The wall, 33 m off, is built by
// naming it (a part aimed at is built at any distance): laid first, its footing is part 0 and
// the wall above part 1. The breath's fireball forms where it strikes it, and with no wall
// there is none.
const CAMPFIRE: Vec3 = [-10, 0, 10];
const CABIN = { min: [4, 0, 8], max: [13, 0, 15] };
const WALL = { min: [60, 0, 20], max: [80, 0, 20] };
const HOVER: Vec3 = [35, 9, 1];
const AIM: Vec3 = [35, 1.5, 10];

export const FIRES: Fire[] = [
  { name: "campfire", class: "campfire", frame: 330, box: "kept", eye: [-10, 1.7, 16], at: [-10, 1, 10], matte: true },
  // The campfire again from RDR2's camp view (D433), the first row kept so history compares:
  // its stone ring an ellipse 0.55 as deep as wide, so the eye 33 degrees over the ring and
  // the camera pitched 26 down, the ring 15% of the picture's width, which at our 45-degree
  // lens is 4.8 m off, about 8.5 flame heights (the first row: 16 degrees, 6.2 m).
  { name: "campfire above", class: "campfire", frame: 420, box: "kept", eye: [-10, 2.6, 14], at: [-10, 0.65, 10], matte: true },
  { name: "house blaze", class: "structure", frame: 760, box: "blazes", eye: [4.25, 3.5, 22], at: [4.25, 2.5, 5.75], matte: false },
  { name: "breath", class: "flamethrower", frame: 960, box: "stream", eye: [13, 5.5, 4], at: [35, 4.5, 6], matte: false },
  { name: "fireball", class: "fireball", frame: 960, box: "ball", eye: [13, 5.5, 4], at: [35, 4.5, 6], matte: false },
];

/// The run's calls: the scene set up, then each fire's camera held from a second before its
/// shot, and `fire.seen` on the shot's frame.
export function calls(): Call[] {
  const out: Call[] = [
    { frame: 2, method: "staged.spawn", params: { name: "campfire", model: "models/props/Campfire.glb", at: CAMPFIRE, facing: [CAMPFIRE[0], 0, CAMPFIRE[2] + 5], base: true, burns: [1.5, 0.7, 0.9] } },
    // A plan is only laid; the hammer's steps build it (free and instant on the yard), or the
    // cabin is a blueprint with nothing to burn and the wall a blue ghost in the fireball's crop.
    { frame: 3, method: "build.lay", params: { what: "wall", min: WALL.min, max: WALL.max, kind: "stone" } },
    { frame: 4, method: "build.fill", params: { part: 0, steps: 100000 } },
    { frame: 5, method: "build.fill", params: { part: 1, steps: 100000 } },
    { frame: 6, method: "build.lay", params: { what: "room", min: CABIN.min, max: CABIN.max, kind: "logs", floor: "stone" } },
    { frame: 7, method: "build.lay", params: { what: "roof", over: [8, 0, 11], kind: "logs", style: "Gable" } },
    { frame: 9, method: "build.fill", params: { steps: 100000 } },
    { frame: 10, method: "flight.spawn", params: { kind: "Dragon", at: [35, 14, -40], facing: HOVER, base: true } },
    { frame: 12, method: "flight.hover", params: { at: HOVER, facing: AIM, base: true } },
    // The cabin's south wall set alight long before its shot, so its fire has spread.
    { frame: 60, method: "fire.light", params: { at: [CABIN.min[0], 0, CABIN.max[2]], to: [CABIN.max[0], 5, CABIN.max[2]] } },
    { frame: 840, method: "flight.breathe", params: { on: true, at: AIM, base: true } },
  ];
  for (const fire of FIRES) {
    for (let f = fire.frame - 40; f <= fire.frame; f++) out.push({ frame: f, method: "build.look", params: { from: fire.eye, at: fire.at } });
    out.push({ frame: fire.frame, method: "fire.seen" });
  }
  return out.sort((a, b) => a.frame - b.frame);
}

const frames = () => [...new Set(FIRES.map((f) => f.frame))].sort((a, b) => a - b);
const still = (dir: string, frame: number) => join(dir, `f-${String(frame).padStart(5, "0")}.png`);
/// A still's flame matte (`--flame-matte`, D433), beside it.
const matteOf = (file: string) => file.replace(/\.png$/, ".matte.png");

/// The GPU memory a yard run needs free, in MiB: the shared slots count runs, not their size,
/// so beside a big town run (7 GB) the yard ran out of memory and drew nothing, as it did
/// with 3.5 GB free.
const ROOM = 5000;

/// Waits until the GPU has `ROOM` free, saying so once; at once where nvidia-smi is missing.
function waitForRoom() {
  for (let told = false; ; told = true) {
    const smi = spawnSync("nvidia-smi", ["--query-gpu=memory.free", "--format=csv,noheader,nounits"], { encoding: "utf8" });
    const free = Number(smi.stdout?.trim().split("\n")[0]);
    if (smi.status !== 0 || !(free < ROOM)) return;
    if (!told) console.log(`fire: waiting for ${ROOM} MiB free on the GPU (${free} free)`);
    spawnSync("sleep", ["10"]);
  }
}

/// Runs the scene at an hour into `dir`, again once if the GPU ran out of memory.
function run(hour: string, dir: string) {
  for (let tries = 0; ; tries++) {
    waitForRoom();
    runOnce(hour, dir);
    if (tries > 0 || !readFileSync(join(dir, "run.log"), "utf8").includes("rendering error: Out of Memory")) break;
    console.log(`fire: the GPU ran out of memory at ${hour}; again`);
  }
  if (!existsSync(still(dir, frames().at(-1)!))) throw new Error(`the run at ${hour} made no still at frame ${frames().at(-1)}: see ${join(dir, "run.log")}`);
}

/// Runs the scene at an hour into `dir`: its calls, its stills and what it said.
function runOnce(hour: string, dir: string) {
  mkdirSync(dir, { recursive: true });
  const callsFile = join(dir, "calls.jsonl");
  writeFileSync(callsFile, calls().map((c) => JSON.stringify(c)).join("\n") + "\n");
  const args = [
    ...["900", resolve(ROOT, "target/release/terra_sandbox"), "--offscreen", "--clean", "--picture", PICTURE],
    ...["--experiment", "play", "--level", "yard", "--free-build", "--mute", "--at", "0,0", "--face", "0,30"],
    ...["--time", HOURS[hour] ?? hour, "--date", DATE, "--calls", callsFile, "--shots", `${join(dir, "f")}@${frames().join(",")}`, "--flame-matte"],
  ];
  const done = spawnSync("timeout", args, { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT }, maxBuffer: 1 << 28 });
  writeFileSync(join(dir, "run.log"), `${done.stdout}${done.stderr}`);
}

/// Where the play HUD begins, as a share of the picture's height: the hints, bars and action
/// bar fill the fifth below it, and a crop stops above it.
const HUD_TOP = 0.78;

/// A crop cut off where the HUD begins.
export function belowHud(box: Box, height: number): Box {
  return { ...box, h: Math.max(0, Math.min(box.h, Math.floor(HUD_TOP * height) - box.y)) };
}

/// How lit the ground and walls round a fire are (docs/fire.md "The metrics"): the mean
/// linear luminance of the band round its crop, out to its width either side and down to its
/// height below, the crop itself left out; and how warm it is, red over blue. At dusk and at
/// night it is read against the same band at noon, so a fire whose light reaches nothing round
/// it reads near nought however bright its flame.
export function surround(pic: Picture, box: Box): { light: number; warmth: number } {
  const lin = (v: number) => {
    const c = v / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  const [x0, x1] = [Math.max(0, box.x - box.w), Math.min(pic.width, box.x + 2 * box.w)];
  const [y0, y1] = [Math.max(0, box.y + Math.round(box.h / 2)), Math.min(pic.height, box.y + 2 * box.h)];
  let [n, r, g, b] = [0, 0, 0, 0];
  for (let y = y0; y < y1; y++)
    for (let x = x0; x < x1; x++) {
      if (x >= box.x && x < box.x + box.w && y >= box.y && y < box.y + box.h) continue;
      const i = 4 * (y * pic.width + x);
      [r, g, b, n] = [r + lin(pic.rgba[i]), g + lin(pic.rgba[i + 1]), b + lin(pic.rgba[i + 2]), n + 1];
    }
  if (!n) return { light: 0, warmth: 0 };
  return { light: (0.2126 * r + 0.7152 * g + 0.0722 * b) / n, warmth: r / Math.max(b, 1e-6) };
}

/// What `fire.seen` said on a frame: each kind's box and the picture's size.
export function seenAt(log: string, frame: number): Record<string, string> {
  const line = log.split("\n").find((l) => l.includes(`Calls: frame ${frame}: fire.seen: fire seen:`));
  if (!line) return {};
  const out: Record<string, string> = {};
  for (const part of line.split("fire seen:")[1].split(";")) {
    const [k, v] = part.trim().split(/\s+/);
    if (k && v && v !== "none") out[k] = v;
  }
  return out;
}

/// A box told on a picture of one size, on the still's own.
export function scaled(crop: string, picture: string, w: number, h: number): Box {
  const [pw, ph] = picture.split("x").map(Number);
  const [, bw, bh, bx, by] = /^(\d+)x(\d+)\+(\d+)\+(\d+)$/.exec(crop)!.map(Number);
  const sx = w / pw;
  const sy = h / ph;
  const x = Math.max(0, Math.floor(bx * sx));
  const y = Math.max(0, Math.floor(by * sy));
  return { x, y, w: Math.min(w - x, Math.round(bw * sx)), h: Math.min(h - y, Math.round(bh * sy)) };
}

/// What each fire and hour has out of its class today (`--hold` writes it): the push check
/// fails a fire only where it is worse than this, until the stage that fixes it lands
/// (docs/fire.md, "The plan").
export const HELD_FILE = resolve(import.meta.dirname, "held.json");
export type HeldOut = Record<string, string[]>;

/// Each fire and hour's numbers out of their class; "seen" for one not seen at all.
export function outOf(rows: Row[]): HeldOut {
  return Object.fromEntries(rows.map((r) => [`${r.fire} ${r.hour}`, r.out < 0 ? ["seen"] : r.held.filter((h) => !h.holds).map((h) => h.called)]));
}

/// Where a run is worse than what is held: a number out that held, or a fire not seen.
export function worseThan(rows: Row[], held: HeldOut): string[] {
  return Object.entries(outOf(rows)).flatMap(([fire, out]) => {
    const was = held[fire] ?? [];
    const row = rows.find((r) => `${r.fire} ${r.hour}` === fire)!;
    return out
      .filter((n) => !was.includes(n))
      .map((n) => {
        const h = row.held.find((x) => x.called === n);
        return h ? `${fire}: ${n} ${h.value.toFixed(h.digits)} fell out of ${h.low.toFixed(h.digits)}-${h.high.toFixed(h.digits)}` : `${fire}: not seen`;
      });
  });
}

export type Row = { fire: string; class: string; hour: string; crop?: string; out: number; held: Held[]; tile?: string; ground?: { light: number; warmth: number } };

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const arg = (name: string, fallback: string) => {
    const i = argv.indexOf(name);
    return i >= 0 ? argv[i + 1] : fallback;
  };
  const hours = arg("--time", "noon,dusk,night").split(",");
  const from = arg("--from", "");
  const out = resolve(ROOT, from || arg("--out", "shots/fire"));
  mkdirSync(out, { recursive: true });
  if (!from) {
    const build = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: ROOT, stdio: "inherit" });
    if (build.status !== 0) process.exit(1);
  }
  const gate = argv.includes("--gate");
  // Until the first run is held, the gate has nothing to hold a fire to and takes no GPU.
  if (gate && !existsSync(HELD_FILE)) {
    console.log(`⚠ fire: nothing held yet (${HELD_FILE}); run \`cargo xtask fire --hold\``);
    process.exit(0);
  }
  const lines: string[] = [];
  /// A line of the table: printed, or under --gate kept only in `<out>/sheet.txt` (D388).
  const tell = (line: string) => {
    lines.push(line);
    if (!gate) console.log(line);
  };
  const classes = existsSync(REFERENCES_FILE) ? references().classes : {};
  const rows: Row[] = [];
  for (const hour of hours) {
    const dir = join(out, hour);
    if (!from) {
      const started = Date.now();
      run(hour, dir);
      tell(`fire: ${hour} shot in ${Math.round((Date.now() - started) / 1000)} s`);
    }
    const log = readFileSync(join(dir, "run.log"), "utf8");
    for (const fire of FIRES) {
      const seen = seenAt(log, fire.frame);
      const file = still(dir, fire.frame);
      const tile = join(dir, `${fire.name.replace(/ /g, "-")}.png`);
      if (!seen[fire.box] || !existsSync(file)) {
        rows.push({ fire: fire.name, class: fire.class, hour, out: -1, held: [] });
        tell(`✗ ${fire.name.padEnd(12)} ${hour.padEnd(6)} not seen (${fire.box}): see ${join(dir, "run.log")}`);
        continue;
      }
      const pic = load(file);
      const box = belowHud(scaled(seen[fire.box], seen.picture, pic.width, pic.height), pic.height);
      const crop = `${box.w}x${box.h}+${box.x}+${box.y}`;
      const known = classes[fire.class];
      // Measured on the flame the renderer drew (D433); a folder from before the matte, by colour.
      const matte = fire.matte && existsSync(matteOf(file)) ? load(matteOf(file)) : undefined;
      const held = known ? judge(measure(pic, box, matte), known.bands) : [];
      const fails = held.filter((h) => !h.holds).length;
      const ground = surround(pic, box);
      rows.push({ fire: fire.name, class: fire.class, hour, crop, out: known ? fails : -1, held, tile, ground });
      tell(`${fails || !known ? "✗" : "✓"} ${fire.name.padEnd(12)} ${hour.padEnd(6)} as ${fire.class}: ${known ? said(held) : "no such class in references.json"}`);
      const noon = rows.find((r) => r.fire === fire.name && r.hour === "noon")?.ground;
      if (hour !== "noon" && noon?.light)
        tell(`  ${fire.name} ${hour}: the ground round it ${((100 * ground.light) / noon.light).toFixed(1)}% as lit as at noon, red over blue ${ground.warmth.toFixed(2)} (noon ${noon.warmth.toFixed(2)})`);
      spawnSync(...imageMagick(["convert", file, "-crop", `${crop}`, "+repage", "-resize", "x300", tile]));
    }
  }
  // The sheet: a row a fire, a column an hour, each crop labelled with how many numbers fall out.
  const tiles = FIRES.flatMap((fire) =>
    hours.flatMap((hour) => {
      const row = rows.find((r) => r.fire === fire.name && r.hour === hour)!;
      const label = `${fire.name} ${hour}: ${row.out < 0 ? "not seen" : row.out ? `${row.out} out` : "holds"}`;
      return row.tile && existsSync(row.tile) ? ["-label", label, row.tile] : ["-label", label, "xc:gray20[300x300]"];
    }),
  );
  const sheet = join(out, "sheet.jpg");
  spawnSync(...imageMagick(["montage", ...tiles, "-tile", `${hours.length}x`, "-geometry", "+6+6", "-background", "#202020", "-fill", "white", "-pointsize", "18", sheet]));
  writeFileSync(join(out, "table.json"), JSON.stringify(rows.map(({ tile: _t, ...r }) => r), null, 1));
  writeFileSync(join(out, "sheet.txt"), `${lines.join("\n")}\n`);
  if (argv.includes("--hold")) {
    writeFileSync(HELD_FILE, `${JSON.stringify(outOf(rows), null, 1)}\n`);
    console.log(`fire: held ${HELD_FILE}`);
  }
  if (gate) {
    const worse = worseThan(rows, existsSync(HELD_FILE) ? JSON.parse(readFileSync(HELD_FILE, "utf8")) : {});
    for (const line of worse) console.log(`✗ ${line}`);
    const within = rows.filter((r) => r.out === 0).length;
    console.log(`${worse.length ? "✗" : "✓"} fire: ${worse.length ? `${worse.length} worse than held` : "no fire worse than held"} (${within} of ${rows.length} in their class); table ${join(out, "sheet.txt")}, sheet ${sheet}`);
    process.exit(worse.length ? 1 : 0);
  }
  const bad = rows.filter((r) => r.out !== 0);
  console.log(`${bad.length ? "✗" : "✓"} fire: ${rows.length - bad.length} of ${rows.length} hold; sheet ${sheet}`);
  process.exit(bad.length ? 1 : 0);
}
