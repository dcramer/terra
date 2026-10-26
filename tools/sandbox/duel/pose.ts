// The pose the game shows, on a duel's fight record (`pose`: every bone of a fighter's model and
// every look it holds, read after every pose writer and the transforms' propagation,
// terra_creatures::melee::pose, D530): what the move check (tools/characters/moves.ts) measures
// on clips one by one, measured on the composed pose a person sees, with its geometry from its
// home (glb.ts, clash.ts, shield_path.ts). Each a check for duel.ts and the combat scorecard's
// `--film`:
//
//   head yaw       the player's head turned off where it aims (its facing, along which its blows
//                  go) through its blows, degrees, + to its left; beside it the head off the hips
//                  and off the game camera's look. The developer, 2026-10-26: "my head turns like 45 degree to the left
//                  when aiming forward and swinging".
//   shield rest    the board's middle over the hips (pelvis), cm, where the player's rests with
//                  the block not held, drawn and in no move (the guard standing and walking): Enshrouded's guard
//                  carries it at the left hip (research/2026-10-25-shield-prior-art.md).
//   shield forearm the strapped board against the forearm it rides (elbow to wrist) in every
//                  drawn frame: the forearm's turn off the board's plane (degrees), the forearm's
//                  middle off the board's back (cm), and the hand strap (the look's origin, D504)
//                  off the fist (socket.hand.L), cm. The developer: "you'd have this wrapped
//                  around the arm in real life ... not uniform with the arm geometry".
//
// Node 22.18 or later; no dependencies.

import { resolve } from "node:path";
import { type Board, board, meshOf } from "../../characters/clash.ts";
import { Glb, type Pose, type Q, type V3, dot, len, place, posed, qinv, rot, sub } from "../../characters/glb.ts";
import { faceOff, gripOff } from "../../characters/shield_path.ts";
import type { Check, Fighter, Frame, Lens } from "./checks.ts";

const ROOT = resolve(import.meta.dirname, "../../..");
/// `[x, y, z, qx, qy, qz, qw, s]`: a place in the model's space (a bone, a held look), or the
/// model's root in the start's terms.
export type Eight = number[];
export type Posed = { root: Eight; bones: Record<string, Eight>; held: { look: string | null; on: string; strapped: boolean; at: Eight }[] };
export type Sheathed = { feet: number[]; facing: number[]; speed: number; shows: string | null; pose: Posed };
export type PoseFrame = Frame & { fighters: (Fighter & { pose?: Posed })[]; sheathed?: Sheathed[] };

/// The bars, what a person sees as wrong.
/// The head off the aim through a blow: eyes on the foe, a glance at most (Enshrouded's
/// fighters keep their heads on the target through a chain, enshrouded-combat-measured).
export const HEAD_DEG = 15;
/// The board's middle over the hips with the block not held: at the hip, within a hand
/// (Enshrouded's guard, research/2026-10-25-shield-prior-art.md: "at the left hip").
export const REST_CM = 10;
/// A strapped board's plane off the forearm: the forearm lies along the board's back.
export const FOREARM_DEG = 10;
/// The forearm's middle off the board's back: a forearm's half-thickness (3.5 cm, moves.ts's
/// pad) and a strap's padding.
export const OFF_CM = 5;
/// The hand strap off the fist (moves.ts's `grip`, held.shield.grip_hand's bar).
export const STRAP_CM = 1;
/// After the block is let go or a move ends, the frames its blend back to the guard may take.
const SETTLE = 20;
/// A body moving faster than this, m/s, walks.
const WALKING = 0.5;

export const eight = (e: Eight): Pose => ({ t: [e[0], e[1], e[2]], r: [e[3], e[4], e[5], e[6]], s: [e[7], e[7], e[7]] });
/// A direction's turn about the model's up (+Y) from ahead (+Z), degrees, + to the model's left (+X).
export const yaw = (v: V3): number => (Math.atan2(v[0], v[2]) * 180) / Math.PI;
/// An angle into -180..180.
export const wrap = (deg: number): number => ((((deg + 180) % 360) + 360) % 360) - 180;

/// The base body the fighters wear, its rest pose in the model's space by bone name.
let rested: Map<string, Pose> | undefined;
function rest(): Map<string, Pose> {
  if (rested) return rested;
  const body = new Glb(resolve(ROOT, "assets/models/character/base.glb"));
  const at = posed(body, undefined, 0);
  rested = new Map(body.gltf.nodes.map((n, i) => [n.name ?? "", at[i]]));
  return rested;
}

/// The way a bone points ahead as posed: its turn from rest applied to the model's ahead (+Z),
/// in the model's space.
export function ahead(pose: Posed, bone: string, at = rest()): V3 | undefined {
  const now = pose.bones[bone];
  const was = at.get(bone);
  if (!now || !was) return undefined;
  const r: Q = [now[3], now[4], now[5], now[6]];
  return rot(r, rot(qinv(was.r), [0, 0, 1]));
}

/// A direction in the start's terms, in the model's space.
export const intoModel = (pose: Posed, v: number[]): V3 => rot(qinv(eight(pose.root).r), v as V3);

/// The head's turn off `aim` (the model's space) and off the hips, degrees, + left.
export function headOff(pose: Posed, aim: V3, at = rest()): { aim: number; hips: number } | undefined {
  const head = ahead(pose, "Head", at);
  const hips = ahead(pose, "pelvis", at);
  if (!head || !hips) return undefined;
  return { aim: wrap(yaw(head) - yaw(aim)), hips: wrap(yaw(head) - yaw(hips)) };
}

/// The shield look's board, in its mesh's frame.
const boards = new Map<string, Board>();
function boardOf(look: string): Board {
  if (!boards.has(look)) boards.set(look, board(meshOf(new Glb(resolve(ROOT, "assets", look)))));
  return boards.get(look)!;
}

/// A strapped look: its board in the model's space, its hand strap (origin) and the strap's
/// bar (+X), and the way its face looks (-Z).
export function strappedBoard(pose: Posed, b?: Board): { board: Board; grip: V3; bar: V3; face: V3 } | undefined {
  const held = pose.held.find((h) => h.strapped && h.look);
  if (!held) return undefined;
  const p = eight(held.at);
  const plate = b ?? boardOf(held.look!);
  return {
    board: { centre: place(p, plate.centre), normal: rot(p.r, plate.normal), radius: plate.radius * p.s[0], half: plate.half * p.s[0] },
    grip: p.t,
    bar: rot(p.r, [1, 0, 0]),
    face: rot(p.r, [0, 0, -1]),
  };
}

/// The board against the forearm it rides: the forearm's turn off its plane (deg), the
/// forearm's middle off its back (cm, outside the board's thickness), the hand strap off the
/// fist (cm).
export function onForearm(pose: Posed, b?: Board): { deg: number; off: number; strap: number } | undefined {
  const s = strappedBoard(pose, b);
  const [elbow, wrist, fist] = ["lowerarm_l", "hand_l", "socket.hand.L"].map((n) => pose.bones[n]);
  if (!s || !elbow || !wrist || !fist) return undefined;
  const [e, w] = [eight(elbow).t, eight(wrist).t];
  const along = sub(w, e);
  const n = s.board.normal.map((x) => x / len(s.board.normal)) as V3;
  const deg = (Math.asin(Math.min(1, Math.abs(dot(along, n)) / len(along))) * 180) / Math.PI;
  const middle = e.map((x, j) => (x + w[j]) / 2) as V3;
  const off = (Math.abs(dot(sub(middle, s.board.centre), n)) - s.board.half) * 100;
  const knuckles = (["index_01_l", "pinky_01_l"].map((k) => pose.bones[k]) as Eight[]).map((k) => (k ? eight(k).t : e)) as [V3, V3];
  return { deg, off, strap: gripOff(eight(fist).t, knuckles, s.grip, s.bar).cm };
}

/// The board's middle over the hips (pelvis), cm, and its face off the chest's ahead, degrees.
export function shieldHigh(pose: Posed, b?: Board, at = rest()): { cm: number; face: number } | undefined {
  const s = strappedBoard(pose, b);
  const hips = pose.bones.pelvis;
  const chest = ahead(pose, "spine_03", at);
  if (!s || !hips || !chest) return undefined;
  return { cm: (s.board.centre[1] - hips[1]) * 100, face: faceOff(s.face, chest) };
}

/// A body on a frame, as the checks read it: who, drawn or not, what it does (a move's name,
/// "block", "walk" or "idle"), its pose, its facing and, for the player on a record from the
/// game camera (the player run's), that camera's look, in the start's terms.
export type Sample = { frame: number; who: string; drawn: boolean; doing: string; pose: Posed; facing: number[]; camera?: number[]; settled: boolean };

/// Every body's pose on every frame, with what it does.
export function samples(frames: PoseFrame[], byCamera: boolean): Sample[] {
  const out: Sample[] = [];
  const last = new Map<string, { feet: number[]; busy: number }>();
  frames.forEach((frame, i) => {
    const bodies = [
      ...frame.fighters.map((f) => ({ who: f.who, drawn: true, feet: f.feet, facing: f.facing, pose: f.pose, move: f.move, guard: f.guard })),
      ...(frame.sheathed ?? []).map((s) => ({ who: "you", drawn: false, feet: s.feet, facing: s.facing, pose: s.pose, move: null, guard: false })),
    ];
    for (const b of bodies) {
      const was = last.get(b.who);
      const speed = was ? Math.hypot(b.feet[0] - was.feet[0], b.feet[2] - was.feet[2]) * 60 : 0;
      const busy = b.move || b.guard ? i : (was?.busy ?? -Infinity);
      last.set(b.who, { feet: b.feet, busy });
      if (!b.pose) continue;
      const doing = b.guard ? "block" : (b.move ?? (speed > WALKING ? "walk" : "idle"));
      const camera = frame.camera as Lens | undefined;
      const seen = byCamera && b.who === "you" ? camera?.looking : undefined;
      out.push({ frame: frame.frame, who: b.who, drawn: b.drawn, doing, pose: b.pose, facing: b.facing, camera: seen, settled: i - busy > SETTLE });
    }
  });
  return out;
}

/// The moves that are blows: every move but the block, the roll and a reel (by the set's names).
const NOT_BLOWS = /^(block|parry|roll|idle|walk|hit|stagger|flinch|broken|down|reel)/i;
export const isBlow = (doing: string) => !NOT_BLOWS.test(doing);

const worstOf = <T>(xs: T[], by: (x: T) => number): T | undefined => xs.reduce<T | undefined>((a, x) => (a === undefined || by(x) > by(a) ? x : a), undefined);
const r1 = (x: number) => Math.round(x * 10) / 10;

/// The player's head off its aim through its blows (`head yaw`): its facing, which its blows go
/// along (the foe it is locked on, else the camera's ahead), the game camera's look beside it;
/// its value the worst, degrees.
export function headYaw(all: Sample[]): Check | undefined {
  const name = "head yaw in blows";
  const mine = all.filter((s) => s.who === "you" && s.drawn);
  if (!mine.length) return undefined;
  const off = mine.flatMap((s) => {
    const h = headOff(s.pose, intoModel(s.pose, s.facing));
    const seen = s.camera && headOff(s.pose, intoModel(s.pose, s.camera));
    return h ? [{ ...s, ...h, camera: seen?.aim }] : [];
  });
  const blows = off.filter((s) => isBlow(s.doing));
  const worst = worstOf(blows, (s) => Math.abs(s.aim));
  if (!worst) return { name, ok: true, line: "no blow of the player's with a pose", faults: 0 };
  const idle = off.filter((s) => s.doing === "idle");
  const mean = (xs: number[]) => xs.reduce((a, x) => a + x, 0) / Math.max(1, xs.length);
  const side = (deg: number) => `${Math.abs(deg).toFixed(0)}° ${deg >= 0 ? "left" : "right"}`;
  const over = blows.filter((s) => Math.abs(s.aim) > HEAD_DEG);
  const camera = worst.camera === undefined ? "" : `, off the camera's look ${side(worst.camera)}`;
  const line = `the player's head off its aim in blows, worst ${side(worst.aim)} (${worst.doing}, frame ${worst.frame}; off the hips ${side(worst.hips)}${camera}), on average ${side(mean(blows.map((s) => s.aim)))}; standing on guard ${side(mean(idle.map((s) => s.aim)))} (bar ${HEAD_DEG}°)`;
  return { name, ok: over.length === 0, line, faults: over.length, value: r1(Math.abs(worst.aim)) };
}

/// The board's middle over the hips with the block not held, drawn and settled in no move
/// (`shield rest`): its value where the player's rests (the median of its frames standing and
/// walking on guard), cm; the highest of either fighter's beside it.
export function shieldRest(all: Sample[], b?: Board): Check | undefined {
  const name = "shield at rest";
  const resting = all.filter((s) => s.drawn && s.settled && (s.doing === "idle" || s.doing === "walk"));
  const high = resting.flatMap((s) => {
    const h = shieldHigh(s.pose, b);
    return h ? [{ ...s, ...h }] : [];
  });
  const mine = high.filter((s) => s.who === "you").sort((x, y) => x.cm - y.cm);
  const rests = mine[Math.floor(mine.length / 2)];
  const worst = worstOf(high, (s) => s.cm);
  if (!rests || !worst) return undefined;
  const raised = all.filter((s) => s.doing === "block" && s.who === "you").flatMap((s) => shieldHigh(s.pose, b) ?? []);
  const over = high.filter((s) => s.cm > REST_CM);
  const whose = worst.who === "you" ? "the player's" : `the ${worst.who}'s`;
  const block = raised.length ? `; raised in its block ${Math.max(...raised.map((h) => h.cm)).toFixed(0)} cm` : "";
  const line = `the player's shield with the block not held rests ${rests.cm.toFixed(0)} cm over the hips (the median of ${mine.length} frames on guard, its face ${rests.face.toFixed(0)}° off ahead)${block}; the highest ${worst.cm.toFixed(0)} cm (${whose} ${worst.doing}, frame ${worst.frame}) (bar ${REST_CM} cm)`;
  return { name, ok: over.length === 0, line, faults: over.length, value: r1(rests.cm) };
}

/// The strapped board against the forearm in every drawn frame (`shield forearm`): three checks,
/// the forearm off the board's plane, the forearm off its back, the hand strap off the fist.
export function shieldForearm(all: Sample[], b?: Board): Check[] {
  const fit = all.filter((s) => s.drawn).flatMap((s) => {
    const f = onForearm(s.pose, b);
    return f ? [{ ...s, ...f }] : [];
  });
  if (!fit.length) return [];
  const of = (key: "deg" | "off" | "strap", name: string, bar: number, unit: string, what: string): Check => {
    const worst = worstOf(fit, (s) => s[key])!;
    const over = fit.filter((s) => s[key] > bar);
    const whose = worst.who === "you" ? "the player's" : `the ${worst.who}'s`;
    const line = `${what} ${worst[key].toFixed(1)} ${unit} at worst (${whose} ${worst.doing}, frame ${worst.frame}), over the bar in ${over.length} of ${fit.length} poses (bar ${bar} ${unit})`;
    return { name, ok: over.length === 0, line, faults: over.length, value: r1(worst[key]) };
  };
  return [
    of("deg", "shield off the forearm's line", FOREARM_DEG, "°", "the forearm's turn off the board's plane"),
    of("off", "shield off the forearm", OFF_CM, "cm", "the forearm's middle off the board's back"),
    of("strap", "shield strap off the fist", STRAP_CM, "cm", "the hand strap off the fist"),
  ];
}

/// Every composed-pose check on a record; the head's also against the game camera's look when
/// the record is the player run's.
export function poseChecks(frames: PoseFrame[], byCamera: boolean): Check[] {
  const all = samples(frames, byCamera);
  if (!all.length) return [];
  return [headYaw(all), shieldRest(all), ...shieldForearm(all)].filter((c): c is Check => c !== undefined);
}
