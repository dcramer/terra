#!/usr/bin/env node
// Makes a clip with NVIDIA's Kimodo, a motion model run on this machine, or with Uthana's paid
// text-to-motion (D532, tools/uthana), in place of the library's
// (research/2026-10-14-animation.md): takes from the clip's entry in art/animation/made.json,
// each carried onto a copy of the mannequin and measured, then the one kept as the clip's take,
// which tools/characters/made_clips.py puts into the game's clips.
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
// ~/.cache/terra-kimodo by default.
//
// An entry with source "uthana" is made by Uthana's text-to-motion 3.0 on our own body instead
// (prompt, seconds: a whole number from 4 to 10, at $0.10 a second), with looped (seconds) for
// a clip Uthana closes into a loop itself, easing its end into its start over that long, kept
// whole. Its takes are GLBs; a take already made is never made again (each is paid for), so
// --samples N makes more. What each cost, by the account's credit, is printed and kept in
// takes.json beside them, and the kept take's motion id goes into its entry (motion).
//
// Each take is measured: the stride or loop and its seam (deg, and cm: the hands, feet and
// head at the loop's end from its start, relative to the hips, before it is eased closed), the
// gait and the limbs' check, how deep the arms go into the body on every template (moves.ts's
// hands, bar 2 cm) and, for a clip that stays where it stands, how far each foot wanders.
// Node 22.18 or later; no dependencies.

import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { basename, dirname, join, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs } from "node:util";
import { balance, character, download, looped, motion } from "../uthana/uthana.ts";
import { Glb, posed } from "./glb.ts";
import { armsInBody } from "./skin.ts";

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
  speeds?: [number, number][]; travels?: boolean;
  source?: "uthana"; looped?: number; motion?: string;
};

/** A take of an Uthana entry: the motion made, the one looped from it, the dollars both cost,
 * and how high its feet were lifted onto our ground (uthana/carry.ts `lift`, m). */
type Made = { motion: string; looped?: string; dollars: number; lift?: number };

/** The takes of `clip` an Uthana entry still needs: of the first `samples`, those not made. */
export function wanted(samples: number, made: (n: number) => boolean): number[] {
  return [...Array(samples).keys()].filter((n) => !made(n));
}

/** Makes the Uthana takes of `entry` that are missing, each at `file(n)`; returns their log. */
async function uthanaTakes(entry: Entry, samples: number, file: (n: number) => string, log: string): Promise<Record<number, Made>> {
  if (!Number.isInteger(entry.seconds) || entry.seconds < 4 || entry.seconds > 10) throw new Error(`Uthana's 3.0 makes 4 to 10 whole seconds, not ${entry.seconds}`);
  const made: Record<number, Made> = existsSync(log) ? JSON.parse(readFileSync(log, "utf8")) : {};
  const id = await character();
  for (const n of wanted(samples, (n) => existsSync(file(n)))) {
    // A take paid for but not on disk (refused, or lost) is downloaded again, free.
    if (!made[n]) {
      const before = await balance();
      const raw = await motion(entry.prompt, { model: "3", length: entry.seconds, footIk: false, character: id });
      const loop = entry.looped ? await looped(id, raw, entry.looped) : undefined;
      const after = await balance();
      made[n] = { motion: raw, ...(loop ? { looped: loop } : {}), dollars: Math.round((before - after) * 10000) / 10000 };
      writeFileSync(log, JSON.stringify(made, null, 2) + "\n");
      console.log(`take ${n}: Uthana motion ${raw}${loop ? `, looped ${loop}` : ""}, $${made[n].dollars} (left $${after.toFixed(2)})`);
    }
    // Carried onto our body and checked against Uthana's reference; a take that does not stand is refused.
    const { lift } = await download(made[n].looped ?? made[n].motion, file(n), { fps: String(FPS), inPlace: false });
    made[n].lift = Math.round(lift * 10000) / 10000;
    writeFileSync(log, JSON.stringify(made, null, 2) + "\n");
  }
  return made;
}

/** How far each foot's ball wanders over the ground through a clip, at most (cm): what a clip
 * that stays where it stands should keep near nothing. */
function wander(body: Glb, file: string, clip: string): number {
  const motion = new Glb(file).clip(clip);
  if (!motion) return 0;
  let most = 0;
  for (const ball of ["ball_l", "ball_r"]) {
    const at = body.named(ball);
    const xz: number[][] = [];
    for (let t = 0; t <= motion.end + 1e-6; t += 1 / 30) {
      const p = posed(body, motion, t)[at].t;
      xz.push([p[0], p[2]]);
    }
    for (const a of xz) for (const b of xz) most = Math.max(most, Math.hypot(a[0] - b[0], a[1] - b[1]));
  }
  return most * 100;
}

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

/** A straight path ahead whose speed goes through `speeds` (seconds, m/s), straight between
 * them and held past the last, the body facing ahead: what holds a start or a stop to its
 * speeds (a stop's braking, a start's pick-up). */
export function profile(speeds: [number, number][], seconds: number) {
  const frames = Math.round(seconds * FPS);
  const at = [...Array(frames).keys()];
  const speed = (t: number) => {
    const next = speeds.findIndex(([s]) => s > t);
    if (next < 0) return speeds[speeds.length - 1][1];
    if (next === 0) return speeds[0][1];
    const [[t0, v0], [t1, v1]] = [speeds[next - 1], speeds[next]];
    return v0 + ((v1 - v0) * (t - t0)) / (t1 - t0);
  };
  let z = 0;
  const along = at.map((i) => {
    const was = z;
    z += speed((i + 0.5) / FPS) / FPS;
    return was;
  });
  return [{
    type: "root2d",
    frame_indices: at,
    smooth_root_2d: along.map((z) => [0, z]),
    global_root_heading: at.map(() => [1, 0]),
  }];
}

/** What a take of `entry` is held to: its path (a start's or stop's speeds), or for a one-shot
 * nothing, the take free to step in, back or aside as its action asks. */
export function constraints(entry: Entry) {
  if (entry.speeds) return profile(entry.speeds, entry.seconds);
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

async function main() {
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
  // Kimodo writes several samples into a folder named after the output's stem; Uthana's takes
  // are laid out the same, as GLBs.
  const ext = entry.source === "uthana" ? ".glb" : ".bvh";
  const take = (n: number) => join(dir, clip, `${clip}_${String(n).padStart(2, "0")}${ext}`);
  const log = join(dir, "takes.json");

  if (values.keep !== undefined) {
    const n = Number(values.keep);
    if (!existsSync(take(n))) throw new Error(`no take ${n}: make the takes first`);
    mkdirSync(MADE, { recursive: true });
    copyFileSync(take(n), join(MADE, `${clip}${ext}`));
    // One take a clip: an Uthana take replaces a Kimodo one, which made_clips.py would not read.
    for (const other of [".glb", ".bvh"]) if (other !== ext) rmSync(join(MADE, `${clip}${other}`), { force: true });
    const made: Record<number, Made> = existsSync(log) ? JSON.parse(readFileSync(log, "utf8")) : {};
    list[clip] = { ...kept(entry, n), ...(made[n] ? { motion: made[n].motion } : {}) };
    writeFileSync(LIST, JSON.stringify(list, null, 2) + "\n");
    console.log(`${clip}: take ${n} kept as ${relative(ROOT, join(MADE, `${clip}${ext}`))}; ` +
      (values.list ? `made_clips.py OUT.glb --list=${named} puts the set into a copy of the mannequin` : "tools/characters/build_bodies.sh puts it into the game"));
    return;
  }

  if (!values.measure && entry.source === "uthana") {
    await uthanaTakes(entry, Number(samples), take, log);
  } else if (!values.measure) {
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
  const body = new Glb(join(ROOT, "assets", "models", "character", "base.glb"));
  const templates = join(ROOT, "assets", "models", "character", "templates.json");
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
    const played = new Glb(figure).clip(clip);
    if (played) {
      const arms = armsInBody(body, templates, played);
      console.log(`        arms in body: worst ${arms.cm.toFixed(1)} cm (${arms.where}), ${(arms.over * 100).toFixed(0)}% of poses over 2 cm` +
        (entry.still && !entry.busy ? `; feet wander ${wander(body, figure, clip).toFixed(1)} cm` : ""));
    }
  }
  console.log(`keep one with: node tools/characters/motion.ts ${clip}${values.list ? ` --list ${named}` : ""} --keep N`);
}

if (import.meta.main) await main();
