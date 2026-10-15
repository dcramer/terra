#!/usr/bin/env node
// Makes a clip with NVIDIA's Kimodo, a motion model run on this machine, in place of the
// library's (research/2026-10-14-animation.md): takes from the clip's entry in
// art/animation/made.json, each carried onto a copy of the mannequin and measured, then the one
// kept as the clip's take, which tools/characters/made_clips.py puts into the game's clips.
//
//   node tools/characters/motion.ts Sprint_Loop              # make its takes and measure them
//   node tools/characters/motion.ts Sprint_Loop --measure    # measure the takes made again
//   node tools/characters/motion.ts Sprint_Loop --keep 2     # keep take 2
//
// An entry: prompt, speed (m/s, held along a straight path; 0 to stand), seconds, seed, take
// (the one kept), samples (takes made, 3 unless it says), and still, for a clip that stays where it stands, such as an idle, with loop
// (seconds, least and most) for how long a loop of it may be, and busy for one that works where
// it stands (a hammer, an axe), looped in its busiest stretch; a stride may also have step, arms
// and fist, which made_clips.py and aim.py apply over the take (README.md, "Made clips"). Kimodo is set up apart from the repository (README.md, "Made clips"), at
// KIMODO_HOME, ~/.cache/terra-kimodo by default. Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const HERE = join(ROOT, "tools", "characters");
const OUT = join(HERE, "out", "motion");
const LIST = join(ROOT, "art", "animation", "made.json");
const MADE = join(ROOT, "art", "animation", "made");
const BLENDER = join(ROOT, "tools", "blender", "run.sh");
const KIMODO = process.env.KIMODO_HOME ?? join(homedir(), ".cache", "terra-kimodo");
const MODEL = "Kimodo-SOMA-RP-v1.1";
const FPS = 30;

type Entry = { prompt: string; speed: number; seconds: number; seed: number; take?: number; samples?: number; still?: boolean; loop?: [number, number]; busy?: boolean };

/** A straight path along Kimodo's forward (+z) at `speed`, a point a frame: what holds a take
 * to the clip's speed. */
export function path(speed: number, seconds: number) {
  const frames = Math.round(seconds * FPS);
  return [{ type: "root2d", frame_indices: [...Array(frames).keys()], smooth_root_2d: [...Array(frames).keys()].map((i) => [0, (speed * i) / FPS]) }];
}

function run(command: string, args: string[], env: Record<string, string> = {}) {
  const result = spawnSync(command, args, { stdio: ["ignore", "pipe", "inherit"], encoding: "utf8", env: { ...process.env, ...env } });
  if (result.status !== 0) throw new Error(`${command} failed`);
  return result.stdout;
}

function main() {
  const { values, positionals } = parseArgs({ allowPositionals: true, options: { keep: { type: "string" }, measure: { type: "boolean" }, samples: { type: "string" } } });
  const clip = positionals[0];
  const list: Record<string, Entry> = JSON.parse(readFileSync(LIST, "utf8"));
  const entry = list[clip];
  if (!entry) throw new Error(`${clip} is not in art/animation/made.json; add its prompt, speed, seconds and seed`);
  // How many takes are made: the entry's, so its kept take can be made again, or 3.
  const samples = values.samples ?? String(entry.samples ?? 3);
  const dir = join(OUT, clip);
  // Kimodo writes several samples into a folder named after the output's stem.
  const take = (n: number) => join(dir, clip, `${clip}_${String(n).padStart(2, "0")}.bvh`);

  if (values.keep !== undefined) {
    const n = Number(values.keep);
    if (!existsSync(take(n))) throw new Error(`no take ${n}: make the takes first`);
    mkdirSync(MADE, { recursive: true });
    copyFileSync(take(n), join(MADE, `${clip}.bvh`));
    list[clip] = { ...entry, take: n, ...(n >= 3 || entry.samples ? { samples: Math.max(entry.samples ?? 3, n + 1) } : {}) };
    writeFileSync(LIST, JSON.stringify(list, null, 2) + "\n");
    console.log(`${clip}: take ${n} kept as art/animation/made/${clip}.bvh; tools/characters/build_bodies.sh puts it into the game`);
    return;
  }

  if (!values.measure) {
    if (!existsSync(join(KIMODO, "venv", "bin", "kimodo_gen"))) throw new Error(`no Kimodo at ${KIMODO}: see tools/characters/README.md, "Made clips"`);
    mkdirSync(dir, { recursive: true });
    const constraints = join(dir, "path.json");
    writeFileSync(constraints, JSON.stringify(path(entry.speed, entry.seconds)));
    // The text encoder on the processor leaves the graphics card about 3 GB of work.
    run(join(KIMODO, "venv", "bin", "kimodo_gen"), [
      entry.prompt, "--model", MODEL, "--duration", String(entry.seconds), "--num_samples", samples,
      "--constraints", constraints, "--output", join(dir, clip), "--bvh", "--bvh_standard_tpose", "--seed", String(entry.seed),
    ], { TEXT_ENCODERS_DIR: join(KIMODO, "text_encoders"), TEXT_ENCODER_DEVICE: "cpu", HF_HUB_OFFLINE: "1", LOCAL_CACHE: "True" });
  }

  // Each take on a copy of the mannequin, measured against the library's clip.
  for (let n = 0; n < Number(samples); n++) {
    if (!existsSync(take(n))) continue;
    const figure = join(dir, `try_${n}.glb`);
    copyFileSync(join(ROOT, "assets", "models", "character", "mannequin.glb"), figure);
    const made = run(BLENDER, [join(HERE, "made_clips.py"), figure, `--try=${clip}:${take(n)}`]);
    const gait = run(BLENDER, [join(HERE, "gait_check.py"), figure, `--clips=${clip}`]);
    const stride = made.split("\n").find((l) => l.startsWith(`MADE ${clip}`)) ?? `no stride in take ${n}`;
    const row = gait.split("\n").find((l) => l.startsWith("GAIT") && l.includes(`try_${n}`)) ?? "";
    console.log(`take ${n}: ${stride.replace(/^MADE \S+ /, "")}\n        ${row.replace(/^GAIT\s+\S+\s+\S+\s+/, "")}`);
  }
  console.log(`keep one with: node tools/characters/motion.ts ${clip} --keep N`);
}

if (import.meta.main) main();
