#!/usr/bin/env node
// A dungeon demo's fixed views and its film (docs/dungeons.md, In the sandbox; D69, D71): the
// camera dollied down the demo's tour as the stamp gives it (its eyes, looks and pace, which
// the run logs as `Dungeon view i`, `Dungeon look i` and `Dungeon pace i`), filmed at the
// game's own speed, and so many stills taken from the film at even times along it, so a
// change to a dungeon's look is judged on the same views and the same walk every round.
//
//   node tools/sandbox/dungeon.ts [--experiment dungeon-mine] [--views 8] [--out shots/dungeon-mine]
//
// Two runs of the release build: a short one for the log, then the film, its camera the
// shot written from that log (`--cine`, the player hidden, the eye adapting as the game's
// does, no motion blur so the stills are sharp). Writes <out>/shot.json, <out>/view-<k>.png,
// <out>/film.mp4 and the film run's log, <out>/film.log. Needs ffmpeg. Node 22.18 or later; no
// dependencies.

import { spawnSync } from "node:child_process";
import { copyFileSync, mkdirSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import { ROOT } from "./client.ts";

/// The frame the shot rolls on: the dungeon laid and lit, the eye settled on the first key.
const ROLLS_AT = 300;
/// The film keeps every second frame of the game's sixty a second.
const STEPS_A_SECOND = 60;
/// The lens the tour is filmed through, in millimetres on cine.rs's Super 35 filmback: 20 mm,
/// 64 degrees across, a wide establishing lens, the one the tour's look (look.rs `FRAME_RAD`)
/// and the doors' film reach (door.rs) are worked out for, between the game's own eye (55
/// degrees high, some 13 mm) and cine.rs's default 35 mm. Through the 35 mm's 39 degrees the
/// mine's round 7 views were crops: a drift's end wall 3.5 m off filled view 4 and its
/// judges saw "the camera point-blank at bare wall", where a walker sees the drift's sets
/// either side and the stair's way in the middle.
export const LENS_MM = 20;

type Point = [number, number, number];

/// The shot down a dungeon's tour from a run's log: a key at each of its eyes, looking where
/// the log says, at the time the dolly's pace brings it there.
export function shotOf(log: string, about: string) {
  const read = (what: string) => {
    const found = new Map<number, string>();
    for (const m of log.matchAll(new RegExp(`Dungeon ${what} (\\d+): ([-\\d.,]+)`, "g"))) found.set(Number(m[1]), m[2]);
    return [...found.keys()].sort((a, b) => a - b).map((i) => found.get(i) as string);
  };
  const point = (s: string) => s.split(",").map(Number) as Point;
  const view = read("view").map(point);
  const look = read("look").map(point);
  const pace = read("pace").map(Number);
  if (view.length < 2 || look.length !== view.length || pace.length !== view.length) {
    throw new Error(`the log gives ${view.length} eyes, ${look.length} looks and ${pace.length} paces`);
  }
  let t = 0;
  const keys = view.map((at, i) => {
    const key = { t: Number(t.toFixed(3)), at, look: look[i], lens: LENS_MM };
    t += pace[i];
    return key;
  });
  return {
    about,
    seconds: keys[keys.length - 1].t,
    keys,
    player: { hidden: true },
    look: { exposure: "auto", shutter: 0 },
  };
}

/// The film frames (`frame-<n>.png`, every second game frame from `ROLLS_AT`) at `count` even
/// times along a shot of `seconds`, each in the middle of its stretch.
export function stillFrames(seconds: number, count: number): number[] {
  return Array.from({ length: count }, (_, k) => {
    const frame = ROLLS_AT + Math.round(((k + 0.5) / count) * seconds * STEPS_A_SECOND);
    return frame - (frame % 2);
  });
}

function sandbox(args: string[]) {
  const run = spawnSync(resolve(ROOT, "target/release/terra_sandbox"), ["--offscreen", "--clean", ...args], {
    cwd: ROOT,
    encoding: "utf8",
    env: { ...process.env, BEVY_ASSET_ROOT: ROOT },
    maxBuffer: 256 * 1024 * 1024,
  });
  const log = `${run.stdout}${run.stderr}`;
  return { ok: run.status === 0 && `${run.stdout}`.includes("SANDBOX saved"), log };
}

function fail(why: string, log?: string): never {
  if (log) {
    const errors = log.split("\n").filter((l) => /ERROR|panicked/.test(l));
    console.error([...new Set(errors)].slice(0, 10).join("\n"));
  }
  console.error(`dungeon: ${why}`);
  process.exit(1);
}

if (import.meta.main) {
  const arg = (name: string, fallback: string) => {
    const at = process.argv.indexOf(name);
    return at > 0 ? process.argv[at + 1] : fallback;
  };
  const experiment = arg("--experiment", "dungeon-mine");
  const count = Number(arg("--views", "8"));
  const out = resolve(ROOT, arg("--out", `shots/${experiment}`));
  const frames = resolve(out, "frames");
  rmSync(frames, { recursive: true, force: true });
  mkdirSync(frames, { recursive: true });

  const build = spawnSync("cargo", ["build", "-q", "--release", "-p", "terra_sandbox"], { cwd: ROOT, stdio: "inherit" });
  if (build.status !== 0) process.exit(1);

  const started = Date.now();
  const probe = sandbox(["--experiment", experiment, "--shots", `${frames}/probe@30`]);
  if (!probe.ok) fail("the first run failed", probe.log);
  let shot: ReturnType<typeof shotOf>;
  try {
    shot = shotOf(probe.log, `${experiment}'s tour`);
  } catch (e) {
    fail(`${(e as Error).message}`, probe.log);
  }
  const shotFile = resolve(out, "shot.json");
  writeFileSync(shotFile, JSON.stringify(shot, null, 1));
  console.log(`dungeon: ${experiment}: ${shot.keys.length} eyes over ${shot.seconds.toFixed(1)} s`);

  const last = ROLLS_AT + Math.ceil(shot.seconds * STEPS_A_SECOND);
  const film = sandbox(["--experiment", experiment, "--cine", `${shotFile}@${ROLLS_AT}`, "--shots", `${frames}/frame@${ROLLS_AT}-${last}/2`]);
  writeFileSync(resolve(out, "film.log"), film.log);
  if (!film.ok) fail(`the film run failed; its log is ${resolve(out, "film.log")}`, film.log);
  const pushed = film.log.split("\n").filter((l) => /door .*pushed|pushed .*door/i.test(l)).length;
  console.log(`dungeon: filmed in ${((Date.now() - started) / 1000).toFixed(0)} s, doors pushed ${pushed}`);

  const saved = new Set(readdirSync(frames));
  stillFrames(shot.seconds, count).forEach((frame, k) => {
    const name = `frame-${String(frame).padStart(5, "0")}.png`;
    if (!saved.has(name)) fail(`no ${name} among the film's frames`);
    copyFileSync(resolve(frames, name), resolve(out, `view-${k}.png`));
  });
  const mp4 = resolve(out, "film.mp4");
  const encode = spawnSync(
    "ffmpeg",
    ["-y", "-loglevel", "error", "-framerate", "30", "-pattern_type", "glob", "-i", `${frames}/frame-*.png`, "-c:v", "libx264", "-preset", "slow", "-crf", "23", "-pix_fmt", "yuv420p", "-movflags", "+faststart", mp4],
    { stdio: "inherit" },
  );
  if (encode.status !== 0) process.exit(1);
  rmSync(frames, { recursive: true, force: true });
  console.log(`dungeon: ${count} views and ${mp4}, ${(statSync(mp4).size / 1e6).toFixed(1)} MB, ${shot.seconds.toFixed(0)} s`);
}
