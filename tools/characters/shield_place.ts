#!/usr/bin/env node
// The shield's board place (D554): where a shield pose puts the board against the chest, the one
// number a shield pose authors. Its centre (the board's middle, clash.ts's `board`), its face (the
// way its painted front, -Z, looks) and its up (the way its -X, a heater's top, looks), in the
// chest's frame: spine_03's joint, turned as the chest has turned from its rest, so the body's own
// axes (+Y up, +Z ahead at rest) carried by the chest, in metres, whatever way the bone's own axes
// run or however it is scaled (a rebuild of the bones, D542, leaves it). The board's place against
// a target, the target kept in art/animation/shield_targets.json, captured from a commit's assets
// on one body template.
//
//   node tools/characters/shield_place.ts                     # every shield pose on every template
//   node tools/characters/shield_place.ts --at 27fa529c       # from that commit's assets
//   node tools/characters/shield_place.ts --at 27fa529c --capture Low_Guard   # write its target
//   node tools/characters/shield_place.ts --json places.json  # every number, by pose and template
//   node tools/characters/shield_place.ts --board             # the strapped board for hold.py's shield op
//
// A line a pose: its place on the target's template at its first frame, how far it moves over the
// clip, how far the other templates' are from it, and how far it is from its target, if it has one.
// Node 22.18 or later; no dependencies.

import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { parseArgs } from "node:util";
import { Glb, type Clip, type Pose, type Q, type V3, dot, len, place, posed, qinv, qmul, rot, sub } from "./glb.ts";
import { type Held, frameOf, hang } from "./held.ts";
import { fitter } from "./shield_arm.ts";
import { type Shape, shaped, skinOf } from "./skin.ts";

export type Place = { centre: V3; face: V3; up: V3 };
/** A pose's target: its place, the commit and template it was captured from, and why. */
export type Target = Place & { from: string; template: string; why: string };
export type Targets = Record<string, Target>;
export const TARGETS = "art/animation/shield_targets.json";

/** The chest's frame at a pose: spine_03's joint, turned as the chest has turned from its rest. */
export function chest(pose: Pose[], rest: Pose[], spine: number): { t: V3; r: Q } {
  return { t: pose[spine].t, r: qmul(pose[spine].r, qinv(rest[spine].r)) };
}

/** The board's place in the chest's frame, `shield` hung in `frame` (held.ts `frameOf`). */
export function boardPlace(pose: Pose[], rest: Pose[], spine: number, frame: Pose, shield: Held): Place {
  const c = chest(pose, rest, spine);
  const B = frame;
  const into = (v: V3): V3 => rot(qinv(c.r), v);
  return { centre: into(sub(place(B, shield.board!.centre), c.t)), face: into(rot(B.r, shield.front)), up: into(rot(B.r, shield.up)) };
}

const unit = (v: V3): V3 => v.map((x) => x / len(v)) as V3;
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
/** A place's turn as three square axes: its face, its up made square to it, and their cross. */
function axes(p: Place): V3[] {
  const f = unit(p.face);
  const u = unit(sub(p.up, f.map((x) => x * dot(p.up, f)) as V3));
  return [f, u, cross(f, u)];
}

/** How far one place is from another: its centre in cm, its turn in degrees (the angle of the
 * one rotation that takes one board's axes onto the other's). */
export function placeOff(a: Place, b: Place): { cm: number; deg: number } {
  const [A, B] = [axes(a), axes(b)];
  const trace = A.reduce((s, x, i) => s + dot(x, B[i]), 0);
  const deg = (Math.acos(Math.min(1, Math.max(-1, (trace - 1) / 2))) * 180) / Math.PI;
  return { cm: len(sub(a.centre, b.centre)) * 100, deg };
}

/** The targets kept in `file`, none if there is no file. */
export function readTargets(file = TARGETS): Targets {
  return existsSync(file) ? (JSON.parse(readFileSync(file, "utf8")).targets as Targets) : {};
}

/** The strapped board in its strap's own frame (held.ts `frameOf`: the true forearm's, at the
 * fist) on `body` at rest: its centre in metres from the fist, its face and up. hold.py's `shield`
 * op solves the arm with it (shield_targets.json's `board`, written by `--board`). */
export function strappedBoard(o: { body: string; shield: string }): Place {
  const body = new Glb(o.body);
  const rest = posed(body, undefined, 0);
  const shield = hang(body, rest, "socket.hand.L", o.shield);
  if (!shield.board) throw new Error(`${o.shield} has no strap: not a shield`);
  const B = frameOf(shield, rest, (n) => body.named(n), (i) => body.parentOf[i]);
  return { centre: rot(qinv(B.r), sub(place(B, shield.board.centre), B.t)), face: shield.front, up: shield.up };
}

/** The strapped board kept in `file` for hold.py, if there is one. */
export function readBoard(file = TARGETS): (Place & { shield: string }) | undefined {
  return existsSync(file) ? JSON.parse(readFileSync(file, "utf8")).board : undefined;
}

/** The files a reading needs, as `commit` had them (through git's filters, so a model kept in LFS
 * comes out whole): each written into a scratch folder, keeping its path under it. Returns the
 * folder. */
export function atCommit(commit: string, files: string[]): string {
  const dir = mkdtempSync(join(tmpdir(), "shield-place-"));
  for (const f of files) {
    mkdirSync(dirname(join(dir, f)), { recursive: true });
    writeFileSync(join(dir, f), execFileSync("git", ["cat-file", "--filters", `${commit}:${f}`], { maxBuffer: 1 << 30 }));
  }
  return dir;
}

/** A reader of board places on a body: its templates, its clips (a `Walk_Loop+Low_Guard` laid by
 * the hold mask as the game lays it) and its shield. */
export function reader(o: { body: string; clips: string; templates: string; shield: string; only?: string[] }) {
  const body = new Glb(o.body);
  const clipsFile = new Glb(o.clips);
  const rest = posed(body, undefined, 0);
  const shield = hang(body, rest, "socket.hand.L", o.shield);
  if (!shield.board) throw new Error(`${o.shield} has no strap: not a shield`);
  const { shapes } = skinOf(body, o.templates, o.only ?? []);
  const spine = body.named("spine_03");
  const frame = (pose: Pose[]) => frameOf(shield, pose, (n) => body.named(n), (i) => body.parentOf[i]);
  const fit = fitter(body, rest, shield);
  const clipOf = (name: string): Clip => {
    const [under, over] = name.split("+");
    const c = over ? clipsFile.layered(clipOf(under), clipOf(over)) : clipsFile.clip(name);
    if (!c) throw new Error(`no clip ${name} in ${o.clips}`);
    return c;
  };
  return {
    shapes,
    clipOf,
    has: (name: string) => name.split("+").every((n) => clipsFile.clip(n) !== undefined),
    at: (shape: Shape, clip: Clip, t: number): Place => {
      // As the game poses it: the shield arm fitted to the body after the clip (shield_arm.ts).
      const pose = fit(shaped(body, shape, clip, t), shape.moved);
      return boardPlace(pose, rest, spine, frame(pose), shield);
    },
  };
}

/** The worst of a clip's board places against `target`, every `step` s on each template; `r.at`
 * the board's place on a template at a time of a clip. */
export function worstOff(r: { shapes: Shape[]; at: (shape: Shape, clip: Clip, t: number) => Place }, clip: Clip, target: Place, step = 1 / 15) {
  const worst = { cm: -Infinity, deg: -Infinity, cmWhere: "", degWhere: "" };
  for (const shape of r.shapes)
    for (let t = 0; t <= clip.end + 1e-6; t += step) {
      const off = placeOff(r.at(shape, clip, t), target);
      const when = `at ${t.toFixed(2)} s, ${shape.name}`;
      if (off.cm > worst.cm) (worst.cm = off.cm), (worst.cmWhere = when);
      if (off.deg > worst.deg) (worst.deg = off.deg), (worst.degWhere = when);
    }
  return worst;
}

function main() {
  const { values: opt } = parseArgs({
    options: {
      at: { type: "string" },
      set: { type: "string", default: "assets/data/moves/sword.shielded.ron" },
      clips: { type: "string", default: "assets/models/character/fight.glb" },
      body: { type: "string", default: "assets/models/character/base.glb" },
      templates: { type: "string", default: "assets/models/character/templates.json" },
      shield: { type: "string", default: "assets/models/held/heater_shield.glb" },
      // Besides the set's guard and moves, the clips a drawn fighter shows (moves.ts's `also`).
      also: { type: "string", default: "Keyed_Guard,Fight_Guard,Fight_Strafe_L,Fight_Strafe_R,Jump_Loop,Walk_Loop+Low_Guard" },
      // The template a pose's place is read on and its target captured from: the one both
      // fighters wear (tools/sandbox/duel/pose.ts).
      on: { type: "string", default: "male" },
      targets: { type: "string", default: TARGETS },
      capture: { type: "string" },
      board: { type: "boolean", default: false },
      why: { type: "string", default: "" },
      fps: { type: "string", default: "15" },
      json: { type: "string" },
    },
  });
  const files = [opt.set!, opt.clips!, opt.body!, opt.templates!, opt.shield!];
  const root = opt.at ? atCommit(opt.at, files) : ".";
  const from = (f: string) => join(root, f);
  const r = reader({ body: from(opt.body!), clips: from(opt.clips!), templates: from(opt.templates!), shield: from(opt.shield!) });
  const set = readFileSync(from(opt.set!), "utf8").replace(/\/\/.*$/gm, "");
  if (opt.at) rmSync(root, { recursive: true });
  const named = [...set.matchAll(/\b(?:guard|clip):\s*"([^"]+)"/g)].map((m) => m[1]);
  const poses = [...new Set([...named, ...opt.also!.split(",").filter(Boolean)])].filter(r.has);
  const on = r.shapes.find((s) => s.name === opt.on);
  if (!on) throw new Error(`no template ${opt.on}`);
  const targets = readTargets(opt.targets);
  const step = 1 / Number(opt.fps);
  const v = (p: V3) => `(${p.map((x) => x.toFixed(3)).join(", ")})`;
  const out: Record<string, unknown> = {};
  for (const name of poses) {
    const clip = r.clipOf(name);
    const first = r.at(on, clip, 0);
    const moves = worstOff({ ...r, shapes: [on] }, clip, first, step);
    const others = Object.fromEntries(r.shapes.map((s) => [s.name, placeOff(r.at(s, clip, 0), first)]));
    const spread = Math.max(...Object.values(others).map((o) => o.cm));
    const target = targets[name];
    const off = target && worstOff(r, clip, target, step);
    console.log(
      `${off && (off.cm > 1 || off.deg > 3) ? "·" : "✓"} ${name}: centre ${v(first.centre)} m, face ${v(first.face)}, up ${v(first.up)} on ${on.name}; ` +
        `over the clip ${moves.cm.toFixed(1)} cm, ${moves.deg.toFixed(1)}°; the templates within ${spread.toFixed(1)} cm` +
        (off ? `; off its target ${off.cm.toFixed(2)} cm (${off.cmWhere}), ${off.deg.toFixed(2)}° (${off.degWhere})` : ""),
    );
    out[name] = { first, over_clip: moves, templates: Object.fromEntries(r.shapes.map((s) => [s.name, r.at(s, clip, 0)])), target: off };
  }
  if (opt.capture) {
    if (!opt.at) throw new Error("--capture needs --at <commit>: a target names the assets it was read from");
    const clip = r.clipOf(opt.capture);
    const p = r.at(on, clip, 0);
    const round = (x: V3) => x.map((n) => Math.round(n * 1e5) / 1e5) as V3;
    const kept = existsSync(opt.targets!) ? JSON.parse(readFileSync(opt.targets!, "utf8")) : { targets: {} };
    const commit = execFileSync("git", ["rev-parse", "--short=8", opt.at], { encoding: "utf8" }).trim();
    kept.targets[opt.capture] = { centre: round(p.centre), face: round(p.face), up: round(p.up), from: commit, template: on.name, why: opt.why };
    writeFileSync(opt.targets!, JSON.stringify(kept, null, 1) + "\n");
    console.log(`✓ ${opt.capture}'s target captured from ${commit} on ${on.name} into ${opt.targets}`);
  }
  if (opt.board) {
    if (opt.at) throw new Error("--board writes the strap of the shield as it is now: no --at");
    const b = strappedBoard({ body: opt.body!, shield: opt.shield! });
    const round = (x: V3) => x.map((n) => Math.round(n * 1e5) / 1e5) as V3;
    const kept = existsSync(opt.targets!) ? JSON.parse(readFileSync(opt.targets!, "utf8")) : { targets: {} };
    kept.board = { centre: round(b.centre), face: round(b.face), up: round(b.up), shield: opt.shield };
    writeFileSync(opt.targets!, JSON.stringify(kept, null, 1) + "\n");
    console.log(`✓ the strapped board of ${opt.shield} written into ${opt.targets}: centre ${v(b.centre)} m from the fist`);
  }
  if (opt.json) writeFileSync(opt.json, JSON.stringify(out, null, 1) + "\n");
}

if (import.meta.main) main();
