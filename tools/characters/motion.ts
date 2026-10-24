#!/usr/bin/env node
// Makes a clip with NVIDIA's Kimodo, a motion model run on this machine, in place of the
// library's (research/2026-10-14-animation.md): takes from the clip's entry in
// art/animation/made.json, each carried onto a copy of the mannequin and measured, then the one
// kept as the clip's take, which tools/characters/made_clips.py puts into the game's clips.
//
//   node tools/characters/motion.ts Sprint_Loop              # make its takes and measure them
//   node tools/characters/motion.ts Sprint_Loop --measure    # measure the takes made again
//   node tools/characters/motion.ts Sprint_Loop --keep 2     # keep take 2
//   node tools/characters/motion.ts Fight_Slash --list art/animation/sets/fight.json
//
// --list names another set of clips (a creature's, a demo's) with its own recipe file, its kept
// takes in the folder beside it named as it is (art/animation/sets/fight/); the game's list and
// mannequin are left alone.
//
// An entry: prompt, speed (m/s, held along a straight path; 0 to stand), seconds, seed, take
// (the one kept), samples (takes made, 3 unless it says), and still, for a clip that stays where it stands, such as an idle, with loop
// (seconds, least and most) for how long a loop of it may be, and busy for one that works where
// it stands (a hammer, an axe), looped in its busiest stretch; a stride may also have step, arms
// and fist, which made_clips.py and aim.py apply over the take (README.md, "Made clips"). way
// (degrees from where the body faces, 90 to its right) lays the path that way, the body held
// facing ahead (a strafe); once makes a one-shot (a blow, a dodge), its path left to the take,
// kept from start to end, or over span (seconds in the take), not closed into a loop. Kimodo is
// set up apart from the repository (README.md, "Made clips"), at KIMODO_HOME,
// ~/.cache/terra-kimodo by default. Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";

const ROOT = join(dirname(fileURLToPath(import.meta.url)), "..", "..");
const HERE = join(ROOT, "tools", "characters");
const OUT = join(HERE, "out", "motion");
const GAME_LIST = join(ROOT, "art", "animation", "made.json");
const BLENDER = join(ROOT, "tools", "blender", "run.sh");
export const KIMODO = process.env.KIMODO_HOME ?? join(homedir(), ".cache", "terra-kimodo");
// Kimodo's latest model (its CHANGELOG, 2026-10-15: trained on corrected splits; D450).
export const MODEL = "Kimodo-SOMA-RP-v1.2";
/** What kimodo_gen runs with: the text encoder on the processor, offline. */
export const KIMODO_ENV = { TEXT_ENCODERS_DIR: join(KIMODO, "text_encoders"), TEXT_ENCODER_DEVICE: "cpu", HF_HUB_OFFLINE: "1", LOCAL_CACHE: "True" };
const FPS = 30;

type Entry = {
  prompt: string; speed: number; seconds: number; seed: number; take?: number; samples?: number;
  still?: boolean; loop?: [number, number]; busy?: boolean; way?: number; once?: boolean; span?: [number, number];
};

/** A straight path at `speed`, a point a frame, `way` degrees to the right of Kimodo's forward
 * (+z, a heading of [1, 0]): what holds a take to the clip's speed. Travelling any way but
 * straight ahead, the body is held facing ahead (a strafe). Kimodo's ground is x and z, y up,
 * and its heading is read from the hips with the body's right at -x when it faces +z
 * (compute_heading_angle), which a strafe's take bears out: laid toward -x, it steps to its
 * right. */
export function path(speed: number, seconds: number, way = 0) {
  const frames = Math.round(seconds * FPS);
  const at = [...Array(frames).keys()];
  const a = (way * Math.PI) / 180;
  // Exactly straight ahead when it is, not off by sin(0)'s rounding.
  const exact = (v: number) => (Math.abs(v) < 1e-12 ? 0 : v);
  const [x, z] = [exact(-Math.sin(a)), exact(Math.cos(a))];
  return [{
    type: "root2d",
    frame_indices: at,
    smooth_root_2d: at.map((i) => [(x * speed * i) / FPS, (z * speed * i) / FPS]),
    ...(way ? { global_root_heading: at.map(() => [1, 0]) } : {}),
  }];
}

/** What a take of `entry` is held to: its path, or for a one-shot nothing, the take free to
 * step in, back or aside as its action asks. */
export function constraints(entry: Entry) {
  return entry.once ? [] : path(entry.speed, entry.seconds, entry.way ?? 0);
}

/** An entry with take `n` kept, and as many samples as make it again. */
export function kept<E extends { take?: number; samples?: number }>(entry: E, n: number): E {
  return { ...entry, take: n, ...(n >= 3 || entry.samples ? { samples: Math.max(entry.samples ?? 3, n + 1) } : {}) };
}

function run(command: string, args: string[], env: Record<string, string> = {}) {
  const result = spawnSync(command, args, { stdio: ["ignore", "pipe", "inherit"], encoding: "utf8", env: { ...process.env, ...env } });
  if (result.status !== 0) throw new Error(`${command} failed`);
  return result.stdout;
}

function main() {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: { keep: { type: "string" }, measure: { type: "boolean" }, samples: { type: "string" }, list: { type: "string" } },
  });
  const clip = positionals[0];
  // The game's clips, or another set's, its kept takes in the folder named as its list is.
  const LIST = values.list ? resolve(values.list) : GAME_LIST;
  const MADE = LIST.replace(/\.json$/, "");
  const named = relative(ROOT, LIST);
  const set = values.list ? [`--list=${LIST}`] : [];
  const list: Record<string, Entry> = JSON.parse(readFileSync(LIST, "utf8"));
  const entry = list[clip];
  if (!entry) throw new Error(`${clip} is not in ${named}; add its prompt, speed, seconds and seed`);
  // How many takes are made: the entry's, so its kept take can be made again, or 3.
  const samples = values.samples ?? String(entry.samples ?? 3);
  const dir = values.list ? join(OUT, basename(MADE), clip) : join(OUT, clip);
  // Kimodo writes several samples into a folder named after the output's stem.
  const take = (n: number) => join(dir, clip, `${clip}_${String(n).padStart(2, "0")}.bvh`);

  if (values.keep !== undefined) {
    const n = Number(values.keep);
    if (!existsSync(take(n))) throw new Error(`no take ${n}: make the takes first`);
    mkdirSync(MADE, { recursive: true });
    copyFileSync(take(n), join(MADE, `${clip}.bvh`));
    list[clip] = kept(entry, n);
    writeFileSync(LIST, JSON.stringify(list, null, 2) + "\n");
    console.log(`${clip}: take ${n} kept as ${relative(ROOT, join(MADE, `${clip}.bvh`))}; ` +
      (values.list ? `made_clips.py OUT.glb --list=${named} puts the set into a copy of the mannequin` : "tools/characters/build_bodies.sh puts it into the game"));
    return;
  }

  if (!values.measure) {
    if (!existsSync(join(KIMODO, "venv", "bin", "kimodo_gen"))) throw new Error(`no Kimodo at ${KIMODO}: see tools/characters/README.md, "Made clips"`);
    mkdirSync(dir, { recursive: true });
    const held = join(dir, "path.json");
    const hold = constraints(entry);
    writeFileSync(held, JSON.stringify(hold));
    // The text encoder on the processor leaves the graphics card about 3 GB of work.
    run(join(KIMODO, "venv", "bin", "kimodo_gen"), [
      entry.prompt, "--model", MODEL, "--duration", String(entry.seconds), "--num_samples", samples,
      ...(hold.length ? ["--constraints", held] : []), "--output", join(dir, clip), "--bvh", "--bvh_standard_tpose", "--seed", String(entry.seed),
    ], KIMODO_ENV);
  }

  // Each take on a copy of the mannequin, measured against the library's clip, and checked as
  // a clip our skinning can follow.
  for (let n = 0; n < Number(samples); n++) {
    if (!existsSync(take(n))) continue;
    const figure = join(dir, `try_${n}.glb`);
    copyFileSync(join(ROOT, "assets", "models", "character", "mannequin.glb"), figure);
    const made = run(BLENDER, [join(HERE, "made_clips.py"), figure, `--try=${clip}:${take(n)}`, ...set]);
    const gait = run(BLENDER, [join(HERE, "gait_check.py"), figure, `--clips=${clip}`]);
    const check = run(BLENDER, [join(HERE, "clip_check.py"), figure, clip]);
    const lines = (text: string, start: string) => text.split("\n").filter((l) => l.startsWith(start));
    const stride = lines(made, `MADE ${clip}`)[0] ?? `no stride in take ${n}`;
    const row = lines(gait, "GAIT").find((l) => l.includes(`try_${n}`)) ?? "";
    const limbs = lines(check, "CLIPCHECK {")[0] ?? "";
    const once = lines(made, `ONCE ${clip}`)[0] ?? lines(made, `WAY ${clip}`)[0] ?? "";
    console.log(`take ${n}: ${stride.replace(/^MADE \S+ /, "")}`);
    for (const l of [once.replace(/^(ONCE|WAY) \S+ /, ""), row.replace(/^GAIT\s+\S+\s+\S+\s+/, ""), limbs.replace(/^CLIPCHECK /, "")]) if (l) console.log(`        ${l}`);
  }
  console.log(`keep one with: node tools/characters/motion.ts ${clip}${values.list ? ` --list ${named}` : ""} --keep N`);
}

if (import.meta.main) main();
