// The numbers a hold is dialled in by, each tried for one run without baking a clip or editing
// source (solve.ts searches them; moves.ts --tune lays them): a carry pose's arm aims and rolls
// and its swing's share (art/animation/sets/fight.json), the give's constants
// (terra_walker::carry_arm) and the grip seat's (terra_walker::grip). A parameter's default is
// read from where it lives, so a run with none set measures what is committed.
//
//   pose.<Clip>.aim.<bone>.up      degrees the bone points above level
//   pose.<Clip>.aim.<bone>.round   degrees about the vertical, from ahead (+Z) toward +X (the body's left)
//   pose.<Clip>.roll.<bone>        degrees about the bone's own line
//   pose.<Clip>.swing              the stride's swing on the arm, its share
//   give.<key>                     carry_arm.ts GIVE's keys (elbow, back, out, from, to, rate, ...)
//   seat.fit, seat.palm            grip.ts FIT_M and PALM_M, metres
//
// A tuned pose is hold.py's aim and roll laid in TypeScript on the held clip's first pose (the
// same swing about the bone's joint, the least turn onto the aim, then the roll about its line),
// only the bones the hold names; every other bone is the baked clip's. It reproduces the baked
// pose to a hundredth of a degree (tune.test.ts), not to the bit: a search compares points all
// emulated, never an emulated one against a baked one.
//
// Node 22.18 or later; no dependencies.

import { readFileSync } from "node:fs";
import { join } from "node:path";
import { GIVE } from "./carry_arm.ts";
import { type Clip, type Glb, type Pose, type Q, type V3, dot, len, posed, qinv, qmul, rot, sub } from "./glb.ts";
import { FIT_M, PALM_M, SEAT } from "./grip.ts";

const ROOT = join(import.meta.dirname, "..", "..");
export const FIGHT = "art/animation/sets/fight.json";
const CARRY_ARM_RS = "crates/terra_walker/src/carry_arm.rs";

/** Parameter name to value, for one run. */
export type Tune = Record<string, number>;
type Hold = { clip: string; bone: string; aim?: Record<string, V3>; roll?: Record<string, number>; turn?: unknown; shield?: unknown };
type Entry = { of?: string; swing?: number; upright?: boolean; hold?: Hold[] };
export type Sets = Record<string, Entry>;

export const readSets = (root = ROOT): Sets => JSON.parse(readFileSync(join(root, FIGHT), "utf8")) as Sets;

const deg = (rad: number) => (rad * 180) / Math.PI;
const rad = (d: number) => (d * Math.PI) / 180;
const r3 = (x: number) => Math.round(x * 1000) / 1000 + 0;
const unit = (v: V3): V3 => v.map((x) => x / len(v)) as V3;
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const axisAngle = (axis: V3, angle: number): Q => {
  const a = unit(axis);
  const s = Math.sin(angle / 2);
  return [a[0] * s, a[1] * s, a[2] * s, Math.cos(angle / 2)];
};
const fromTo = (a: V3, b: V3): Q => {
  const [x, y] = [unit(a), unit(b)];
  const c = cross(x, y);
  return len(c) < 1e-9 ? [0, 0, 0, 1] : axisAngle(c, Math.acos(Math.max(-1, Math.min(1, dot(x, y)))));
};

/** An aim as its two angles, degrees, and back as fight.json writes one (three decimals). */
export const anglesOf = (v: V3) => ({ up: deg(Math.asin(v[1] / len(v))), round: deg(Math.atan2(v[0], v[2])) });
export const aimOf = (up: number, round: number): V3 => [r3(Math.cos(rad(up)) * Math.sin(rad(round))), r3(Math.sin(rad(up))), r3(Math.cos(rad(up)) * Math.cos(rad(round)))];

/** The give's keys with the constant each is read from, from carry_arm.ts's own text, so a
 * constant added there is a parameter here. */
const giveNames = (root = ROOT): Record<string, string> =>
  Object.fromEntries([...readFileSync(join(root, "tools/characters/carry_arm.ts"), "utf8").matchAll(/(\w+): of\("(\w+)"\)/g)].map((m) => [m[1], m[2]]));

/** One parameter: its committed value and the edits that would commit another. */
export type Param = { name: string; start: number; kind: "pose" | "give" | "seat"; clip?: string };
/** An edit to commit: `key` in `file` set to `value`. */
export type EditOf = { file: string; key: string; value: number | V3 };

const say = (name: string, why: string, names: string[]) =>
  new Error(`no parameter ${name}: ${why} (D556). Name one of: ${names.join(", ")}; a pose's as pose.<Clip>.aim.<bone>.up|round, pose.<Clip>.roll.<bone> or pose.<Clip>.swing (the clips with a hold in ${FIGHT})`);

function holdWith(sets: Sets, name: string, clip: string, key: "aim" | "roll", bone: string): { hold: Hold; at: number } {
  const entry = sets[clip];
  const poses = Object.keys(sets).filter((k) => sets[k].hold?.some((h) => h.aim || h.roll));
  if (!entry?.hold) throw say(name, `${FIGHT} has no clip ${clip} with a hold`, poses.map((p) => `pose.${p}`));
  const at = entry.hold.findIndex((h) => h[key]?.[bone] !== undefined);
  const all = entry.hold.flatMap((h) => [...Object.keys(h.aim ?? {}).flatMap((b) => [`pose.${clip}.aim.${b}.up`, `pose.${clip}.aim.${b}.round`]), ...Object.keys(h.roll ?? {}).map((b) => `pose.${clip}.roll.${b}`)]);
  if (at < 0) throw say(name, `${clip}'s holds give no ${key} for ${bone}`, [...all, `pose.${clip}.swing`]);
  return { hold: entry.hold[at], at };
}

/** A parameter by name, with its committed value; an unknown one is an error that lists what
 * there is. */
export function paramOf(name: string, sets: Sets = readSets()): Param {
  const [group, a, b, c, d] = name.split(".");
  if (group === "give") {
    const names = Object.keys(giveNames());
    if (!(a in GIVE) || b !== undefined) throw say(name, "carry_arm.ts's GIVE has no such key", names.map((k) => `give.${k}`));
    return { name, start: GIVE0[a], kind: "give" };
  }
  if (group === "seat") {
    if ((a !== "fit" && a !== "palm") || b !== undefined) throw say(name, "the seat has two numbers", ["seat.fit", "seat.palm"]);
    return { name, start: a === "fit" ? FIT_M : PALM_M, kind: "seat" };
  }
  if (group !== "pose") throw say(name, "a parameter is a pose's, the give's or the seat's", [...Object.keys(giveNames()).map((k) => `give.${k}`), "seat.fit", "seat.palm"]);
  if (b === "swing" && c === undefined) {
    if (sets[a]?.swing === undefined) throw say(name, `${FIGHT}'s ${a} has no swing`, Object.keys(sets).filter((k) => sets[k].swing !== undefined).map((k) => `pose.${k}.swing`));
    return { name, start: sets[a].swing!, kind: "pose", clip: a };
  }
  if (b === "roll" && d === undefined) return { name, start: holdWith(sets, name, a, "roll", c).hold.roll![c], kind: "pose", clip: a };
  if (b === "aim" && (d === "up" || d === "round")) return { name, start: anglesOf(holdWith(sets, name, a, "aim", c).hold.aim![c])[d], kind: "pose", clip: a };
  holdWith(sets, name, a, "aim", c ?? "");
  throw say(name, "an aim has .up and .round, a roll neither", [`pose.${a}.aim.${c}.up`, `pose.${a}.aim.${c}.round`, `pose.${a}.roll.${c}`]);
}

/** The give as committed, kept before any tune changes it. */
const GIVE0: Record<string, number> = { ...GIVE };

/** `sets` with the pose parameters of `tune` laid in: an aim with either angle tuned is rewritten
 * from both (three decimals, as the file holds it), every other value left as it is. */
export function setsWith(tune: Tune, sets: Sets = readSets()): { sets: Sets; edits: EditOf[]; clips: string[] } {
  const out = structuredClone(sets);
  const edits = new Map<string, EditOf>();
  const clips = new Set<string>();
  for (const name of Object.keys(tune).sort()) {
    const p = paramOf(name, sets);
    if (p.kind !== "pose") continue;
    const [, clip, key, bone, angle] = name.split(".");
    clips.add(clip);
    if (key === "swing") {
      out[clip].swing = tune[name];
      edits.set(name, { file: FIGHT, key: `${clip}.swing`, value: tune[name] });
    } else if (key === "roll") {
      const { at } = holdWith(sets, name, clip, "roll", bone);
      out[clip].hold![at].roll![bone] = tune[name];
      edits.set(name, { file: FIGHT, key: `${clip}.hold[${at}].roll.${bone}`, value: tune[name] });
    } else {
      const { hold, at } = holdWith(sets, name, clip, "aim", bone);
      const was = anglesOf(hold.aim![bone]);
      const stem = `pose.${clip}.aim.${bone}`;
      const aim = aimOf(angle === "up" ? tune[name] : (tune[`${stem}.up`] ?? was.up), angle === "round" ? tune[name] : (tune[`${stem}.round`] ?? was.round));
      out[clip].hold![at].aim![bone] = aim;
      edits.set(stem, { file: FIGHT, key: `${clip}.hold[${at}].aim.${bone}`, value: aim });
    }
  }
  return { sets: out, edits: [...edits.values()], clips: [...clips].sort() };
}

/** Every edit that commits `tune`: fight.json's keys, the give's constants in carry_arm.rs (its
 * twin reads them) and the seat's in grip.rs and grip.ts, both. */
export function editsOf(tune: Tune, sets: Sets = readSets()): EditOf[] {
  const out = setsWith(tune, sets).edits;
  const names = giveNames();
  for (const name of Object.keys(tune).sort()) {
    const p = paramOf(name, sets);
    const key = name.split(".")[1];
    if (p.kind === "give") out.push({ file: CARRY_ARM_RS, key: names[key] ?? key, value: tune[name] });
    if (p.kind === "seat") for (const file of ["crates/terra_walker/src/grip.rs", "tools/characters/grip.ts"]) out.push({ file, key: key === "fit" ? "FIT_M" : "PALM_M", value: tune[name] });
  }
  return out;
}

/** The first pose of a carry clip as `entry`'s holds make it, each aimed or rolled bone's turn
 * in its own frame: hold.py's `_held_pose` on `body` from `clips`. */
export function heldPose(body: Glb, clips: Glb, name: string, entry: Entry): Map<string, Q> {
  const out = new Map<string, Q>();
  const below = (root: number): number[] =>
    body.gltf.nodes.map((_, i) => i).filter((i) => {
      for (let j: number | undefined = i; j !== undefined; j = body.parentOf[j]) if (j === root) return true;
      return false;
    });
  for (const hold of entry.hold ?? []) {
    if (hold.turn || hold.shield) throw new Error(`${name}: its hold on ${hold.bone} has a ${hold.turn ? "turn" : "shield"}, which tune.ts does not lay (only aim and roll): tune another pose, or add it to tune.ts heldPose as hold.py's _held_pose does it`);
    const from = clips.clip(hold.clip);
    if (!from) throw new Error(`${name}: no clip ${hold.clip} in ${clips.file} to hold a part at`);
    const pose: Pose[] = posed(body, from, 0);
    const lineOf = (bone: number): V3 => unit(rot(pose[bone].r, [0, 1, 0]));
    const turnAt = (bone: number, q: Q) => {
      const head = pose[bone].t;
      for (const i of below(bone)) {
        const d = rot(q, sub(pose[i].t, head));
        pose[i] = { t: [head[0] + d[0], head[1] + d[1], head[2] + d[2]], r: qmul(q, pose[i].r), s: pose[i].s };
      }
    };
    for (const [bone, way] of Object.entries(hold.aim ?? {})) turnAt(body.named(bone), fromTo(lineOf(body.named(bone)), way));
    for (const [bone, d] of Object.entries(hold.roll ?? {})) turnAt(body.named(bone), axisAngle(lineOf(body.named(bone)), rad(d)));
    for (const bone of new Set([...Object.keys(hold.aim ?? {}), ...Object.keys(hold.roll ?? {})])) {
      const i = body.named(bone);
      out.set(bone, qmul(qinv(pose[body.parentOf[i]!].r), pose[i].r));
    }
  }
  return out;
}

/** `tune` laid for this run: the give's and the seat's numbers set where their code reads them,
 * and what moves.ts asks of a pose: a tuned carry clip in place of the baked one (undefined: not
 * tuned) and a tuned swing's share. */
export function tuneWith(tune: Tune, body: Glb, clips: Glb): { clip: (name: string) => Clip | undefined; swing: (pose: string) => number | undefined } {
  const committed = readSets();
  for (const name of Object.keys(tune)) {
    const p = paramOf(name, committed);
    const key = name.split(".")[1];
    if (p.kind === "give") (GIVE as Record<string, number>)[key] = tune[name];
    if (p.kind === "seat") SEAT[key as "fit" | "palm"] = tune[name];
  }
  const { sets, clips: tuned } = setsWith(tune, committed);
  const made = new Map<string, Clip>();
  return {
    clip: (name) => {
      if (!tuned.includes(name)) return undefined;
      if (!made.has(name)) {
        const baked = clips.clip(name);
        if (!baked) throw new Error(`no clip ${name} in ${clips.file}: a tuned pose replaces a baked one's aimed bones; bake it once first (build_bodies.sh --step fight)`);
        const tracks = new Map(baked.tracks);
        for (const [bone, q] of heldPose(body, clips, name, sets[name])) tracks.set(bone, { ...baked.tracks.get(bone), r: () => q });
        made.set(name, { ...baked, tracks });
      }
      return made.get(name);
    },
    swing: (pose) => (tuned.includes(pose) ? sets[pose].swing : undefined),
  };
}
