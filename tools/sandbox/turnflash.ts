#!/usr/bin/env node
// No frame goes dark while the view turns (a continuous check, tools/continuous/checks.json):
// the play start at dusk, as turnlight.ts, the view turned through the four quarters twice
// (the second round with every pipeline made), every frame of each turn shot; no frame's
// land may be darker than DARK of the turn's two ends, nor its near-black share over theirs
// by more than BLACK_OVER. Bevy 0.20 gave a view the `IRRADIANCE_VOLUME` shader def only
// while a volume was in its frustum, so the den's volume entering or leaving the picture
// changed every material's pipeline and the view's bind group layout in the middle of a
// turn, and grass and needles drew black until the new ones were made (light-fragment j11,
// j12; the patch: every 3D view holds a list of volumes, empty or not, patches/README.md).
//
//   node tools/sandbox/turnflash.ts [--out target/check/turnflash] [--from <dir>] [--frames]
//
// Builds the release sandbox, then runs it through the GPU scheduler (`cargo xtask gpu still`).
// --from measures a folder a run wrote instead; --frames prints every frame's numbers. A line
// a stage; exits 1 on a dark frame. Deletes the stills of a run that passes.
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { light } from "./groundlight.ts";
import { type Picture, readPng } from "./png.ts";

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
/// The first aim (north, unshot), the first shot turn, the frames between turns, the turns
/// (twice round), and the frames shot before and after each turn's start.
const FIRST = 120;
const TURNS_FROM = 300;
const EVERY = 150;
const TURNS = 8;
const BEFORE = 5;
const AFTER = 60;
/// The ground and the player, as fractions of the picture (turnlight's), and the whole
/// picture under the top edge, where grass, trunks and crowns stand.
const LAND = [[0.2, 0.6, 0.8, 0.85]];
const ALL = [0.0, 0.1, 1.0, 0.9];
/// A pixel is near black with every channel under this of its picture's mean light, in
/// linear light: the eye opens and closes after a turn, so a count under a fixed value moves
/// by points with no fault (2.0 and 3.8% of one view in two runs at 6 of 255).
const BLACK_OF = 0.03;
/// The least a frame's land, over its whole picture's light, may be of the darker end of
/// its turn; the most its near-black share may stand over the blacker end's, as a share of
/// the picture; and the least its whole picture's light may be of the darker end's (the eye
/// moves it by a half and more, a picture not drawn is under a twentieth).
const DARK = 0.8;
const BLACK_OVER = 0.02;
const DIM = 0.1;

const turnAt = (t: number) => TURNS_FROM + t * EVERY;
const quarterOf = (t: number) => QUARTERS[(t + 1) % 4];
const framesOf = (t: number) => Array.from({ length: BEFORE + AFTER }, (_, i) => turnAt(t) - BEFORE + i);
const stillOf = (out: string, frame: number) => join(out, `f-${String(frame).padStart(5, "0")}.png`);

function run(out: string): boolean {
  const build = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: ROOT, encoding: "utf8" });
  if (build.status !== 0) {
    console.log(`turnflash: ✗ build\n${build.stderr.split("\n").slice(-20).join("\n")}`);
    return false;
  }
  console.log("turnflash: built");
  rmSync(out, { recursive: true, force: true });
  mkdirSync(out, { recursive: true });
  const aim = (frame: number, [, east, south]: [string, number, number]) => ({ frame, method: "play.aim", params: { at: [START[0] + east, START[1] + south] } });
  const calls = [aim(FIRST, QUARTERS[0]), ...Array.from({ length: TURNS }, (_, t) => aim(turnAt(t), quarterOf(t)))];
  const file = join(out, "calls.jsonl");
  writeFileSync(file, calls.map((c) => JSON.stringify(c)).join("\n") + "\n");
  const shots = Array.from({ length: TURNS }, (_, t) => framesOf(t)).flat();
  const sandbox = [resolve(ROOT, "target/release/terra_sandbox"), "--offscreen", "--clean", "--picture", "960x540", "--experiment", "play", "--mute",
    "--time", "19.25", "--calls", file, "--shots", `${join(out, "f")}@${shots.join(",")}`];
  const done = spawnSync("cargo", ["xtask", "gpu", "still", "--", "timeout", "900", ...sandbox], { cwd: ROOT, encoding: "utf8", env: { ...process.env, BEVY_ASSET_ROOT: ROOT }, maxBuffer: 1 << 28 });
  writeFileSync(join(out, "run.log"), `${done.stdout}${done.stderr}`);
  if (done.status !== 0) {
    console.log(`turnflash: ✗ run exited ${done.status}, see ${join(out, "run.log")}`);
    return false;
  }
  console.log("turnflash: ran");
  return true;
}

/// The share of a box's pixels that are near black, given the box's mean light.
export function nearBlack(p: Picture, [x0, y0, x1, y1]: number[], mean: number): number {
  const under = Math.min(BLACK_OF * mean, 0.0031308) * 12.92 * 255;
  let n = 0;
  let black = 0;
  for (let y = Math.floor(y0 * p.height); y < Math.floor(y1 * p.height); y++) {
    for (let x = Math.floor(x0 * p.width); x < Math.floor(x1 * p.width); x++) {
      const i = (y * p.width + x) * 4;
      if (p.rgba[i] < under && p.rgba[i + 1] < under && p.rgba[i + 2] < under) black++;
      n++;
    }
  }
  return n ? black / n : 0;
}

const median = (v: number[]) => [...v].sort((a, b) => a - b)[Math.floor(v.length / 2)];
const ends = (v: number[]) => [median(v.slice(0, BEFORE)), median(v.slice(-5))];

/// One turn's frames, the first BEFORE of them before it starts: which are dark against the
/// turn's two ends (the frames before it and its last five), and the worst of each measure.
/// `lands` is each frame's land over its picture's light, `blacks` its near-black share,
/// `lights` its whole picture's light.
export function judged(lands: number[], blacks: number[], lights: number[]): { wrong: number[]; least: number; most: number; dimmest: number } {
  const floor = Math.min(...ends(lands));
  const roof = Math.max(...ends(blacks));
  const dark = Math.min(...ends(lights));
  const wrong: number[] = [];
  let least = Infinity;
  let most = -Infinity;
  let dimmest = Infinity;
  lands.forEach((land, i) => {
    const share = floor > 1e-6 ? land / floor : 1;
    const over = blacks[i] - roof;
    const dim = dark > 1e-6 ? lights[i] / dark : 1;
    least = Math.min(least, share);
    most = Math.max(most, over);
    dimmest = Math.min(dimmest, dim);
    if (share < DARK || over > BLACK_OVER || dim < DIM) wrong.push(i);
  });
  return { wrong, least, most, dimmest };
}

if (import.meta.main) {
  const argv = process.argv.slice(2);
  const flag = (name: string) => {
    const i = argv.indexOf(name);
    return i < 0 ? undefined : argv[i + 1];
  };
  const from = flag("--from");
  const out = resolve(from ?? flag("--out") ?? join(ROOT, "target/check/turnflash"));
  if (!from && !run(out)) process.exit(1);
  let holds = true;
  const told: string[] = [];
  for (let t = 0; t < TURNS; t++) {
    const frames = framesOf(t);
    const missing = frames.find((f) => !existsSync(stillOf(out, f)));
    if (missing !== undefined) {
      console.log(`turnflash: ✗ no still ${stillOf(out, missing)}; the run's log is ${join(out, "run.log")}`);
      process.exit(1);
    }
    const pictures = frames.map((f) => readPng(stillOf(out, f)));
    // Over the whole picture's light: the view snaps round in a frame or two and the eye
    // then opens or closes over the next hundred, which moves the land's light alone by half.
    const lights = pictures.map((p) => light(p, [ALL]));
    const lands = pictures.map((p, i) => light(p, LAND) / Math.max(lights[i], 1e-6));
    const blacks = pictures.map((p, i) => nearBlack(p, ALL, lights[i]));
    const { wrong, least, most, dimmest } = judged(lands, blacks, lights);
    if (argv.includes("--frames")) {
      frames.forEach((f, i) => console.log(`  ${f} light ${lights[i].toFixed(4)} land ${lands[i].toFixed(4)} black ${(blacks[i] * 100).toFixed(1)}%${wrong.includes(i) ? " ✗" : ""}`));
    }
    holds &&= wrong.length === 0;
    const round = t < 4 ? "first" : "again";
    const which = wrong.length ? `, ${wrong.length} wrong from frame ${frames[wrong[0]]}` : "";
    told.push(`${round} to ${quarterOf(t)[0]} land ${least.toFixed(2)} black +${(Math.max(most, 0) * 100).toFixed(1)} light ${dimmest.toFixed(2)}${which}`);
  }
  const fix = holds ? "" : "; a frame went dark in the middle of a turn: something in the picture changed the view's pipelines or bind group layout (a shader def set by what the frustum holds, as IRRADIANCE_VOLUME was: patches/README.md's bevy_pbr row, light_probe/mod.rs gather_light_probes), so make the def constant for the view, and do not raise the bars";
  console.log(`turnflash: ${holds ? "✓" : "✗"} every frame of each turn, its land over its picture's light, of the turn's darker end (at least ${DARK}) and its near-black share over the blacker end's (at most +${BLACK_OVER * 100} points) and its picture's light of the darker end's (at least ${DIM}): ${told.join("; ")}${fix}${holds && !from ? "" : `; stills in ${out}`}`);
  if (holds && !from) for (let t = 0; t < TURNS; t++) for (const f of framesOf(t)) rmSync(stillOf(out, f), { force: true });
  process.exit(holds ? 0 : 1);
}
