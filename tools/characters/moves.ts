#!/usr/bin/env node
// The move check (D447, docs/movement.md "Second round"): what a move set's clips get wrong on
// the bodies that play them, measured from the exported models without Blender or the game.
// For every move of the set, and the clips a drawn fighter shows besides, on every body
// template (thin man, woman, heavy, strong):
//
//   hands   how deep a hand, forearm, elbow or upper arm goes into the body (torso, head,
//           thighs), skinned
//   held    how deep a held look (sword, shield) goes into it
//   clash   held look against held look (clash.ts, D481): how deep the sword (blade, guard,
//           hilt) goes through the shield's board, and the board through the sword arm (right
//           hand and forearm); a bar for every move and clip, scored (tools/combat) for every
//           move
//   arc     a blow's tip at its fastest in its active frames: its bearing from ahead (+ left)
//           and how far ahead, which way it sweeps, how far the hips have turned; a blow
//           whose clip has a spec (D450, clip_spec.ts) fails a sweep or contact against it
//   guard   a block's shield at its hold: how far its face is turned from ahead, how far
//           in front of the hips it stands
//   chain   how far a move's last pose is from the next one's first, and each blow's first
//           and last pose from the guard's: hands, forearms, head and blade tip, metres
//   carry   where a blow carries its shield while its sword swings (its active frames; the
//           guard is its own row), shield_path.ts, research/2026-10-25-shield-prior-art.md:
//           the board's middle above the left shoulder at its highest, the left elbow below the
//           shoulder at its least, the painted face off the chest's forward at its most; and
//           for each chained pair, the shield's round trip as the next blow starts at the
//           worst press of the window, playing what the runner picks there (its `chained`
//           take mixed in by the outgoing's `carry`, D454), crossfaded over the game's move
//           blend; each chained take is checked as a row of its own. Numbers for the scorecard
//           (tools/combat), not bars here: a row "blows" and a row "chain" take the worst
//   grip    the shield's grip (its look's origin, as it is hung: a strapped shield's hand strap,
//           D504) against the left fist: how far it is from socket.hand.L's origin, and how far
//           its bar (+Y; a strapped look's hand strap, across its board, +X) turns off the line
//           across the palm (index to little finger's first joints), the worst of every move
//           and clip on every template, and the guard's first frame; a row "grip" for the
//           scorecard (held.shield.grip_*, research/2026-10-25-shield-prior-art.md fault 3)
//   board   each shield pose with a target (art/animation/shield_targets.json, D554): how far the
//           board is from it in the chest's frame, cm and degrees, the worst of its clip on every
//           template (shield_place.ts); a row "<pose> board" for the scorecard
//   spec    a clip with a spec: from and back to the guard; one that holds the guard
//           ("held") within a blend of it throughout; a block's shield arm up in front and
//           still while it is "raised"
//
// A held look hangs from its hand socket, or from the bone its file straps it to (a shield on the
// forearm), as the game hangs it; bends laid over the clips (--bend) are applied as the game does.
// A line a row; ✗ marks what is over its bar; exit 1 if any is.
//
//   node tools/characters/moves.ts
//   node tools/characters/moves.ts [--set assets/data/moves/sword.shielded.ron] [--fps 15] [--verbose]
//   node tools/characters/moves.ts --spec art/animation/sets/fight.json   # the clips' specs (the default)
//   node tools/characters/moves.ts --json rows.json                       # the rows also as JSON
//
// Node 22.18 or later; no dependencies.

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { againstSpec, type Blow } from "./clip_spec.ts";
import { type Ball, type Board, through } from "./clash.ts";
import { type Held, carriedClips, carryHold, carryTilt, frameOf, hang } from "./held.ts";
import { TARGETS, boardPlace, readTargets, worstOff } from "./shield_place.ts";
import { PACE_FRAMES } from "./phases.ts";
import { type Shape, armPoints, shaped, skinOf } from "./skin.ts";
import { MOVE_BLEND_S, ROUND_TRIP_S, faceOff, gripOff, roundTrip } from "./shield_path.ts";
import { parseArgs } from "node:util";
import { Glb, type Clip, type Pose, type Q, type V3, len, place, posed, qinv, qmul, rot, sub, turn } from "./glb.ts";
import { type Holding, keptOff, reach } from "./arms.ts";
import { carryArm } from "./carry_arm.ts";
import { about, atWrist as atWristShare, elbowOpen, inFist as inFistShare, wristTurn } from "./carry_hand.ts";
import { fitter } from "./shield_arm.ts";
import { type Handle, FIST_CM, WRAP_DEG, close, fistOff, handRest, lookGrip, seated, tipped as tipHandle } from "./grip.ts";

const { values: opt } = parseArgs({
  options: {
    set: { type: "string", default: "assets/data/moves/sword.shielded.ron" },
    clips: { type: "string", default: "assets/models/character/fight.glb" },
    body: { type: "string", default: "assets/models/character/base.glb" },
    templates: { type: "string", default: "assets/models/character/templates.json" },
    // What the hands hold, socket=look (`terra_creatures::melee`, DRAWN): an off-hand look with a
    // strap is a shield; another (a torch) is carried in the fist, with no board.
    hold: { type: "string", multiple: true, default: ["socket.hand.R=assets/models/held/arming_sword.glb", "socket.hand.L=assets/models/held/heater_shield.glb"] },
    // Bends laid over the clips, BONE=x,y,z degrees about the body's axes (staged bends); none in the prototype.
    bend: { type: "string", multiple: true, default: [] },
    also: { type: "string", default: "Low_Guard,Keyed_Guard,Fight_Guard,Fight_Strafe_L,Fight_Strafe_R,Walk_Loop+Low_Guard,Jog_Fwd_Loop+Low_Guard,Sprint_Loop+Low_Guard,Jump_Loop" },
    // The guard every blow starts and ends at: the set's own `guard` by default (D450), else
    // Fight_Guard for a set that names none (clip.ts's scratch sets).
    guard: { type: "string" },
    // The clips' specs: a blow's sweep and contact (`"blow"`), by clip; a clip `X#N` (a take
    // the clip step tries) is X's.
    spec: { type: "string", default: "art/animation/sets/fight.json" },
    // Each shield pose's board place, the one number it authors (D554, shield_place.ts).
    targets: { type: "string", default: TARGETS },
    fps: { type: "string", default: "15" },
    // Only these body templates, comma-separated; all of them otherwise.
    template: { type: "string", default: "" },
    verbose: { type: "boolean", default: false },
    // The rows also as JSON, for a caller (clip.ts, tools/combat/scorecard.ts): [{move, clip,
    // bad, gaps, measured}], gaps the metres from the guard at the move's start and end, a
    // blow's only; measured each number the row was judged by, named by what it is.
    json: { type: "string" },
    // Carried: in the clips the game carries in (terra_walker::character::CARRIED_IN), a look in
    // hand turned its `carry_tilt` about its X, as character/held.rs's `tilt` turns it (fit.ts).
    carry: { type: "boolean", default: false },
    // The hanging hands kept off the hips after the clip, as terra_walker::arms keeps them
    // (arms.ts, D535): the walker's composed pose (fit.ts).
    arms: { type: "boolean", default: false },
    // The fists closed on what the hands hold, as terra_walker::grip closes them (grip.ts);
    // `--no-grip` leaves the clips' fingers.
    grip: { type: "boolean", default: true },
  },
  allowNegative: true,
});

// The bars: what a person sees as wrong.
const HAND_CM = 2; // a hand or arm through the skin
const HELD_CM = 1; // a sword or shield through the body
const CLASH_CM = 0; // a move's or clip's sword through its own shield, or the shield through the sword arm (D481)
const ARC_DEG = 35; // a blow's contact off ahead
const AHEAD_M = 0.5; // a blow's contact this far in front of the hips at least
const FACE_DEG = 35; // a raised shield's face off ahead
const CHAIN_M = 0.1; // a pose jump between moves that a blend shows
// A block's shield arm while raised (its spec's "raised"), from the hips: the left hand this
// far ahead and up at least, the elbow ahead (at the guard it is beside the body), and still.
const RAISED_M = 0.3;
const RAISED_UP_M = 0.42; // across the face, above a guard's forearm across the chest
const ELBOW_M = 0.2;
const STILL_M = 0.08;

type Move = {
  name: string;
  clip: string;
  from: number;
  speed: number;
  done: number;
  active?: [number, number];
  next?: string;
  cancel?: number;
  hold?: number;
  /** The same blow with its shield out from its first frame, mixed in when chained. */
  chained?: string;
  /** How its clip carries the shield out: eased out over `into`, back over `out` (clip s). */
  carry?: { into: [number, number]; out: [number, number] };
  /** A chained take's row: entered from a blow's carry, not from the guard. */
  entered?: "chain";
};

/** The clip a set names as its guard (`guard: "..."` outside its moves), if any. */
function setGuard(file: string): string | undefined {
  const text = readFileSync(file, "utf8").replace(/\/\/.*$/gm, "");
  return text.slice(0, text.search(/\bmoves\s*:/)).match(/\bguard:\s*"([^"]+)"/)?.[1];
}

function carryOf(body: string): Move["carry"] {
  const m = body.match(/\bcarry:\s*\(\s*in:\s*\((-?[\d.]+),\s*(-?[\d.]+)\),\s*out:\s*\((-?[\d.]+),\s*(-?[\d.]+)\)\s*\)/);
  return m ? { into: [Number(m[1]), Number(m[2])], out: [Number(m[3]), Number(m[4])] } : undefined;
}

function readSet(file: string): Move[] {
  const text = readFileSync(file, "utf8").replace(/\/\/.*$/gm, "");
  const moves: Move[] = [];
  for (const [, name, body] of text.matchAll(/"(\w+)":\s*\(([\s\S]*?)\),\s*(?="\w+":|\})/g)) {
    const num = (field: string) => {
      const m = body.match(new RegExp(`\\b${field}:\\s*(-?[\\d.]+)`));
      return m ? Number(m[1]) : undefined;
    };
    const active = body.match(/\bactive:\s*\((-?[\d.]+),\s*(-?[\d.]+)\)/);
    moves.push({
      name,
      clip: body.match(/\bclip:\s*"([^"]+)"/)![1],
      from: num("from") ?? 0,
      speed: num("speed") ?? 1,
      done: num("done")!,
      active: active ? [Number(active[1]), Number(active[2])] : undefined,
      next: body.match(/\bnext:\s*"([^"]+)"/)?.[1],
      cancel: num("cancel"),
      hold: num("hold"),
      chained: body.match(/\bchained:\s*"([^"]+)"/)?.[1],
      carry: carryOf(body),
    });
  }
  return moves;
}

const clipsFile = new Glb(opt.clips!);
const body = new Glb(opt.body!);
// `--set none`: no moves, only the clips `--also` names (fit.ts, for a class with no set).
const moves = opt.set === "none" ? [] : readSet(opt.set!);
const fps = Number(opt.fps);
const bends = new Map<string, Q>(
  opt.bend!.map((b) => {
    const [bone, deg] = b.split("=");
    return [bone, turn(deg.split(",").map(Number) as V3)];
  }),
);
const clips = new Map<string, Clip>();
// `--carry`: each carry pose a held look names (its `carry_hold`, D560) with the clavicle of the
// arm it goes on, set once the looks are hung below.
let carryPoses: [string, string][] = [];
const clipOf = (name: string): Clip => {
  if (!clips.has(name)) {
    // `Walk_Loop+Keyed_Guard`: a locomotion clip under a stance's hold, laid by the mask.
    const [under, over] = name.split("+");
    let c = over ? clipsFile.layered(clipOf(under), clipOf(over)) : clipsFile.clip(name);
    if (!c) throw new Error(`no clip ${name} in ${opt.clips}`);
    // A carried clip with no stance hold takes each carry pose on its arm alone, as
    // terra_walker::figure lays a `CarryHold` (Nodes::over_one); the clip keeps its name.
    if (!over && carriedIn.includes(name)) for (const [pose, root] of carryPoses) c = { ...clipsFile.layered(c, clipOf(pose), [root]), name };
    clips.set(name, c);
  }
  return clips.get(name)!;
};

// The body: its skin, which of its vertices are torso, head or thigh (by their heaviest joint),
// and each template's morph mix and joint offsets (skin.ts).
const { shapes, skinned } = skinOf(body, opt.templates!, opt.template!.split(",").filter(Boolean));

// The pose as the game composes it: the hanging hands kept off the hips (`--arms`), then the shield
// arm fitted to the body (shield_arm.ts, D554), each as the game does after the clips.
const poseOf = (shape: Shape, clip: Clip, t: number): Pose[] => fitShield(armsKept(shape, clip, t), shape.moved);
// `--arms`: the hanging hands then kept off the hips as the game keeps them (arms.ts, D535).
// The fists closed first (`inFist`), as terra_walker::grip runs before arms::keep_off_hips.
const armsKept = (shape: Shape, played: Clip, t: number): Pose[] => {
  const clip = atWrists(inFist(shape, played));
  const base = new Map(carriedArms(shape, clip, t));
  // A tool carried by the rule: its forearm kept down (carry_hand.ts), as
  // terra_walker::carry_hand opens the elbow before the fists and the arms.
  const elbows = elbowsOpened(shaped(body, shape, clip, t, base), clip);
  for (const [bone, q] of elbows) base.set(bone, q);
  const pose = shaped(body, shape, clip, t, base);
  if (!opt.arms) return pose;
  const kept = keptOff(body, pose, holdingIn(clip));
  if (!kept.size) return pose;
  for (const [bone, q] of base) kept.set(bone, kept.has(bone) ? qmul(kept.get(bone)!, q) : q);
  // The game opens the elbow in the upper arm's frame before the arm is turned out: the
  // forearm's turn goes on after the upper arm's, so it is turned with it.
  for (const [bone, q] of elbows) {
    const out = kept.get(bone.replace("lowerarm", "upperarm"));
    if (out) kept.set(bone, qmul(qmul(out, q), qinv(out)));
  }
  return shaped(body, shape, clip, t, kept);
};

// Held looks, hung as `terra_walker::character` hangs them (held.ts).
const rest = posed(body, undefined, 0);
const held: Held[] = opt.hold!.map((h) => {
  const [hand, file] = h.split("=");
  return hang(body, rest, hand, file);
});
// Each look's points as carried (--carry): a look on its hand socket turned its carry_tilt about
// its X, which is the socket's (a strapped look's is 0 and stays).
const carriedIn = opt.carry ? carriedClips() : [];
const tiltOf = new Map(held.map((h, i) => [h.name, h.socket === h.hand ? carryTilt(opt.hold![i].split("=")[1]) : 0]));
// A carry pose goes on the arm of the hand the look is in, a strapped look's on its fist's side.
const poseOfLook = held.map((_, i) => (opt.carry ? carryHold(opt.hold![i].split("=")[1]) : undefined));
carryPoses = held.flatMap((h, i): [string, string][] => {
  const pose = poseOfLook[i];
  return pose ? [[pose, `clavicle_${(h.strap?.fist ?? h.strap?.bone ?? h.hand).slice(-1).toLowerCase()}`]] : [];
});
const carriedPoses = new Set(held.filter((_, i) => poseOfLook[i]).map((h) => h.name));
// A carry pose's arm held in the body's frame, not the chest's (carry_arm.ts, D560), in a carried
// clip with no stance hold, as terra_walker::carry_arm holds it before the fists and the arms.
const carryTurns = carryArm(body, rest, carryPoses.map(([, root]) => root));
function carriedArms(shape: Shape, clip: Clip, t: number): Map<string, Q> {
  if (!carryPoses.length || !carriedNow(clip) || clip.name.includes("+")) return bends;
  const turns = carryTurns(shaped(body, shape, clip, t, bends));
  for (const [bone, q] of bends) turns.set(bone, turns.has(bone) ? qmul(turns.get(bone)!, q) : q);
  return turns;
}
const tipped = (deg: number, points: V3[]) => {
  const [c, s] = [Math.cos((deg * Math.PI) / 180), Math.sin((deg * Math.PI) / 180)];
  return points.map(([x, y, z]) => [x, y * c - z * s, y * s + z * c] as V3);
};
// The share of its tilt a look takes in the fist; the wrist takes the rest (carry_hand.ts).
const fistTilt = (h: Held) => inFistShare(tiltOf.get(h.name)!);
const carriedPoints = new Map(held.map((h) => [h.name, tipped(fistTilt(h), h.points)]));
const carriedNow = (clip: Clip) => carriedIn.includes(clip.name.split("+")[0]);
// The hands carrying a look by the rule (a tilt, no carry pose): each turned at the wrist after
// its clip, about its socket's X, by the tilt past the fist's share, as terra_walker::carry_hand
// turns it; and its forearm kept down (`elbowsOpened`).
const ruled = held.filter((h, i) => h.socket === h.hand && tiltOf.get(h.name) && !poseOfLook[i]);
const wrists = new Map<Clip, Clip>();
function atWrists(clip: Clip): Clip {
  if (!ruled.length || !carriedNow(clip)) return clip;
  if (!wrists.has(clip)) {
    const tracks = new Map(clip.tracks);
    for (const h of ruled) {
      const hand = `hand_${h.hand.slice(-1).toLowerCase()}`;
      const turn = wristTurn(body.rest(body.named(h.hand)).r, atWristShare(tiltOf.get(h.name)!));
      const track = clip.tracks.get(hand);
      const from = track?.r ?? (() => body.rest(body.named(hand)).r);
      tracks.set(hand, { ...track, r: (time: number) => qmul(from(time) as Q, turn) });
    }
    wrists.set(clip, { ...clip, tracks });
  }
  return wrists.get(clip)!;
}
function elbowsOpened(pose: Pose[], clip: Clip): Map<string, Q> {
  const out = new Map<string, Q>();
  if (!carriedNow(clip)) return out;
  for (const h of ruled) {
    const s = h.hand.slice(-1).toLowerCase();
    const P = (n: string) => pose[body.named(n)].t;
    const open = elbowOpen(sub(P(`lowerarm_${s}`), P(`upperarm_${s}`)), sub(P(`hand_${s}`), P(`lowerarm_${s}`)));
    if (open) out.set(`lowerarm_${s}`, about(open.axis, -open.open));
  }
  return out;
}
const pointsIn = (h: Held, clip: Clip) => (carriedNow(clip) ? carriedPoints.get(h.name)! : h.points);
// `--arms`: what each hand holds counts in its arm's spacing, as the game counts it: a look on
// its hand socket by its reach (arms.ts), tipped when carried.
const reaches = held.filter((h) => h.socket === h.hand).map((h) => ({ h, points: reach(h.points) }));
const holdingIn = (clip: Clip): Holding[] =>
  reaches.map(({ h, points }) => ({ socket: h.socket, points: carriedNow(clip) ? tipped(fistTilt(h), points) : points }));

// The fists (grip.ts): what a hand socket holds closes its fingers on it after the clip,
// as terra_walker::grip lays them, each template's from its own hand, the handle seated against
// the palm for a power grip (held.ts seats the look) and tipped when carried. `--no-grip` leaves
// the clips' fingers.
const fists = opt.grip
  ? held.flatMap((h, i) => {
      const g = h.socket === h.hand ? lookGrip(opt.hold![i].split("=")[1]) : undefined;
      if (!g) return [];
      return [{ h, side: h.hand === "socket.hand.R" ? "r" : "l", grip: g.grip, handle: g.grip === "power" ? seated(g.handle) : g.handle }];
    })
  : [];
type Fist = (typeof fists)[number];
const handleNow = (f: Fist, clip: Clip): Handle => (carriedNow(clip) ? tipHandle(f.handle, fistTilt(f.h)) : f.handle);
const fistPoses = new Map<string, Map<string, Q>>();
const fistPose = (shape: Shape, f: Fist, clip: Clip): Map<string, Q> => {
  const key = `${shape.name} ${f.h.name} ${carriedNow(clip)}`;
  if (!fistPoses.has(key)) {
    const { locals, socket } = handRest(body, f.side, shape.moved);
    fistPoses.set(key, close(locals, socket, handleNow(f, clip), f.grip, f.side));
  }
  return fistPoses.get(key)!;
};
const fisted = new Map<Clip, Map<string, Clip>>();
function inFist(shape: Shape, clip: Clip): Clip {
  if (!fists.length) return clip;
  if (!fisted.has(clip)) fisted.set(clip, new Map());
  const byShape = fisted.get(clip)!;
  if (!byShape.has(shape.name)) {
    const tracks = new Map(clip.tracks);
    for (const f of fists) for (const [bone, q] of fistPose(shape, f, clip)) tracks.set(bone, { ...clip.tracks.get(bone), r: () => q });
    byShape.set(shape.name, { ...clip, tracks });
  }
  return byShape.get(shape.name)!;
}


const at = (name: string) => {
  const i = body.named(name);
  if (i < 0) throw new Error(`no ${name} in ${opt.body}`);
  return i;
};
// The frame a held look's points are in at a pose (held.ts): its socket's, or its strap's.
const frame = (h: Held, pose: Pose[]): Pose => frameOf(h, pose, at, (i) => body.parentOf[i]);
const node = Object.fromEntries(
  ["pelvis", "Head", "spine_03", "thigh_l", "thigh_r", "socket.hand.L", "index_01_l", "pinky_01_l", ...["l", "r"].flatMap((s) => [`hand_${s}`, `lowerarm_${s}`, `upperarm_${s}`, `middle_01_${s}`, `middle_02_${s}`, `index_02_${s}`, `thumb_02_${s}`])].map((n) => [n, at(n)]),
);
const mid = (a: V3, b: V3): V3 => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2, (a[2] + b[2]) / 2];

type Worst = { cm: number; where: string };
const worse = (w: Worst, cm: number, where: string) => (cm > w.cm ? { cm, where } : w);

// The sword arm as balls, from the joints: the forearm a centimetre apart, as thick as the
// hands check pads it (the elbow 4 cm, its middle 3.5), and the hand's.
function swordArm(P: (n: string) => V3): Ball[] {
  const [elbow, wrist] = [P("lowerarm_r"), P("hand_r")];
  const n = Math.max(1, Math.ceil(len(sub(wrist, elbow)) / 0.01));
  const arm: Ball[] = [...Array(n + 1).keys()].map((i) => ({ p: elbow.map((x, j) => x + ((wrist[j] - x) * i) / n) as V3, r: 0.04 - (0.01 * i) / n, part: "forearm_r" }));
  arm.push({ p: mid(P("hand_r"), P("middle_01_r")), r: 0.025, part: "hand_r" });
  for (const f of ["middle_02_r", "index_02_r", "thumb_02_r"]) arm.push({ p: P(f), r: 0.012, part: "hand_r" });
  return arm;
}
const sword = held.find((h) => h.balls);
const shield = held.find((h) => h.board);
const fitShield = shield ? fitter(body, rest, shield) : (pose: Pose[]) => pose;

// Where the shield is carried (shield_path.ts): the board's middle as the clash hangs it, the
// chest's forward (spine_03's, ahead at rest), and the middle in the chest's frame, which a
// shield arm held still against the chest keeps however the body turns.
const chestRest = qinv(rest[node.spine_03].r);
const chestAhead = (pose: Pose[]): V3 => rot(pose[node.spine_03].r, rot(chestRest, [0, 0, 1]));
const shieldMiddle = (pose: Pose[]): V3 => place(frame(shield!, pose), shield!.board!.centre);
function inChest(pose: Pose[]): V3 {
  const c = pose[node.spine_03];
  return rot(qinv(c.r), sub(shieldMiddle(pose), c.t)).map((x) => x / c.s[0]) as V3;
}

// Hands, forearms and held looks against the body, and the held looks against each other, over
// a span of a clip, on every template; the shield's carry over `carrying` (a blow's active
// frames: while its sword swings, the guard is dropped).
function penetration(clip: Clip, from: number, to: number, carrying: [number, number] = [from, to]) {
  let hands: Worst = { cm: -Infinity, where: "" };
  let heldW: Worst = { cm: -Infinity, where: "" };
  // Each look's own, so a fit judges its item and not its partner (fit.ts).
  const byLook = new Map<string, Worst>(held.map((h) => [h.name, { cm: -Infinity, where: "" }]));
  // A look carried in a pose in its hand (D560, a torch's flame up): degrees its handle (+Y) is
  // off the world's up, the worst while carried.
  const upBy = new Map<string, Worst>(held.filter((h) => h.socket === h.hand && carriedPoses.has(h.name)).map((h) => [h.name, { cm: -Infinity, where: "" }]));
  let swordW: Worst = { cm: -Infinity, where: "" };
  let armW: Worst = { cm: -Infinity, where: "" };
  // The carry: cm above the shoulder, m of the elbow below it (the least, kept negated), degrees off.
  let high: Worst = { cm: -Infinity, where: "" };
  let elbow: Worst = { cm: -Infinity, where: "" };
  let face: Worst = { cm: -Infinity, where: "" };
  // The shield's grip against the left fist: cm off it, degrees off the line across the palm.
  let gripCm: Worst = { cm: -Infinity, where: "" };
  let gripDeg: Worst = { cm: -Infinity, where: "" };
  for (const shape of shapes) {
    for (let t = from; t <= to + 1e-6; t += 1 / fps) {
      const pose = poseOf(shape, clip, t);
      const inside = skinned(shape, pose);
      const P = (n: string) => pose[node[n]].t;
      for (const s of ["l", "r"]) {
        const pts = armPoints(P, s);
        for (const [q, pad, what] of pts) {
          const { depth, part } = inside(q);
          hands = worse(hands, (depth - pad) * 100, `${what} in ${part} at ${t.toFixed(2)} s, ${shape.name}`);
        }
      }
      for (const h of held) {
        const socket = frame(h, pose);
        if (upBy.has(h.name) && carriedNow(clip)) {
          const up = rot(socket.r, [0, 1, 0]);
          const deg = (Math.acos(Math.max(-1, Math.min(1, up[1] / len(up)))) * 180) / Math.PI;
          upBy.set(h.name, worse(upBy.get(h.name)!, deg, `at ${t.toFixed(2)} s, ${shape.name}`));
        }
        for (const q of pointsIn(h, clip)) {
          const { depth, part } = inside(place(socket, q));
          const where = `${h.name} in ${part} at ${t.toFixed(2)} s, ${shape.name}`;
          heldW = worse(heldW, depth * 100, where);
          byLook.set(h.name, worse(byLook.get(h.name)!, depth * 100, where));
        }
      }
      if (sword && shield) {
        const [S, B] = [frame(sword, pose), frame(shield, pose)];
        const b = shield.board!;
        const disc: Board = { centre: place(B, b.centre), normal: rot(B.r, b.normal), radius: b.radius * B.s[0], half: b.half * B.s[0] };
        const blade = through(sword.balls!.map((q) => ({ ...q, p: place(S, q.p), r: q.r * S.s[0] })), disc);
        swordW = worse(swordW, blade.cm, `${blade.part} in ${shield.name} at ${t.toFixed(2)} s, ${shape.name}`);
        const arm = through(swordArm(P), disc);
        armW = worse(armW, arm.cm, `${shield.name} in ${arm.part} at ${t.toFixed(2)} s, ${shape.name}`);
        const g = gripOff(pose[node["socket.hand.L"]].t, [P("index_01_l"), P("pinky_01_l")], place(B, shield.grip), rot(B.r, shield.bar));
        gripCm = worse(gripCm, g.cm, `at ${t.toFixed(2)} s, ${shape.name}`);
        gripDeg = worse(gripDeg, g.deg, `at ${t.toFixed(2)} s, ${shape.name}`);
        if (t < carrying[0] - 1e-6 || t > carrying[1] + 1e-6) continue;
        const when = `at ${t.toFixed(2)} s, ${shape.name}`;
        const shoulder = P("upperarm_l");
        high = worse(high, (shieldMiddle(pose)[1] - shoulder[1]) * 100, when);
        elbow = worse(elbow, P("lowerarm_l")[1] - shoulder[1], when);
        face = worse(face, faceOff(rot(B.r, shield.front), chestAhead(pose)), when);
      }
    }
  }
  return { hands, held: heldW, byLook, upBy, sword: swordW, arm: armW, carry: { high, elbow: { cm: -elbow.cm, where: elbow.where }, face }, grip: { cm: gripCm, deg: gripDeg } };
}

// Points that say where a pose is, relative to the hips: hands, forearms, head, blade tip.
function marks(clip: Clip, t: number): V3[] {
  const pose = poseOf(shapes[0], clip, t);
  const hips = pose[node.pelvis].t;
  const sword = held.find((h) => h.hand === "socket.hand.R");
  const pts = ["hand_l", "hand_r", "lowerarm_l", "lowerarm_r", "Head"].map((n) => pose[node[n]].t);
  if (sword) pts.push(place(pose[at(sword.socket)], sword.tip));
  return pts.map((p) => sub(p, hips));
}
const gap = (a: V3[], b: V3[]) => Math.max(...a.map((p, i) => len(sub(p, b[i]))));

// A blow's tip in its active frames, in the model's space from the hips at the move's start.
function arc(move: Move) {
  const clip = clipOf(move.clip);
  const sword = held.find((h) => h.hand === "socket.hand.R")!;
  const start = poseOf(shapes[0], clip, move.from)[node.pelvis].t;
  let fastest = { speed: -1, tip: [0, 0, 0] as V3, t: 0, hips: 0 };
  let first: V3 | undefined;
  let last: V3 = [0, 0, 0];
  const [a, b] = move.active!;
  const tipAt = (t: number) => sub(place(poseOf(shapes[0], clip, t)[at(sword.socket)], sword.tip), start);
  // Its speed over a few frames either side, as phases.ts paces it: a frame's jump is no blow.
  const h = PACE_FRAMES / 60;
  for (let t = a; t <= b + 1e-6; t += 1 / 120) {
    const pose = poseOf(shapes[0], clip, t);
    const tip = sub(place(pose[at(sword.socket)], sword.tip), start);
    const speed = len(sub(tipAt(Math.min(clip.end, t + h)), tipAt(Math.max(0, t - h)))) / (2 * h);
    const across = sub(pose[node.thigh_l].t, pose[node.thigh_r].t);
    const hips = (Math.atan2(-across[2], across[0]) * 180) / Math.PI;
    if (speed > fastest.speed) fastest = { speed, tip, t, hips };
    first ??= tip;
    last = tip;
  }
  const bearing = (Math.atan2(fastest.tip[0], fastest.tip[2]) * 180) / Math.PI;
  const sweep = first && last[0] > first[0] ? "to the left" : "to the right";
  return { bearing, ahead: fastest.tip[2], up: fastest.tip[1], sweep, hips: fastest.hips, speed: fastest.speed, t: fastest.t, first: first ?? last, last };
}

function guard(move: Move) {
  const shield = held.find((h) => h.board);
  if (!shield || move.hold === undefined) return undefined;
  const pose = poseOf(shapes[0], clipOf(move.clip), move.hold);
  const socket = frame(shield, pose);
  const face = rot(socket.r, shield.thin);
  const off = (Math.acos(Math.min(1, Math.abs(face[2]) / len(face))) * 180) / Math.PI;
  const centre = sub(place(socket, shield.centre), pose[node.pelvis].t);
  const head = sub(pose[node.Head].t, pose[node.pelvis].t);
  return { off, ahead: centre[2], aboveHead: centre[1] - head[1] };
}

const cm = (x: number) => `${x.toFixed(1)} cm`;
// The clash's numbers and its words, where there is a sword and a shield to measure.
type Pen = ReturnType<typeof penetration>;
const clashed = (pen: Pen): Record<string, number> => (sword && shield ? { sword_shield_cm: pen.sword.cm, shield_arm_cm: pen.arm.cm } : {});
// The carry's numbers of a clip that is no blow (a guard, a walk): what a blow's start and end hold.
const carriedBy = (pen: Pen): Record<string, number> =>
  sword && shield ? { shield_high_cm: pen.carry.high.cm, shield_elbow_below_m: pen.carry.elbow.cm, shield_face_back_deg: pen.carry.face.cm } : {};
// Every move and clip keeps the sword out of its own shield and the shield out of the sword arm (D481).
const clashBad = (pen: Pen): string[] =>
  sword && shield
    ? [
        ...(pen.sword.cm > CLASH_CM ? [`sword in shield ${cm(pen.sword.cm)} deep, ${pen.sword.where}`] : []),
        ...(pen.arm.cm > CLASH_CM ? [`shield in arm ${cm(pen.arm.cm)} deep, ${pen.arm.where}`] : []),
      ]
    : [];
const clashParts = (pen: Pen) => (sword && shield ? [`sword in shield ${cm(pen.sword.cm)} (${pen.sword.where})`, `shield in arm ${cm(pen.arm.cm)} (${pen.arm.where})`] : []);
const failures: string[] = [];
type Row = { move?: string; clip: string; bad: string[]; gaps?: [number, number]; measured: Record<string, number> };
const rows: Row[] = [];
const row = (what: string, parts: string[], bad: string[], json: Omit<Row, "bad">) => {
  for (const [k, v] of Object.entries(json.measured)) json.measured[k] = Math.round(v * 1000) / 1000;
  console.log(`${bad.length ? "✗" : "✓"} ${what}: ${parts.join("; ")}`);
  for (const b of bad) failures.push(`${what}: ${b}`);
  rows.push({ ...json, bad });
};
const specEntries = existsSync(opt.spec!) ? (JSON.parse(readFileSync(opt.spec!, "utf8")) as Record<string, { blow?: Blow; of?: string }>) : {};
// A clip made from another with a part held (hold.py, `of`) is held to the blow its own clip is.
const specs: Record<string, Blow> = Object.fromEntries(
  Object.entries(specEntries)
    .map(([clip, e]) => [clip, e.blow ?? (e.of ? specEntries[e.of]?.blow : undefined)] as const)
    .filter((e): e is readonly [string, Blow] => !!e[1]),
);
const guardClip = clipOf(opt.guard ?? (moves.length ? setGuard(opt.set!) : undefined) ?? "Fight_Guard");
const guardMarks = moves.length ? marks(guardClip, 0) : [];
/** Each held look's own depth, `held_cm.<look>`. */
const heldBy = (pen: Pen) => ({
  ...Object.fromEntries([...pen.byLook].map(([name, w]) => [`held_cm.${name}`, w.cm])),
  // `held_up_deg.<look>`: a look carried in a pose, its handle off up (a carried clip's rows).
  ...Object.fromEntries([...pen.upBy].filter(([, w]) => w.cm > -Infinity).map(([name, w]) => [`held_up_deg.${name}`, w.cm])),
});
const byName = new Map(moves.map((m) => [m.name, m]));
const smooth = (lo: number, hi: number, t: number) => {
  const x = Math.max(0, Math.min(1, (t - lo) / (hi - lo)));
  return x * x * (3 - 2 * x);
};
/** How far out a move's clip carries its shield at clip time `t` (terra_motion's Carry::at). */
const carryAt = (move: Move, t: number) => (move.carry ? smooth(...move.carry.into, t) * (1 - smooth(...move.carry.out, t)) : 0);
/** Two clips played together, `b` taking `w` of the weight: each bone's rotation blended
 * (normalised, the shorter way), its translation and scale mixed, as Bevy blends a node's. */
function mixed(a: Clip, b: Clip, w: number): Clip {
  const tracks: Clip["tracks"] = new Map();
  for (const bone of new Set([...a.tracks.keys(), ...b.tracks.keys()])) {
    const [x, y] = [a.tracks.get(bone) ?? {}, b.tracks.get(bone) ?? {}];
    const track: Clip["tracks"] extends Map<string, infer T> ? T : never = {};
    for (const k of ["t", "r", "s"] as const) {
      const [f, g] = [x[k] ?? y[k], y[k] ?? x[k]];
      if (!f || !g) continue;
      track[k] = (t: number) => {
        const [p, q] = [f(t), g(t)];
        const sign = k === "r" && p.reduce((sum, v, j) => sum + v * q[j], 0) < 0 ? -1 : 1;
        const m = p.map((v, j) => v + (q[j] * sign - v) * w);
        const l = k === "r" ? Math.hypot(...m) : 1;
        return m.map((v) => v / l);
      };
    }
    tracks.set(bone, track);
  }
  return { name: `${a.name}+${b.name}`, end: Math.min(a.end, b.end), tracks };
}
/** What a chained blow plays, pressed with the blow it cuts short at clip time `t`, as the
 * game's runner picks it (terra_motion::moves::Runner::takes): its clip, with its `chained`
 * take mixed in by how far out that blow still carries its shield. */
function takesAfter(move: Move, t: number, next: Move): { clip: Clip; name: string } {
  const out = next.chained ? carryAt(move, t) : 0;
  if (out >= 0.999) return { clip: clipOf(next.chained!), name: next.chained! };
  if (out > 0.001) return { clip: mixed(clipOf(next.clip), clipOf(next.chained!), out), name: `${next.clip} + ${(out * 100).toFixed(0)}% ${next.chained}` };
  return { clip: clipOf(next.clip), name: next.clip };
}
// Every move, and each chained take as a row of its own: it plays the move from a blow's carry.
const checked: Move[] = moves.flatMap((m) => (m.chained ? [m, { ...m, name: `${m.name}.chained`, clip: m.chained, chained: undefined, entered: "chain" as const }] : [m]));
// The worst carry of every blow, for the row "blows".
const carried = { high: { cm: -Infinity, where: "" }, elbow: { cm: Infinity, where: "" }, face: { cm: -Infinity, where: "" } };
// The shield's grip against the fist, the worst of every move and clip, for the row "grip".
const gripped = { cm: { cm: -Infinity, where: "" }, deg: { cm: -Infinity, where: "" } };
const grips = (pen: Pen, what: string) => {
  if (pen.grip.cm.cm > gripped.cm.cm) gripped.cm = { cm: pen.grip.cm.cm, where: `${what} ${pen.grip.cm.where}` };
  if (pen.grip.deg.cm > gripped.deg.cm) gripped.deg = { cm: pen.grip.deg.cm, where: `${what} ${pen.grip.deg.where}` };
};

for (const move of checked) {
  const clip = clipOf(move.clip);
  const parts: string[] = [];
  const bad: string[] = [];
  let gaps: [number, number] | undefined;
  const pen = penetration(clip, move.from, move.done, move.active);
  grips(pen, move.name);
  const measured: Record<string, number> = { hands_cm: pen.hands.cm, held_cm: pen.held.cm, ...heldBy(pen), ...clashed(pen) };
  parts.push(`hands ${cm(pen.hands.cm)}`, `held ${cm(pen.held.cm)}`, ...clashParts(pen));
  if (pen.hands.cm > HAND_CM) bad.push(`hands ${cm(pen.hands.cm)} deep, ${pen.hands.where}`);
  if (pen.held.cm > HELD_CM) bad.push(`held ${cm(pen.held.cm)} deep, ${pen.held.where}`);
  const blow = specs[move.clip.split("#")[0]];
  bad.push(...clashBad(pen));
  if (move.active && sword && shield) {
    const { high, elbow, face } = pen.carry;
    measured.shield_high_cm = high.cm;
    measured.shield_elbow_below_m = elbow.cm;
    measured.shield_face_back_deg = face.cm;
    parts.push(`shield ${cm(high.cm)} above the shoulder at most, elbow ${elbow.cm.toFixed(2)} m below it at least, face ${face.cm.toFixed(0)}° off the chest's forward at most`);
    const of = (w: Worst) => ({ cm: w.cm, where: `${move.name} ${w.where}` });
    if (high.cm > carried.high.cm) carried.high = of(high);
    if (elbow.cm < carried.elbow.cm) carried.elbow = of(elbow);
    if (face.cm > carried.face.cm) carried.face = of(face);
  }
  if (move.active) {
    const a = arc(move);
    parts.push(`contact ${a.bearing.toFixed(0)}° ${a.bearing > 0 ? "left" : "right"} of ahead, ${a.ahead.toFixed(2)} m ahead, sweeping ${a.sweep}, hips turned ${a.hips.toFixed(0)}°`);
    measured.contact_off_deg = Math.abs(a.bearing);
    measured.contact_ahead_m = a.ahead;
    if (blow) {
      const off = againstSpec(blow, a.first, a.last, a.bearing);
      measured.spec_misses = off.length;
      bad.push(...off);
    }
    if (Math.abs(a.bearing) > ARC_DEG || a.ahead < AHEAD_M) bad.push(`contact ${Math.abs(a.bearing).toFixed(0)}° ${a.bearing > 0 ? "left" : "right"} of ahead and ${a.ahead.toFixed(2)} m ahead at ${a.t.toFixed(2)} s (bar ${ARC_DEG}°, ${AHEAD_M} m)`);
  }
  if (blow?.held) {
    // A clip that holds a guard (it is the guard) stays within a blend of its first pose.
    let drift = { m: 0, t: 0 };
    const pose0 = marks(clip, move.from);
    for (let t = move.from; t <= move.done + 1e-6; t += 1 / fps) {
      const m = gap(marks(clip, t), pose0);
      if (m > drift.m) drift = { m, t };
    }
    parts.push(`drifts ${drift.m.toFixed(2)} m from its first pose`);
    measured.drift_m = drift.m;
    if (drift.m > CHAIN_M) bad.push(`drifts ${drift.m.toFixed(2)} m from its first pose at ${drift.t.toFixed(2)} s`);
    // Its shield faces ahead, as a block's does at its hold.
    const g = guard({ ...move, hold: move.from });
    if (g) {
      parts.push(`shield face ${g.off.toFixed(0)}° off ahead`);
      measured.guard_face_deg = g.off;
      if (g.off > FACE_DEG) bad.push(`guard's shield face ${g.off.toFixed(0)}° off ahead (bar ${FACE_DEG}°)`);
    }
  }
  if (blow?.raised) {
    // A block's shield arm up in front of the body and still; its face is the hold's check.
    const [a, b] = blow.raised;
    let low = { hand: Infinity, elbow: Infinity, up: Infinity };
    let moved = 0;
    const first = marks(clip, a)[0];
    for (let t = a; t <= b + 1e-6; t += 1 / fps) {
      const [hand, , elbow] = marks(clip, t);
      low = { hand: Math.min(low.hand, hand[2]), elbow: Math.min(low.elbow, elbow[2]), up: Math.min(low.up, hand[1]) };
      moved = Math.max(moved, len(sub(hand, first)));
    }
    measured.raised_hand_ahead_m = low.hand;
    measured.raised_hand_up_m = low.up;
    measured.raised_elbow_ahead_m = low.elbow;
    measured.raised_moves_m = moved;
    parts.push(`raised ${a.toFixed(2)}-${b.toFixed(2)} s: left hand ${low.hand.toFixed(2)} m ahead and ${low.up.toFixed(2)} m up, elbow ${low.elbow.toFixed(2)} m ahead, moving ${moved.toFixed(2)} m`);
    if (low.hand < RAISED_M || low.elbow < ELBOW_M || low.up < RAISED_UP_M)
      bad.push(`shield arm not up in front while raised: hand ${low.hand.toFixed(2)} m ahead, ${low.up.toFixed(2)} m up, elbow ${low.elbow.toFixed(2)} m ahead (bars ${RAISED_M}, ${RAISED_UP_M}, ${ELBOW_M} m)`);
    if (moved > STILL_M) bad.push(`shield hand moves ${moved.toFixed(2)} m while raised (bar ${STILL_M} m)`);
  }
  if (move.active || (blow && !blow.held)) {
    const startGap = gap(marks(clip, move.from), guardMarks);
    const endGap = gap(marks(clip, move.done), guardMarks);
    parts.push(`from guard ${startGap.toFixed(2)} m, back to guard ${endGap.toFixed(2)} m`);
    gaps = [startGap, endGap];
    measured.from_guard_m = startGap;
    measured.to_guard_m = endGap;
    if (startGap > CHAIN_M && !move.entered) bad.push(`starts ${startGap.toFixed(2)} m from the guard pose`);
    if (endGap > CHAIN_M) bad.push(`ends ${endGap.toFixed(2)} m from the guard pose`);
  }
  if (move.next) {
    const next = byName.get(move.next);
    if (next) {
      // At its end the shield is back at the guard: the next plays what it would from there.
      const g = gap(marks(clip, move.done), marks(takesAfter(move, move.done, next).clip, next.from));
      parts.push(`into ${move.next} ${g.toFixed(2)} m`);
      measured.into_next_m = g;
      if (g > CHAIN_M) bad.push(`its last pose is ${g.toFixed(2)} m from ${move.next}'s first`);
      if (next.clip === move.clip && next.from === move.from) bad.push(`chains into ${move.next}, the same clip from the same place`);
    }
  }
  const g = guard(move);
  if (g) {
    measured.shield_face_deg = g.off;
    parts.push(`shield face ${g.off.toFixed(0)}° off ahead, ${g.ahead.toFixed(2)} m ahead, ${g.aboveHead.toFixed(2)} m above the head`);
    if (g.off > FACE_DEG) bad.push(`shield face ${g.off.toFixed(0)}° off ahead (bar ${FACE_DEG}°)`);
  }
  row(`${move.name} (${move.clip})`, parts, bad, { move: move.name, clip: move.clip, gaps, measured });
  if (opt.verbose) for (const b of bad) console.log(`    ${b}`);
}
if (shield && carried.high.cm > -Infinity) {
  const { high, elbow, face } = carried;
  row(
    "blows, the shield's carry",
    [`highest ${cm(high.cm)} above the shoulder (${high.where})`, `elbow ${elbow.cm.toFixed(2)} m below the shoulder at least (${elbow.where})`, `face ${face.cm.toFixed(0)}° off the chest's forward at most (${face.where})`],
    [],
    { clip: "blows", measured: { shield_high_cm: high.cm, shield_elbow_below_m: elbow.cm, shield_face_back_deg: face.cm } },
  );
}
// Each chained pair, the next blow started anywhere in the window (from the cancel, where a
// press buffered into it starts, to the outgoing's end), playing what the game's runner picks
// there: the shield's middle in the chest's frame, the outgoing clip playing on as the incoming
// fades in over the move blend, the worst press.
const trips: { pair: string; m: number; where: string }[] = [];
if (shield)
  for (const move of moves) {
    const next = move.next ? byName.get(move.next) : undefined;
    if (!next || move.cancel === undefined) continue;
    const out = clipOf(move.clip);
    const pair = `${move.name} into ${next.name}`;
    for (let press = move.cancel; press < move.done - 1e-6; press += move.speed / 60) {
      const into = takesAfter(move, press, next);
      for (const shape of shapes) {
        const along = (clip: Clip, from: number, speed: number) => (u: number) => inChest(poseOf(shape, clip, Math.min(clip.end, from + u * speed)));
        const r = roundTrip(along(out, press, move.speed), along(into.clip, next.from, next.speed), MOVE_BLEND_S, ROUND_TRIP_S, 1 / 60);
        const i = trips.findIndex((x) => x.pair === pair);
        const trip = { pair, m: r.m, where: `pressed at ${press.toFixed(2)} s into ${into.name}, ${r.at.toFixed(2)} s after, ${shape.name}` };
        if (i < 0) trips.push(trip);
        else if (r.m > trips[i].m) trips[i] = trip;
      }
    }
  }
if (trips.length) {
  const worst = trips.reduce((a, b) => (b.m > a.m ? b : a));
  row("chain, the shield's round trip", trips.map((x) => `${x.pair} ${x.m.toFixed(2)} m (${x.where})`), [], { clip: "chain", measured: { shield_round_trip_m: worst.m } });
}
for (const name of opt.also!.split(",").filter(Boolean)) {
  const clip = clipOf(name);
  const pen = penetration(clip, 0, clip.end);
  grips(pen, name);
  const bad: string[] = [];
  if (pen.hands.cm > HAND_CM) bad.push(`hands ${cm(pen.hands.cm)} deep, ${pen.hands.where}`);
  if (pen.held.cm > HELD_CM) bad.push(`held ${cm(pen.held.cm)} deep, ${pen.held.where}`);
  bad.push(...clashBad(pen));
  row(name, [`hands ${cm(pen.hands.cm)}`, `held ${cm(pen.held.cm)}`, ...clashParts(pen)], bad, { clip: name, measured: { hands_cm: pen.hands.cm, held_cm: pen.held.cm, ...heldBy(pen), ...clashed(pen), ...carriedBy(pen) } });
  if (opt.verbose) for (const b of bad) console.log(`    ${b}`);
}
// The fists on what they hold (grip.ts fistOff): each finger bone's gap to the handle's surface,
// either way, the worst of every move's and clip's frames on every template, by look. Numbers for
// the scorecard (held.fist.*, bar FIST_CM), not bars here.
if (fists.length) {
  const measured: Record<string, number> = {};
  const parts: string[] = [];
  const played = [...new Set([...checked.map((m) => m.clip), ...opt.also!.split(",").filter(Boolean)])].map(clipOf);
  for (const f of fists) {
    let worst = { cm: 0, where: "" };
    let wrap = { deg: Infinity, where: "" };
    for (const clip of played)
      for (const shape of shapes)
        for (let t = 0; t <= clip.end + 1e-6; t += 1 / fps) {
          const off = fistOff(body, poseOf(shape, clip, t), f.side, at(f.h.hand), handleNow(f, clip), f.grip);
          const when = `${clip.name} ${t.toFixed(2)} s, ${shape.name}`;
          if (Math.abs(off.cm) > Math.abs(worst.cm)) worst = { cm: off.cm, where: `${off.where}, ${when}` };
          if (off.wrap < wrap.deg) wrap = { deg: off.wrap, where: `${off.short}, ${when}` };
        }
    measured[`fist_cm.${f.h.name}`] = Math.abs(worst.cm);
    measured[`fist_wrap_deg.${f.h.name}`] = wrap.deg;
    parts.push(`${f.h.name} ${cm(worst.cm)} (${worst.where})${Math.abs(worst.cm) > FIST_CM ? ", over the bar" : ""}, round it ${wrap.deg.toFixed(0)} deg at least (${wrap.where})${wrap.deg < WRAP_DEG ? ", short of the bar" : ""}`);
  }
  // A look carried by the rule (D564): how far above level its working end rises at most, degrees
  // (negative: always tipped down), the handle's +Y, every carried clip and template.
  for (const h of ruled) {
    let high = { deg: -Infinity, where: "" };
    for (const clip of played.filter(carriedNow))
      for (const shape of shapes)
        for (let t = 0; t <= clip.end + 1e-6; t += 1 / fps) {
          const way = rot(frame(h, poseOf(shape, clip, t)).r, tipHandle({ at: [0, 0, 0], way: [0, 1, 0], r: 0 }, fistTilt(h)).way);
          const deg = (Math.asin(Math.max(-1, Math.min(1, way[1] / len(way)))) * 180) / Math.PI;
          if (deg > high.deg) high = { deg, where: `${clip.name} ${t.toFixed(2)} s, ${shape.name}` };
        }
    if (high.deg === -Infinity) continue;
    measured[`carried_rise_deg.${h.name}`] = high.deg;
    parts.push(`${h.name} carried tipped ${high.deg <= 0 ? "down" : "up"}, ${Math.abs(high.deg).toFixed(0)} deg ${high.deg <= 0 ? "below" : "above"} level at the highest (${high.where})`);
  }
  measured.fist_cm = Math.max(...Object.entries(measured).filter(([k]) => k.startsWith("fist_cm.")).map(([, v]) => v));
  measured.fist_wrap_deg = Math.min(...Object.entries(measured).filter(([k]) => k.startsWith("fist_wrap_deg.")).map(([, v]) => v));
  row("fist, the fingers on what the hands hold", parts, [], { clip: "fist", measured });
}
// Each shield pose with a target against it (D554, shield_place.ts): the board's place in the
// chest's frame, the worst of its clip on every template. Numbers for the scorecard, not bars here.
if (shield)
  for (const [name, target] of Object.entries(readTargets(opt.targets))) {
    if (!name.split("+").every((n) => clipsFile.clip(n))) continue;
    const off = worstOff({ shapes, at: (s, c, t) => boardPlace(poseOf(s, c, t), rest, node.spine_03, frame(shield, poseOf(s, c, t)), shield) }, clipOf(name), target, 1 / fps);
    const parts = [`${off.cm.toFixed(1)} cm off (${off.cmWhere})`, `${off.deg.toFixed(1)}° off (${off.degWhere}), target from ${target.from} on ${target.template}`];
    row(`${name}, the board against its target`, parts, [], { clip: `${name} board`, measured: { board_target_cm: off.cm, board_target_deg: off.deg } });
  }
if (shield && gripped.cm.cm > -Infinity) {
  // The guard's first frame, where a drawn fighter stands, and the worst of every clip.
  const atGuard = penetration(guardClip, 0, 0).grip.deg;
  row(
    "grip, the fist on the shield's hand strap",
    [`strap ${cm(gripped.cm.cm)} from the fist at most (${gripped.cm.where})`, `strap ${gripped.deg.cm.toFixed(0)}° off the line across the palm at most (${gripped.deg.where}), ${atGuard.cm.toFixed(0)}° in the guard`],
    [],
    { clip: "grip", measured: { grip_hand_cm: gripped.cm.cm, grip_palm_deg: gripped.deg.cm, grip_palm_guard_deg: atGuard.cm } },
  );
}
if (opt.json) writeFileSync(opt.json, JSON.stringify(rows, null, 1) + "\n");
if (failures.length) {
  console.log(`✗ ${failures.length} over the bars:`);
  for (const f of failures) console.log(`  ${f}`);
  process.exit(1);
}
console.log("✓ every move within the bars");
