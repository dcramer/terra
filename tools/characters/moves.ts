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
//           hand and forearm); a bar for a blow (a move with active frames), measured and scored
//           (tools/combat) for every move
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
//           cancel, crossfaded over the game's move blend. Numbers for the scorecard
//           (tools/combat), not bars here: a row "blows" and a row "chain" take the worst
//   spec    a clip with a spec: from and back to the guard; one that holds the guard
//           ("held") within a blend of it throughout; a block's shield arm up in front and
//           still while it is "raised"
//
// A held look hangs from its hand socket, or from the bone its file straps it to (a shield on the
// forearm), as the game hangs it; bends laid over the clips (--bend) are applied as the game does.
// A line a row; ✗ marks what is over its bar; exit 1 if any is.
//
//   node tools/characters/moves.ts
//   node tools/characters/moves.ts [--set assets/data/moves/sword_and_shield.ron] [--fps 15] [--verbose]
//   node tools/characters/moves.ts --spec art/animation/sets/fight.json   # the clips' specs (the default)
//   node tools/characters/moves.ts --json rows.json                       # the rows also as JSON
//
// Node 22.18 or later; no dependencies.

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { againstSpec, type Blow } from "./clip_spec.ts";
import { type Ball, type Board, board, meshOf, sliced, through } from "./clash.ts";
import { PACE_FRAMES } from "./phases.ts";
import { MOVE_BLEND_S, ROUND_TRIP_S, faceOff, roundTrip } from "./shield_path.ts";
import { parseArgs } from "node:util";
import { Glb, type Clip, type Pose, type Q, type V3, dot, len, matrix, mmul, place, posed, qinv, rot, sub, turn } from "./glb.ts";

const { values: opt } = parseArgs({
  options: {
    set: { type: "string", default: "assets/data/moves/sword_and_shield.ron" },
    clips: { type: "string", default: "assets/models/character/fight.glb" },
    body: { type: "string", default: "assets/models/character/base.glb" },
    templates: { type: "string", default: "assets/models/character/templates.json" },
    // What the prototype holds in the hands (`terra_creatures::melee`, SWORD and SHIELD).
    hold: { type: "string", multiple: true, default: ["socket.hand.R=assets/models/held/arming_sword.glb", "socket.hand.L=assets/models/held/round_shield.glb"] },
    // Bends laid over the clips, BONE=x,y,z degrees about the body's axes (staged bends); none in the prototype.
    bend: { type: "string", multiple: true, default: [] },
    also: { type: "string", default: "Keyed_Guard,Fight_Guard,Fight_Strafe_L,Fight_Strafe_R,Fight_Walk,Jump_Loop" },
    // The guard every blow starts and ends at: the set's own `guard` by default (D450), else
    // Fight_Guard for a set that names none (clip.ts's scratch sets).
    guard: { type: "string" },
    // The clips' specs: a blow's sweep and contact (`"blow"`), by clip; a clip `X#N` (a take
    // the clip step tries) is X's.
    spec: { type: "string", default: "art/animation/sets/fight.json" },
    fps: { type: "string", default: "15" },
    // Only these body templates, comma-separated; all of them otherwise.
    template: { type: "string", default: "" },
    verbose: { type: "boolean", default: false },
    // The rows also as JSON, for a caller (clip.ts, tools/combat/scorecard.ts): [{move, clip,
    // bad, gaps, measured}], gaps the metres from the guard at the move's start and end, a
    // blow's only; measured each number the row was judged by, named by what it is.
    json: { type: "string" },
  },
});

// The bars: what a person sees as wrong.
const HAND_CM = 2; // a hand or arm through the skin
const HELD_CM = 1; // a sword or shield through the body
const CLASH_CM = 0; // a blow's sword through its own shield, or the shield through the sword arm (D481)
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

type Move = { name: string; clip: string; from: number; speed: number; done: number; active?: [number, number]; next?: string; cancel?: number; hold?: number };

/** The clip a set names as its guard (`guard: "..."` outside its moves), if any. */
function setGuard(file: string): string | undefined {
  const text = readFileSync(file, "utf8").replace(/\/\/.*$/gm, "");
  return text.slice(0, text.search(/\bmoves\s*:/)).match(/\bguard:\s*"([^"]+)"/)?.[1];
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
    });
  }
  return moves;
}

const clipsFile = new Glb(opt.clips!);
const body = new Glb(opt.body!);
const moves = readSet(opt.set!);
const fps = Number(opt.fps);
const bends = new Map<string, Q>(
  opt.bend!.map((b) => {
    const [bone, deg] = b.split("=");
    return [bone, turn(deg.split(",").map(Number) as V3)];
  }),
);
const clips = new Map<string, Clip>();
const clipOf = (name: string): Clip => {
  if (!clips.has(name)) {
    const c = clipsFile.clip(name);
    if (!c) throw new Error(`no clip ${name} in ${opt.clips}`);
    clips.set(name, c);
  }
  return clips.get(name)!;
};

// The body: its skin, which of its vertices are torso, head or thigh (by their heaviest joint),
// and each template's morph mix and joint offsets.
const skin = body.gltf.skins![0];
const prim = body.gltf.meshes![0].primitives[0];
const positions = body.read(prim.attributes.POSITION);
const normals = body.read(prim.attributes.NORMAL);
const joints = body.read(prim.attributes.JOINTS_0);
const weights = body.read(prim.attributes.WEIGHTS_0);
const ibm = body.read(skin.inverseBindMatrices);
const targetNames = body.gltf.meshes![0].extras?.targetNames ?? [];
const TARGET = /^(pelvis|spine_0\d|neck_01|Head|thigh_[lr]|thigh_twist_01_[lr])$/;
const hit: number[] = [];
const partOf: string[] = [];
for (let v = 0; v < positions.length / 3; v++) {
  let best = 0;
  for (let k = 1; k < 4; k++) if (weights[v * 4 + k] > weights[v * 4 + best]) best = k;
  const name = body.gltf.nodes[skin.joints[joints[v * 4 + best]]].name ?? "";
  if (TARGET.test(name)) {
    hit.push(v);
    partOf[v] = name.replace(/_0\d|_twist_01/g, "");
  }
}
const rigNode = body.gltf.nodes.find((n) => (n as { extras?: { morphs?: string } }).extras?.morphs) as { extras: { morphs: string } } | undefined;
const offsets: Record<string, Record<string, V3>> = rigNode ? JSON.parse(rigNode.extras.morphs) : {};
const templates: Record<string, { morphs: Record<string, number> }> = JSON.parse(readFileSync(opt.templates!, "utf8"));

type Shape = { name: string; positions: Float64Array; moved: Map<number, V3>; shift: Map<number, V3> };
const only = opt.template!.split(",").filter(Boolean);
const shapes: Shape[] = Object.entries(templates).filter(([name]) => only.length === 0 || only.includes(name)).map(([name, t]) => {
  const pos = Float64Array.from(positions);
  for (const [morph, w] of Object.entries(t.morphs)) {
    const k = targetNames.indexOf(morph);
    if (k < 0 || !prim.targets) continue;
    const d = body.read(prim.targets[k].POSITION);
    for (let i = 0; i < pos.length; i++) pos[i] += w * d[i];
  }
  // A morph moves joints in the model's space (`terra_walker::appearance`): the joint's rest
  // translation in its parent's frame takes the difference from its parent's move.
  const shift = new Map<number, V3>();
  body.gltf.nodes.forEach((n, i) => {
    const by: V3 = [0, 0, 0];
    for (const [morph, w] of Object.entries(t.morphs)) {
      const d = n.name ? offsets[morph]?.[n.name] : undefined;
      if (d) for (let j = 0; j < 3; j++) by[j] += w * d[j];
    }
    shift.set(i, by);
  });
  // As the game does: the move less the parent's, in the parent's rest frame (its rotation
  // and scale), added to the joint's translation, the clip's where it sets one.
  const rest = posed(body, undefined, 0);
  const moved = new Map<number, V3>();
  body.gltf.nodes.forEach((_, i) => {
    const p = body.parentOf[i];
    const d = sub(shift.get(i)!, p === undefined ? [0, 0, 0] : shift.get(p)!);
    if (len(d) === 0) return;
    const local = p === undefined ? d : rot(qinv(rest[p].r), d).map((x, j) => x / rest[p].s[j]);
    moved.set(i, local as V3);
  });
  return { name, positions: pos, moved, shift };
});

function poseOf(shape: Shape, clip: Clip, t: number): Pose[] {
  // A morph's joints moved over the clip (`terra_walker::appearance`).
  const keep = { ...clip, tracks: new Map(clip.tracks) };
  for (const [node, by] of shape.moved) {
    const name = body.gltf.nodes[node].name;
    if (!name) continue;
    const track = keep.tracks.get(name);
    const rest = body.rest(node).t;
    const from = track?.t ?? (() => rest);
    keep.tracks.set(name, { ...track, t: (time: number) => from(time).map((x, j) => x + by[j]) });
  }
  return posed(body, keep, t, bends);
}

// Held looks: a few hundred of their vertices in the frame of the node they hang from, and the
// blade's tip. A look hangs from the hand socket it is held in, or, strapped (a shield on the
// forearm), from the bone its file's `strap` names, as `terra_walker::character` hangs it.
// The sword (in the right hand) also as balls, the shield (in the left) as its board, for the
// clash (clash.ts), in the frame of the node they hang from.
// `front` is the way the look's painted front (its -Z) looks, in that frame.
type Held = { hand: string; socket: string; name: string; points: V3[]; tip: V3; thin: V3; front: V3; centre: V3; balls?: Ball[]; board?: Board };
type Strap = { bone: string; at: V3; face: V3; grip: V3 };
const rest = posed(body, undefined, 0);
// A strap's place in its bone's frame: `at` in metres (the hand socket's, which the bone's
// scale is undone to), the look turned so its front (-Z) looks along `face` and its grip
// (+Y) runs along `grip`.
function strapped(strap: Strap, ratio: number): Pose {
  const unit = (v: V3): V3 => v.map((x) => x / len(v)) as V3;
  const y = unit(strap.grip);
  const back = strap.face.map((x) => -x) as V3;
  const z = unit(sub(back, y.map((x) => x * dot(back, y)) as V3));
  const x: V3 = [y[1] * z[2] - y[2] * z[1], y[2] * z[0] - y[0] * z[2], y[0] * z[1] - y[1] * z[0]];
  // The quaternion of the rotation whose columns are x, y and z.
  const [m00, m11, m22] = [x[0], y[1], z[2]];
  const trace = m00 + m11 + m22;
  let r: Q;
  if (trace > 0) {
    const k = 2 * Math.sqrt(1 + trace);
    r = [(y[2] - z[1]) / k, (z[0] - x[2]) / k, (x[1] - y[0]) / k, k / 4];
  } else if (m00 > m11 && m00 > m22) {
    const k = 2 * Math.sqrt(1 + m00 - m11 - m22);
    r = [k / 4, (x[1] + y[0]) / k, (z[0] + x[2]) / k, (y[2] - z[1]) / k];
  } else if (m11 > m22) {
    const k = 2 * Math.sqrt(1 + m11 - m00 - m22);
    r = [(x[1] + y[0]) / k, k / 4, (y[2] + z[1]) / k, (z[0] - x[2]) / k];
  } else {
    const k = 2 * Math.sqrt(1 + m22 - m00 - m11);
    r = [(z[0] + x[2]) / k, (y[2] + z[1]) / k, k / 4, (x[1] - y[0]) / k];
  }
  return { t: strap.at.map((v) => v * ratio) as V3, r, s: [ratio, ratio, ratio] };
}
const held: Held[] = opt.hold!.map((h) => {
  const [hand, file] = h.split("=");
  const g = new Glb(file);
  const node = g.gltf.nodes.findIndex((n) => n.mesh !== undefined);
  const strap = (g.gltf.nodes[node] as { extras?: { strap?: Strap } }).extras?.strap;
  const socket = strap?.bone ?? hand;
  const hung = strap ? strapped(strap, rest[body.named(hand)].s[0] / rest[body.named(strap.bone)].s[0]) : undefined;
  const put = (p: V3): V3 => (hung ? place(hung, p) : p);
  const pose = g.rest(node);
  const p = g.read(g.gltf.meshes![g.gltf.nodes[node].mesh!].primitives[0].attributes.POSITION);
  const all: V3[] = [];
  for (let i = 0; i < p.length; i += 3) all.push(place(pose, [p[i], p[i + 1], p[i + 2]]));
  const step = Math.max(1, Math.floor(all.length / 300));
  const points = all.filter((_, i) => i % step === 0);
  const tip = all.reduce((a, b) => (len(b) > len(a) ? b : a));
  const lo: V3 = [Infinity, Infinity, Infinity];
  const hi: V3 = [-Infinity, -Infinity, -Infinity];
  for (const q of all) for (let j = 0; j < 3; j++) (lo[j] = Math.min(lo[j], q[j])), (hi[j] = Math.max(hi[j], q[j]));
  const size = hi.map((x, j) => x - lo[j]);
  const axis = size.indexOf(Math.min(...size));
  const thin: V3 = [0, 0, 0];
  thin[axis] = 1;
  const centre = lo.map((x, j) => (x + hi[j]) / 2) as V3;
  const name = file.split("/").at(-1)!.replace(".glb", "");
  const k = hung?.s[0] ?? 1;
  const turned = (v: V3): V3 => (hung ? rot(hung.r, v) : v);
  const mesh = hand === "socket.hand.R" || hand === "socket.hand.L" ? meshOf(g) : undefined;
  const balls = hand === "socket.hand.R" ? sliced(mesh!, tip).map((b) => ({ ...b, p: put(b.p), r: b.r * k })) : undefined;
  const disc = hand === "socket.hand.L" ? board(mesh!) : undefined;
  const plate = disc && { centre: put(disc.centre), normal: turned(disc.normal), radius: disc.radius * k, half: disc.half * k };
  const front = turned(rot(pose.r, [0, 0, -1]));
  return { hand, socket, name, points: points.map(put), tip: put(tip), thin: turned(thin), front, centre: put(centre), balls, board: plate };
});

// The skinned torso, head and thighs at a pose, gridded for the nearest-vertex search.
const CELL = 0.1;
function skinned(shape: Shape, pose: Pose[]) {
  const mats = skin.joints.map((j, k) => {
    const s = shape.shift.get(j)!;
    const unbind = mmul(ibm.subarray(k * 16, k * 16 + 16) as unknown as number[], matrix({ t: [-s[0], -s[1], -s[2]], r: [0, 0, 0, 1], s: [1, 1, 1] }));
    return mmul(matrix(pose[j]), unbind);
  });
  const grid = new Map<string, { p: V3; n: V3; part: string }[]>();
  for (const v of hit) {
    const p: V3 = [0, 0, 0];
    const n: V3 = [0, 0, 0];
    for (let k = 0; k < 4; k++) {
      const w = weights[v * 4 + k];
      if (w === 0) continue;
      const m = mats[joints[v * 4 + k]];
      const [x, y, z] = [shape.positions[v * 3], shape.positions[v * 3 + 1], shape.positions[v * 3 + 2]];
      const [a, b, c] = [normals[v * 3], normals[v * 3 + 1], normals[v * 3 + 2]];
      for (let j = 0; j < 3; j++) {
        p[j] += w * (m[j] * x + m[4 + j] * y + m[8 + j] * z + m[12 + j]);
        n[j] += w * (m[j] * a + m[4 + j] * b + m[8 + j] * c);
      }
    }
    const l = len(n) || 1;
    const key = p.map((x) => Math.floor(x / CELL)).join(",");
    if (!grid.has(key)) grid.set(key, []);
    grid.get(key)!.push({ p, n: [n[0] / l, n[1] / l, n[2] / l], part: partOf[v] });
  }
  // How deep a point is inside the skin (positive inside), by its nearest vertex's normal.
  return (q: V3): { depth: number; part: string } => {
    const [cx, cy, cz] = q.map((x) => Math.floor(x / CELL));
    let best: { p: V3; n: V3; part: string } | undefined;
    let bestD = Infinity;
    for (let i = -2; i <= 2; i++)
      for (let j = -2; j <= 2; j++)
        for (let k = -2; k <= 2; k++)
          for (const v of grid.get(`${cx + i},${cy + j},${cz + k}`) ?? []) {
            const d = len(sub(v.p, q));
            if (d < bestD) (bestD = d), (best = v);
          }
    return best ? { depth: dot(sub(best.p, q), best.n), part: best.part } : { depth: -1, part: "" };
  };
}

const at = (name: string) => {
  const i = body.named(name);
  if (i < 0) throw new Error(`no ${name} in ${opt.body}`);
  return i;
};
const node = Object.fromEntries(
  ["pelvis", "Head", "spine_03", "thigh_l", "thigh_r", ...["l", "r"].flatMap((s) => [`hand_${s}`, `lowerarm_${s}`, `upperarm_${s}`, `middle_01_${s}`, `middle_02_${s}`, `index_02_${s}`, `thumb_02_${s}`])].map((n) => [n, at(n)]),
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

// Where the shield is carried (shield_path.ts): the board's middle as the clash hangs it, the
// chest's forward (spine_03's, ahead at rest), and the middle in the chest's frame, which a
// shield arm held still against the chest keeps however the body turns.
const chestRest = qinv(rest[node.spine_03].r);
const chestAhead = (pose: Pose[]): V3 => rot(pose[node.spine_03].r, rot(chestRest, [0, 0, 1]));
const shieldMiddle = (pose: Pose[]): V3 => place(pose[at(shield!.socket)], shield!.board!.centre);
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
  let swordW: Worst = { cm: -Infinity, where: "" };
  let armW: Worst = { cm: -Infinity, where: "" };
  // The carry: cm above the shoulder, m of the elbow below it (the least, kept negated), degrees off.
  let high: Worst = { cm: -Infinity, where: "" };
  let elbow: Worst = { cm: -Infinity, where: "" };
  let face: Worst = { cm: -Infinity, where: "" };
  for (const shape of shapes) {
    for (let t = from; t <= to + 1e-6; t += 1 / fps) {
      const pose = poseOf(shape, clip, t);
      const inside = skinned(shape, pose);
      const P = (n: string) => pose[node[n]].t;
      for (const s of ["l", "r"]) {
        const pts: [V3, number, string][] = [
          [mid(P(`hand_${s}`), P(`middle_01_${s}`)), 0.025, `hand_${s}`],
          [P(`middle_02_${s}`), 0.012, `hand_${s}`],
          [P(`index_02_${s}`), 0.012, `hand_${s}`],
          [P(`thumb_02_${s}`), 0.012, `hand_${s}`],
          [mid(P(`lowerarm_${s}`), P(`hand_${s}`)), 0.035, `forearm_${s}`],
          [P(`lowerarm_${s}`), 0.04, `elbow_${s}`],
          [mid(P(`upperarm_${s}`), P(`lowerarm_${s}`)), 0.045, `upper arm_${s}`],
        ];
        for (const [q, pad, what] of pts) {
          const { depth, part } = inside(q);
          hands = worse(hands, (depth - pad) * 100, `${what} in ${part} at ${t.toFixed(2)} s, ${shape.name}`);
        }
      }
      for (const h of held) {
        const socket = pose[at(h.socket)];
        for (const q of h.points) {
          const { depth, part } = inside(place(socket, q));
          heldW = worse(heldW, depth * 100, `${h.name} in ${part} at ${t.toFixed(2)} s, ${shape.name}`);
        }
      }
      if (sword && shield) {
        const [S, B] = [pose[at(sword.socket)], pose[at(shield.socket)]];
        const b = shield.board!;
        const disc: Board = { centre: place(B, b.centre), normal: rot(B.r, b.normal), radius: b.radius * B.s[0], half: b.half * B.s[0] };
        const blade = through(sword.balls!.map((q) => ({ ...q, p: place(S, q.p), r: q.r * S.s[0] })), disc);
        swordW = worse(swordW, blade.cm, `${blade.part} in ${shield.name} at ${t.toFixed(2)} s, ${shape.name}`);
        const arm = through(swordArm(P), disc);
        armW = worse(armW, arm.cm, `${shield.name} in ${arm.part} at ${t.toFixed(2)} s, ${shape.name}`);
        if (t < carrying[0] - 1e-6 || t > carrying[1] + 1e-6) continue;
        const when = `at ${t.toFixed(2)} s, ${shape.name}`;
        const shoulder = P("upperarm_l");
        high = worse(high, (shieldMiddle(pose)[1] - shoulder[1]) * 100, when);
        elbow = worse(elbow, P("lowerarm_l")[1] - shoulder[1], when);
        face = worse(face, faceOff(rot(B.r, shield.front), chestAhead(pose)), when);
      }
    }
  }
  return { hands, held: heldW, sword: swordW, arm: armW, carry: { high, elbow: { cm: -elbow.cm, where: elbow.where }, face } };
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
  const shield = held.find((h) => h.hand === "socket.hand.L");
  if (!shield || move.hold === undefined) return undefined;
  const pose = poseOf(shapes[0], clipOf(move.clip), move.hold);
  const socket = pose[at(shield.socket)];
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
const guardClip = clipOf(opt.guard ?? setGuard(opt.set!) ?? "Fight_Guard");
const guardMarks = marks(guardClip, 0);
const byName = new Map(moves.map((m) => [m.name, m]));
// The worst carry of every blow, for the row "blows".
const carried = { high: { cm: -Infinity, where: "" }, elbow: { cm: Infinity, where: "" }, face: { cm: -Infinity, where: "" } };

for (const move of moves) {
  const clip = clipOf(move.clip);
  const parts: string[] = [];
  const bad: string[] = [];
  let gaps: [number, number] | undefined;
  const pen = penetration(clip, move.from, move.done, move.active);
  const measured: Record<string, number> = { hands_cm: pen.hands.cm, held_cm: pen.held.cm, ...clashed(pen) };
  parts.push(`hands ${cm(pen.hands.cm)}`, `held ${cm(pen.held.cm)}`, ...clashParts(pen));
  if (pen.hands.cm > HAND_CM) bad.push(`hands ${cm(pen.hands.cm)} deep, ${pen.hands.where}`);
  if (pen.held.cm > HELD_CM) bad.push(`held ${cm(pen.held.cm)} deep, ${pen.held.where}`);
  const blow = specs[move.clip.split("#")[0]];
  if (move.active && sword && shield) {
    // A blow moves the shield out of its sword's path (D481); a guard or block is a later bar.
    if (pen.sword.cm > CLASH_CM) bad.push(`sword in shield ${cm(pen.sword.cm)} deep, ${pen.sword.where}`);
    if (pen.arm.cm > CLASH_CM) bad.push(`shield in arm ${cm(pen.arm.cm)} deep, ${pen.arm.where}`);
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
    if (startGap > CHAIN_M) bad.push(`starts ${startGap.toFixed(2)} m from the guard pose`);
    if (endGap > CHAIN_M) bad.push(`ends ${endGap.toFixed(2)} m from the guard pose`);
  }
  if (move.next) {
    const next = byName.get(move.next);
    if (next) {
      const g = gap(marks(clip, move.done), marks(clipOf(next.clip), next.from));
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
// Each chained pair, the next blow started at the cancel (a press buffered into the window
// starts it as it opens, terra_motion::moves): the shield's middle in the chest's frame, the
// outgoing clip playing on as the incoming fades in over the move blend.
const trips: { pair: string; m: number; where: string }[] = [];
if (shield)
  for (const move of moves) {
    const next = move.next ? byName.get(move.next) : undefined;
    if (!next || move.cancel === undefined) continue;
    const [out, into] = [clipOf(move.clip), clipOf(next.clip)];
    for (const shape of shapes) {
      const along = (clip: Clip, from: number, speed: number) => (u: number) => inChest(poseOf(shape, clip, Math.min(clip.end, from + u * speed)));
      const r = roundTrip(along(out, move.cancel, move.speed), along(into, next.from, next.speed), MOVE_BLEND_S, ROUND_TRIP_S, 1 / 60);
      const pair = `${move.name} into ${next.name}`;
      const i = trips.findIndex((x) => x.pair === pair);
      const trip = { pair, m: r.m, where: `${r.at.toFixed(2)} s after the cancel, ${shape.name}` };
      if (i < 0) trips.push(trip);
      else if (r.m > trips[i].m) trips[i] = trip;
    }
  }
if (trips.length) {
  const worst = trips.reduce((a, b) => (b.m > a.m ? b : a));
  row("chain, the shield's round trip", trips.map((x) => `${x.pair} ${x.m.toFixed(2)} m (${x.where})`), [], { clip: "chain", measured: { shield_round_trip_m: worst.m } });
}
for (const name of opt.also!.split(",").filter(Boolean)) {
  const clip = clipOf(name);
  const pen = penetration(clip, 0, clip.end);
  const bad: string[] = [];
  if (pen.hands.cm > HAND_CM) bad.push(`hands ${cm(pen.hands.cm)} deep, ${pen.hands.where}`);
  if (pen.held.cm > HELD_CM) bad.push(`held ${cm(pen.held.cm)} deep, ${pen.held.where}`);
  row(name, [`hands ${cm(pen.hands.cm)}`, `held ${cm(pen.held.cm)}`, ...clashParts(pen)], bad, { clip: name, measured: { hands_cm: pen.hands.cm, held_cm: pen.held.cm, ...clashed(pen), ...carriedBy(pen) } });
  if (opt.verbose) for (const b of bad) console.log(`    ${b}`);
}
if (opt.json) writeFileSync(opt.json, JSON.stringify(rows, null, 1) + "\n");
if (failures.length) {
  console.log(`✗ ${failures.length} over the bars:`);
  for (const f of failures) console.log(`  ${f}`);
  process.exit(1);
}
console.log("✓ every move within the bars");
