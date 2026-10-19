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
// `<out>/sheet.jpg` (the crops, a row a fire, a column an hour) and `<out>/table.json`. Every
// run takes the GPU through `~/.cache/terra/gpu.queue`. --from measures a folder a run wrote
// (`<hour>/run.log` and its stills) instead of running. Exits 1 when a fire is out of its
// class. Node 22.18 or later, ffmpeg, ImageMagick; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";
import { imageMagick } from "../sandbox/client.ts";
import { type Box, type Held, judge, load, measure, REFERENCES_FILE, references, said } from "./measure.ts";

const ROOT = resolve(import.meta.dirname, "../..");
const QUEUE = join(homedir(), ".cache/terra/gpu.queue");

/// The hours, on the equinox: noon, the sun just down, and full night.
export const HOURS: Record<string, string> = { noon: "12", dusk: "18.4", night: "22" };
const DATE = "09-22";
const PICTURE = "1920x1080";

type Vec3 = [number, number, number];
type Call = { frame: number; method: string; params?: unknown };
/// A fire on the sheet: its name, the class of photographs it is held to, the frame it is
/// shot on, which of `fire.seen`'s boxes is its crop, and the camera it is shot from.
export type Fire = { name: string; class: string; frame: number; box: "kept" | "blazes" | "stream" | "ball"; eye: Vec3; at: Vec3 };

// Where things stand, in metres east, up and south of the start; cubes are half a metre.
const CAMPFIRE: Vec3 = [-10, 0, 10];
const CABIN = { min: [20, 0, 20], max: [29, 0, 27] };
const WALL = { min: [60, 0, 20], max: [80, 0, 20] };
const HOVER: Vec3 = [35, 9, 1];
const AIM: Vec3 = [35, 1.5, 10];

export const FIRES: Fire[] = [
  { name: "campfire", class: "campfire", frame: 330, box: "kept", eye: [-10, 1.7, 16], at: [-10, 1, 10] },
  { name: "house blaze", class: "structure", frame: 760, box: "blazes", eye: [12.25, 3.5, 30], at: [12.25, 3, 12] },
  { name: "breath", class: "flamethrower", frame: 960, box: "stream", eye: [21, 5, 5], at: [35, 4.5, 6] },
  { name: "fireball", class: "fireball", frame: 960, box: "ball", eye: [21, 5, 5], at: [35, 4.5, 6] },
];

/// The run's calls: the scene set up, then each fire's camera held from a second before its
/// shot, and `fire.seen` on the shot's frame.
export function calls(): Call[] {
  const out: Call[] = [
    { frame: 2, method: "staged.spawn", params: { name: "campfire", model: "models/props/Campfire.glb", at: CAMPFIRE, facing: [CAMPFIRE[0], 0, CAMPFIRE[2] + 5], base: true, burns: [1.5, 0.7, 0.9] } },
    { frame: 4, method: "build.lay", params: { what: "room", min: CABIN.min, max: CABIN.max, kind: "logs", floor: "stone" } },
    { frame: 6, method: "build.lay", params: { what: "roof", over: [24, 0, 23], kind: "logs", style: "Gable" } },
    { frame: 8, method: "build.lay", params: { what: "wall", min: WALL.min, max: WALL.max, kind: "stone" } },
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

/// Runs the scene at an hour into `dir`: its calls, its stills and what it said.
function run(hour: string, dir: string) {
  mkdirSync(dir, { recursive: true });
  const callsFile = join(dir, "calls.jsonl");
  writeFileSync(callsFile, calls().map((c) => JSON.stringify(c)).join("\n") + "\n");
  const last = frames().at(-1)!;
  const args = [
    ...[QUEUE, "timeout", "900", resolve(ROOT, "target/release/terra_sandbox"), "--offscreen", "--clean", "--picture", PICTURE],
    ...["--experiment", "play", "--level", "yard", "--free-build", "--mute", "--at", "0,0", "--face", "0,30"],
    ...["--time", HOURS[hour] ?? hour, "--date", DATE, "--calls", callsFile, "--shots", `${join(dir, "f")}@${frames().join(",")}`],
  ];
  const done = spawnSync("flock", args, { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT }, maxBuffer: 1 << 28 });
  writeFileSync(join(dir, "run.log"), `${done.stdout}${done.stderr}`);
  if (!existsSync(still(dir, last))) throw new Error(`the run at ${hour} made no still at frame ${last}: see ${join(dir, "run.log")}`);
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

export type Row = { fire: string; class: string; hour: string; crop?: string; out: number; held: Held[]; tile?: string };

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
      const box = scaled(seen[fire.box], seen.picture, pic.width, pic.height);
      const crop = `${box.w}x${box.h}+${box.x}+${box.y}`;
      const known = classes[fire.class];
      const held = known ? judge(measure(pic, box), known.bands) : [];
      const fails = held.filter((h) => !h.holds).length;
      rows.push({ fire: fire.name, class: fire.class, hour, crop, out: known ? fails : -1, held, tile });
      tell(`${fails || !known ? "✗" : "✓"} ${fire.name.padEnd(12)} ${hour.padEnd(6)} as ${fire.class}: ${known ? said(held) : "no such class in references.json"}`);
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
